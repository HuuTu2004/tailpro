use app_core::application::AppHandle as CoreHandle;
use tauri::Manager;

fn preview_requested(window: &tauri::WebviewWindow) -> bool {
    cfg!(debug_assertions)
        && window.url().ok().is_some_and(|url| {
            url.query_pairs()
                .any(|(key, value)| key == "preview" && value == "1")
        })
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManagerSection {
    Connections,
    Settings,
}
impl ManagerSection {
    fn label(self) -> &'static str {
        match self {
            Self::Connections => "connections-manager",
            Self::Settings => "settings-manager",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Connections => "TailPro · Connections",
            Self::Settings => "TailPro · Settings",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Connections => "connections",
            Self::Settings => "settings",
        }
    }
}
pub fn settings_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if matches!(window.label(), "main" | "settings-manager") {
        Ok(())
    } else {
        Err("Settings window required".into())
    }
}
pub fn connections_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if matches!(
        window.label(),
        "main" | "connections-manager" | "connection-editor"
    ) {
        Ok(())
    } else {
        Err("Connections window required".into())
    }
}

#[tauri::command]
pub async fn open_workspace_manager(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    section: ManagerSection,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Open workspace management from the main window".into());
    }
    if let Some(existing) = app.get_webview_window(section.label()) {
        // Focus the existing draft instead of silently replacing unsaved edits.
        existing
            .show()
            .map_err(|_| "Could not show workspace management")?;
        existing
            .unminimize()
            .map_err(|_| "Could not restore workspace management")?;
        return existing
            .set_focus()
            .map_err(|_| "Could not focus workspace management".into());
    }
    let preview = preview_requested(&window);
    let path = format!(
        "index.html?manager={}{}",
        section.name(),
        if preview { "&preview=1" } else { "" }
    );
    tauri::WebviewWindowBuilder::new(&app, section.label(), tauri::WebviewUrl::App(path.into()))
        .title(if preview {
            format!("{} · Demo", section.title())
        } else {
            section.title().into()
        })
        .inner_size(760.0, 600.0)
        .min_inner_size(640.0, 520.0)
        .decorations(true)
        .skip_taskbar(false)
        .resizable(true)
        .minimizable(true)
        .maximizable(true)
        .closable(true)
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
        .map_err(|_| "Could not open workspace management".to_string())?;
    Ok(())
}
#[tauri::command]
pub fn close_workspace_manager(window: tauri::WebviewWindow) -> Result<(), String> {
    if !matches!(window.label(), "connections-manager" | "settings-manager") {
        return Err("Only the management window can be closed by this action".into());
    }
    window
        .close()
        .map_err(|_| "Could not close workspace management".into())
}
pub fn manager_closed(app: &tauri::AppHandle, label: &str) {
    if label == "connection-editor" {
        app.state::<CoreHandle>().cancel_connection_test();
        if let Some(connections) = app.get_webview_window("connections-manager") {
            let _ = connections.show();
            let _ = connections.set_focus();
        }
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
    }
}

#[tauri::command]
pub async fn open_connection_editor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !matches!(window.label(), "main" | "connections-manager") {
        return Err("Open the connection editor from Connections".into());
    }
    if let Some(existing) = app.get_webview_window("connection-editor") {
        existing
            .show()
            .map_err(|_| "Could not show connection editor")?;
        existing
            .unminimize()
            .map_err(|_| "Could not restore connection editor")?;
        return existing
            .set_focus()
            .map_err(|_| "Could not focus connection editor".into());
    }
    let preview = preview_requested(&window);
    tauri::WebviewWindowBuilder::new(
        &app,
        "connection-editor",
        tauri::WebviewUrl::App(if preview {
            "index.html?manager=new-connection&preview=1".into()
        } else {
            "index.html?manager=new-connection".into()
        }),
    )
    .title(if preview {
        "TailPro · New connection · Demo"
    } else {
        "TailPro · New connection"
    })
    .inner_size(600.0, 560.0)
    .min_inner_size(500.0, 480.0)
    .decorations(true)
    .skip_taskbar(false)
    .resizable(true)
    .minimizable(true)
    .maximizable(true)
    .closable(true)
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
    .map_err(|_| "Could not open connection editor".to_string())?;
    Ok(())
}

#[tauri::command]
pub fn close_connection_editor(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "connection-editor" {
        return Err("Connection editor window required".into());
    }
    window
        .close()
        .map_err(|_| "Could not close connection editor".into())
}
