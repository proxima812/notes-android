// Like the reminders plugin, this is driven by the Rust core rather than by the
// WebView: the core decides what file to write and what to do with one that was
// read, so nothing here is exposed to JavaScript.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
