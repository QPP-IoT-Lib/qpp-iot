//! AVR watchdog RC oscillator, timed against Timer1, as a [`JitterSource`].
//!
//! The watchdog runs from its own ~128 kHz RC oscillator, independent of
//! the 16 MHz crystal that clocks the CPU and Timer1. Each sample is one
//! watchdog period (~16 ms) measured in Timer1 ticks; the RC oscillator's
//! phase noise lands in the low bits. Ported unchanged from
//! `qpp-rng/hil-firmware/avr` (with `c/wdt_jitter.c`), where it was
//! measured at 1.62 (Mega 2560) and 2.24 (Nano) bits/sample, so about
//! one sample per 16 ms and 8 samples per output byte at credit 1.
//!
//! Build with `AVR_MCU=atmega2560` for the Mega 2560 (default
//! `atmega328p`), and `-C target-cpu=<mcu>` as for any AVR Rust build.

use qpp_rng_iot::JitterSource;

unsafe extern "C" {
    fn qpp_wdt_init();
    fn qpp_wdt_take(out: *mut u32) -> u8;
}

/// Blocks until the next watchdog interrupt and returns its Timer1
/// timestamp.
fn wdt_next() -> u32 {
    let mut t = 0u32;
    while unsafe { qpp_wdt_take(&mut t) } == 0 {}
    t
}

/// Watchdog-period samples in Timer1 ticks.
pub struct WdtJitter {
    prev: u32,
}

impl WdtJitter {
    /// Starts Timer1 and the watchdog interrupt, and waits for the first
    /// watchdog tick (up to ~16 ms).
    ///
    /// # Safety
    /// Takes over Timer1, its overflow interrupt and the watchdog timer
    /// (interrupt-only mode) for the lifetime of the program; at most one
    /// `WdtJitter` may exist and nothing else may use those. **Global
    /// interrupts must be enabled** (`sei`) before sampling, or `sample`
    /// blocks forever.
    pub unsafe fn new() -> Self {
        use entropy_timer::{HighResTimer, PlatformTimer};
        PlatformTimer.init();
        unsafe { qpp_wdt_init() };
        Self { prev: wdt_next() }
    }
}

impl JitterSource for WdtJitter {
    fn sample(&mut self) -> u32 {
        let t = wdt_next();
        let d = t.wrapping_sub(self.prev);
        self.prev = t;
        d
    }
}
