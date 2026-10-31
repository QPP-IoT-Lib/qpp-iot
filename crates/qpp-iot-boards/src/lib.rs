//! Physical entropy sources for QPP-IoT, one module per supported board.
//!
//! Each source implements [`qpp_rng_iot::JitterSource`]: a raw
//! measurement of something that is physically independent of the CPU
//! clock, with its entropy in the low-order bits. Why this matters, and
//! why timing the CPU's own work (the original QPP-RNG design) does not
//! work on bare-metal chips, is in `qpp-rng/hil-results/REPORT.md`.
//!
//! | Feature | Board | Source | NIST `ea_non_iid` (1 M samples) | Credit to use |
//! |---|---|---|---|---|
//! | `avr-wdt` | Arduino Uno / Nano / Mega 2560 | watchdog RC oscillator vs Timer1 | 1.62 (Mega), 2.24 (Nano) bits/sample | 1 |
//! | `nrf52840` | makerdiary nRF52840 MDK | on-chip RNG, bias correction on | 7.24 bits/byte | 4 |
//! | `esp32` | ESP32 | on-chip RNG, SAR ADC noise source on | 7.15 bits/byte | 4 |
//! | `mock` | host only | deterministic PRNG, **no entropy** | n/a | n/a |
//!
//! The credit is the `min_entropy_bits` you pass to
//! [`qpp_rng_iot::QppRngIot::new`] (via `qpp_iot::QppEntropy::new`). It
//! sets both the health-test cutoffs and how many samples go into each
//! output byte, so use the table's value or re-assess your own board.
//!
//! Enable the feature for your board only: a board feature on the wrong
//! target is a compile error.

#![no_std]

#[cfg(all(feature = "avr-wdt", not(target_arch = "avr")))]
compile_error!("feature `avr-wdt` only builds for target_arch = \"avr\"");

#[cfg(all(feature = "nrf52840", not(all(target_arch = "arm", target_os = "none"))))]
compile_error!("feature `nrf52840` only builds for bare-metal ARM (thumbv7em-none-eabihf)");

#[cfg(all(feature = "esp32", not(target_arch = "xtensa")))]
compile_error!("feature `esp32` only builds for xtensa-esp32-none-elf");

#[cfg(feature = "avr-wdt")]
pub mod avr_wdt;
#[cfg(feature = "nrf52840")]
pub mod nrf52840;
#[cfg(feature = "esp32")]
pub mod esp32;
#[cfg(feature = "mock")]
pub mod mock;
