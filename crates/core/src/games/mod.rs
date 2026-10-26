//! Mini-games (spec §12, §14): the Arabic Bridge games, Rupiah Reader and
//! Pasar!. Pure logic over build-time content passed in as text, so the same
//! code runs in the app and in tests. Every game is deterministic for a seed.

pub mod bridge;
pub mod pasar;
pub mod rupiah;

/// A small deterministic PRNG (SplitMix64): the same seed gives the same
/// rounds on every device.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`; `n` must be > 0.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            let x = a.below(10);
            assert_eq!(x, b.below(10));
            assert!(x < 10);
        }
        let mut v: Vec<u32> = (0..20).collect();
        Rng::new(1).shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
        assert_ne!(v, sorted);
    }
}
