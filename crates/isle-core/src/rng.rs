//! A small seeded generator, saved with the game so a resumed island rolls the same dice.

use serde::{Deserialize, Serialize};

/// SplitMix64: one word of state, good enough for dice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x6a09_e667_f3bc_c908)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A whole number in `0..n`.
    pub(crate) fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % u64::from(n.max(1))) as u32
    }

    /// A whole number in `lo..=hi`.
    pub(crate) fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    /// True `percent` times in a hundred.
    pub(crate) fn chance(&mut self, percent: i32) -> bool {
        (self.below(100) as i32) < percent
    }

    /// A twenty-sided die.
    pub(crate) fn d20(&mut self) -> i32 {
        self.range(1, 20)
    }
}
