use app_core::api::{AddLogPathRequest, AddLogPathResult};
use tauri::{Emitter, Manager};

const LABEL: &str = "log-path-editor";

pub(crate) fn editor_host(window: &tauri::WebviewWindow) -> Result<String, String> {
    if window.label() != LABEL {
        return Err("Add log window required".into());
    }
    window
        .url()
        .map_err(|_| "Could not read Add log context")?
        .query_pairs()
        .find(|(key, _)| key == "host")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| "Select a host before adding a log".into())
}

#[tauri::command]
pub async fn open_log_path_editor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    host_id: String,
) -> Result<(), String> {
    if window.label() != "main" || host_id.is_empty() || host_id.len() > 128 {
        return Err("Open Add log from a host in the main window".into());
    }
    if let Some(existing) = app.get_webview_window(LABEL) {
        existing.show().map_err(|_| "Could not show Add log")?;
        existing
            .unminimize()
            .map_err(|_| "Could not restore Add log")?;
        existing
            .set_focus()
            .map_err(|_| "Could not focus Add log")?;
        return if editor_host(&existing)? == host_id {
            Ok(())
        } else {
            Err("Close the Add log window for the other host first.".into())
        };
    }
    let mut route = tauri::Url::parse("tauri://localhost/index.html")
        .map_err(|_| "Could not create Add log route")?;
    route
        .query_pairs_mut()
        .append_pair("manager", "add-log")
        .append_pair("host", &host_id);
    let path = format!("index.html?{}", route.query().unwrap_or_default());
    tauri::WebviewWindowBuilder::new(&app, LABEL, tauri::WebviewUrl::App(path.into()))
        .title("TailPro · Open logs")
        .inner_size(920.0, 660.0)
        .min_inner_size(700.0, 520.0)
        .decorations(true)
        .resizable(true)
        .on_navigation(|url| {
            let host = url.host_str().unwrap_or_default();
            (url.scheme() == "tauri" && host == "localhost")
                || (matches!(url.scheme(), "http" | "https") && host == "tauri.localhost")
                || (cfg!(debug_assertions)
                    && url.scheme() == "http"
                    && matches!(host, "localhost" | "127.0.0.1")
                    && url.port() == Some(1420))
        })
        .build()
        .map_err(|_| "Could not open Add log window".to_string())?;
    Ok(())
}

#[tauri::command]
pub fn close_log_path_editor(window: tauri::WebviewWindow) -> Result<(), String> {
    editor_host(&window)?;
    window.close().map_err(|_| "Could not close Add log".into())
}

#[tauri::command]
pub fn submit_log_path(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    request_id: String,
    path: String,
) -> Result<(), String> {
    let host_id = editor_host(&window)?;
    let path = path.trim();
    if request_id.is_empty() || request_id.len() > 64 {
        return Err("Invalid Add log request".into());
    }
    if path.len() > 4096 || !path.starts_with('/') || path.contains(['\r', '\n', '\0']) {
        return Err("Enter an absolute log file path.".into());
    }
    app.emit_to(
        "main",
        "log-path-submit",
        AddLogPathRequest {
            request_id,
            host_id,
            path: path.to_owned(),
        },
    )
    .map_err(|_| "Could not send the log path to the main window".into())
}

#[tauri::command]
pub fn finish_log_path(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    request_id: String,
    error: Option<String>,
) -> Result<(), String> {
    if window.label() != "main"
        || request_id.len() > 64
        || error.as_ref().is_some_and(|value| value.len() > 512)
    {
        return Err("Add log results must come from the main window".into());
    }
    app.emit_to(
        LABEL,
        "log-path-result",
        AddLogPathResult { request_id, error },
    )
    .map_err(|_| "Could not confirm the log path".into())
}
