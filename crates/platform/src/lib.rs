//! Private local storage primitives shared by the desktop and read-only adapter.
use std::{fs, io, path::Path};
#[cfg(unix)]
pub fn protect(path: &Path, directory: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
}
#[cfg(unix)]
pub fn check_private(path: &Path, directory: bool) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink()
        || meta.is_dir() != directory
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o777 != if directory { 0o700 } else { 0o600 }
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{check_private, protect};
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    if path.exists() {
        return check_private(path, true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)?;
    }
    #[cfg(windows)]
    {
        fs::create_dir(path)?;
        protect(path, true)?;
    }
    check_private(path, true)
}
pub fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()?;
    }
    #[cfg(windows)]
    let _ = path;
    Ok(())
}
