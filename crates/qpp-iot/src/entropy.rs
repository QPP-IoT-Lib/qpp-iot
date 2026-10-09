//! The entropy pipeline: physical jitter -> QPP-RNG -> SHA-256 -> `Rng`.

use core::convert::Infallible;

use conditioning::Sha256Conditioner;
use qpp_rng_iot::{DEFAULT_ARRAY_SIZE, JitterSource, PanicOnHealthFailure, QppRngIot};
use rand_core::TryRng;
use rng_core::{QppRngSource, RngDiagnostics};

/// A conditioned random byte stream, ready to make keys.
///
/// ```text
/// JitterSource ──► QppRngIot ──► Sha256Conditioner ──► rand_core::Rng
///  (the board)     (SP 800-90B     (64 raw bytes in,
///                   health tests)   32 out)
/// ```
///
/// Raw QPP-RNG output carries a measurable bias and is not uniform; the
/// SHA-256 stage is what makes it suitable for keys and nonces
/// (`qpp-rng/docs/how-it-works.md` §3), so this is the only way the
/// library offers to turn a source into an `Rng`.
///
/// # Health-test failures panic
///
/// If the source fails the SP 800-90B repetition-count or adaptive-
/// proportion test (a stuck or degraded oscillator), the next read
/// panics instead of returning weak bytes. The conditioner needs an
/// infallible source, so a non-panicking path needs `conditioning` to
/// accept a fallible one first (see the roadmap in the README). Until
/// then, set `panic = "abort"` and treat a panic as "reset and
/// re-seed", never as "carry on".
///
/// # Not yet `CryptoRng`
///
/// This type deliberately does not implement `rand_core::CryptoRng`:
/// that marker is a security claim to downstream code, to be added only
/// after every validation gate in the README is met.
pub struct QppEntropy<J: JitterSource> {
    inner: Sha256Conditioner<PanicOnHealthFailure<J, DEFAULT_ARRAY_SIZE>>,
}

impl<J: JitterSource> QppEntropy<J> {
    /// Builds the pipeline over `source`, crediting `min_entropy_bits`
    /// of min-entropy to each of its samples (the "Credit to use"
    /// column in the `qpp-iot-boards` docs). This sets the health-test
    /// cutoffs and the number of samples absorbed per output byte.
    pub fn new(source: J, min_entropy_bits: u8) -> Self {
        Self {
            inner: Sha256Conditioner::new(PanicOnHealthFailure(QppRngIot::new(
                source,
                min_entropy_bits,
            ))),
        }
    }
}

impl<J: JitterSource> TryRng for QppEntropy<J> {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        self.inner.try_next_u32()
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        self.inner.try_next_u64()
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        self.inner.try_fill_bytes(dst)
    }
}

impl<J: JitterSource> QppRngSource for QppEntropy<J> {
    fn diagnostics(&self) -> RngDiagnostics {
        self.inner.diagnostics()
    }
}
