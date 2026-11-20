use crate::api::{Error, HostProfile, Result, SavedView};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub hosts: Vec<HostProfile>,
    pub pins: Vec<Pin>,
    #[serde(default)]
    pub saved_views: Vec<SavedView>,
    #[serde(default)]
    pub service_mappings: Vec<crate::api::ServiceMapping>,
}
#[derive(Serialize, Deserialize)]
pub struct Pin {
    pub host_id: String,
    pub remote_identity: String,
    pub path: String,
}
pub struct SettingsStore {
    path: PathBuf,
}
impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn load(&self) -> Result<Settings> {
        if !self.path.exists() {
            return Ok(Settings {
                version: 1,
                ..Settings::default()
            });
        }
        if fs::metadata(&self.path)?.len() > 1024 * 1024 {
            return Err(Error::Limit("settings 1 MiB"));
        }
        let settings: Settings = serde_json::from_slice(&fs::read(&self.path)?)?;
        if settings.version != 1 {
            return Err(Error::NeedsAction("unsupported settings version".into()));
        }
        Ok(settings)
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        let bytes = serde_json::to_vec(settings)?;
        if bytes.len() > 1024 * 1024
            || settings.hosts.len() > 100
            || settings.pins.len() > 10_000
            || settings.saved_views.len() > 32
            || settings.service_mappings.len() > 100
        {
            return Err(Error::Limit("settings"));
        }
        for host in &settings.hosts {
            if settings
                .pins
                .iter()
                .filter(|p| p.host_id == host.id)
                .count()
                > 100
            {
                return Err(Error::Limit("100 pins/host"));
            }
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::Invalid("settings parent".into()))?;
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let temp = parent.join(format!(".settings-{:x}", rand::random::<u64>()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}
