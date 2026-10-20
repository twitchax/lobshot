//! FNV-1a, 64-bit: tiny, stable, and defined by its spec rather than by a library version, so a
//! hash recorded today still matches tomorrow and on every target.

/// Hashes `bytes`.
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

#[cfg(test)]
mod unit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn matches_the_reference_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
