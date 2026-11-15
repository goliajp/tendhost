use anyhow::{Context, Result};
use sysinfo::{Disks, System};
use tendhost_protocol::{Disk, Facts, Metrics};

pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn hostname() -> Result<String> {
    System::host_name().context("reading this machine's hostname")
}

pub fn facts() -> Result<Facts> {
    let mut system = System::new().context("opening the system information source")?;
    system.refresh_memory();
    system.refresh_cpu_all();
    Ok(Facts {
        hostname: hostname()?,
        os: System::name().context("reading the operating system name")?,
        os_version: System::os_version().context("reading the operating system version")?,
        arch: System::cpu_arch(),
        cpu_count: system.cpus().len() as u32,
        memory_bytes: system.total_memory(),
        daemon_version: DAEMON_VERSION.to_string(),
    })
}

pub fn metrics() -> Result<Metrics> {
    let mut system = System::new().context("opening the system information source")?;
    system.refresh_memory();
    let found = Disks::new_with_refreshed_list()
        .context("listing disks")?
        .iter()
        .map(|disk| Volume {
            mount: disk.mount_point().to_string_lossy().into_owned(),
            total_bytes: disk.total_space(),
            used_bytes: disk.total_space().saturating_sub(disk.available_space()),
            read_only: disk.is_read_only(),
        })
        .collect();
    let disks = tidy(found, std::env::consts::OS);
    Ok(Metrics {
        load1: System::load_average()
            .context("reading the load average")?
            .one as f32,
        memory_used_bytes: system.used_memory(),
        uptime_secs: System::uptime().context("reading the uptime")?,
        disks,
    })
}

/// A mounted volume as the system lists it.
struct Volume {
    mount: String,
    total_bytes: u64,
    used_bytes: u64,
    read_only: bool,
}

/// The volumes worth showing as disks, one per mount, sorted.
///
/// macOS splits the startup disk into a sealed system volume at `/` and
/// the volume holding everything that changes at `/System/Volumes/Data`,
/// plus several small service volumes. What people mean by "the disk" is
/// the data volume, so it is shown as `/` and the rest of
/// `/System/Volumes` is left out, as are read-only mounts (disk images).
fn tidy(mut volumes: Vec<Volume>, os: &str) -> Vec<Disk> {
    const DATA: &str = "/System/Volumes/Data";
    volumes.retain(|v| v.total_bytes > 0);
    if os == "macos" {
        if volumes.iter().any(|v| v.mount == DATA) {
            volumes.retain(|v| v.mount != "/");
        }
        volumes.retain(|v| {
            v.mount == DATA || !(v.mount.starts_with("/System/Volumes/") || v.read_only)
        });
        volumes
            .iter_mut()
            .filter(|v| v.mount == DATA)
            .for_each(|v| v.mount = "/".to_string());
    }
    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));
    // one volume can be listed once per mount option set
    volumes.dedup_by(|a, b| a.mount == b.mount);
    volumes
        .into_iter()
        .map(|v| Disk {
            mount: v.mount,
            total_bytes: v.total_bytes,
            used_bytes: v.used_bytes,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_machine_reports_plausible_facts() {
        let facts = facts().unwrap();
        assert!(!facts.hostname.is_empty());
        assert!(!facts.arch.is_empty());
        assert!(facts.cpu_count >= 1);
        assert!(facts.memory_bytes > 64 << 20);
    }

    fn volume(mount: &str, used_bytes: u64, read_only: bool) -> Volume {
        Volume {
            mount: mount.into(),
            total_bytes: 1000,
            used_bytes,
            read_only,
        }
    }

    #[test]
    fn on_a_mac_the_data_volume_stands_for_the_startup_disk() {
        let volumes = vec![
            volume("/", 10, true),
            volume("/System/Volumes/Data", 600, false),
            volume("/System/Volumes/VM", 5, false),
            volume("/System/Volumes/Preboot", 5, false),
            volume("/Volumes/Backup", 300, false),
            volume("/Volumes/Some Installer", 900, true),
        ];
        let disks = tidy(volumes, "macos");
        let got: Vec<_> = disks
            .iter()
            .map(|d| (d.mount.as_str(), d.used_bytes))
            .collect();
        assert_eq!(got, [("/", 600), ("/Volumes/Backup", 300)]);
    }

    #[test]
    fn elsewhere_every_volume_with_a_size_is_kept_once() {
        let mut empty = volume("/proc", 0, false);
        empty.total_bytes = 0;
        let volumes = vec![
            volume("/home", 2, false),
            volume("/", 1, false),
            volume("/", 1, false),
            volume("/mnt/ro", 3, true),
            empty,
        ];
        let got: Vec<_> = tidy(volumes, "linux")
            .into_iter()
            .map(|d| d.mount)
            .collect();
        assert_eq!(got, ["/", "/home", "/mnt/ro"]);
    }

    #[test]
    fn this_machine_reports_plausible_metrics() {
        let metrics = metrics().unwrap();
        assert!(metrics.memory_used_bytes > 0);
        assert!(metrics.load1 >= 0.0);
        assert!(!metrics.disks.is_empty());
        assert!(metrics.disks.iter().all(|d| d.used_bytes <= d.total_bytes));
        let mut mounts: Vec<_> = metrics.disks.iter().map(|d| &d.mount).collect();
        mounts.dedup();
        assert_eq!(mounts.len(), metrics.disks.len());
    }
}
