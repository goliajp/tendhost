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
    let mut disks: Vec<Disk> = Disks::new_with_refreshed_list()
        .context("listing disks")?
        .iter()
        .filter(|disk| disk.total_space() > 0)
        .map(|disk| Disk {
            mount: disk.mount_point().to_string_lossy().into_owned(),
            total_bytes: disk.total_space(),
            used_bytes: disk.total_space().saturating_sub(disk.available_space()),
        })
        .collect();
    // one volume can be listed once per mount option set
    disks.sort_by(|a, b| a.mount.cmp(&b.mount));
    disks.dedup_by(|a, b| a.mount == b.mount);
    Ok(Metrics {
        load1: System::load_average()
            .context("reading the load average")?
            .one as f32,
        memory_used_bytes: system.used_memory(),
        uptime_secs: System::uptime().context("reading the uptime")?,
        disks,
    })
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
