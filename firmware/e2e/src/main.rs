//! End-to-end check of the QPP-IoT stack on a real board.
//!
//! The board's physical source feeds `QppEntropy` (QPP-RNG with health
//! tests, conditioned with SHA-256), which makes an Ascon-AEAD128 key and
//! a random starting nonce. Three messages (0, 16 and 64 bytes, with 16
//! bytes of associated data) are then encrypted, decrypted and checked,
//! and a copy with one flipped bit must be rejected. Everything is
//! reported as ASCII lines over the UART at 115200 baud:
//!
//! ```text
//! QPPE2E chip=atmega328p credit=1 cpu_hz=16000000 ram_top=2304 statics_end=... painted=...
//! KEY=<hex> NONCE=<hex> T_KEY=<cycles> T_NONCE=<cycles> T_INIT=<cycles>
//! MSG len=16 NONCE=<hex> AD=<hex> PT=<hex> CT=<hex> TAG=<hex> T_ENC=<cycles> T_DEC=<cycles> DEC=ok TAMPER=rejected
//! STACK_PEAK=<bytes> STACK_HEADROOM=<bytes> RESULT=PASS
//! DONE
//! ```
//!
//! `scripts/e2e-check.py` resets the board, reads this report and
//! re-computes every ciphertext and tag with the ascon-c reference.
//!
//! Message `k` uses the random nonce plus `k` (little-endian), the
//! counter-from-a-random-start pattern, so no nonce repeats under the key.
//! Peak stack use is measured by painting the free RAM between the
//! statics and the stack before anything runs, then finding the lowest
//! byte that was overwritten; it includes interrupt handlers.

#![no_std]
#![no_main]
#![cfg_attr(target_arch = "avr", feature(asm_experimental_arch))]

use core::ptr::{read_volatile, write_volatile};

use qpp_iot::{QppEntropy, aead};

#[cfg(target_arch = "avr")]
mod board {
    use core::ptr::{read_volatile, write_volatile};

    #[cfg(mcu_atmega2560)]
    pub const CHIP: &str = "atmega2560";
    #[cfg(not(mcu_atmega2560))]
    pub const CHIP: &str = "atmega328p";
    pub const CPU_HZ: u32 = 16_000_000;
    /// Credited min-entropy per watchdog sample (`qpp-iot-boards` docs).
    pub const CREDIT: u8 = 1;
    /// One past RAMEND.
    #[cfg(mcu_atmega2560)]
    pub const RAM_TOP: usize = 0x2200;
    #[cfg(not(mcu_atmega2560))]
    pub const RAM_TOP: usize = 0x0900;

    // USART0: same data-space addresses on the ATmega328P and ATmega2560.
    const UCSR0A: *mut u8 = 0xC0 as *mut u8;
    const UCSR0B: *mut u8 = 0xC1 as *mut u8;
    const UCSR0C: *mut u8 = 0xC2 as *mut u8;
    const UBRR0L: *mut u8 = 0xC4 as *mut u8;
    const UBRR0H: *mut u8 = 0xC5 as *mut u8;
    const UDR0: *mut u8 = 0xC6 as *mut u8;
    const SPL: *const u8 = 0x5D as *const u8;
    const SPH: *const u8 = 0x5E as *const u8;

    pub fn init() {
        // 115200 baud with U2X: UBRR = 16 MHz / (8 * 115200) - 1 = 16.
        let ubrr: u16 = (CPU_HZ / 8 / 115_200 - 1) as u16;
        unsafe {
            write_volatile(UCSR0A, 1 << 1); // U2X0
            write_volatile(UBRR0H, (ubrr >> 8) as u8);
            write_volatile(UBRR0L, ubrr as u8);
            write_volatile(UCSR0C, 0b0000_0110); // 8N1
            write_volatile(UCSR0B, 1 << 3); // TXEN0
        }
    }

    pub fn write(b: u8) {
        unsafe {
            while read_volatile(UCSR0A) & (1 << 5) == 0 {} // UDRE0
            write_volatile(UDR0, b);
        }
    }

    /// Lets the host finish opening the port after the DTR auto-reset.
    pub fn delay_for_host() {
        for _ in 0..400_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }

    /// The watchdog source and the Timer1 overflow both run on interrupts.
    pub fn enable_interrupts() {
        unsafe { core::arch::asm!("sei") };
    }

    pub fn sp() -> usize {
        unsafe { read_volatile(SPL) as usize | (read_volatile(SPH) as usize) << 8 }
    }

    /// First byte after `.data`, `.bss` and `.noinit` (avr-libc linker script).
    pub fn statics_end() -> usize {
        unsafe extern "C" {
            static __heap_start: u8;
        }
        core::ptr::addr_of!(__heap_start) as usize
    }

    pub fn source() -> qpp_iot::boards::avr_wdt::WdtJitter {
        // SAFETY: this image owns Timer1 and the watchdog, and makes one source.
        unsafe { qpp_iot::boards::avr_wdt::WdtJitter::new() }
    }

    /// Timer1 at 16 MHz, prescaler 1: CPU cycles. Started by `source()`.
    pub fn cycles() -> u32 {
        use entropy_timer::{HighResTimer, PlatformTimer};
        PlatformTimer.tick() as u32
    }
}

#[cfg(target_arch = "arm")]
mod board {
    use core::ptr::{read_volatile, write_volatile};

    pub const CHIP: &str = "nrf52840";
    pub const CPU_HZ: u32 = 64_000_000;
    /// Credited min-entropy per RNG byte (`qpp-iot-boards` docs).
    pub const CREDIT: u8 = 4;
    pub const RAM_TOP: usize = 0x2004_0000;

    const TX_PIN: u32 = 20; // wired to the DAPLink USB-serial bridge
    const P0_OUTSET: *mut u32 = 0x5000_0508 as *mut u32;
    const P0_DIRSET: *mut u32 = 0x5000_0518 as *mut u32;
    // Legacy (non-EasyDMA) UART0, so TX needs no RAM buffer.
    const UART0: usize = 0x4000_2000;
    const UART_STARTTX: *mut u32 = (UART0 + 0x008) as *mut u32;
    const UART_TXDRDY: *mut u32 = (UART0 + 0x11C) as *mut u32;
    const UART_ENABLE: *mut u32 = (UART0 + 0x500) as *mut u32;
    const UART_PSELRTS: *mut u32 = (UART0 + 0x508) as *mut u32;
    const UART_PSELTXD: *mut u32 = (UART0 + 0x50C) as *mut u32;
    const UART_PSELCTS: *mut u32 = (UART0 + 0x510) as *mut u32;
    const UART_PSELRXD: *mut u32 = (UART0 + 0x514) as *mut u32;
    const UART_TXD: *mut u32 = (UART0 + 0x51C) as *mut u32;
    const UART_BAUDRATE: *mut u32 = (UART0 + 0x524) as *mut u32;
    const UART_CONFIG: *mut u32 = (UART0 + 0x56C) as *mut u32;
    const BAUD_115200: u32 = 0x01D7_E000;
    const CLOCK_HFCLKSTART: *mut u32 = 0x4000_0000 as *mut u32;
    const CLOCK_HFCLKSTARTED: *mut u32 = 0x4000_0100 as *mut u32;
    const DEMCR: *mut u32 = 0xE000_EDFC as *mut u32;
    const DWT_CTRL: *mut u32 = 0xE000_1000 as *mut u32;
    const DWT_CYCCNT: *mut u32 = 0xE000_1004 as *mut u32;

    pub fn init() {
        unsafe {
            // 32 MHz crystal, for an accurate baud rate.
            write_volatile(CLOCK_HFCLKSTARTED, 0);
            write_volatile(CLOCK_HFCLKSTART, 1);
            while read_volatile(CLOCK_HFCLKSTARTED) == 0 {}

            write_volatile(P0_OUTSET, 1 << TX_PIN);
            write_volatile(P0_DIRSET, 1 << TX_PIN);
            write_volatile(UART_PSELTXD, TX_PIN);
            write_volatile(UART_PSELRXD, 0xFFFF_FFFF);
            write_volatile(UART_PSELRTS, 0xFFFF_FFFF);
            write_volatile(UART_PSELCTS, 0xFFFF_FFFF);
            write_volatile(UART_BAUDRATE, BAUD_115200);
            write_volatile(UART_CONFIG, 0);
            write_volatile(UART_ENABLE, 4);
            write_volatile(UART_STARTTX, 1);

            // DWT cycle counter.
            write_volatile(DEMCR, read_volatile(DEMCR) | 1 << 24);
            write_volatile(DWT_CYCCNT, 0);
            write_volatile(DWT_CTRL, read_volatile(DWT_CTRL) | 1);
        }
    }

    pub fn write(b: u8) {
        unsafe {
            write_volatile(UART_TXDRDY, 0);
            write_volatile(UART_TXD, b as u32);
            while read_volatile(UART_TXDRDY) == 0 {}
        }
    }

    /// Lets the host's serial reader settle after the reset.
    pub fn delay_for_host() {
        for _ in 0..2_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }

    pub fn enable_interrupts() {}

    pub fn sp() -> usize {
        let sp: usize;
        unsafe { core::arch::asm!("mov {}, sp", out(reg) sp) };
        sp
    }

    pub fn statics_end() -> usize {
        cortex_m_rt::heap_start() as usize
    }

    pub fn source() -> qpp_iot::boards::nrf52840::HwRng {
        // SAFETY: nothing else in this image uses the RNG peripheral.
        unsafe { qpp_iot::boards::nrf52840::HwRng::new() }
    }

    pub fn cycles() -> u32 {
        unsafe { read_volatile(DWT_CYCCNT) }
    }
}

#[cfg(target_arch = "avr")]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    run()
}

#[cfg(target_arch = "arm")]
#[cortex_m_rt::entry]
fn main() -> ! {
    run()
}

const PAINT: u8 = 0xA5;
const AD_LEN: usize = 16;
const MSG_LENS: [usize; 3] = [0, 16, 64];

fn run() -> ! {
    board::init();
    board::delay_for_host();
    let (lo, hi) = paint_stack();
    board::enable_interrupts();

    puts("QPPE2E chip=");
    puts(board::CHIP);
    puts(" credit=");
    dec(board::CREDIT as u32);
    puts(" cpu_hz=");
    dec(board::CPU_HZ);
    puts(" ram_top=");
    dec(board::RAM_TOP as u32);
    puts(" statics_end=");
    dec(lo as u32);
    puts(" painted=");
    dec((hi - lo) as u32);
    puts("\n");

    let mut rng = QppEntropy::new(board::source(), board::CREDIT);
    let t0 = board::cycles();
    let key = aead::generate_key(&mut rng);
    let t1 = board::cycles();
    let nonce0 = aead::generate_random_nonce(&mut rng);
    let t2 = board::cycles();
    let cipher = aead::BaselineAsconAead128::new(&key);
    let t3 = board::cycles();
    puts("KEY=");
    hex(&key);
    puts(" NONCE=");
    hex(&nonce0);
    puts(" T_KEY=");
    dec(t1.wrapping_sub(t0));
    puts(" T_NONCE=");
    dec(t2.wrapping_sub(t1));
    puts(" T_INIT=");
    dec(t3.wrapping_sub(t2));
    puts("\n");

    let ad: [u8; AD_LEN] = core::array::from_fn(|i| 0xA0 ^ i as u8);
    let mut all_ok = true;
    for (k, &len) in MSG_LENS.iter().enumerate() {
        let nonce = nonce_add(&nonce0, k as u8);
        let mut buf = [0u8; 64];
        let msg = &mut buf[..len];
        for (i, b) in msg.iter_mut().enumerate() {
            *b = pattern(i);
        }
        puts("MSG len=");
        dec(len as u32);
        puts(" NONCE=");
        hex(&nonce);
        puts(" AD=");
        hex(&ad);
        puts(" PT=");
        hex(msg);

        let t0 = board::cycles();
        let tag = cipher.encrypt_in_place(&nonce, &ad, msg);
        let t1 = board::cycles();
        let Ok(tag) = tag else {
            puts(" ENC=error\n");
            all_ok = false;
            continue;
        };
        puts(" CT=");
        hex(msg);
        puts(" TAG=");
        hex(&tag);
        puts(" T_ENC=");
        dec(t1.wrapping_sub(t0));

        let t0 = board::cycles();
        let opened = cipher.decrypt_in_place(&nonce, &ad, msg, &tag).is_ok();
        let t1 = board::cycles();
        let dec_ok = opened && msg.iter().enumerate().all(|(i, &b)| b == pattern(i));
        puts(" T_DEC=");
        dec(t1.wrapping_sub(t0));
        puts(if dec_ok { " DEC=ok" } else { " DEC=FAIL" });

        // Same message again (same ciphertext), one bit flipped: must be rejected.
        let mut bad_tag = cipher.encrypt_in_place(&nonce, &ad, msg).unwrap_or(tag);
        if len > 0 {
            msg[0] ^= 1;
        } else {
            bad_tag[0] ^= 1;
        }
        let rejected = cipher.decrypt_in_place(&nonce, &ad, msg, &bad_tag).is_err();
        puts(if rejected { " TAMPER=rejected\n" } else { " TAMPER=ACCEPTED\n" });
        all_ok &= dec_ok && rejected;
    }

    let low = lowest_touched(lo, hi);
    puts("STACK_PEAK=");
    dec((board::RAM_TOP - low) as u32);
    puts(" STACK_HEADROOM=");
    dec((low - lo) as u32);
    puts(if all_ok { " RESULT=PASS\nDONE\n" } else { " RESULT=FAIL\nDONE\n" });
    loop {}
}

/// Plaintext byte `i` of every test message.
fn pattern(i: usize) -> u8 {
    i as u8 ^ 0x5A
}

/// `n + k`, as a little-endian 128-bit integer.
fn nonce_add(n: &[u8; 16], k: u8) -> [u8; 16] {
    let mut out = *n;
    let mut carry = k as u16;
    for b in out.iter_mut() {
        let s = *b as u16 + carry;
        *b = s as u8;
        carry = s >> 8;
        if carry == 0 {
            break;
        }
    }
    out
}

/// Fills the free RAM between the statics and (a margin below) the
/// current stack pointer with `PAINT`.
#[inline(never)]
fn paint_stack() -> (usize, usize) {
    let lo = board::statics_end();
    let hi = board::sp() - 64;
    let mut p = lo;
    while p < hi {
        unsafe { write_volatile(p as *mut u8, PAINT) };
        p += 1;
    }
    (lo, hi)
}

/// Lowest address the stack has reached since `paint_stack`.
fn lowest_touched(lo: usize, hi: usize) -> usize {
    let mut p = lo;
    while p < hi && unsafe { read_volatile(p as *const u8) } == PAINT {
        p += 1;
    }
    p
}

fn puts(s: &str) {
    for b in s.bytes() {
        board::write(b);
    }
}

fn hex(bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    for &b in bytes {
        board::write(DIGITS[(b >> 4) as usize]);
        board::write(DIGITS[(b & 0x0F) as usize]);
    }
}

fn dec(mut v: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for &b in &buf[i..] {
        board::write(b);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // A failed SP 800-90B health test panics inside QppEntropy.
    puts("\nPANIC\n");
    loop {}
}
