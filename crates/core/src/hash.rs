//! Integer hashing that stands in for random number generation.
//!
//! Everything random in the world is a hash of its inputs rather than a draw from a stateful RNG, so any
//! value can be computed on its own, in any order, and comes out the same every time.

/// The SplitMix64 finalizer: a fast bijective mixer whose output bits each depend on every input bit.
pub(crate) const fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Folds `value` into `state`. Mixing the value before xoring keeps `combine(a, b)` and `combine(b, a)`
/// unrelated, and keeps nearby values (adjacent lattice points) from producing nearby states.
pub(crate) const fn combine(state: u64, value: u64) -> u64 {
    mix(state ^ mix(value.wrapping_add(0x9e37_79b9_7f4a_7c15)))
}

/// Hash of a seed and a lattice point: 64 bits, each depending on every bit of all three inputs, so
/// neighboring points come out unrelated. The same on every machine.
#[must_use]
pub const fn hash_point(seed: u64, x: i64, y: i64) -> u64 {
    combine(combine(seed, x as u64), y as u64)
}

/// A sub-seed for one named stream, so streams drawn from the same world seed don't correlate.
pub(crate) const fn derive(seed: u64, label: &[u8; 8]) -> u64 {
    combine(seed, u64::from_le_bytes(*label))
}

/// A stream of pseudo-random draws, for the places that need many values from one key: evolving a pool,
/// a turn of monster moves. It is still a pure function of its key, so two streams from the same key give
/// the same draws.
///
/// Key a stream by everything the draws should depend on (see [`Rng::keyed`]) rather than threading one
/// stream through unrelated systems, so that adding a draw in one place never shifts the draws in another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// A stream starting from `state`.
    #[must_use]
    pub const fn new(state: u64) -> Rng {
        Rng(state)
    }

    /// A stream keyed by a world seed, a label naming what it is for, and any number of further inputs.
    #[must_use]
    pub fn keyed(seed: u64, label: &[u8; 8], keys: &[u64]) -> Rng {
        Rng(keys.iter().fold(derive(seed, label), |state, &k| combine(state, k)))
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    /// A uniform draw from `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        // 24 bits fill an f32 mantissa exactly, so every value is representable and below 1.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A uniform draw from `0..n`. `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        // The high bits, scaled rather than taken modulo, so small ranges aren't biased.
        (((self.next_u64() >> 32) * n as u64) >> 32) as usize
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    /// An approximately normal draw with mean 0 and standard deviation 1: the sum of four uniforms, rescaled.
    /// Bounded to about ±3.5, which suits mutation steps better than a true normal's rare huge jumps.
    pub fn normal(&mut self) -> f32 {
        let sum: f32 = (0..4).map(|_| self.unit()).sum();
        // Four uniforms have mean 2 and variance 1/3.
        (sum - 2.0) * 3f32.sqrt()
    }

    /// An index drawn in proportion to `weights`. Weights must be non-negative; if they are all zero, the
    /// draw is uniform. `weights` must not be empty.
    pub fn weighted(&mut self, weights: &[f32]) -> usize {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return self.below(weights.len());
        }
        let mut target = self.unit() * total;
        for (i, &w) in weights.iter().enumerate() {
            if target < w {
                return i;
            }
            target -= w;
        }
        // Rounding can leave a sliver past the last weight; it belongs to the last positive one.
        weights.iter().rposition(|&w| w > 0.0).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_hash_apart() {
        let a = hash_point(7, 10, 10);
        let b = hash_point(7, 11, 10);
        let c = hash_point(7, 10, 11);
        // Roughly half the bits should differ between neighbors; a weak hash shows up as far fewer.
        assert!((a ^ b).count_ones() > 16);
        assert!((a ^ c).count_ones() > 16);
        assert!((b ^ c).count_ones() > 16);
    }

    #[test]
    fn order_matters() {
        assert_ne!(hash_point(1, 2, 3), hash_point(1, 3, 2));
    }

    #[test]
    fn streams_replay_and_stay_in_range() {
        let draws = |key| {
            let mut rng = Rng::keyed(9, b"testtest", &[key]);
            (0..64).map(|_| (rng.unit(), rng.below(7), rng.normal())).collect::<Vec<_>>()
        };
        assert_eq!(draws(1), draws(1));
        assert_ne!(draws(1), draws(2));
        for (u, b, n) in draws(3) {
            assert!((0.0..1.0).contains(&u) && b < 7 && n.abs() < 3.5);
        }
    }

    #[test]
    fn weighted_draws_skip_zero_weights() {
        let mut rng = Rng::new(5);
        for _ in 0..200 {
            assert_ne!(rng.weighted(&[1.0, 0.0, 2.0]), 1);
        }
    }

    #[test]
    fn labels_separate_streams() {
        assert_ne!(derive(42, b"elevatio"), derive(42, b"moisture"));
    }
}
