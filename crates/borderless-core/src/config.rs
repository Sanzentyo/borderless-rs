use crate::favorite::Favorite;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PollInterval(#[serde(with = "duration_ms")] pub Duration);

impl Default for PollInterval {
    fn default() -> Self {
        Self(Duration::from_secs(3))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub slow_window_detection: bool,
    #[serde(default, rename = "poll_interval_ms")]
    pub poll_interval: PollInterval,
    #[serde(default)]
    pub favorites: Vec<Favorite>,
}

impl AppConfig {
    #[must_use]
    pub const fn effective_poll_interval(&self) -> Duration {
        if self.slow_window_detection {
            Duration::from_secs(10)
        } else {
            self.poll_interval.0
        }
    }
}

mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = u64::try_from(value.as_millis()).map_err(serde::ser::Error::custom)?;
        serializer.serialize_u64(millis)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(Duration::from_millis)
    }
}
