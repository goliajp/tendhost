use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

pub type Timestamp = DateTime<Utc>;

/// The only source of "now". Code that reads the wall clock directly cannot
/// be tested against expiry, so nothing outside this file does.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Utc::now()
    }
}

/// A clock that moves only when told to.
pub struct ManualClock(Mutex<Timestamp>);

impl ManualClock {
    pub fn at(start: Timestamp) -> Self {
        Self(Mutex::new(start))
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.0.lock().expect("clock lock");
        *now += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().expect("clock lock")
    }
}
