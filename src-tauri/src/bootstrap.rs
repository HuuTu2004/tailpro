use app_core::application::AppHandle;
use std::sync::Mutex;
use tauri::Manager;
struct BridgeState(Mutex<Option<app_core::application::bridge::Bridge>>);
pub fn run() {
    let app = tauri::Builder::default().plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. }
                    if window.label() == "main" && (app.get_webview_window("connections-manager").is_some() || app.get_webview_window("settings-manager").is_some() || app.get_webview_window("connection-editor").is_some() || app.get_webview_window("log-path-editor").is_some()) =>
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::CloseRequested { .. } if window.label() == "log-path-editor" => {
                    if let Some(editor) = app.get_webview_window("log-path-editor") { if let Ok(host) = super::log_path_window::editor_host(&editor) { app.state::<AppHandle>().cancel_discovery(&host); app.state::<AppHandle>().cancel_source_open(&host); } }
                }
                tauri::WindowEvent::CloseRequested { .. } if window.label() == "connection-editor" => { app.state::<AppHandle>().cancel_connection_attempt(); }
                tauri::WindowEvent::Destroyed if matches!(window.label(), "connections-manager" | "settings-manager" | "connection-editor" | "log-path-editor") => {
                    super::windows::manager_closed(app, window.label());
                }
                _ => {}
            }
        })
        .setup(|app| {
            let settings = app.path().app_config_dir()?.join("settings.json");
            let core = AppHandle::new(settings);
            let bridge = tauri::async_runtime::block_on(app_core::application::bridge::Bridge::start(core.clone()));
            if bridge.is_err() { eprintln!("Local MCP bridge unavailable; check private runtime ownership or existing instance"); }
            app.manage(BridgeState(Mutex::new(bridge.ok())));
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            super::connection_sources::begin_connection,
            super::connection_sources::connection_attempt,
            super::connection_sources::answer_authentication,
            super::connection_sources::cancel_connection_attempt,
            super::connection_sources::pick_identity_file,
            super::connection_sources::forget_host,
            super::connection_sources::inspect_host,
            super::connection_sources::browse_directory,
            super::connection_sources::discovery_preferences,
            super::connection_sources::save_discovery_preferences,
            super::connection_sources::remembered_sources,
            super::connection_sources::open_sources,
            super::connection_sources::cancel_source_open,
            super::connection_sources::saved_workspace,
            super::connection_sources::save_workspace,
            super::connection_sources::set_restore_readers,
            super::connection_sources::restore_host_sources,
            super::windows::open_workspace_manager,
            super::windows::close_workspace_manager,
            super::log_path_window::open_log_path_editor,
            super::log_path_window::close_log_path_editor,
            super::log_path_window::submit_log_path,
            super::log_path_window::finish_log_path,
            super::windows::open_connection_editor,
            super::windows::close_connection_editor,
            super::commands::test_connection,
            super::commands::cancel_connection_test,
            super::commands::save_host_profile,
            super::commands::debug_read,
            super::commands::topology,
            super::commands::topology_revision,
            super::commands::service_mappings,
            super::commands::save_service_mapping,
            super::commands::delete_service_mapping,
            super::commands::grants,
            super::commands::enroll,
            super::commands::revoke,
            super::commands::prepare_export,
            super::commands::commit_export,
            super::commands::cancel_prepared,
            super::commands::explain_provider,
            super::commands::configure_explainer,
            super::commands::store_provider_key,
            super::commands::prepare_explanation,
            super::commands::explain,
            super::commands::cancel_discovery,
            super::commands::pins,
            super::commands::set_environment,
            super::commands::health,
            super::commands::status,
            super::commands::import_config,
            super::commands::saved_hosts,
            super::commands::connect,
            super::commands::disconnect,
            super::commands::discover,
            super::commands::complete,
            super::commands::start_source,
            super::commands::stop_source,
            super::commands::pin,
            super::commands::open_view,
            super::commands::close_view,
            super::commands::view_notice,
            super::commands::view_page,
            super::commands::event_detail,
            super::commands::preview_parser,
            super::commands::inspect_kubernetes,
            super::commands::apply_parser,
            super::commands::ingestion_diagnostics,
            super::commands::cancel_detail,
            super::commands::cancel_page,
            super::commands::saved_views,
            super::commands::save_view,
            super::commands::delete_view,
        ])
        .build(tauri::generate_context!())
        .expect("TailPro bootstrap");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            let core = handle.state::<AppHandle>().inner().clone();
            let bridge = handle.state::<BridgeState>().0.lock().unwrap().take();
            tauri::async_runtime::block_on(async {
                if let Some(bridge) = bridge {
                    bridge.close().await;
                }
                core.shutdown().await;
            });
        }
    });
}
