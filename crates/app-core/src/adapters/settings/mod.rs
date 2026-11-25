use crate::api::{Error, HostProfile, Result, SavedView};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{self, OpenOptions},
    io::Write,
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
    #[serde(default)]
    pub discovery: Vec<HostPreferences>,
    #[serde(default)]
    pub remembered_sources: Vec<crate::api::WorkspaceSource>,
    #[serde(default)]
    pub workspace: crate::api::SavedWorkspace,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct HostPreferences {
    pub host_id: String,
    pub remote_identity: String,
    pub preferences: crate::api::DiscoveryPreferences,
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
    pub fn directory(&self) -> Result<PathBuf> {
        self.path
            .parent()
            .map(|p| p.to_owned())
            .ok_or_else(|| Error::Invalid("settings parent".into()))
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
            || settings.discovery.len() > 100
            || settings.remembered_sources.len() > 800
            || settings.workspace.sources.len() > 8
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
        tailpro_platform::protect(parent, true)?;
        let temp = parent.join(format!(".settings-{:x}", rand::random::<u64>()));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&temp)?;
            tailpro_platform::protect(&temp, false)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            tailpro_platform::sync_parent(parent)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}
