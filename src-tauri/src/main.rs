#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod bootstrap;
mod commands;
mod connection_sources;
mod log_path_window;
mod windows;
fn main() {
    if std::env::var_os("TAILPRO_ASKPASS_SOCKET").is_some() {
        std::process::exit(app_core::adapters::ssh::auth::askpass_main());
    }
    bootstrap::run();
}
