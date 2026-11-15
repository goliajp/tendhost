//! Running `tendhostd run` as a service of the host's own service manager:
//! systemd on Linux, launchd on macOS.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const LABEL: &str = "dev.tendhost.hostd";
pub const UNIT: &str = "tendhostd.service";

/// Where the service definition goes and how it is loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Manager {
    Systemd,
    /// a LaunchDaemon, started at boot
    LaunchdSystem,
    /// a LaunchAgent of the user with this uid, started at login
    LaunchdUser {
        uid: u32,
        home: PathBuf,
    },
}

impl Manager {
    pub fn detect(os: &str, uid: u32, home: Option<&str>) -> Result<Self> {
        match (os, uid) {
            ("linux", 0) => Ok(Manager::Systemd),
            ("linux", _) => bail!("installing the service on Linux needs root: run it with sudo"),
            ("macos", 0) => Ok(Manager::LaunchdSystem),
            ("macos", uid) => {
                let home = home.context("HOME is not set")?;
                Ok(Manager::LaunchdUser {
                    uid,
                    home: PathBuf::from(home),
                })
            }
            (other, _) => bail!(
                "no service manager is supported on {other}; run `tendhostd run` under your own"
            ),
        }
    }

    pub fn definition_path(&self) -> PathBuf {
        match self {
            Manager::Systemd => Path::new("/etc/systemd/system").join(UNIT),
            Manager::LaunchdSystem => {
                Path::new("/Library/LaunchDaemons").join(format!("{LABEL}.plist"))
            }
            Manager::LaunchdUser { home, .. } => home
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist")),
        }
    }

    pub fn definition(&self, exe: &Path, config: &Path) -> String {
        match self {
            Manager::Systemd => systemd_unit(exe, config),
            _ => launchd_plist(exe, config),
        }
    }

    fn start_commands(&self) -> Vec<Vec<String>> {
        let path = self.definition_path().display().to_string();
        let own = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        match self {
            Manager::Systemd => {
                vec![
                    own(&["systemctl", "daemon-reload"]),
                    own(&["systemctl", "enable", "--now", UNIT]),
                ]
            }
            Manager::LaunchdSystem => vec![own(&["launchctl", "bootstrap", "system", &path])],
            Manager::LaunchdUser { uid, .. } => {
                vec![own(&[
                    "launchctl",
                    "bootstrap",
                    &format!("gui/{uid}"),
                    &path,
                ])]
            }
        }
    }

    fn stop_commands(&self) -> Vec<Vec<String>> {
        let own = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        match self {
            Manager::Systemd => vec![own(&["systemctl", "disable", "--now", UNIT])],
            Manager::LaunchdSystem => {
                vec![own(&["launchctl", "bootout", &format!("system/{LABEL}")])]
            }
            Manager::LaunchdUser { uid, .. } => {
                vec![own(&[
                    "launchctl",
                    "bootout",
                    &format!("gui/{uid}/{LABEL}"),
                ])]
            }
        }
    }

    /// Writes the definition and starts the service; it also starts at
    /// every boot (or login, for a user's LaunchAgent) from now on.
    pub fn install(&self, exe: &Path, config: &Path) -> Result<()> {
        let path = self.definition_path();
        if path.exists() {
            // replacing a loaded definition: unload the old one first
            self.stop_commands().iter().for_each(|command| {
                let _ = run(command);
            });
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, self.definition(exe, config))
            .with_context(|| format!("writing {}", path.display()))?;
        self.start_commands()
            .iter()
            .try_for_each(|command| run(command))
    }

    /// Stops the service and removes its definition. The host stays
    /// enrolled: its configuration is left alone.
    pub fn uninstall(&self) -> Result<()> {
        let path = self.definition_path();
        if !path.exists() {
            bail!(
                "no service is installed ({} does not exist)",
                path.display()
            );
        }
        self.stop_commands()
            .iter()
            .try_for_each(|command| run(command))?;
        std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        if *self == Manager::Systemd {
            run(&["systemctl".to_string(), "daemon-reload".to_string()])?;
        }
        Ok(())
    }
}

fn run(command: &[String]) -> Result<()> {
    let line = command.join(" ");
    let output = Command::new(&command[0])
        .args(&command[1..])
        .output()
        .with_context(|| format!("running {line}"))?;
    if !output.status.success() {
        bail!(
            "{line} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Exit status 78 means the server removed this host; restarting cannot
/// fix that, so systemd is told not to.
fn systemd_unit(exe: &Path, config: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=tendhost host daemon\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         ExecStart={} --config {} run\n\
         Restart=always\n\
         RestartSec=5\n\
         RestartPreventExitStatus=78\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        systemd_word(exe),
        systemd_word(config)
    )
}

/// One word of a systemd command line: quoted, with `%` and `$` doubled so
/// systemd expands neither.
fn systemd_word(path: &Path) -> String {
    let text = path
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{}\"", text.replace('%', "%%").replace('$', "$$"))
}

fn launchd_plist(exe: &Path, config: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--config</string>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>60</integer>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
        xml(exe),
        xml(config)
    )
}

fn xml(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manager_follows_the_system_and_who_installs() {
        assert_eq!(Manager::detect("linux", 0, None).unwrap(), Manager::Systemd);
        assert!(
            Manager::detect("linux", 1000, Some("/home/ada"))
                .unwrap_err()
                .to_string()
                .contains("sudo")
        );
        assert_eq!(
            Manager::detect("macos", 0, None).unwrap(),
            Manager::LaunchdSystem
        );
        let user = Manager::detect("macos", 501, Some("/Users/ada")).unwrap();
        assert_eq!(
            user.definition_path(),
            Path::new("/Users/ada/Library/LaunchAgents/dev.tendhost.hostd.plist")
        );
        assert!(Manager::detect("freebsd", 0, None).is_err());
    }

    #[test]
    fn the_systemd_unit_runs_the_daemon_and_does_not_restart_a_removed_host() {
        let unit = Manager::Systemd.definition(
            Path::new("/usr/local/bin/tendhostd"),
            Path::new("/etc/tendhost/hostd.toml"),
        );
        assert!(unit.contains(
            "ExecStart=\"/usr/local/bin/tendhostd\" --config \"/etc/tendhost/hostd.toml\" run\n"
        ));
        assert!(
            unit.contains("Restart=always\n") && unit.contains("RestartPreventExitStatus=78\n")
        );
    }

    #[test]
    fn paths_with_spaces_and_specifiers_stay_one_word() {
        let unit = systemd_unit(
            Path::new("/opt/my tools/tendhostd"),
            Path::new("/etc/100%/$x.toml"),
        );
        assert!(unit.contains(
            "ExecStart=\"/opt/my tools/tendhostd\" --config \"/etc/100%%/$$x.toml\" run"
        ));
        let plist = launchd_plist(Path::new("/Users/a&b/tendhostd"), Path::new("/x/<y>.toml"));
        assert!(plist.contains("<string>/Users/a&amp;b/tendhostd</string>"));
        assert!(plist.contains("<string>/x/&lt;y&gt;.toml</string>"));
    }

    #[test]
    fn the_commands_address_the_right_domain() {
        let user = Manager::LaunchdUser {
            uid: 501,
            home: "/Users/ada".into(),
        };
        assert_eq!(
            user.start_commands(),
            [[
                "launchctl",
                "bootstrap",
                "gui/501",
                "/Users/ada/Library/LaunchAgents/dev.tendhost.hostd.plist"
            ]]
        );
        assert_eq!(
            user.stop_commands(),
            [["launchctl", "bootout", "gui/501/dev.tendhost.hostd"]]
        );
        assert_eq!(
            Manager::LaunchdSystem.stop_commands(),
            [["launchctl", "bootout", "system/dev.tendhost.hostd"]]
        );
        assert_eq!(
            Manager::Systemd.start_commands()[1],
            ["systemctl", "enable", "--now", "tendhostd.service"]
        );
    }
}
