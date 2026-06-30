//! Codec throughput benchmark (custom harness; visibility only, no numeric gate — SC-008).
//!
//! Run with `cargo bench -p truefix-core`.

use std::time::Instant;

use truefix_core::{decode, encode, Field, Message};

fn sample() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "SENDER"));
    m.header.set(Field::string(56, "TARGET"));
    m.header.set(Field::string(52, "20240101-12:00:00.000"));
    m.body.set(Field::string(11, "ORDER-1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m.body.set(Field::string(44, "150.25"));
    m
}

fn main() {
    let m = sample();
    let bytes = encode(&m);
    let n: u32 = 500_000;

    let start = Instant::now();
    for _ in 0..n {
        let _ = encode(&m);
    }
    let enc = start.elapsed();

    let start = Instant::now();
    for _ in 0..n {
        let _ = decode(&bytes);
    }
    let dec = start.elapsed();

    let per = |d: std::time::Duration| f64::from(n) / d.as_secs_f64();
    println!("message size: {} bytes", bytes.len());
    println!("encode: {:>12.0} msg/s   ({:?}/msg)", per(enc), enc / n);
    println!("decode: {:>12.0} msg/s   ({:?}/msg)", per(dec), dec / n);
}
