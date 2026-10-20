//! A tiny seeded generator for placement. Its output is fixed by the algorithm (`SplitMix64`), not by
//! a library version, so the same seed places the same tufts forever and the art stays stable.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        // The top 53 bits fill an f64's mantissa exactly.
        #[allow(clippy::cast_precision_loss)]
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        unit
    }

    /// Uniform in `[low, high)`.
    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    /// One of `items`, uniformly. `items` must not be empty.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        // A slice's length fits in u64, and the remainder fits back in usize.
        #[allow(clippy::cast_possible_truncation)]
        let index = (self.next_u64() % items.len() as u64) as usize;
        &items[index]
    }
}

#[cfg(test)]
mod unit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn matches_the_splitmix64_reference() {
        // The first outputs for seed 1234567 from the algorithm's reference implementation.
        let mut rng = Rng::new(1_234_567);
        assert_eq!(
            [rng.next_u64(), rng.next_u64(), rng.next_u64()],
            [6_457_827_717_110_365_317, 3_203_168_211_198_807_973, 9_817_491_932_198_370_423]
        );
    }

    #[test]
    fn units_stay_in_range() {
        let mut rng = Rng::new(9);
        assert!((0..1000).map(|_| rng.unit()).all(|u| (0.0..1.0).contains(&u)));
        assert!((0..1000).map(|_| rng.range(2.0, 3.0)).all(|u| (2.0..3.0).contains(&u)));
    }
}
