//! The character a player creates once at the first login (0025, ADR-025): a nickname and a
//! look. The look is four palette indexes — the server only checks they are inside the
//! tables, the colours themselves live in the client (Godot recolours the mannequin's zones).
//! One character per account: `create` writes only while `nickname IS NULL`, so a name can
//! neither be swapped nor squatted in bulk; the unique key of migration 0025 decides races.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::caps;
use crate::error::WorldError;
use crate::nickname::{self, Checked};
use crate::rename;

/// Sizes of the tables the client draws from (`web/src/character/palette.ts`): colours for the
/// body, hair and outfit, the body types (0 = male HumanM, 1 = female HumanF, ADR-027), the builds (0 = average, 1 = slim,
/// 2 = sturdy, ADR-029) and the parts of
/// `head_parts.glb` / `outfits.glb` (index 0 of hair, beard, hat, glasses and outfit = none).
pub const BODY_COUNT: u8 = 2;
pub const BUILD_COUNT: u8 = 3;
pub const SKIN_COUNT: u8 = 8;
pub const SHIRT_COUNT: u8 = 12;
pub const PANTS_COUNT: u8 = 12;
pub const SHOES_COUNT: u8 = 6;
pub const HAIR_COLOR_COUNT: u8 = 12;
pub const HAIR_COUNT: u8 = 10;
pub const FACE_COUNT: u8 = 5;
pub const BEARD_COUNT: u8 = 6;
pub const HAT_COUNT: u8 = 8;
pub const GLASSES_COUNT: u8 = 3;
pub const OUTFIT_COUNT: u8 = 9;
pub const OUTFIT_COLOR_COUNT: u8 = 12;

/// Name checks a player may run per day (typing a name checks it live, so this is generous).
/// Abuse guards, not economy: they stay constants rather than `settings`.
pub const NICK_CHECKS_PER_DAY: i32 = 200;
pub(crate) const CAP_NICK_CHECK: &str = "nickname_check";
/// Failed claims per day: `create` answers "taken" too, so every claim costs one unit that a
/// successful claim gives back (a player who gets a character keeps a clean budget).
pub const NICK_CLAIMS_PER_DAY: i32 = 30;
pub(crate) const CAP_NICK_CLAIM: &str = "nickname_claim";
/// Constraint of the unique key in migration 0025.
/// Partial (`WHERE nickname_key IS NOT NULL`, 0025), and that matters for locking: Postgres counts
/// only full unique indexes as "key" columns, so an UPDATE of the name stays a no-key update and
/// does not wait on the KEY SHARE a fulfilment holds on the user row (restyle.rs). Making this
/// index full, or adding `UNIQUE (nickname_key)`, brings the rename/fulfil deadlock back.
pub(crate) const KEY_INDEX: &str = "users_nickname_key_uidx";

/// A look: indexes into the client's colour tables (`web/src/character/palette.ts`). Reading is
/// tolerant of fields a newer release adds (it also travels between api instances over Redis,
/// so an old instance must not drop the update); what a *client* may send is `LookRequest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    pub skin: u8,
    pub shirt: u8,
    pub pants: u8,
    pub shoes: u8,
    /// Stage 3 (hair, face, beard, hat, glasses): rows stored before it read as 0 (bald, calm
    /// face, nothing else) and a client that predates it may leave the fields out.
    #[serde(default)]
    pub hair_color: u8,
    #[serde(default)]
    pub hair: u8,
    #[serde(default)]
    pub face: u8,
    #[serde(default)]
    pub beard: u8,
    #[serde(default)]
    pub hat: u8,
    #[serde(default)]
    pub glasses: u8,
    /// Stage 3b (outfits): a skinned shell over the body and its colour; 0 = no outfit.
    #[serde(default)]
    pub outfit: u8,
    #[serde(default)]
    pub outfit_color: u8,
    /// Stage 4 (female body, ADR-027): 0 = male, 1 = female; rows stored before it read as male.
    #[serde(default)]
    pub body: u8,
    /// Stage 5 (builds, ADR-029): 0 = average, 1 = slim, 2 = sturdy; rows stored before it read as average.
    #[serde(default)]
    pub build: u8,
}

/// The look as a client sends it: unknown fields are refused, so a later stage (clothes)
/// is an explicit contract change, not something a client can sneak in.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LookRequest {
    pub skin: u8,
    pub shirt: u8,
    pub pants: u8,
    pub shoes: u8,
    #[serde(default)]
    pub hair_color: u8,
    #[serde(default)]
    pub hair: u8,
    #[serde(default)]
    pub face: u8,
    #[serde(default)]
    pub beard: u8,
    #[serde(default)]
    pub hat: u8,
    #[serde(default)]
    pub glasses: u8,
    #[serde(default)]
    pub outfit: u8,
    #[serde(default)]
    pub outfit_color: u8,
    #[serde(default)]
    pub body: u8,
    #[serde(default)]
    pub build: u8,
}

impl From<LookRequest> for Look {
    fn from(r: LookRequest) -> Self {
        Self {
            skin: r.skin,
            shirt: r.shirt,
            pants: r.pants,
            shoes: r.shoes,
            hair_color: r.hair_color,
            hair: r.hair,
            face: r.face,
            beard: r.beard,
            hat: r.hat,
            glasses: r.glasses,
            outfit: r.outfit,
            outfit_color: r.outfit_color,
            body: r.body,
            build: r.build,
        }
    }
}

impl Look {
    pub fn is_valid(&self) -> bool {
        self.skin < SKIN_COUNT
            && self.shirt < SHIRT_COUNT
            && self.pants < PANTS_COUNT
            && self.shoes < SHOES_COUNT
            && self.hair_color < HAIR_COLOR_COUNT
            && self.hair < HAIR_COUNT
            && self.face < FACE_COUNT
            && self.beard < BEARD_COUNT
            && self.hat < HAT_COUNT
            && self.glasses < GLASSES_COUNT
            && self.outfit < OUTFIT_COUNT
            && self.outfit_color < OUTFIT_COLOR_COUNT
            && self.body < BODY_COUNT
            && self.build < BUILD_COUNT
    }

    /// Reads the stored column; a value that is not a valid look counts as "no look".
    pub fn from_db(value: Option<serde_json::Value>) -> Option<Self> {
        let value = value.filter(serde_json::Value::is_object)?;
        let look: Self = serde_json::from_value(value).ok()?;
        look.is_valid().then_some(look)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Character {
    pub nickname: String,
    pub look: Look,
}

/// Answer of the live name check while a player types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NickCheck {
    Available,
    /// Wrong shape; the reason is worded for the player.
    Invalid(&'static str),
    NotAllowed,
    Taken,
}

/// `POST /me/character`: validates, then claims the name for this account, once.
pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    raw_nickname: &str,
    look: Look,
) -> Result<Character, WorldError> {
    if !look.is_valid() {
        return Err(WorldError::InvalidLook);
    }
    // Every claim costs one unit of the day's budget, refunded when it succeeds: a taken-name
    // or not-allowed answer is an oracle like the live check and must not be free to repeat.
    let mut tx = pool.begin().await?;
    caps::consume(&mut tx, user_id, CAP_NICK_CLAIM, NICK_CLAIMS_PER_DAY).await?;
    tx.commit().await?;
    let Checked { nickname, key } = nickname::validate(raw_nickname)?;
    let stored = serde_json::to_value(look).map_err(|_| WorldError::InvalidLook)?;
    // A name another account gave up within 30 days stays with it (0030, ADR-039), a blocked one
    // with nobody (0031, ADR-040); the key lock keeps the check and the claim together.
    let mut tx = pool.begin().await?;
    rename::lock_keys(&mut tx, &[&key]).await?;
    rename::claimable(&mut tx, user_id, &key).await?;
    let result = sqlx::query(
        "UPDATE cf.users SET nickname = $2, nickname_key = $3, look = $4 WHERE id = $1 AND nickname IS NULL",
    )
    .bind(user_id)
    .bind(&nickname)
    .bind(&key)
    .bind(&stored)
    .execute(&mut *tx)
    .await;
    match result {
        Ok(done) if done.rows_affected() == 1 => {
            tx.commit().await?;
            // The character exists now: a failed refund must not turn that into an error.
            if let Err(e) = refund_claim(pool, user_id).await {
                tracing::warn!(error = %e, "nickname claim refund failed");
            }
            Ok(Character { nickname, look })
        }
        Ok(_) => {
            // No row changed: either the account already has its character, or it is gone.
            let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM cf.users WHERE id = $1)")
                .bind(user_id)
                .fetch_one(pool)
                .await?;
            Err(if exists {
                WorldError::CharacterExists
            } else {
                WorldError::NotFound
            })
        }
        Err(sqlx::Error::Database(db)) if db.constraint() == Some(KEY_INDEX) => {
            Err(WorldError::NicknameTaken)
        }
        Err(e) => Err(e.into()),
    }
}

async fn refund_claim(pool: &PgPool, user_id: Uuid) -> Result<(), WorldError> {
    let mut tx = pool.begin().await?;
    caps::release(&mut tx, user_id, CAP_NICK_CLAIM, 1).await?;
    tx.commit().await?;
    Ok(())
}

/// The account's character, if it has created one (presence handshake).
pub async fn load(pool: &PgPool, user_id: Uuid) -> Result<Option<Character>, WorldError> {
    let row = sqlx::query("SELECT nickname, look FROM cf.users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let nickname: Option<String> = row.try_get("nickname")?;
    let look = Look::from_db(row.try_get("look")?);
    Ok(nickname
        .zip(look)
        .map(|(nickname, look)| Character { nickname, look }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    const TOP: Look = Look {
        skin: 7,
        shirt: 11,
        pants: 11,
        shoes: 5,
        hair_color: 11,
        hair: 9,
        face: 4,
        beard: 5,
        hat: 7,
        glasses: 2,
        outfit: 8,
        outfit_color: 11,
        body: 1,
        build: 2,
    };

    #[test]
    fn look_indexes_must_sit_inside_the_tables() {
        assert!(TOP.is_valid());
        for bad in [
            Look { skin: 8, ..TOP },
            Look { shirt: 12, ..TOP },
            Look { pants: 12, ..TOP },
            Look { shoes: 6, ..TOP },
            Look { skin: 255, ..TOP },
            Look {
                hair_color: 12,
                ..TOP
            },
            Look { hair: 10, ..TOP },
            Look { face: 5, ..TOP },
            Look { beard: 6, ..TOP },
            Look { hat: 8, ..TOP },
            Look { glasses: 3, ..TOP },
            Look { outfit: 9, ..TOP },
            Look {
                outfit_color: 12,
                ..TOP
            },
            Look { body: 2, ..TOP },
            Look { body: 255, ..TOP },
            Look { build: 3, ..TOP },
            Look { build: 255, ..TOP },
        ] {
            assert!(!bad.is_valid(), "{bad:?}");
        }
    }

    #[test]
    fn stored_look_round_trips_and_garbage_is_no_look() {
        let json = serde_json::to_value(TOP).unwrap();
        assert_eq!(Look::from_db(Some(json)), Some(TOP));
        assert_eq!(Look::from_db(None), None);
        let old = serde_json::json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 });
        let read = Look::from_db(Some(old)).unwrap();
        assert_eq!(
            (read.skin, read.hair, read.face, read.hat),
            (1, 0, 0, 0),
            "rows from before stage 3 read as bald"
        );
        assert_eq!(read.body, 0, "rows from before the female body read as male");
        assert_eq!(read.build, 0, "rows from before the builds read as average");
        assert_eq!(
            Look::from_db(Some(
                serde_json::json!({ "skin": 99, "shirt": 0, "pants": 0, "shoes": 0 })
            )),
            None
        );
        assert_eq!(
            Look::from_db(Some(
                serde_json::json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "hat": 99 })
            )),
            None,
            "a part outside its table is no look"
        );
        // A field a newer release adds is ignored when reading, refused when a client sends it.
        let newer = serde_json::json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "wings": 1 });
        assert!(Look::from_db(Some(newer.clone())).is_some());
        assert!(serde_json::from_value::<LookRequest>(newer).is_err());
        assert_eq!(Look::from_db(Some(serde_json::json!([1, 2, 3, 4]))), None);
        assert_eq!(
            Look::from_db(Some(
                serde_json::json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "body": 2 })
            )),
            None,
            "a body type outside its table is no look"
        );
        assert_eq!(
            Look::from_db(Some(
                serde_json::json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": 3 })
            )),
            None,
            "a build outside its table is no look"
        );
    }

    #[test]
    fn a_client_may_leave_the_new_parts_out() {
        let request: LookRequest =
            serde_json::from_value(serde_json::json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 }))
                .unwrap();
        let look = Look::from(request);
        assert!(look.is_valid());
        assert_eq!(
            (
                look.hair,
                look.hair_color,
                look.face,
                look.beard,
                look.hat,
                look.glasses,
                look.outfit,
                look.outfit_color,
                look.body,
                look.build
            ),
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
        );
    }
}
