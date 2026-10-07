//! Nickname rules (ADR-025): the name a player picks once at the first login and everybody
//! else sees over the character's head. Nothing here touches the database — the unique key
//! and the format `CHECK` in migration 0025 are the last barrier, this is the first.
//!
//! Threats it answers: impersonation ("Admin", "Support", "Adm1n", "ClashTeam"), lookalikes
//! of a taken name ("Cool_Guy" / "Coo1Guy" / "C00lGuy"), markup and control characters in a
//! string other players render, and the crudest obscenities. The filter is deliberately
//! small and best-effort; a moderation action in the admin panel is the real remedy.

use crate::error::WorldError;

pub const MIN_LEN: usize = 3;
pub const MAX_LEN: usize = 16;

/// A name that passed every rule: what is shown and what is unique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub nickname: String,
    /// Lowercase, lookalikes folded, underscores dropped: two names with one key are one name.
    pub key: String,
}

/// Names nobody may take, also with digits after them and behind the game's name
/// ("Dev7", "ClashDev"). Compared by key, so "Adm1n" is caught too.
const RESERVED: &[&str] = &[
    "admin",
    "administrator",
    "moderator",
    "mod",
    "support",
    "staff",
    "system",
    "server",
    "clash",
    "clashfarm",
    "clashofstaking",
    "official",
    "team",
    "dev",
    "developer",
    "owner",
    "root",
    "gm",
    "help",
    "service",
];

/// Generic words refused only as the whole name: "Player1" is fine, "Player" is not.
const RESERVED_EXACT: &[&str] = &[
    "null",
    "undefined",
    "anonymous",
    "guest",
    "farmer",
    "player",
    "bot",
    "npc",
    "wallet",
    "solana",
    "phantom",
];

/// Words that make a name look official when they start or end it ("AdminTom", "TomSupport"),
/// also behind a `Clash`/`CF` prefix or trailing digits ("ClashSupport1", "Admin99").
const AUTHORITY: &[&str] = &[
    "admin",
    "administrator",
    "moderator",
    "support",
    "staff",
    "official",
    "clashfarm",
    "clashteam",
    "airdrop",
    "giveaway",
];

/// Long, unmistakable words refused anywhere in a name ("xModeratorx"). Shorter ones would catch
/// innocent names ("Badminton" holds "admin"), so they only count at the edges.
const INFIX: &[&str] = &["administrator", "moderator", "clashfarm", "clashteam"];

/// Crude obscenities, matched anywhere in the key. At least four letters each, so that
/// innocent names ("Hulk", "Torpedo", "Badminton") are not caught by a short fragment.
const BLOCKED: &[&str] = &[
    "fuck", "shit", "cunt", "nigg", "fagg", "whore", "bitch", "pussy", "porn", "penis", "vagina", "nazi",
    "hitler", "blyat", "pizd", "pidor", "pidar", "yebat", "zhopa", "mudak", "gandon", "rapist", "retard",
];

/// Lowercase, lookalike digits and `i` folded (`1`/`i` → `l`, `0` → `o`, `5` → `s`, `3` → `e`,
/// `4` → `a`, `7` → `t`, `8` → `b`), underscores dropped. Rude words are matched on this form.
fn fold(nick: &str) -> String {
    nick.chars()
        .filter(|c| *c != '_')
        .map(|c| match c.to_ascii_lowercase() {
            '1' | 'i' => 'l',
            '0' => 'o',
            '5' => 's',
            '3' => 'e',
            '4' => 'a',
            '7' => 't',
            '8' => 'b',
            other => other,
        })
        .collect()
}

/// The uniqueness key: `fold` plus `rn` → `m` and `vv` → `w`. It is STORED in
/// `users.nickname_key` and in the append-only `nickname_history.old_key` (the 30-day hold of a
/// renamed player's old name, 0030), so changing this function after the first character exists
/// needs a backfill migration of every user key — and, for history rows younger than 30 days, a
/// reviewed exception to the append-only trigger, or the hold against lookalikes weakens.
pub fn key_of(nick: &str) -> String {
    fold(nick).replace("rn", "m").replace("vv", "w")
}

/// Format rules only (no reserved/obscene words): why the shape is wrong, if it is.
fn shape_issue(raw: &str) -> Option<&'static str> {
    let len = raw.chars().count();
    if len < MIN_LEN {
        return Some("use at least 3 characters");
    }
    if len > MAX_LEN {
        return Some("use at most 16 characters");
    }
    if !raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("letters, digits and _ only");
    }
    if !raw.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return Some("start with a letter");
    }
    if raw.ends_with('_') || raw.contains("__") {
        return Some("no trailing or doubled underscore");
    }
    None
}

/// The key without a trailing run of digits ("Admin99" → "admln"), the way a name is read aloud.
fn stem_of(raw: &str) -> String {
    key_of(raw.trim_end_matches(|c: char| c.is_ascii_digit()))
}

/// Strips the game's own name from the front: "ClashDev" → "dev", "CF_Owner" → "owner".
fn bare_of(key: &str) -> &str {
    key.strip_prefix("clash")
        .or_else(|| key.strip_prefix("cf"))
        .unwrap_or(key)
}

fn looks_official(raw: &str, key: &str) -> bool {
    let stem = stem_of(raw);
    let edges = AUTHORITY.iter().map(|w| key_of(w)).any(|w| {
        [key, stem.as_str()]
            .iter()
            .any(|k| k.starts_with(&w) || k.ends_with(&w) || bare_of(k).starts_with(&w))
    });
    edges || INFIX.iter().any(|w| key.contains(&key_of(w)))
}

fn is_allowed(raw: &str, key: &str) -> bool {
    let stem = stem_of(raw);
    // A reserved word is refused bare, with digits after it, and behind the game's name.
    let reserved = RESERVED.iter().map(|w| key_of(w)).any(|w| {
        [key, stem.as_str()]
            .iter()
            .any(|k| *k == w || (bare_of(k) == w && *k != bare_of(k)))
    }) || RESERVED_EXACT.iter().any(|w| key == key_of(w));
    let plain = fold(raw);
    let rude = BLOCKED.iter().any(|w| plain.contains(&fold(w)));
    !(reserved || rude || looks_official(raw, key))
}

/// Prefix of the names a moderator's reset hands out (0031, ADR-040): `Farmer_` and 8 hex
/// digits. From 0031 on players cannot choose the prefix (any capitalisation) nor a lookalike of a
/// placeholder ("Farmer1a2b3c4d", "Farm3r_1A2B3C4D"), so a new name of that shape means "reset by a
/// moderator". Nothing trusts the shape: the free rename comes from the history (`rename_free`).
pub const PLACEHOLDER_PREFIX: &str = "Farmer_";

/// Call after `shape_issue` (the name is ASCII then).
fn is_placeholder_like(raw: &str) -> bool {
    // A placeholder's key: "farmer" and 8 hex digits as `fold` sees them (0→o, 1→l, 3→e, 4→a,
    // 5→s, 7→t, 8→b), so homoglyphs ("Farrner…", "Farmerla…") are caught as well.
    const FOLDED_HEX: &str = "ol2easbt69cdf";
    let prefixed = raw
        .get(..PLACEHOLDER_PREFIX.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(PLACEHOLDER_PREFIX));
    let key = key_of(raw);
    let lookalike = key.len() == 14
        && key.starts_with("farmer")
        && key
            .get(6..)
            .is_some_and(|h| h.chars().all(|c| FOLDED_HEX.contains(c)));
    prefixed || lookalike
}

/// Runs every rule. `InvalidNickname` carries a short reason for the player;
/// `NicknameNotAllowed` deliberately does not say which list matched.
pub fn validate(raw: &str) -> Result<Checked, WorldError> {
    if let Some(why) = shape_issue(raw) {
        return Err(WorldError::InvalidNickname(why));
    }
    let key = key_of(raw);
    if is_placeholder_like(raw) || !is_allowed(raw, &key) {
        return Err(WorldError::NicknameNotAllowed);
    }
    Ok(Checked {
        nickname: raw.to_owned(),
        key,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn ok(s: &str) -> Checked {
        validate(s).unwrap_or_else(|e| panic!("{s} should pass: {e}"))
    }

    fn shape(s: &str) -> &'static str {
        match validate(s) {
            Err(WorldError::InvalidNickname(why)) => why,
            other => panic!("{s} should fail the format rules: {other:?}"),
        }
    }

    fn refused(s: &str) {
        assert!(
            matches!(validate(s), Err(WorldError::NicknameNotAllowed)),
            "{s} should be not allowed"
        );
    }

    #[test]
    fn good_names_pass_and_keep_their_spelling() {
        assert_eq!(ok("Mikoriko").nickname, "Mikoriko");
        for s in [
            "Bob",
            "Farm_Queen",
            "Hulk",
            "Badminton",
            "Torpedo",
            "Grape1",
            "a_b",
            "Zed99",
        ] {
            ok(s);
        }
        assert_eq!(ok("abcdefghijklmnop").nickname.len(), 16);
    }

    #[test]
    fn the_moderation_placeholder_is_not_for_players() {
        for s in [
            "Farmer_1a2b3c4d",
            "farmer_bob",
            "FARMER_x",
            "Farmer1a2b3c4d",
            "Farm3r_1A2B3C4D",
            "Farrner1a2b3c4d",
            "Farmerla2b3c4d",
        ] {
            refused(s);
        }
        // Only the exact prefix: farmers are welcome otherwise (bare "Farmer" is RESERVED_EXACT).
        for s in ["FarmerJoe", "Farmer1", "Farm_Boy", "Farmer1a2b3c4dx"] {
            ok(s);
        }
        // The placeholder itself has a valid shape (the users_nickname_format CHECK holds).
        assert_eq!(shape_issue("Farmer_1a2b3c4d"), None);
    }

    #[test]
    fn shape_rules() {
        assert_eq!(shape(""), "use at least 3 characters");
        assert_eq!(shape("ab"), "use at least 3 characters");
        assert_eq!(shape("abcdefghijklmnopq"), "use at most 16 characters");
        for bad in [
            "a b c",
            "Тарас",
            "tom-cat",
            "tom.cat",
            "<b>bob</b>",
            "bob\n",
            "😀smile",
            " bob",
            "bob ",
        ] {
            assert_eq!(shape(bad), "letters, digits and _ only", "{bad:?}");
        }
        assert_eq!(shape("1abc"), "start with a letter");
        assert_eq!(shape("_abc"), "start with a letter");
        assert_eq!(shape("abc_"), "no trailing or doubled underscore");
        assert_eq!(shape("a__b"), "no trailing or doubled underscore");
    }

    #[test]
    fn lookalikes_share_one_key() {
        let key = ok("CoolGuy").key;
        for twin in ["Coo1Guy", "C00lGuy", "Cool_Guy", "COOLGUY", "CooIGuy", "cool_guy"] {
            assert_eq!(ok(twin).key, key, "{twin}");
        }
        assert_ne!(ok("CoolGuys").key, key);
    }

    #[test]
    fn reserved_and_official_looking_names_are_refused() {
        for s in [
            "admin",
            "Admin",
            "ADMIN",
            "Adm1n",
            "Admln",
            "Moderator",
            "Support",
            "Farmer",
            "Clash",
            "ClashFarm",
            "root",
            "Null",
            "Solana",
        ] {
            refused(s);
        }
        for s in [
            "AdminTom",
            "Tom_Admin",
            "SupportGuy",
            "TomSupport",
            "ClashTeam_1",
            "Official_Bob",
            "xStaff",
            "ClashSupport1",
            "Admin99",
            "Support_2",
            "CF_Admin",
            "ClashAdmin",
            "xModeratorx",
            "ClashDev",
            "CF_Owner",
            "Clash2",
            "FreeAirdrop",
            "Adrnin",
        ] {
            refused(s);
        }
    }

    #[test]
    fn obscenities_are_refused_even_in_leetspeak() {
        for s in [
            "fuck",
            "F_u_c_k",
            "Sh1t",
            "Big_Shit",
            "N4zi",
            "Hitler88",
            "PIZDA",
            "b1tch",
            "porn_king",
        ] {
            refused(s);
        }
    }
}
