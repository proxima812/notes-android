pub mod error;

use serde::Serialize;
use tauri::Manager;

/// Basic runtime facts about the installed app, used by the diagnostics screen
/// to prove the WebView ↔ Rust bridge and the private data directory both work.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub name: String,
    pub version: String,
    pub data_dir: String,
    pub platform: String,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> Result<AppInfoDto, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Приватная директория приложения недоступна".to_owned())?;

    Ok(AppInfoDto {
        name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
        data_dir: data_dir.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_owned(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
