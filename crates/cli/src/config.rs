use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Which server the command line talks to and as whom. Holds an api
/// token, so the file is readable by its owner only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub server: String,
    pub token: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!("reading {}; sign in first: tendhost login", path.display())
        })?;
        toml::from_str(&text)
            .with_context(|| format!("{} is not a valid configuration", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .with_context(|| format!("writing {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(toml::to_string(self)?.as_bytes())?;
        Ok(())
    }
}

/// `~/.config/tendhost/cli.toml`
pub fn default_path(home: Option<&str>) -> Result<PathBuf> {
    let home = home.context("HOME is not set; pass --config")?;
    Ok(Path::new(home).join(".config/tendhost/cli.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_configuration_loads_back_and_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/cli.toml");
        let config = Config {
            server: "https://tend.example.com".into(),
            token: "tht_x".into(),
        };
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn a_missing_file_says_to_sign_in() {
        let error = Config::load(Path::new("/nonexistent/cli.toml")).unwrap_err();
        assert!(format!("{error:#}").contains("tendhost login"));
    }
}
