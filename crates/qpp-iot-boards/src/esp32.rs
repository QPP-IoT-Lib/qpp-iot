//! ESP32 on-chip RNG as a [`JitterSource`].
//!
//! The RNG is only documented as truly random while the radio or the SAR
//! ADC noise source is on, so [`HwRng::new`] takes the ADC and leaves that
//! noise source on for good. Ported from `qpp-rng/hil-firmware/esp32`; the
//! assessment is in `qpp-rng/hil-results/REPORT.md`.

use esp_hal::peripherals::{ADC1, RNG};
use esp_hal::rng::{Rng, TrngSource};
use qpp_rng_iot::JitterSource;

/// The on-chip RNG, one byte per sample.
pub struct HwRng(Rng);

impl HwRng {
    /// Starts the SAR ADC noise source. It stays on, and ADC1 stays
    /// unavailable, for the lifetime of the program.
    pub fn new(rng: RNG<'static>, adc: ADC1<'static>) -> Self {
        core::mem::forget(TrngSource::new(rng, adc));
        Self(Rng::new())
    }
}

impl JitterSource for HwRng {
    fn sample(&mut self) -> u32 {
        self.0.random() & 0xFF
    }
}
