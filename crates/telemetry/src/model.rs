use chrono::Duration;
use serde::{Deserialize, Serialize};
use tendhost_kernel::Timestamp;

/// A host not heard from for this long is offline. Three missed syncs at
/// the default interval.
pub const OFFLINE_AFTER: Duration = Duration::seconds(90);
pub const RETENTION: Duration = Duration::days(7);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub at: Timestamp,
    pub load1: f32,
    pub memory_used_bytes: u64,
    pub uptime_secs: u64,
    pub disks: Vec<Disk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    pub host_id: String,
    pub last_seen: Timestamp,
    pub online: bool,
}

/// At most `points` samples standing for `samples`: the span is cut into
/// equal stretches of time and each stretch with samples becomes one, at
/// the stretch's last sample time, with the mean load and memory and the
/// last sample's disks. Fewer samples than `points` come back unchanged.
pub(crate) fn thin(samples: Vec<Sample>, points: usize) -> Vec<Sample> {
    if points == 0 || samples.len() <= points {
        return samples;
    }
    let (first, last) = (samples[0].at, samples[samples.len() - 1].at);
    let span = (last - first).num_milliseconds().max(1) as f64;
    let mut stretches: Vec<Vec<Sample>> = vec![Vec::new(); points];
    for sample in samples {
        let share = (sample.at - first).num_milliseconds() as f64 / span;
        stretches[((share * points as f64) as usize).min(points - 1)].push(sample);
    }
    stretches
        .into_iter()
        .filter(|stretch| !stretch.is_empty())
        .map(|stretch| {
            let n = stretch.len() as f64;
            let load = stretch.iter().map(|s| f64::from(s.load1)).sum::<f64>() / n;
            let memory = stretch
                .iter()
                .map(|s| s.memory_used_bytes as f64)
                .sum::<f64>()
                / n;
            let last = stretch
                .into_iter()
                .next_back()
                .expect("the stretch is not empty");
            Sample {
                load1: load as f32,
                memory_used_bytes: memory as u64,
                ..last
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn samples(n: i64) -> Vec<Sample> {
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        (0..n)
            .map(|i| Sample {
                at: t0 + Duration::seconds(30 * i),
                load1: i as f32,
                memory_used_bytes: 100 * i as u64,
                uptime_secs: i as u64,
                disks: vec![Disk {
                    mount: "/".into(),
                    total_bytes: 100,
                    used_bytes: i as u64,
                }],
            })
            .collect()
    }

    #[test]
    fn many_samples_become_at_most_the_asked_number_covering_the_whole_span() {
        let thinned = thin(samples(1000), 100);
        assert!(thinned.len() <= 100 && thinned.len() >= 99);
        assert_eq!(thinned.last().unwrap().at, samples(1000).last().unwrap().at);
        assert!(thinned.windows(2).all(|pair| pair[0].at < pair[1].at));
    }

    #[test]
    fn a_stretch_is_the_mean_of_its_load_and_memory_with_its_last_disks() {
        let thinned = thin(samples(4), 2);
        assert_eq!(thinned.len(), 2);
        assert_eq!(
            (
                thinned[0].load1,
                thinned[0].memory_used_bytes,
                thinned[0].disks[0].used_bytes
            ),
            (0.5, 50, 1)
        );
        assert_eq!((thinned[1].load1, thinned[1].disks[0].used_bytes), (2.5, 3));
    }

    #[test]
    fn few_samples_and_no_limit_are_left_alone() {
        assert_eq!(thin(samples(5), 10), samples(5));
        assert_eq!(thin(samples(5), 0), samples(5));
        assert!(thin(Vec::new(), 10).is_empty());
    }
}
