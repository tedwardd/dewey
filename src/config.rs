use crate::errors::CliError;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_DIR_NAME: &str = "dewey";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub default_module: Option<String>,
    pub download_dir: Option<String>,
}

impl Config {
    pub fn expanded_download_dir(&self) -> Option<String> {
        self.download_dir.as_ref().map(|d| expand_tilde(d))
    }
}

pub fn config_dir() -> Result<PathBuf, CliError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliError::Config("HOME not set; cannot locate config directory".into()))?;
    Ok(PathBuf::from(home).join(".config").join(CONFIG_DIR_NAME))
}

pub fn modules_dir() -> Result<PathBuf, CliError> {
    if let Some(dir) = std::env::var_os("DEWEY_MODULES") {
        if dir.is_empty() {
            return Err(CliError::Config("DEWEY_MODULES is empty".into()));
        }
        return Ok(PathBuf::from(dir));
    }
    Ok(config_dir()?.join("modules"))
}

pub fn load_config() -> Result<Config, CliError> {
    load_config_from(&config_dir()?)
}

pub fn load_config_from(dir: &Path) -> Result<Config, CliError> {
    let path = dir.join("config.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| CliError::Config(format!("cannot read {}: {e}", path.display())))?;
    let cfg: Config = toml::from_str(&text)
        .map_err(|e| CliError::Config(format!("invalid {}: {e}", path.display())))?;
    if let Some(m) = &cfg.default_module {
        if m.trim().is_empty() {
            return Err(CliError::Config(
                "config.toml: default_module must not be empty".into(),
            ));
        }
    }
    Ok(cfg)
}

pub fn expand_tilde(s: &str) -> String {
    if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
        return s.to_string();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dewey-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_config_file_returns_defaults() {
        let dir = temp_dir();
        let cfg = load_config_from(&dir).unwrap();
        assert!(cfg.default_module.is_none());
        assert!(cfg.download_dir.is_none());
    }

    #[test]
    fn valid_config_parses() {
        let dir = temp_dir();
        fs::write(
            dir.join("config.toml"),
            "default_module = \"gutenberg\"\ndownload_dir = \"~/Downloads\"\n",
        )
        .unwrap();
        let cfg = load_config_from(&dir).unwrap();
        assert_eq!(cfg.default_module.as_deref(), Some("gutenberg"));
        let expected = format!("{}/Downloads", std::env::var("HOME").unwrap());
        assert_eq!(cfg.expanded_download_dir().as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn invalid_toml_is_config_error() {
        let dir = temp_dir();
        fs::write(dir.join("config.toml"), "not = = toml").unwrap();
        let err = load_config_from(&dir).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn empty_default_module_is_config_error() {
        let dir = temp_dir();
        fs::write(dir.join("config.toml"), "default_module = \"\"\n").unwrap();
        let err = load_config_from(&dir).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn expand_tilde_replaces_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/x"), format!("{home}/x"));
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
        assert_eq!(expand_tilde("relative"), "relative");
    }
}
