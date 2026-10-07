//! Hashing helpers used for identifiers we store instead of raw values
//! (session tokens, IP, user agent — docs/SECURITY.md §8).

use sha2::{Digest, Sha256};

/// HMAC-like keyed hash: `SHA256(key || 0x00 || data)`. Used to derive stored
/// token identifiers from a server secret so that rotating the secret
/// invalidates everything derived from it. Not for passwords.
pub fn keyed_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update([0u8]);
    hasher.update(data);
    hasher.finalize().into()
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn keyed_hash_depends_on_key_and_data() {
        let a = keyed_sha256(b"k1", b"data");
        let b = keyed_sha256(b"k2", b"data");
        let c = keyed_sha256(b"k1", b"other");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, keyed_sha256(b"k1", b"data"));
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            hex::encode(sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
