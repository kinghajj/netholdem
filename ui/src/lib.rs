mod app;
mod client;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run_app() -> Result<(), JsValue> {
    Ok(yew::start_app::<app::App>())
}
