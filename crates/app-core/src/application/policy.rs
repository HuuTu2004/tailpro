use crate::api::{Error, Result};
/// Paths are literal, absolute UTF-8; remote shell quoting is centralized here.
pub fn path(value: &str) -> Result<()> {
    if value.len() > 4096
        || !value.starts_with('/')
        || value.chars().any(char::is_control)
        || value.split('/').any(|s| s == "..")
    {
        return Err(Error::Invalid(
            "expected absolute literal path without controls/traversal".into(),
        ));
    }
    Ok(())
}
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub fn container_id(value: &str) -> Result<()> {
    if !(12..=64).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Invalid("expected Docker container ID".into()));
    }
    Ok(())
}

pub fn unit(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 255
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b))
    {
        return Err(Error::Invalid("literal systemd unit name required".into()));
    }
    Ok(())
}
