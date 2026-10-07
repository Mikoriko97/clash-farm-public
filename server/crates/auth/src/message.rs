//! Sign-in message (EIP-4361 layout, used for both Solana and Ethereum).
//! The server builds the text; the client signs it byte-for-byte; the server
//! parses it back strictly and checks every field (docs/SECURITY.md §3).

use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::error::AuthError;
use crate::wallet::{WalletKind, WalletTag};

pub const STATEMENT: &str = "Sign in to Clash Farm. No transaction, no fees.";
/// How long a freshly issued message stays signable.
pub const VALIDITY: Duration = Duration::minutes(5);
/// Tolerated clock skew for `Issued At`.
pub const SKEW: Duration = Duration::minutes(5);
pub const SOLANA_CHAIN: &str = "mainnet";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInMessage {
    pub kind: WalletKind,
    pub domain: String,
    pub address: String,
    pub statement: String,
    pub uri: String,
    pub version: String,
    pub chain_id: String,
    pub nonce: String,
    pub issued_at: OffsetDateTime,
    pub expiration_time: OffsetDateTime,
}

impl SignInMessage {
    pub fn new(
        domain: &str,
        wallet: &WalletTag,
        evm_chain_id: Option<u64>,
        nonce: &str,
        now: OffsetDateTime,
    ) -> Self {
        let chain_id = match wallet.kind {
            WalletKind::Sol => SOLANA_CHAIN.to_owned(),
            WalletKind::Evm => evm_chain_id.unwrap_or(1).to_string(),
        };
        Self {
            kind: wallet.kind,
            domain: domain.to_owned(),
            address: wallet.address.clone(),
            statement: STATEMENT.to_owned(),
            uri: format!("https://{domain}"),
            version: "1".to_owned(),
            chain_id,
            nonce: nonce.to_owned(),
            issued_at: now,
            expiration_time: now + VALIDITY,
        }
    }

    /// Exact text the wallet signs.
    pub fn render(&self) -> String {
        let network = match self.kind {
            WalletKind::Sol => "Solana",
            WalletKind::Evm => "Ethereum",
        };
        format!(
            "{domain} wants you to sign in with your {network} account:\n{address}\n\n{statement}\n\nURI: {uri}\nVersion: {version}\nChain ID: {chain}\nNonce: {nonce}\nIssued At: {issued}\nExpiration Time: {expires}",
            domain = self.domain,
            address = self.address,
            statement = self.statement,
            uri = self.uri,
            version = self.version,
            chain = self.chain_id,
            nonce = self.nonce,
            issued = fmt_time(self.issued_at),
            expires = fmt_time(self.expiration_time),
        )
    }

    /// Strict parser: exact line layout, fixed field order, nothing extra.
    pub fn parse(text: &str) -> Result<Self, AuthError> {
        if text.len() > 2048 || text.contains('\r') {
            return Err(AuthError::InvalidMessage("size or line endings"));
        }
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.len() != 11 {
            return Err(AuthError::InvalidMessage("line count"));
        }
        let (kind, domain) = parse_header(lines[0])?;
        if !lines[2].is_empty() || !lines[4].is_empty() {
            return Err(AuthError::InvalidMessage("layout"));
        }
        Ok(Self {
            kind,
            domain,
            address: lines[1].to_owned(),
            statement: lines[3].to_owned(),
            uri: field(lines[5], "URI: ")?.to_owned(),
            version: field(lines[6], "Version: ")?.to_owned(),
            chain_id: field(lines[7], "Chain ID: ")?.to_owned(),
            nonce: field(lines[8], "Nonce: ")?.to_owned(),
            issued_at: parse_time(field(lines[9], "Issued At: ")?)?,
            expiration_time: parse_time(field(lines[10], "Expiration Time: ")?)?,
        })
    }

    /// The message's `URI` line as the page origin the wallet sees (`http://localhost:9100`
    /// through a tunnel); wallets refuse a URI whose origin differs from the page's.
    pub fn with_uri(mut self, uri: &str) -> Self {
        self.uri = uri.to_owned();
        self
    }

    /// Everything that must hold before a signature is even looked at; the URI must be
    /// `https://<domain>`.
    pub fn validate(
        &self,
        expected_domain: &str,
        wallet: &WalletTag,
        allowed_evm_chains: &[u64],
        now: OffsetDateTime,
    ) -> Result<(), AuthError> {
        let uri = format!("https://{expected_domain}");
        self.validate_with_uri(expected_domain, &uri, wallet, allowed_evm_chains, now)
    }

    /// Same checks with an explicit expected `URI` (exact match, scheme included).
    pub fn validate_with_uri(
        &self,
        expected_domain: &str,
        expected_uri: &str,
        wallet: &WalletTag,
        allowed_evm_chains: &[u64],
        now: OffsetDateTime,
    ) -> Result<(), AuthError> {
        if self.domain != expected_domain || self.uri != expected_uri {
            return Err(AuthError::WrongDomain);
        }
        if self.kind != wallet.kind || !same_address(self, wallet) {
            return Err(AuthError::InvalidMessage("address"));
        }
        if self.statement != STATEMENT || self.version != "1" {
            return Err(AuthError::InvalidMessage("statement or version"));
        }
        if self.nonce.len() != 64 || !self.nonce.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AuthError::InvalidMessage("nonce format"));
        }
        self.validate_chain(allowed_evm_chains)?;
        let skewed = self.issued_at > now + SKEW || self.issued_at < now - SKEW - VALIDITY;
        if skewed || now >= self.expiration_time || self.expiration_time - self.issued_at > VALIDITY * 2 {
            return Err(AuthError::Expired);
        }
        Ok(())
    }

    fn validate_chain(&self, allowed_evm_chains: &[u64]) -> Result<(), AuthError> {
        match self.kind {
            WalletKind::Sol if self.chain_id == SOLANA_CHAIN => Ok(()),
            WalletKind::Evm => {
                let id: u64 = self.chain_id.parse().map_err(|_| AuthError::ChainNotAllowed)?;
                if allowed_evm_chains.contains(&id) {
                    Ok(())
                } else {
                    Err(AuthError::ChainNotAllowed)
                }
            }
            _ => Err(AuthError::ChainNotAllowed),
        }
    }
}

fn same_address(message: &SignInMessage, wallet: &WalletTag) -> bool {
    match wallet.kind {
        WalletKind::Sol => message.address == wallet.address,
        WalletKind::Evm => message.address.to_ascii_lowercase() == wallet.address,
    }
}

fn parse_header(line: &str) -> Result<(WalletKind, String), AuthError> {
    if let Some(domain) = line.strip_suffix(" wants you to sign in with your Solana account:") {
        return valid_domain(domain).map(|d| (WalletKind::Sol, d));
    }
    if let Some(domain) = line.strip_suffix(" wants you to sign in with your Ethereum account:") {
        return valid_domain(domain).map(|d| (WalletKind::Evm, d));
    }
    Err(AuthError::InvalidMessage("header"))
}

fn valid_domain(domain: &str) -> Result<String, AuthError> {
    let ok = !domain.is_empty()
        && domain.len() <= 253
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    if ok {
        Ok(domain.to_owned())
    } else {
        Err(AuthError::InvalidMessage("domain"))
    }
}

fn field<'a>(line: &'a str, prefix: &'static str) -> Result<&'a str, AuthError> {
    line.strip_prefix(prefix)
        .filter(|v| !v.is_empty())
        .ok_or(AuthError::InvalidMessage("field"))
}

fn fmt_time(t: OffsetDateTime) -> String {
    // Seconds precision keeps the text stable across languages/wallets.
    t.replace_nanosecond(0)
        .unwrap_or(t)
        .format(&Rfc3339)
        .unwrap_or_default()
}

fn parse_time(raw: &str) -> Result<OffsetDateTime, AuthError> {
    OffsetDateTime::parse(raw, &Rfc3339).map_err(|_| AuthError::InvalidMessage("timestamp"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    const SOL: &str = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";
    const NONCE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    fn sol_wallet() -> WalletTag {
        WalletTag::parse(&format!("sol:{SOL}")).unwrap()
    }

    #[test]
    fn render_parse_roundtrip_and_validate() {
        let msg = SignInMessage::new("play.example", &sol_wallet(), None, NONCE, now());
        let text = msg.render();
        let parsed = SignInMessage::parse(&text).unwrap();
        assert_eq!(parsed, msg);
        parsed
            .validate("play.example", &sol_wallet(), &[1], now() + Duration::seconds(30))
            .unwrap();
    }

    #[test]
    fn evm_chain_must_be_allowed() {
        let wallet = WalletTag::parse("evm:0xabcdef0123456789abcdef0123456789abcdef01").unwrap();
        let msg = SignInMessage::new("play.example", &wallet, Some(56), NONCE, now());
        assert!(matches!(
            msg.validate("play.example", &wallet, &[1, 8453], now()),
            Err(AuthError::ChainNotAllowed)
        ));
        let ok = SignInMessage::new("play.example", &wallet, Some(8453), NONCE, now());
        ok.validate("play.example", &wallet, &[1, 8453], now()).unwrap();
    }

    #[test]
    fn wrong_domain_expired_and_tampered_are_rejected() {
        let msg = SignInMessage::new("play.example", &sol_wallet(), None, NONCE, now());
        assert!(matches!(
            msg.validate("evil.example", &sol_wallet(), &[], now()),
            Err(AuthError::WrongDomain)
        ));
        assert!(matches!(
            msg.validate("play.example", &sol_wallet(), &[], now() + Duration::minutes(6)),
            Err(AuthError::Expired)
        ));
        assert!(matches!(
            msg.validate("play.example", &sol_wallet(), &[], now() - Duration::minutes(6)),
            Err(AuthError::Expired)
        ));
        let tampered = msg.render().replace(STATEMENT, "Approve token transfer");
        assert!(SignInMessage::parse(&tampered)
            .unwrap()
            .validate("play.example", &sol_wallet(), &[], now())
            .is_err());
    }

    #[test]
    fn parser_rejects_extra_lines_and_crlf() {
        let msg = SignInMessage::new("play.example", &sol_wallet(), None, NONCE, now());
        assert!(SignInMessage::parse(&(msg.render() + "\nResources:\n- x")).is_err());
        assert!(SignInMessage::parse(&msg.render().replace('\n', "\r\n")).is_err());
        assert!(SignInMessage::parse("hello").is_err());
    }
}
