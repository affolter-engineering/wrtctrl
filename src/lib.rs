#![recursion_limit = "256"]

pub mod app;
pub mod device_clients;
pub mod modals;


#[cfg(feature = "ssr")]
pub mod log_buffer;


#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use app::App;

    #[cfg(feature = "hydrate")]
    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(App);
}
