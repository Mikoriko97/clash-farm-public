//! Transaction signature as sent by the client: base58, 64 bytes.

/// Returns the canonical base58 string or `None` when malformed.
pub fn normalize(signature: &str) -> Option<String> {
    if signature.len() < 80 || signature.len() > 90 {
        return None;
    }
    let bytes = bs58::decode(signature).into_vec().ok()?;
    if bytes.len() != 64 {
        return None;
    }
    Some(signature.to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn accepts_64_byte_base58_only() {
        let good = bs58::encode([7u8; 64]).into_string();
        assert_eq!(normalize(&good).as_deref(), Some(good.as_str()));
        assert!(normalize(&bs58::encode([7u8; 32]).into_string()).is_none());
        assert!(normalize("").is_none());
        assert!(normalize(
            "0OIl-not-base58-0OIl-not-base58-0OIl-not-base58-0OIl-not-base58-0OIl-not-base58-0OIl"
        )
        .is_none());
    }
}
