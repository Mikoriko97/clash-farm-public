//! Signature verification over the exact message bytes.
//! Solana: ed25519 (`signMessage`). Ethereum: EIP-191 `personal_sign` with
//! secp256k1 public-key recovery, address compared to the claimed one.

use ed25519_dalek::{Signature, VerifyingKey};
use k256::ecdsa::{RecoveryId, Signature as EcdsaSignature, VerifyingKey as EcdsaVerifyingKey};
use sha3::{Digest, Keccak256};

use crate::error::AuthError;
use crate::wallet::{WalletKind, WalletTag};

/// Dispatches on wallet kind. `signature` is raw bytes (64 for ed25519, 65 for secp256k1).
pub fn verify(wallet: &WalletTag, message: &[u8], signature: &[u8]) -> Result<(), AuthError> {
    match wallet.kind {
        WalletKind::Sol => verify_solana(&wallet.solana_pubkey()?, message, signature),
        WalletKind::Evm => verify_evm(&wallet.address, message, signature),
    }
}

pub fn verify_solana(pubkey: &[u8; 32], message: &[u8], signature: &[u8]) -> Result<(), AuthError> {
    let key = VerifyingKey::from_bytes(pubkey).map_err(|_| AuthError::BadSignature)?;
    let sig = Signature::from_slice(signature).map_err(|_| AuthError::BadSignature)?;
    key.verify_strict(message, &sig)
        .map_err(|_| AuthError::BadSignature)
}

/// EIP-191: keccak256("\x19Ethereum Signed Message:\n" + len + message).
pub fn eip191_hash(message: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(b"\x19Ethereum Signed Message:\n");
    hasher.update(message.len().to_string().as_bytes());
    hasher.update(message);
    hasher.finalize().into()
}

pub fn verify_evm(address_lower: &str, message: &[u8], signature: &[u8]) -> Result<(), AuthError> {
    if signature.len() != 65 {
        return Err(AuthError::BadSignature);
    }
    // Only canonical encodings: v ∈ {0,1,27,28}, low-S (reject malleable twins).
    let v = match signature[64] {
        v @ (0 | 1) => v,
        v @ (27 | 28) => v - 27,
        _ => return Err(AuthError::BadSignature),
    };
    let recovery = RecoveryId::from_byte(v).ok_or(AuthError::BadSignature)?;
    let sig = EcdsaSignature::from_slice(&signature[..64]).map_err(|_| AuthError::BadSignature)?;
    if sig.normalize_s() != sig {
        return Err(AuthError::BadSignature);
    }
    let digest = eip191_hash(message);
    let key = EcdsaVerifyingKey::recover_from_prehash(&digest, &sig, recovery)
        .map_err(|_| AuthError::BadSignature)?;
    let recovered = address_from_verifying_key(&key);
    if recovered == address_lower {
        Ok(())
    } else {
        Err(AuthError::BadSignature)
    }
}

pub fn address_from_verifying_key(key: &EcdsaVerifyingKey) -> String {
    let point = key.to_sec1_point(false);
    let hash = Keccak256::digest(&point.as_bytes()[1..]);
    format!("0x{}", hex::encode(&hash[12..]))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use k256::ecdsa::SigningKey as EcdsaSigningKey;

    #[test]
    fn solana_signature_roundtrip() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let message = b"hello";
        let sig = signing.sign(message);
        let pubkey = signing.verifying_key().to_bytes();
        verify_solana(&pubkey, message, &sig.to_bytes()).unwrap();
        assert!(verify_solana(&pubkey, b"hellp", &sig.to_bytes()).is_err());
        assert!(verify_solana(&[1u8; 32], message, &sig.to_bytes()).is_err());
    }

    #[test]
    fn evm_signature_roundtrip_and_wrong_address() {
        let signing = EcdsaSigningKey::from_slice(&[9u8; 32]).unwrap();
        let message = b"hello";
        let (sig, recid) = signing.sign_prehash_recoverable(&eip191_hash(message));
        let mut bytes = sig.to_bytes().to_vec();
        bytes.push(recid.to_byte() + 27);
        let address = address_from_verifying_key(signing.verifying_key());
        verify_evm(&address, message, &bytes).unwrap();
        assert!(verify_evm(&address, b"hellp", &bytes).is_err());
        assert!(verify_evm("0x0000000000000000000000000000000000000001", message, &bytes).is_err());
        assert!(verify_evm(&address, message, &bytes[..64]).is_err());
        // Non-canonical v and malleable high-S twin are rejected.
        let mut bad_v = bytes.clone();
        bad_v[64] = 29;
        assert!(verify_evm(&address, message, &bad_v).is_err());
        let s: k256::Scalar = *sig.s();
        let high_s = k256::Scalar::ZERO - s; // n - s
        let mut twin = sig.r().to_bytes().to_vec();
        twin.extend_from_slice(&high_s.to_bytes());
        twin.push((recid.to_byte() ^ 1) + 27);
        assert!(verify_evm(&address, message, &twin).is_err());
    }

    #[test]
    fn eip191_known_vector() {
        // keccak256("\x19Ethereum Signed Message:\n5hello")
        assert_eq!(
            hex::encode(eip191_hash(b"hello")),
            "50b2c43fd39106bafbba0da34fc430e1f91e3c96ea2acee2bc34119f92b37750"
        );
    }
}
