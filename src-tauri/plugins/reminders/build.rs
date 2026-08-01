// The plugin is driven by the Rust core, never by the WebView, so it exposes no
// commands to JavaScript and therefore declares none here.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
