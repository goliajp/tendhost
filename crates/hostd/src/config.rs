use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tendhost_protocol::Mode;

/// What enrollment leaves on the host. The credential is in here, so the
/// file is readable by its owner only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub server: String,
    pub host_id: String,
    pub credential: String,
    /// What this host lets the server do. Changed only by editing this
    /// file on the host.
    pub mode: Mode,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "reading {}; has this host been enrolled? (tendhostd enroll)",
                path.display()
            )
        })?;
        toml::from_str(&text)
            .with_context(|| format!("{} is not a valid configuration", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string(self).context("encoding the configuration")?;
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
        // an existing file keeps its old mode through `open`
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(text.as_bytes())?;
        Ok(())
    }
}

/// `/etc/tendhost/hostd.toml` for root, `~/.config/tendhost/hostd.toml`
/// otherwise.
pub fn default_path(is_root: bool, home: Option<&str>) -> Result<PathBuf> {
    if is_root {
        return Ok(PathBuf::from("/etc/tendhost/hostd.toml"));
    }
    let home = home.context("HOME is not set; pass --config")?;
    Ok(Path::new(home).join(".config/tendhost/hostd.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            server: "https://tend.example.com".into(),
            host_id: "host_1".into(),
            credential: "thh_secret".into(),
            mode: Mode::Observe,
        }
    }

    #[test]
    fn a_saved_configuration_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/hostd.toml");
        config().save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config());
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_private_even_when_it_replaces_a_readable_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hostd.toml");
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        config().save(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_missing_file_says_to_enroll() {
        let err = Config::load(Path::new("/nonexistent/hostd.toml")).unwrap_err();
        assert!(format!("{err:#}").contains("tendhostd enroll"));
    }

    #[test]
    fn the_default_path_depends_on_who_runs_it() {
        assert_eq!(
            default_path(true, None).unwrap(),
            Path::new("/etc/tendhost/hostd.toml")
        );
        assert_eq!(
            default_path(false, Some("/home/ada")).unwrap(),
            Path::new("/home/ada/.config/tendhost/hostd.toml")
        );
        assert!(default_path(false, None).is_err());
    }
}
