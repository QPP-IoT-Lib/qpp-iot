//! nRF52840 on-chip RNG peripheral as a [`JitterSource`].
//!
//! The peripheral samples thermal noise; digital bias correction is
//! switched on. Ported unchanged from `qpp-rng/hil-firmware/nrf52840`,
//! where it was measured at 7.24 bits/byte (NIST `ea_non_iid`, 1 M
//! samples). Bias correction makes it slow (about 0.7 ms per byte).
//!
//! The nRF52840's LFRC-vs-cycle-counter cross-clock source was tried
//! first and assessed at only 0.60 bits/sample, so it is not offered.

use core::ptr::{read_volatile, write_volatile};
use qpp_rng_iot::JitterSource;

const RNG_START: *mut u32 = 0x4000_D000 as *mut u32;
const RNG_VALRDY: *mut u32 = 0x4000_D100 as *mut u32;
const RNG_CONFIG: *mut u32 = 0x4000_D504 as *mut u32;
const RNG_VALUE: *mut u32 = 0x4000_D508 as *mut u32;

/// The on-chip RNG, one byte per sample.
pub struct HwRng(());

impl HwRng {
    /// Starts the RNG peripheral with bias correction on.
    ///
    /// # Safety
    /// Takes over the RNG peripheral's registers for the lifetime of the
    /// program: nothing else (a SoftDevice, a HAL driver) may use the
    /// peripheral while this exists, and at most one `HwRng` may exist.
    pub unsafe fn new() -> Self {
        unsafe {
            write_volatile(RNG_CONFIG, 1);
            write_volatile(RNG_VALRDY, 0);
            write_volatile(RNG_START, 1);
        }
        Self(())
    }
}

impl JitterSource for HwRng {
    fn sample(&mut self) -> u32 {
        unsafe {
            while read_volatile(RNG_VALRDY) == 0 {}
            let v = read_volatile(RNG_VALUE) as u8;
            write_volatile(RNG_VALRDY, 0);
            v as u32
        }
    }
}
