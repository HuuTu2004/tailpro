use app_core::{api::*, application::AppHandle};
use tauri::State;
type Response<T> = std::result::Result<T, String>;
fn wire<T>(result: app_core::api::Result<T>) -> Response<T> {
    result.map_err(|error| match error {
        app_core::api::Error::Io(_) | app_core::api::Error::Json(_) => {
            "I/O or protocol failure; verify source access and remote prerequisites".into()
        }
        other => other.to_string(),
    })
}
#[tauri::command]
pub fn health(app: State<'_, AppHandle>) -> Health {
    app.health()
}
#[tauri::command]
pub fn status(app: State<'_, AppHandle>) -> AppStatus {
    app.status()
}
#[tauri::command]
pub async fn import_config(app: State<'_, AppHandle>) -> Response<ConfigImport> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.import_config()))
        .await
        .map_err(|_| "worker stopped".to_string())?
}
#[tauri::command]
pub fn saved_hosts(app: State<'_, AppHandle>) -> Response<Vec<HostProfile>> {
    wire(app.saved_hosts())
}
#[tauri::command]
pub async fn connect(
    app: State<'_, AppHandle>,
    profile: HostProfile,
    consent: bool,
) -> Response<()> {
    wire(app.connect_with_consent(profile, consent).await)
}
#[tauri::command]
pub async fn disconnect(app: State<'_, AppHandle>, host_id: String) -> Response<()> {
    wire(app.disconnect(&host_id).await)
}
#[tauri::command]
pub async fn discover(
    app: State<'_, AppHandle>,
    request: DiscoveryRequest,
) -> Response<DiscoveryResult> {
    wire(app.discover(request).await)
}
#[tauri::command]
pub async fn complete(
    app: State<'_, AppHandle>,
    host_id: String,
    parent: String,
    prefix: String,
) -> Response<DiscoveryResult> {
    wire(app.complete(&host_id, parent, prefix).await)
}
#[tauri::command]
pub async fn start_source(
    app: State<'_, AppHandle>,
    host_id: String,
    spec: SourceSpec,
) -> Response<String> {
    wire(app.start_source(&host_id, spec).await)
}
#[tauri::command]
pub async fn stop_source(app: State<'_, AppHandle>, source_id: String) -> Response<()> {
    wire(app.stop_source(&source_id).await)
}
#[tauri::command]
pub fn pin(app: State<'_, AppHandle>, host_id: String, path: String) -> Response<()> {
    wire(app.pin(&host_id, &path))
}
#[tauri::command]
pub async fn preview(app: State<'_, AppHandle>) -> Response<Preview> {
    wire(app.preview().await)
}

#[tauri::command]
pub fn cancel_discovery(app: State<'_, AppHandle>, host_id: String) {
    app.cancel_discovery(&host_id);
}
#[tauri::command]
pub fn pins(app: State<'_, AppHandle>, host_id: String) -> Response<Vec<String>> {
    wire(app.pins(&host_id))
}
#[tauri::command]
pub fn set_environment(
    app: State<'_, AppHandle>,
    host_id: String,
    environment: Environment,
) -> Response<()> {
    wire(app.set_environment(&host_id, environment))
}

#[tauri::command]
pub async fn open_view(app: State<'_, AppHandle>) -> Response<String> {
    wire(app.open_view().await)
}
#[tauri::command]
pub async fn close_view(app: State<'_, AppHandle>, view_id: String) -> Response<()> {
    wire(app.close_view(&view_id).await)
}
#[tauri::command]
pub async fn view_notice(
    app: State<'_, AppHandle>,
    view_id: String,
    ack: Option<String>,
) -> Response<ViewNotice> {
    wire(app.view_notice(&view_id, ack).await)
}
#[tauri::command]
pub async fn view_page(app: State<'_, AppHandle>, request: ViewRequest) -> Response<ViewPage> {
    wire(app.view_page(request).await)
}
#[tauri::command]
pub async fn event_detail(
    app: State<'_, AppHandle>,
    request: DetailRequest,
) -> Response<EventDetail> {
    wire(app.event_detail(request).await)
}
#[tauri::command]
pub fn cancel_detail(app: State<'_, AppHandle>, view_id: String, generation: u32) -> Response<()> {
    wire(app.cancel_detail(&view_id, generation))
}
#[tauri::command]
pub async fn saved_views(app: State<'_, AppHandle>) -> Response<Vec<SavedView>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.saved_views()))
        .await
        .map_err(|_| "worker stopped".to_string())?
}
#[tauri::command]
pub async fn save_view(app: State<'_, AppHandle>, view: SavedView) -> Response<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.save_view(view)))
        .await
        .map_err(|_| "worker stopped".to_string())?
}
#[tauri::command]
pub async fn delete_view(app: State<'_, AppHandle>, name: String) -> Response<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.delete_view(&name)))
        .await
        .map_err(|_| "worker stopped".to_string())?
}

#[tauri::command]
pub fn cancel_page(app: State<'_, AppHandle>, view_id: String, generation: u32) -> Response<()> {
    wire(app.cancel_page(&view_id, generation))
}

fn main_window(window: &tauri::WebviewWindow) -> Response<()> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Main window required".into())
    }
}
#[tauri::command]
pub fn grants(window: tauri::WebviewWindow, app: State<'_, AppHandle>) -> Response<Vec<GrantInfo>> {
    main_window(&window)?;
    Ok(app.grants())
}
#[tauri::command]
pub fn enroll(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    client: String,
    sources: Vec<String>,
    consent: bool,
) -> Response<Enrollment> {
    main_window(&window)?;
    let directory = wire(app_core::application::bridge::runtime_dir())?;
    // An unavailable bridge cannot hand out usable enrollment secrets.
    wire(app_core::application::bridge::read_registry(
        &directory.join("registry.json"),
    ))?;
    wire(app.enroll(
        client,
        sources,
        consent,
        directory.to_string_lossy().into_owned(),
    ))
}
#[tauri::command]
pub fn revoke(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    grant: String,
) -> Response<()> {
    main_window(&window)?;
    app.revoke(&grant);
    Ok(())
}
#[tauri::command]
pub async fn prepare_export(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    selection: Selection,
    format: ExportFormat,
) -> Response<PreparedPreview> {
    main_window(&window)?;
    wire(app.prepare_export(selection, format).await)
}
#[tauri::command]
pub async fn commit_export(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    id: String,
    digest: String,
    destination: String,
    overwrite: bool,
) -> Response<()> {
    main_window(&window)?;
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        wire(app.commit_export(&id, &digest, std::path::Path::new(&destination), overwrite))
    })
    .await
    .map_err(|_| "Export worker stopped".to_string())?
}
#[tauri::command]
pub fn cancel_prepared(window: tauri::WebviewWindow, app: State<'_, AppHandle>) -> Response<()> {
    main_window(&window)?;
    app.cancel_prepared();
    Ok(())
}
#[tauri::command]
pub fn explain_provider(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
) -> Response<Option<ExplainProvider>> {
    main_window(&window)?;
    Ok(app.explain_provider())
}
#[tauri::command]
pub fn configure_explainer(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    provider: Option<ExplainProvider>,
) -> Response<()> {
    main_window(&window)?;
    wire(app.configure_explainer(provider))
}
#[tauri::command]
pub async fn store_provider_key(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    key: String,
) -> Response<()> {
    main_window(&window)?;
    wire(app.store_provider_key(key).await)
}
#[tauri::command]
pub async fn prepare_explanation(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    selection: Selection,
) -> Response<PreparedPreview> {
    main_window(&window)?;
    wire(app.prepare_explanation(selection).await)
}
#[tauri::command]
pub async fn explain(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    id: String,
    digest: String,
    consent: bool,
) -> Response<Explanation> {
    main_window(&window)?;
    wire(app.explain(&id, &digest, consent).await)
}

#[tauri::command]
pub async fn debug_read(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    view_id: String,
    generation: u32,
    tool: String,
    request: ReadRequest,
) -> Response<ReadResponse> {
    main_window(&window)?;
    wire(app.debug_read(&view_id, generation, &tool, request).await)
}
#[tauri::command]
pub async fn topology(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    request: TopologyRequest,
) -> Response<TopologyGraph> {
    main_window(&window)?;
    wire(app.topology(request).await)
}
#[tauri::command]
pub async fn service_mappings(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
) -> Response<Vec<ServiceMapping>> {
    main_window(&window)?;
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.service_mappings()))
        .await
        .map_err(|_| "Settings worker stopped".to_string())?
}
#[tauri::command]
pub async fn save_service_mapping(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    mapping: ServiceMapping,
) -> Response<()> {
    main_window(&window)?;
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.save_service_mapping(mapping)))
        .await
        .map_err(|_| "Settings worker stopped".to_string())?
}
#[tauri::command]
pub async fn delete_service_mapping(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    source: String,
) -> Response<()> {
    main_window(&window)?;
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || wire(app.delete_service_mapping(&source)))
        .await
        .map_err(|_| "Settings worker stopped".to_string())?
}

#[tauri::command]
pub async fn topology_revision(
    window: tauri::WebviewWindow,
    app: State<'_, AppHandle>,
    request: TopologyRequest,
) -> Response<String> {
    main_window(&window)?;
    wire(app.topology_revision(request).await)
}
