#!/usr/bin/env bash
# Build the end-to-end check firmware (firmware/e2e) for an AVR board or the
# ESP32 and print its flash/static-RAM use.
#
#   scripts/build-e2e.sh <atmega328p|atmega2560|esp32>
#
# Needs: nightly Rust with rust-src, avr-gcc 14 (Homebrew avr-gcc@14 on
# macOS, gcc-avr on Debian/Ubuntu; or set AVR_GCC_BIN). Same flags as
# scripts/build-avr.sh, including the compact RustCrypto backends.
# The ESP32 needs the espup toolchain ("esp") with xtensa-esp32-elf-gcc on
# PATH. Its rustc can trail the workspace's rust-version by a patch release.
# The nRF52840 build needs none of this:
#   (cd firmware/e2e && cargo build --release --target thumbv7em-none-eabihf)
set -euo pipefail

mcu="${1:?mcu: atmega328p (Arduino Uno/Nano), atmega2560 (Mega 2560) or esp32}"
case "$mcu" in
  atmega328p | atmega2560 | esp32) ;;
  *) echo "unknown mcu $mcu" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$mcu" = esp32 ]; then
  cd "$here/firmware/e2e"
  CC_xtensa_esp32_none_elf=xtensa-esp32-elf-gcc cargo +esp build --release --ignore-rust-version \
    --target xtensa-esp32-none-elf -Z build-std=core
  elf=target/xtensa-esp32-none-elf/release/qpp-iot-e2e
  xtensa-esp32-elf-size -A "$elf"
  echo "$elf"
  exit 0
fi
if [ -z "${AVR_GCC_BIN:-}" ]; then
  if command -v brew >/dev/null 2>&1; then
    AVR_GCC_BIN="$(brew --prefix avr-gcc@14)/bin"
  else
    AVR_GCC_BIN="$(dirname "$(command -v avr-gcc)")"
  fi
fi
export PATH="$AVR_GCC_BIN:$PATH" CC_avr_none=avr-gcc AR_avr_none=avr-ar AVR_MCU="$mcu"
export RUSTFLAGS="-C target-cpu=$mcu --cfg sha2_backend_soft=\"compact\" --cfg ascon_backend=\"soft-compact\""
target_dir="${CARGO_TARGET_DIR:-$here/firmware/e2e/target}/$mcu"

cd "$here/firmware/e2e"
CARGO_TARGET_DIR="$target_dir" cargo +nightly build --release --target avr-none -Z build-std=core
elf="$target_dir/avr-none/release/qpp-iot-e2e.elf"
avr-size "$elf"
echo "$elf"
