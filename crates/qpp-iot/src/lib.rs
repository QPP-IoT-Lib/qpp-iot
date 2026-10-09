//! QPP-IoT: physical-entropy random numbers and Ascon-AEAD128 for
//! bare-metal IoT devices, behind one dependency.
//!
//! This crate is a thin facade over three pieces that live and are
//! versioned elsewhere:
//!
//! * [`QppEntropy`]: `qpp-rng-iot` (QPP-RNG with health tests) fed by a
//!   board's physical jitter source and conditioned with SHA-256
//!   (QPP-RNG-IoT repository);
//! * [`aead`]: the baseline Ascon-AEAD128 from `ascon-iot` (ASCON-IoT
//!   repository);
//! * `boards` (with a board feature): the per-board entropy sources in
//!   `qpp-iot-boards`.
//!
//! ```ignore
//! // On an nRF52840 (feature `nrf52840`):
//! use qpp_iot::{QppEntropy, aead};
//!
//! let source = unsafe { qpp_iot::boards::nrf52840::HwRng::new() };
//! let mut rng = QppEntropy::new(source, 4);          // credit: 4 bits/byte
//!
//! let key = aead::generate_key(&mut rng);
//! let nonce = aead::generate_random_nonce(&mut rng); // see the nonce note in `ascon-iot`
//! let cipher = aead::BaselineAsconAead128::new(&key);
//! let mut msg = *b"hello";
//! let tag = cipher.encrypt_in_place(&nonce, b"ad", &mut msg)?;
//! ```
//!
//! # Status
//!
//! A scaffold: the plumbing builds and is tested on the host with a
//! mock source, but the library is not yet validated as a whole. The
//! gates are in the README.

#![no_std]

pub mod entropy;

pub use entropy::QppEntropy;
pub use qpp_rng_iot::{HealthError, JitterSource};
pub use rng_core::{QppRngSource, RngDiagnostics};

/// Ascon-AEAD128 (NIST SP 800-232), re-exported from `ascon-iot`.
pub mod aead {
    pub use ascon_iot::*;
}

/// Per-board entropy sources; enable the feature for your board
/// (`avr-wdt` or `nrf52840`).
#[cfg(any(feature = "avr-wdt", feature = "nrf52840"))]
pub use qpp_iot_boards as boards;
