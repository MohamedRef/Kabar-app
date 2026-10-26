//! Hybrid logical clock (Kulkarni et al.): timestamps that follow wall time
//! but never go backwards, even when a device's clock does.

use kabar_protocol::{DeviceId, Hlc};

#[derive(Clone, Debug)]
pub struct HlcClock {
    device: DeviceId,
    last_wall_ms: u64,
    last_counter: u32,
}

impl HlcClock {
    pub fn new(device: DeviceId) -> Self {
        Self { device, last_wall_ms: 0, last_counter: 0 }
    }

    /// Restores a clock so it never issues a timestamp at or below `last`.
    pub fn resume(device: DeviceId, last: &Hlc) -> Self {
        Self { device, last_wall_ms: last.wall_ms, last_counter: last.counter }
    }

    /// Timestamp for a local event. `now_ms` is the device's wall clock.
    pub fn tick(&mut self, now_ms: u64) -> Hlc {
        if now_ms > self.last_wall_ms {
            self.last_wall_ms = now_ms;
            self.last_counter = 0;
        } else {
            self.last_counter += 1;
        }
        self.current()
    }

    /// Merges a timestamp received from another device, then ticks.
    pub fn observe(&mut self, remote: &Hlc, now_ms: u64) -> Hlc {
        let wall = now_ms.max(self.last_wall_ms).max(remote.wall_ms);
        self.last_counter = if wall == self.last_wall_ms && wall == remote.wall_ms {
            self.last_counter.max(remote.counter) + 1
        } else if wall == self.last_wall_ms {
            self.last_counter + 1
        } else if wall == remote.wall_ms {
            remote.counter + 1
        } else {
            0
        };
        self.last_wall_ms = wall;
        self.current()
    }

    fn current(&self) -> Hlc {
        Hlc { wall_ms: self.last_wall_ms, counter: self.last_counter, device: self.device.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(name: &str) -> HlcClock {
        HlcClock::new(DeviceId(name.into()))
    }

    #[test]
    fn is_monotonic_when_wall_clock_goes_backwards() {
        let mut c = clock("mac");
        let a = c.tick(1_000);
        let b = c.tick(900);
        let d = c.tick(1_000);
        assert!(a < b && b < d);
        assert_eq!(b.wall_ms, 1_000);
    }

    #[test]
    fn observing_a_future_remote_moves_past_it() {
        let mut phone = clock("s24u");
        let mut tablet = clock("tab");
        let remote = tablet.tick(5_000);
        let local = phone.observe(&remote, 1_000);
        assert!(local > remote);
        assert!(phone.tick(1_001) > local);
    }

    #[test]
    fn resume_never_repeats() {
        let mut c = clock("mac");
        let last = c.tick(2_000);
        let mut resumed = HlcClock::resume(DeviceId("mac".into()), &last);
        assert!(resumed.tick(2_000) > last);
    }
}
