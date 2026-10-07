//! Canonical wallet identity `sol:<base58>` | `evm:<0x lowercase hex>` (ADR-012/013).
//! The same string is stored in `users.wallet` (migration 0001 enforces the format).

use std::fmt;

use crate::error::AuthError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletKind {
    Sol,
    Evm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletTag {
    pub kind: WalletKind,
    /// Base58 public key (Sol) or `0x` + 40 lowercase hex (Evm).
    pub address: String,
}

impl WalletTag {
    /// Parses and normalises a tag. Solana keys must decode to 32 bytes; EVM
    /// addresses are lower-cased so one wallet cannot register twice.
    pub fn parse(tag: &str) -> Result<Self, AuthError> {
        if let Some(address) = tag.strip_prefix("sol:") {
            let decoded = bs58::decode(address)
                .into_vec()
                .map_err(|_| AuthError::InvalidWallet)?;
            if decoded.len() != 32 {
                return Err(AuthError::InvalidWallet);
            }
            return Ok(Self {
                kind: WalletKind::Sol,
                address: address.to_owned(),
            });
        }
        if let Some(address) = tag.strip_prefix("evm:") {
            let hex_part = address.strip_prefix("0x").ok_or(AuthError::InvalidWallet)?;
            if hex_part.len() != 40 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(AuthError::InvalidWallet);
            }
            return Ok(Self {
                kind: WalletKind::Evm,
                address: format!("0x{}", hex_part.to_ascii_lowercase()),
            });
        }
        Err(AuthError::InvalidWallet)
    }

    /// Solana public key bytes (only for `Sol`).
    pub fn solana_pubkey(&self) -> Result<[u8; 32], AuthError> {
        if self.kind != WalletKind::Sol {
            return Err(AuthError::InvalidWallet);
        }
        let decoded = bs58::decode(&self.address)
            .into_vec()
            .map_err(|_| AuthError::InvalidWallet)?;
        decoded.try_into().map_err(|_| AuthError::InvalidWallet)
    }
}

impl fmt::Display for WalletTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            WalletKind::Sol => write!(f, "sol:{}", self.address),
            WalletKind::Evm => write!(f, "evm:{}", self.address),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    const SOL: &str = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";

    #[test]
    fn parses_and_normalises() {
        let sol = WalletTag::parse(&format!("sol:{SOL}")).unwrap();
        assert_eq!(sol.kind, WalletKind::Sol);
        assert_eq!(sol.to_string(), format!("sol:{SOL}"));
        let evm = WalletTag::parse("evm:0xABCDEF0123456789ABCDEF0123456789ABCDEF01").unwrap();
        assert_eq!(evm.to_string(), "evm:0xabcdef0123456789abcdef0123456789abcdef01");
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "sol:",
            "sol:notbase58!!",
            "sol:abc",
            "evm:abc",
            "evm:0x123",
            "btc:1abc",
            &format!("SOL:{SOL}"),
        ] {
            assert!(WalletTag::parse(bad).is_err(), "{bad}");
        }
    }
}
