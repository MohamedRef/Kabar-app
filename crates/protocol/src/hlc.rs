use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable identifier of one installation of Kabar (one per device).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hybrid logical clock timestamp.
///
/// Ordered by wall time, then counter, then device id, so two devices can
/// never produce equal timestamps and last-writer-wins is deterministic.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hlc {
    /// Milliseconds since the Unix epoch.
    pub wall_ms: u64,
    /// Logical counter for events within the same millisecond.
    pub counter: u32,
    pub device: DeviceId,
}

impl Ord for Hlc {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.wall_ms, self.counter, &self.device).cmp(&(other.wall_ms, other.counter, &other.device))
    }
}

impl PartialOrd for Hlc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hlc(wall_ms: u64, counter: u32, device: &str) -> Hlc {
        Hlc { wall_ms, counter, device: DeviceId(device.into()) }
    }

    #[test]
    fn orders_by_wall_then_counter_then_device() {
        assert!(hlc(1, 9, "z") < hlc(2, 0, "a"));
        assert!(hlc(2, 0, "z") < hlc(2, 1, "a"));
        assert!(hlc(2, 1, "a") < hlc(2, 1, "b"));
    }

    #[test]
    fn round_trips_through_json() {
        let h = hlc(1_761_472_800_000, 3, "s24u");
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(serde_json::from_str::<Hlc>(&json).unwrap(), h);
    }
}
