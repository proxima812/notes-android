// Unlike the other in-tree plugins, this one is called from the WebView.
//
// It has to be: what the microphone is hearing has to be on the screen while it
// is being heard, and routing a loudness reading through the Rust core sixteen
// times a second would buy nothing but latency. The core still owns every rule
// about what the finished words *mean* — it just does not need to hold the
// stream while they are being said.
const COMMANDS: &[&str] = &[
    "start",
    "stop",
    "cancel",
    "availability",
    "request_permission",
    "language_support",
    "download_language",
    "open_app_settings",
    "take_dictation_request",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
