// Compiles the AVR watchdog shim, only when the `avr-wdt` feature is on
// and the target is AVR. Set AVR_MCU for anything but the ATmega328P
// (e.g. AVR_MCU=atmega2560 for the Mega 2560), as for entropy-timer.
fn main() {
    println!("cargo:rerun-if-changed=c/wdt_jitter.c");
    println!("cargo:rerun-if-env-changed=AVR_MCU");

    let avr_feature = std::env::var_os("CARGO_FEATURE_AVR_WDT").is_some();
    let is_avr = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("avr");
    if !(avr_feature && is_avr) {
        return;
    }
    let mcu = std::env::var("AVR_MCU")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "atmega328p".to_string());
    cc::Build::new()
        .file("c/wdt_jitter.c")
        .flag(format!("-mmcu={mcu}"))
        .opt_level_str("s")
        .compile("qpp_wdt_jitter");
}
