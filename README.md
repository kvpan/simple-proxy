# Wasm Proxy

A very simple proxy that can be extensible with wasm components

# Architecture

The proxy defines an interface to be implemented by wasm components.
This interface corresponds to handlers to events raised by the proxy.
For now the only event is `page-viewed` which receives the URL of the page as payload.

Event processing is done concurrently to add as little overhead as possible to the
forwarding of the content.

# Usage

Make sure that you allow direnv to load the Nix environment (`direnv allow`) or run `nix develop` manually.

## Building the component

```
build-component
```

This will build the amplitude component. It can be found in `target/wasm32-wasi/release/amplited.wasm`.

## Running the proxy

```
run-proxy
```

The proxy will be listening on port 8080.

## Testing

To see the proxy working use this command

```
http -v --proxy=http:http://localhost:8080 lemonde.edgee.dev
```

This has a similar effect as adding `lemonde.edgee.dev 127.0.0.1` to your `/etc/hosts` file.
This request will match the configuration of the proxy and will be redirected.
The component implementation just prints the viewed URL.
