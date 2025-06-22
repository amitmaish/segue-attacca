#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod app;

use app::*;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App)
}
