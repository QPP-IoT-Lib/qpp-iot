//! Host tests of the whole pipeline with the mock source. These prove
//! the plumbing (types, conditioning, health-test wiring, AEAD round
//! trip), NOT the quality of any board's entropy.

use qpp_iot::{QppEntropy, QppRngSource, aead};
use qpp_iot_boards::mock::{MockJitter, StuckJitter};
use rand_core::Rng;

#[test]
fn key_and_nonce_from_the_pipeline_round_trip_through_ascon() {
    let mut rng = QppEntropy::new(MockJitter::new(1), 2);

    let key = aead::generate_key(&mut rng);
    let nonce = aead::generate_random_nonce(&mut rng);
    let cipher = aead::BaselineAsconAead128::new(&key);

    let original = *b"QPP-IoT scaffold";
    let mut msg = original;
    let tag = cipher.encrypt_in_place(&nonce, b"header", &mut msg).unwrap();
    assert_ne!(msg, original);

    cipher.decrypt_in_place(&nonce, b"header", &mut msg, &tag).unwrap();
    assert_eq!(msg, original);

    // A flipped ciphertext bit must fail authentication.
    let mut msg = original;
    let tag = cipher.encrypt_in_place(&nonce, b"header", &mut msg).unwrap();
    msg[0] ^= 1;
    assert!(cipher.decrypt_in_place(&nonce, b"header", &mut msg, &tag).is_err());
}

#[test]
fn successive_keys_differ_and_looks_uniform() {
    let mut rng = QppEntropy::new(MockJitter::new(7), 2);
    let k1 = aead::generate_key(&mut rng);
    let k2 = aead::generate_key(&mut rng);
    assert_ne!(k1, k2);

    // Conditioned output: bit balance within a loose bound over 4 KiB.
    let mut buf = [0u8; 4096];
    rng.fill_bytes(&mut buf);
    let ones: u32 = buf.iter().map(|b| b.count_ones()).sum();
    let frac = ones as f64 / (buf.len() * 8) as f64;
    assert!((0.48..0.52).contains(&frac), "ones fraction {frac}");
}

#[test]
fn diagnostics_are_forwarded() {
    let mut rng = QppEntropy::new(MockJitter::new(3), 2);
    let _ = rng.next_u32();
    let d = rng.diagnostics();
    assert!(d.last_permutation_count > 0);
}

#[test]
#[should_panic(expected = "repetition count")]
fn a_dead_source_panics_instead_of_producing_bytes() {
    let mut rng = QppEntropy::new(StuckJitter(271_434), 2);
    let mut buf = [0u8; 64];
    rng.fill_bytes(&mut buf);
}
