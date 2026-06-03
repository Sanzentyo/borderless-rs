use borderless_core::{
    AppConfig, AppliedStateStore, CoreResult, OriginalWindowState, SettingsStore,
};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct TomlSettingsStore {
    path: PathBuf,
    applied_path: PathBuf,
}

impl Default for TomlSettingsStore {
    fn default() -> Self {
        let (path, applied_path) = ProjectDirs::from("dev", "BorderlessOxide", "BorderlessOxide")
            .map_or_else(
                || {
                    (
                        PathBuf::from("borderless-oxide.toml"),
                        PathBuf::from("borderless-oxide-applied.toml"),
                    )
                },
                |dirs| {
                    (
                        dirs.config_dir().join("config.toml"),
                        dirs.data_local_dir().join("applied-windows.toml"),
                    )
                },
            );
        Self { path, applied_path }
    }
}

impl TomlSettingsStore {
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        let applied_path = path.with_file_name("applied-windows.toml");
        Self { path, applied_path }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AppliedStateFile {
    #[serde(default)]
    windows: Vec<OriginalWindowState>,
}

impl SettingsStore for TomlSettingsStore {
    fn load_config(&self) -> CoreResult<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|_| borderless_core::CoreError::Transition("read config failed"))?;
        toml::from_str(&text)
            .map_err(|_| borderless_core::CoreError::Transition("parse config failed"))
    }

    fn save_config(&self, config: &AppConfig) -> CoreResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| borderless_core::CoreError::Transition("create config dir failed"))?;
        }
        let text = toml::to_string_pretty(config)
            .map_err(|_| borderless_core::CoreError::Transition("serialize config failed"))?;
        fs::write(&self.path, text)
            .map_err(|_| borderless_core::CoreError::Transition("write config failed"))
    }
}

impl AppliedStateStore for TomlSettingsStore {
    fn load_applied_states(&self) -> CoreResult<Vec<OriginalWindowState>> {
        if !self.applied_path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.applied_path)
            .map_err(|_| borderless_core::CoreError::Transition("read applied state failed"))?;
        let state = toml::from_str::<AppliedStateFile>(&text)
            .map_err(|_| borderless_core::CoreError::Transition("parse applied state failed"))?;
        Ok(state.windows)
    }

    fn save_applied_states(&self, states: &[OriginalWindowState]) -> CoreResult<()> {
        if let Some(parent) = self.applied_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| borderless_core::CoreError::Transition("create state dir failed"))?;
        }
        let state = AppliedStateFile {
            windows: states.to_vec(),
        };
        let text = toml::to_string_pretty(&state).map_err(|_| {
            borderless_core::CoreError::Transition("serialize applied state failed")
        })?;
        fs::write(&self.applied_path, text)
            .map_err(|_| borderless_core::CoreError::Transition("write applied state failed"))
    }
}
