use data_encoding::BASE32_NOPAD;
use rand::Rng;

use crate::Timestamp;

/// `<prefix>_<time><random>`: ids of one kind sort by creation time, and the
/// prefix says what an id refers to wherever it turns up (logs, urls).
pub fn new(prefix: &str, now: Timestamp) -> String {
    let mut bytes = [0u8; 16];
    bytes[..6].copy_from_slice(&now.timestamp_millis().to_be_bytes()[2..]);
    rand::rng().fill_bytes(&mut bytes[6..]);
    format!("{prefix}_{}", BASE32_NOPAD.encode(&bytes).to_lowercase())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::*;

    #[test]
    fn later_ids_sort_after_earlier_ones() {
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let a = new("host", t);
        let b = new("host", t + Duration::milliseconds(1));
        assert!(a < b);
        assert!(a.starts_with("host_"));
    }

    #[test]
    fn ids_made_in_the_same_millisecond_differ() {
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert_ne!(new("host", t), new("host", t));
    }
}
