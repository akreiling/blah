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

/// Hash of a seed and a lattice point.
pub(crate) const fn hash_point(seed: u64, x: i64, y: i64) -> u64 {
    combine(combine(seed, x as u64), y as u64)
}

/// A sub-seed for one named stream, so streams drawn from the same world seed don't correlate.
pub(crate) const fn derive(seed: u64, label: &[u8; 8]) -> u64 {
    combine(seed, u64::from_le_bytes(*label))
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
    fn labels_separate_streams() {
        assert_ne!(derive(42, b"elevatio"), derive(42, b"moisture"));
    }
}
