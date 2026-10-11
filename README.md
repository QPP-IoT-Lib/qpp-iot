# QPP-IoT

Physical-entropy random numbers (QPP-RNG) and Ascon-AEAD128 for
bare-metal IoT devices, behind one dependency.

This is the **scaffold of the final library**: it combines two
independently versioned repositories without merging them, and adds the
board-specific glue neither of them has. It builds and its plumbing is
tested on the host; it is **not yet validated as a whole** (see
[Status](#status-and-what-is-left)).

```
 board hardware                 QPP-RNG-IoT repo                   ASCON-IoT repo
┌──────────────┐   samples   ┌──────────────────────────┐        ┌────────────────┐
│ JitterSource ├────────────►│ qpp-rng-iot  (health     │ bytes  │ ascon-iot      │
│ (this repo:  │             │   tests + QPP walk)      ├───────►│ generate_key,  │
│  qpp-iot-    │             │ conditioning (SHA-256)   │ Rng    │ nonce, encrypt │
│  boards)     │             └──────────────────────────┘        └────────────────┘
└──────────────┘                          └──────── QppEntropy (qpp-iot) ───────┘
```

## Layout

| Path | What |
|---|---|
| `crates/qpp-iot/` | the library you depend on: `QppEntropy` (source → QPP-RNG → SHA-256 → `rand_core::Rng`), `aead` (re-export of `ascon-iot`), `boards` (re-export of the board you enabled) |
| `crates/qpp-iot-boards/` | one `JitterSource` per board, behind a feature: `avr-wdt` (Arduino Uno/Nano/Mega 2560), `nrf52840`, and a host-only `mock` with **no entropy** |
| `firmware/avr-fit/` | smallest image of the whole stack on AVR; guards "fits an ATmega328P" |
| `firmware/e2e/` | end-to-end check on a real board (Nano/Uno, Mega 2560, nRF52840): key, nonce, encrypt/decrypt/tamper, timings, peak stack |
| `scripts/build-avr.sh` | builds `avr-fit` with the required flags and reports flash/RAM use |
| `scripts/build-e2e.sh` | builds `firmware/e2e` for an AVR chip |
| `scripts/e2e-check.py` | resets a board running `firmware/e2e` and checks its report against the ascon-c reference |

Not here, on purpose: the hardware test firmware, measurement tools and
results (`qpp-rng/hil-firmware`, `qpp-rng/hil-results`), the host test
harness (`qpp-rng/crates/test-harness`), the thesis.

## How the two repositories are combined

`qpp-iot-lib/` holds three sibling checkouts:

| Directory | Repository | Provides |
|---|---|---|
| `qpp-rng/` | QPP-RNG-IoT | `qpp-rng-iot`, `conditioning`, `rng-core`, `entropy-timer` |
| `ascon/` | ASCON-IoT | `ascon-iot` (baseline Ascon-AEAD128, generic over any `rand_core::Rng`) |
| `qpp-iot/` | (this) | the facade and the board sources |

The root `Cargo.toml` depends on the first two by **path** while
developing, so a change in either is picked up here immediately. Before
a release, replace each path with a pinned git revision (the way
`ascon/Cargo.toml` already pins `qpp-rng-reference`); the exact lines
are in the root `Cargo.toml`. Keeping the repos separate keeps them
independently versioned, which ASCON-IoT's README states as a goal. Merge
them into one repository only if you will always release them in
lockstep.

## Using it

```rust
// on an nRF52840, with feature `nrf52840`
use qpp_iot::{QppEntropy, aead};

let source = unsafe { qpp_iot::boards::nrf52840::HwRng::new() };
let mut rng = QppEntropy::new(source, 4);   // credit: 4 bits of min-entropy per sample

let key = aead::generate_key(&mut rng);
let nonce = aead::generate_random_nonce(&mut rng);
let cipher = aead::BaselineAsconAead128::new(&key);
let mut msg = *b"hello";
let tag = cipher.encrypt_in_place(&nonce, b"header", &mut msg)?;
```

The **credit** (`QppEntropy::new`'s second argument) is the min-entropy
you can honestly claim per source sample. It sets the SP 800-90B
health-test cutoffs and how many samples go into each output byte.
Use the board's value from the table in `qpp-iot-boards`' docs, or
assess your own board first; a credit that is too high silently weakens
the output.

## Building and testing

| Target | Command |
|---|---|
| Host: all tests (plumbing, with the mock source) | `cargo test --workspace` |
| Host: demo | `cargo run -p qpp-iot --example host_demo` |
| nRF52840 | `cargo build -p qpp-iot --features nrf52840 --target thumbv7em-none-eabihf --release` |
| AVR size check (Nano/Uno chip, Mega 2560 chip) | `scripts/build-avr.sh atmega328p`, `scripts/build-avr.sh atmega2560` |
| End-to-end firmware, AVR | `scripts/build-e2e.sh atmega328p` (or `atmega2560`), then flash the ELF with avrdude |
| End-to-end firmware, nRF52840 | `cd firmware/e2e && cargo build --release --target thumbv7em-none-eabihf` |
| End-to-end check on the board | `uv run scripts/e2e-check.py --port <port> [--reset break for the nRF52840] --ascon-c <ascon_c_driver>` |

A board feature on the wrong target is a compile error, so
`--all-features` does not work on any single target.

### AVR: two compiler flags are mandatory

Measured on the ATmega2560 with the stack above:

| Build | Flash |
|---|---|
| default | 136 KB (SHA-256 alone 79 KB, Ascon's permutation 30 KB) |
| + `--cfg sha2_backend_soft="compact"` | 59 KB |
| + `--cfg ascon_backend="soft-compact"` | **16 KB** (49% of an ATmega328P) |

The QPP-RNG entropy path itself is under 3 KB. Both flags are
compile-time `cfg`s of the RustCrypto crates, which a library cannot
set: **every AVR application using this library must pass them** in its
`RUSTFLAGS` (`scripts/build-avr.sh` shows how, alongside
`-C target-cpu=<mcu>`, `-Z build-std=core` on nightly, and
`AVR_MCU=<mcu>` for the C shims). With the compact backends, Ascon-AEAD128
on the ATmega328P at 16 MHz takes 4.4 ms for an empty message with 16 B
of associated data, then about 860 cycles per message byte (7.8 ms for
64 B). Measured on the board with `firmware/e2e`, see
`qpp-rng/hil-results/REPORT.md` §6.

Two more things measured on the board:

- **Use `opt-level = "z"` when the image also decrypts.** At `"s"`, LLVM
  inlines Ascon's permutation into every caller, 3–5.5 KB each on AVR.
  Key, nonce, encryption and decryption together come to 34.7 KB on an
  ATmega328P, 1.9 KB more than it has. At `"z"` they come to 21.0 KB.
- **Static RAM is `.data` + `.bss`, not just `.bss`.** On AVR, Rust's
  constants (including SHA-256's 256-byte table) are copied to RAM at
  startup. `avr-size`'s default output hides that inside `text`; use
  `avr-size -A`. `avr-fit` uses 366 B of static RAM, and `firmware/e2e`
  uses 662 B plus a peak stack of about 1 KB.

## Status and what is left

What exists and has been checked: the whole stack builds for the host,
the nRF52840 and both AVR chips; host tests cover the key/nonce →
Ascon round trip, authentication failure, diagnostics, and a dead source
being rejected; the AVR image links and fits. On the Arduino Nano,
the Arduino Mega 2560 and the nRF52840, `firmware/e2e` ran the whole stack from the board's
physical source. Every reset gave a different key and nonce, every
ciphertext and tag matched the ascon-c reference, and tampered messages
were rejected (`qpp-rng/hil-results/REPORT.md` §6).

Before this is "the final library", in rough order:

**Validate**
- [ ] Re-run `qpp-rng/hil-firmware/tools/smoke.sh <board>` on each board:
      the refactored `iot` firmware has not been run on hardware.
- [x] Finish the 1 MB output runs on the Arduinos and assess them with
      NIST `ea_iid` / SP 800-22: both pass, like the nRF52840's.
- [ ] Re-check the credited entropy per source (`avr-wdt`: 1 bit,
      `nrf52840`: 4 bits) against the final long-run data.
- [x] Run the final image on the boards, not just the host mock: end to
      end key generation and encryption on the Nano, Mega 2560 and
      nRF52840 (`firmware/e2e`).
- [x] Measure **stack/RAM** on the ATmega328P (2 KB total): 662 B static
      (`firmware/e2e`; 366 B for `avr-fit`) + about 1 KB peak stack,
      leaving 357 B untouched.
- [x] Measure speed and seed time on AVR with the compact backends: a
      128-bit key and its nonce take 8.7 s (source-limited); Ascon takes
      4.4–7.8 ms for 0–64 B messages.

**Complete the design**
- [ ] **Nonce strategy.** `generate_random_nonce` does not guarantee
      uniqueness across resets (ascon-iot says so itself). A deployed
      device needs a counter or similar that survives power loss.
- [ ] **Non-panicking health failure.** `QppEntropy` panics on a failed
      health test because `conditioning::Sha256Conditioner` only takes
      an infallible source. Make the conditioner accept a `TryRng`
      (change in QPP-RNG-IoT), then return `Result` here.
- [ ] **Re-seeding policy** after a health failure or a long uptime.
- [ ] **`rand_core::CryptoRng`** on `QppEntropy` only once every
      validation item above is done; it is a security claim to callers.
- [ ] Move the two board sources into one place: `qpp-rng/hil-firmware`
      has its own copies of the AVR watchdog and nRF RNG code
      (ported here unchanged) and should depend on `qpp-iot-boards`
      instead.
- [ ] Add the Arduino Uno/Nano/Mega MCU selection and a per-board
      `CREDIT` constant so users don't pass magic numbers.

**Release**
- [ ] Pin both sibling dependencies to git revisions (root `Cargo.toml`).
- [ ] CI: host tests, the nRF52840 build, `scripts/build-avr.sh` for both
      chips (it fails if the stack stops fitting).
- [ ] `publish = false` → per-crate, once a release is pinned.
- [ ] License and security-claims text: say what was and wasn't validated.
