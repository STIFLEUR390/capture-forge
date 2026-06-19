use oxichrome::prelude::*;

#[oxichrome::extension(
    name = "Capture Forge",
    version = "0.1.0",
    permissions = ["storage"]
)]
struct Extension;

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Capture Forge started!");
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    oxichrome::log!("Capture Forge installed: {:?}", details);
}
