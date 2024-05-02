#[allow(warnings)]
mod bindings;

use bindings::Guest;

struct Component;

impl Guest for Component {
    fn page_viewed(url: String) {
        println!(
            "###### COMPONENT ######\nPage viewed: {}\n#######################\n",
            url
        );
    }
}

bindings::export!(Component with_types_in bindings);
