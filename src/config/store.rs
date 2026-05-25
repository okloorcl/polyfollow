use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::{AppConfig, DEFAULT_CONFIG_FILE};

pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .context("failed to resolve config directory")?;
    Ok(base.join("polyfollow").join(DEFAULT_CONFIG_FILE))
}

pub fn load_or_default(path: &PathBuf) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let config = toml::from_str::<AppConfig>(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    config.validate()?;
    Ok(config)
}

pub fn save(path: &PathBuf, config: &AppConfig) -> Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(config)?;

    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        file.write_all(text.as_bytes())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    #[cfg(not(unix))]
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}
