//! A stand-in source for host tests and examples.
//!
//! **It contains no entropy.** It is a seeded PRNG (SplitMix64) shaped
//! like a healthy jitter source (a fixed period plus noise in the low
//! bits), so the rest of the pipeline can be exercised without
//! hardware. Never use it to make real keys.

use qpp_rng_iot::JitterSource;

/// Deterministic fake jitter around a fixed period.
pub struct MockJitter {
    state: u64,
}

impl MockJitter {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl JitterSource for MockJitter {
    fn sample(&mut self) -> u32 {
        // Sum of four 6-bit draws on a 271,400-tick period: roughly the
        // spread of the AVR watchdog measurements.
        let noise: u32 = (0..4).map(|_| (self.next_u64() % 64) as u32).sum();
        271_400 + noise
    }
}

/// A source stuck at one value: what a dead oscillator looks like. The
/// health tests must reject it.
pub struct StuckJitter(pub u32);

impl JitterSource for StuckJitter {
    fn sample(&mut self) -> u32 {
        self.0
    }
}
