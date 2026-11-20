fn main() -> std::io::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/shared/contracts/generated.ts");
    let content = app_core::api::typescript();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(&content) {
        std::fs::write(path, content)?;
    }
    Ok(())
}
