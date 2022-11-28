use chrono::TimeZone;
use std::fmt::{Debug, Formatter};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct TimePair {
    clock: Instant,
    system: SystemTime,
}

impl TimePair {
    pub fn now() -> Self {
        let clock = Instant::now();
        let system = SystemTime::now();
        Self { clock, system }
    }

    pub fn as_ts(&self) -> Instant {
        self.clock
    }

    pub fn to_rfc3339(&self) -> Option<String> {
        let diff = self.system.duration_since(UNIX_EPOCH).ok()?;
        let ts = chrono::Utc
            .timestamp_millis_opt(diff.as_millis().try_into().ok()?)
            .single()?;
        Some(ts.to_rfc3339())
    }
}

impl Debug for TimePair {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.to_rfc3339(), f)
    }
}
