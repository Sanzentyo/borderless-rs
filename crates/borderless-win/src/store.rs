use borderless_core::{AppConfig, CoreResult, SettingsStore};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct TomlSettingsStore {
    path: PathBuf,
}

impl Default for TomlSettingsStore {
    fn default() -> Self {
        let path = ProjectDirs::from("dev", "BorderlessOxide", "BorderlessOxide").map_or_else(
            || PathBuf::from("borderless-oxide.toml"),
            |dirs| dirs.config_dir().join("config.toml"),
        );
        Self { path }
    }
}

impl TomlSettingsStore {
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }
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
