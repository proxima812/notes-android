// Driven by the Rust core rather than the WebView, like the other in-tree
// plugins, so nothing here is exposed to JavaScript.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
