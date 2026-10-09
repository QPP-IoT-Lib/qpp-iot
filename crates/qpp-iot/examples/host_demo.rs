//! Host demo of the pipeline with the mock source (no real entropy):
//! `cargo run -p qpp-iot --example host_demo`
//!
//! On a board, replace `MockJitter` with that board's source from
//! `qpp_iot::boards` and pass the credit from its documentation.

use qpp_iot::{QppEntropy, aead};
use qpp_iot_boards::mock::MockJitter;

fn main() {
    let mut rng = QppEntropy::new(MockJitter::new(0xC0FFEE), 2);
    let key = aead::generate_key(&mut rng);
    let nonce = aead::generate_random_nonce(&mut rng);
    let cipher = aead::BaselineAsconAead128::new(&key);

    let mut msg = *b"hello, QPP-IoT!";
    let tag = cipher.encrypt_in_place(&nonce, b"demo", &mut msg).unwrap();
    println!("ciphertext: {msg:02x?}\ntag:        {tag:02x?}");
    cipher.decrypt_in_place(&nonce, b"demo", &mut msg, &tag).unwrap();
    println!("decrypted:  {}", String::from_utf8_lossy(&msg));
}
