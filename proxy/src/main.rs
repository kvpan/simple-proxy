use std::collections::HashMap;

use axum::extract::{Host, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

#[derive(Clone)]
struct AppState {
    proxies: HashMap<&'static str, ProxyConfig>,
    sender: tokio::sync::broadcast::Sender<Events>,
}

#[derive(Clone)]
struct ProxyConfig {
    target: String,
}

#[derive(Clone, Debug)]
enum Events {
    PageView(String),
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // --- [ Wasmtime setup ] ---

    let wasm_path = std::env::var("WASM_PATH").unwrap();
    let wasm = std::fs::read(format!("{}/amplitude.wasm", wasm_path)).unwrap();

    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    config.async_support(true);

    let engine = wasmtime::Engine::new(&config).unwrap();

    let wasi = wasmtime_wasi::WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_env()
        .build_p1();
    let mut store = wasmtime::Store::new(&engine, wasi);
    let component = wasmtime::component::Component::new(&engine, wasm).unwrap();

    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();

    let instance = linker
        .instantiate_async(&mut store, &component)
        .await
        .unwrap();
    // --- [/ Wasmtime setup ] ---

    // --- [ Event processing ] ---
    let (tx, mut rx) = tokio::sync::broadcast::channel::<Events>(32);
    tokio::spawn(async move {
        loop {
            let event = rx.recv().await.unwrap();
            match event {
                Events::PageView(url) => {
                    let instance = instance.clone();
                    let func = instance
                        .get_func(&mut store, "page-viewed")
                        .expect("function not found");
                    let args = [wasmtime::component::Val::String(url.clone())];
                    match func.call_async(&mut store, &args, &mut []).await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Error calling function: {:?}", e);
                        }
                    }
                }
            }
        }
    });
    // --- [/ Event processing ] ---

    let mut proxies = HashMap::new();
    proxies.insert(
        "lemonde.edgee.dev",
        ProxyConfig {
            target: String::from("www.lemonde.fr"),
        },
    );

    let app_state = AppState {
        proxies,
        sender: tx,
    };

    let router = axum::Router::new()
        .fallback(proxy_handler)
        .with_state(app_state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Listening on {}", addr);
    axum::serve(listener, router).await.unwrap();
}

async fn proxy_handler(
    State(state): State<AppState>,
    mut headers: axum::http::HeaderMap,
    Host(host): Host,
    method: axum::http::Method,
    uri: axum::http::Uri,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let host = host
        .split(':')
        .next()
        .ok_or((StatusCode::BAD_REQUEST, format!("Invalid host: {host}")))?;

    if !state.proxies.contains_key(host) {
        return Err((StatusCode::BAD_GATEWAY, format!("Bad host: {host}")));
    }

    let client = reqwest::Client::new();
    let path = uri.path_and_query().unwrap();
    let protocol = uri.scheme_str().unwrap_or("http");
    let target_host = &state.proxies[host].target;
    let target = format!("{protocol}://{target_host}{path}");

    headers.insert("host", target_host.parse().unwrap());

    let resp = client
        .request(method, &target)
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Error: {}", e.to_string())))?;

    state.sender.send(Events::PageView(target)).unwrap();

    Ok((
        resp.status(),
        resp.headers().clone(),
        resp.bytes().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", e.to_string()),
            )
        })?,
    ))
}
