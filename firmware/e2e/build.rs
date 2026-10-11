// nRF52840: put memory.x where cortex-m-rt's link.x can find it.
// AVR: tell the firmware which chip it is for, so it knows where RAM ends.
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-env-changed=AVR_MCU");
    println!("cargo::rustc-check-cfg=cfg(mcu_atmega2560)");

    match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("arm") => {
            let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
            fs::copy("memory.x", out.join("memory.x")).unwrap();
            println!("cargo:rustc-link-search={}", out.display());
        }
        Ok("avr") => {
            if env::var("AVR_MCU").as_deref() == Ok("atmega2560") {
                println!("cargo:rustc-cfg=mcu_atmega2560");
            }
        }
        _ => {}
    }
}
