use app_core::{api::*, application::AppHandle};
use std::result::Result;
use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;

fn source_window(window: &tauri::WebviewWindow, host: &str) -> Result<(), String> {
    if window.label() == "main" {
        return Ok(());
    }
    if window.label() != "log-path-editor" || super::log_path_window::editor_host(window)? != host {
        return Err("Open logs window required for this host".into());
    }
    Ok(())
}
fn editor(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "connection-editor" {
        Ok(())
    } else {
        Err("Connection editor required".into())
    }
}

#[tauri::command]
pub async fn begin_connection(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    profile: HostProfile,
    consent: bool,
    test_only: bool,
) -> Result<String, String> {
    editor(&window)?;
    core.begin_connection(profile, consent, test_only)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn connection_attempt(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    id: String,
) -> Result<ConnectionAttempt, String> {
    editor(&window)?;
    core.connection_attempt(&id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn answer_authentication(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    attempt: String,
    challenge: String,
    value: String,
    verified: bool,
) -> Result<(), String> {
    editor(&window)?;
    core.answer_authentication(&attempt, &challenge, value, verified)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cancel_connection_attempt(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
) -> Result<(), String> {
    editor(&window)?;
    core.cancel_connection_attempt();
    Ok(())
}
#[tauri::command]
pub async fn pick_identity_file(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    editor(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select SSH private key")
            .blocking_pick_file()
            .map(|file| {
                file.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|_| "Choose a local key file".to_owned())
            })
            .transpose()
    })
    .await
    .map_err(|_| "Key file picker closed".to_owned())?
}
#[tauri::command]
pub async fn forget_host(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    core: State<'_, AppHandle>,
    host_id: String,
) -> Result<(), String> {
    super::windows::connections_window(&window)?;
    core.forget_host(&host_id)
        .await
        .map_err(|e| e.to_string())?;
    app.emit("connection-profiles-changed", ())
        .map_err(|_| "Could not refresh saved connections".to_owned())
}

#[tauri::command]
pub async fn inspect_host(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
    refresh: bool,
) -> Result<HostCapabilities, String> {
    source_window(&window, &host_id)?;
    core.inspect_host(&host_id, refresh)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn browse_directory(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
    path: String,
    cursor: Option<String>,
) -> Result<DirectoryPage, String> {
    source_window(&window, &host_id)?;
    core.browse_directory(&host_id, path, cursor)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn discovery_preferences(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
) -> Result<DiscoveryPreferences, String> {
    source_window(&window, &host_id)?;
    core.discovery_preferences(&host_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn save_discovery_preferences(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
    preferences: DiscoveryPreferences,
) -> Result<(), String> {
    source_window(&window, &host_id)?;
    core.save_discovery_preferences(&host_id, preferences)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn remembered_sources(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
) -> Result<Vec<SourceSpec>, String> {
    source_window(&window, &host_id)?;
    core.remembered_sources(&host_id).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn open_sources(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    core: State<'_, AppHandle>,
    host_id: String,
    specs: Vec<SourceSpec>,
) -> Result<Vec<OpenSourceResult>, String> {
    source_window(&window, &host_id)?;
    let results = core
        .open_sources(&host_id, specs)
        .await
        .map_err(|e| e.to_string())?;
    let source_ids = results.iter().filter_map(|r| r.source_id.clone()).collect();
    app.emit_to(
        "main",
        "sources-opened",
        SourcesOpened {
            host_id,
            source_ids,
        },
    )
    .map_err(|_| "Readers opened; could not focus main window".to_owned())?;
    Ok(results)
}
#[tauri::command]
pub fn saved_workspace(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
) -> Result<SavedWorkspace, String> {
    if !matches!(
        window.label(),
        "main" | "log-path-editor" | "settings-manager"
    ) {
        return Err("Workspace window required".into());
    }
    core.saved_workspace().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn save_workspace(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    selection: WorkspaceSelection,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Main workspace required".into());
    }
    core.save_workspace(selection).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn set_restore_readers(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    core: State<'_, AppHandle>,
    enabled: bool,
) -> Result<(), String> {
    if !matches!(
        window.label(),
        "main" | "log-path-editor" | "settings-manager"
    ) {
        return Err("Workspace window required".into());
    }
    core.set_restore_readers(enabled)
        .map_err(|e| e.to_string())?;
    app.emit("workspace-preferences-changed", ())
        .map_err(|_| "Could not refresh workspace preferences".to_owned())
}
#[tauri::command]
pub async fn restore_host_sources(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
) -> Result<Vec<OpenSourceResult>, String> {
    if window.label() != "main" {
        return Err("Main workspace required".into());
    }
    core.restore_host_sources(&host_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_source_open(
    window: tauri::WebviewWindow,
    core: State<'_, AppHandle>,
    host_id: String,
) -> Result<(), String> {
    source_window(&window, &host_id)?;
    core.cancel_source_open(&host_id);
    Ok(())
}
