//! TOTP (RFC 6238: HMAC-SHA1, 6 digits, 30-second steps) as the admin's second factor
//! (ADR-023). Secrets live only in the environment (`CF_ADMIN_TOTP_SECRETS`), one per
//! wallet; this module never logs or prints them. `cf-admin totp-new` makes a fresh one.

use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use sha1::Sha1;
use subtle::ConstantTimeEq;

/// Time step of RFC 6238 in seconds.
pub const STEP_SECS: i64 = 30;
/// Steps before and after "now" that are still accepted (clock drift, typing time).
const WINDOW: u64 = 1;
const DIGITS: u32 = 1_000_000;
/// RFC 4226 asks for at least 128 bits; `generate` makes 160.
const MIN_SECRET_BYTES: usize = 16;
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// A shared secret; `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct TotpSecret(Vec<u8>);

impl fmt::Debug for TotpSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TotpSecret(***)")
    }
}

impl TotpSecret {
    /// 20 random bytes (160 bits, the RFC 4226 recommendation).
    pub fn generate() -> Self {
        let mut bytes = [0u8; 20];
        rand::rng().fill_bytes(&mut bytes);
        Self(bytes.to_vec())
    }

    /// Base32 (RFC 4648) as authenticator apps expect it; case, spaces, dashes and
    /// padding are tolerated. At least 16 bytes.
    pub fn from_base32(text: &str) -> Option<Self> {
        let bytes = decode_base32(text)?;
        (bytes.len() >= MIN_SECRET_BYTES).then_some(Self(bytes))
    }

    pub fn to_base32(&self) -> String {
        encode_base32(&self.0)
    }

    /// `otpauth://` URI for the authenticator app; algorithm, digits and period explicit.
    pub fn otpauth_uri(&self, account: &str) -> String {
        format!(
            "otpauth://totp/Clash%20Farm%20admin:{}?secret={}&issuer=Clash%20Farm%20admin&algorithm=SHA1&digits=6&period=30",
            percent_encode(account),
            self.to_base32()
        )
    }

    /// The 6-digit code of one time step (RFC 4226 dynamic truncation). `None` only if
    /// HMAC refuses the key, which it never does for any length.
    pub fn code(&self, step: u64) -> Option<u32> {
        let mut mac = Hmac::<Sha1>::new_from_slice(&self.0).ok()?;
        mac.update(&step.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = usize::from(digest.last()? & 0x0f);
        let four = digest.get(offset..offset + 4)?;
        let binary = u32::from_be_bytes([four[0] & 0x7f, four[1], four[2], four[3]]);
        Some(binary % DIGITS)
    }

    /// Checks `submitted` against now ±1 step, comparing every candidate in constant time;
    /// returns the matching step so the caller can refuse a second use of it.
    pub fn verify(&self, submitted: &str, now_unix: i64) -> Option<u64> {
        if now_unix < 0 || submitted.len() != 6 || !submitted.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let now_step = u64::try_from(now_unix / STEP_SECS).ok()?;
        let mut matched = None;
        for step in now_step.saturating_sub(WINDOW)..=now_step.saturating_add(WINDOW) {
            let expected = format!("{:06}", self.code(step)?);
            let hit: bool = expected.as_bytes().ct_eq(submitted.as_bytes()).into();
            if hit && matched.is_none() {
                matched = Some(step);
            }
        }
        matched
    }
}

/// RFC 4648 base32 without padding.
pub fn encode_base32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for &b in bytes {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(BASE32[((buf >> bits) & 31) as usize]));
        }
        buf &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(char::from(BASE32[((buf << (5 - bits)) & 31) as usize]));
    }
    out
}

/// Case-insensitive; `=`, spaces and dashes are skipped; any other character fails.
pub fn decode_base32(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 5 / 8);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for c in text.chars().map(|c| c.to_ascii_uppercase()) {
        let value = match c {
            '=' | ' ' | '-' => continue,
            'A'..='Z' => u32::from(c) - u32::from('A'),
            '2'..='7' => u32::from(c) - u32::from('2') + 26,
            _ => return None,
        };
        buf = (buf << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// RFC 6238 appendix B, SHA1 key `12345678901234567890`.
    const KEY: &[u8] = b"12345678901234567890";
    const KEY_B32: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    fn secret() -> TotpSecret {
        TotpSecret::from_base32(KEY_B32).unwrap()
    }

    #[test]
    fn base32_round_trips_and_tolerates_formatting() {
        assert_eq!(encode_base32(KEY), KEY_B32);
        assert_eq!(decode_base32(KEY_B32).unwrap(), KEY);
        assert_eq!(decode_base32("gezd gnbv-gy3t qojq====").unwrap(), &KEY[..10]);
        assert!(decode_base32("GEZ1").is_none(), "1 is not base32");
        assert_eq!(encode_base32(b""), "");
        assert_eq!(
            decode_base32(&encode_base32(b"\xff\x00\x01")).unwrap(),
            b"\xff\x00\x01"
        );
        assert!(
            TotpSecret::from_base32("GEZDGNBVGY3TQOJQ").is_none(),
            "10 bytes is too short"
        );
    }

    #[test]
    fn matches_rfc_6238_vectors() {
        let s = secret();
        // (unix time, 8-digit reference) → last 6 digits.
        for (t, reference) in [
            (59, 94_287_082u32),
            (1_111_111_109, 7_081_804),
            (1_234_567_890, 89_005_924),
            (2_000_000_000, 69_279_037),
            (20_000_000_000, 65_353_130),
        ] {
            let step = u64::try_from(t / STEP_SECS).unwrap();
            assert_eq!(s.code(step).unwrap(), reference % DIGITS, "t={t}");
        }
    }

    #[test]
    fn verify_uses_a_one_step_window_and_rejects_malformed_codes() {
        let s = secret();
        assert_eq!(s.verify("287082", 59), Some(1), "exact step");
        assert_eq!(s.verify("287082", 89), Some(1), "previous step still valid");
        assert_eq!(s.verify("287082", 29), Some(1), "next step too");
        assert_eq!(s.verify("287082", 120), None, "two steps late");
        assert_eq!(s.verify("28708", 59), None);
        assert_eq!(s.verify("28708a", 59), None);
        assert_eq!(s.verify(" 287082", 59), None, "caller trims");
        assert_eq!(s.verify("287082", -1), None);
    }

    #[test]
    fn uri_carries_the_parameters_and_debug_hides_the_secret() {
        let s = secret();
        let uri = s.otpauth_uri("sol:abc");
        assert!(
            uri.starts_with("otpauth://totp/Clash%20Farm%20admin:sol%3Aabc?secret=GEZD"),
            "{uri}"
        );
        assert!(uri.ends_with("&algorithm=SHA1&digits=6&period=30"));
        assert_eq!(format!("{s:?}"), "TotpSecret(***)");
        assert_ne!(TotpSecret::generate(), TotpSecret::generate());
        assert_eq!(TotpSecret::generate().to_base32().len(), 32);
    }
}
