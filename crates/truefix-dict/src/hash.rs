//! Stable content hash (FNV-1a, 64-bit) used to prove the codegen track and the runtime track
//! derive from the same dictionary source (Constitution Principle IV). The identical algorithm is
//! duplicated in `build.rs`.

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
