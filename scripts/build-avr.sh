#!/usr/bin/env bash
# Build the AVR size-check image and report how much of the chip it uses.
#
#   scripts/build-avr.sh <atmega328p|atmega2560>
#
# Needs: nightly Rust with rust-src, avr-gcc 14 (brew install avr-gcc@14).
# Fails if the image doesn't fit the chip's flash.
#
# WHY THE EXTRA RUSTFLAGS: the RustCrypto crates default to fully unrolled
# code that is enormous on an 8-bit CPU (measured on the ATmega2560:
# SHA-256 alone is 79 KB, Ascon's permutation 30 KB; 136 KB in total, which
# does not fit the Nano's 32 KB). Their compact backends are selected with
# compile-time cfg flags, which a library cannot set for you: every AVR
# application must pass them. Result: 16 KB.
set -euo pipefail

mcu="${1:?mcu: atmega328p (Arduino Uno/Nano) or atmega2560 (Mega 2560)}"
case "$mcu" in
  atmega328p) flash_bytes=32768 ;;   # minus the bootloader: ~0.5 KB (Uno/optiboot) to 2 KB (old Nano)
  atmega2560) flash_bytes=262144 ;;
  *) echo "unknown mcu $mcu" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "$0")/.." && pwd)"
avr_gcc_bin="${AVR_GCC_BIN:-$(brew --prefix avr-gcc@14 2>/dev/null)/bin}"
export PATH="$avr_gcc_bin:$PATH" CC_avr_none=avr-gcc AR_avr_none=avr-ar AVR_MCU="$mcu"
export RUSTFLAGS="-C target-cpu=$mcu --cfg sha2_backend_soft=\"compact\" --cfg ascon_backend=\"soft-compact\""

cd "$here/firmware/avr-fit"
cargo +nightly build --release --target avr-none -Z build-std=core
elf="target/avr-none/release/avr-fit.elf"

read -r text data bss _ < <(avr-size "$elf" | tail -1)
flash=$((text + data))
ram=$((data + bss))
echo
echo "$mcu: flash $flash / $flash_bytes bytes ($((flash * 100 / flash_bytes))%), static RAM $ram bytes"
echo "(stack use is not included; it is not measured here -- the ATmega328P has 2 KB of RAM in total)"
