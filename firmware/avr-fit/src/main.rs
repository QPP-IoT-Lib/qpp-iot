#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use qpp_iot::{QppEntropy, aead};

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    // WdtJitter needs global interrupts (Timer1 overflow + watchdog).
    unsafe { core::arch::asm!("sei") };
    // SAFETY: this image owns Timer1 and the watchdog, and creates one source.
    let source = unsafe { qpp_iot::boards::avr_wdt::WdtJitter::new() };
    let mut rng = QppEntropy::new(source, 1); // credit: 1 bit/sample (see qpp-iot-boards docs)

    let key = aead::generate_key(&mut rng);
    let nonce = aead::generate_random_nonce(&mut rng);
    let cipher = aead::BaselineAsconAead128::new(&key);

    let mut msg = *b"fit check";
    let Ok(tag) = cipher.encrypt_in_place(&nonce, b"ad", &mut msg) else {
        loop {}
    };
    // Keep the results observable so nothing is optimized away.
    unsafe { core::ptr::write_volatile(&mut msg[0], msg[0] ^ tag[0]) };
    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
