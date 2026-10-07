use std::collections::BTreeMap;

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::{LedgerError, CHECK_VIOLATION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Currency {
    Credits,
    Xp,
    Clash,
}

impl Currency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Credits => "CREDITS",
            Self::Xp => "XP",
            Self::Clash => "CLASH",
        }
    }
}

/// One line of a posting. `user_id: None` means a `system:*` account.
#[derive(Debug, Clone)]
pub struct Entry {
    pub user_id: Option<Uuid>,
    pub account: String,
    pub currency: Currency,
    pub delta: i64,
}

impl Entry {
    pub fn user(user_id: Uuid, account: &str, currency: Currency, delta: i64) -> Self {
        Self {
            user_id: Some(user_id),
            account: account.to_owned(),
            currency,
            delta,
        }
    }

    pub fn system(account: &str, currency: Currency, delta: i64) -> Self {
        Self {
            user_id: None,
            account: account.to_owned(),
            currency,
            delta,
        }
    }
}

/// Validates the posting, updates `balances` for user entries (locking rows in a
/// stable order) and appends the ledger rows. Must run inside the caller's
/// transaction so the game effect and the money move together or not at all.
pub async fn post(
    conn: &mut PgConnection,
    ref_type: &str,
    ref_id: Uuid,
    entries: &[Entry],
) -> Result<(), LedgerError> {
    validate(entries)?;
    let mut ordered: Vec<&Entry> = entries.iter().filter(|e| e.user_id.is_some()).collect();
    ordered.sort_by(|a, b| (a.user_id, &a.account).cmp(&(b.user_id, &b.account)));
    for entry in ordered {
        apply_balance(conn, entry).await?;
    }
    for entry in entries {
        sqlx::query(
            "INSERT INTO cf.ledger (user_id, account, delta, currency, ref_type, ref_id)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(entry.user_id)
        .bind(&entry.account)
        .bind(entry.delta)
        .bind(entry.currency.as_str())
        .bind(ref_type)
        .bind(ref_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn validate(entries: &[Entry]) -> Result<(), LedgerError> {
    if entries.len() < 2 {
        return Err(LedgerError::EmptyPosting);
    }
    let mut sums: BTreeMap<Currency, i64> = BTreeMap::new();
    for entry in entries {
        if entry.delta == 0 {
            return Err(LedgerError::ZeroDelta);
        }
        if entry.user_id.is_none() != entry.account.starts_with("system:") {
            return Err(LedgerError::BadAccount);
        }
        let sum = sums.entry(entry.currency).or_insert(0);
        *sum = sum
            .checked_add(entry.delta)
            .ok_or(LedgerError::Unbalanced(entry.currency.as_str()))?;
    }
    for (currency, sum) in sums {
        if sum != 0 {
            return Err(LedgerError::Unbalanced(currency.as_str()));
        }
    }
    Ok(())
}

/// Ensures the balance row exists (amount 0, never violates the CHECK), then
/// UPDATEs it under the row lock: concurrent postings on one balance serialise
/// and a debit below zero trips `CHECK (amount >= 0)`. A plain upsert would not
/// do: Postgres evaluates the CHECK on the proposed row before ON CONFLICT, so
/// every debit would be rejected. No retry loop: a failed statement aborts the
/// enclosing transaction anyway.
async fn apply_balance(conn: &mut PgConnection, entry: &Entry) -> Result<(), LedgerError> {
    sqlx::query(
        "INSERT INTO cf.balances (user_id, account, amount) VALUES ($1, $2, 0)
         ON CONFLICT (user_id, account) DO NOTHING",
    )
    .bind(entry.user_id)
    .bind(&entry.account)
    .execute(&mut *conn)
    .await?;
    let updated = map_check(
        sqlx::query(
            "UPDATE cf.balances SET amount = amount + $3, updated_at = now()
             WHERE user_id = $1 AND account = $2",
        )
        .bind(entry.user_id)
        .bind(&entry.account)
        .bind(entry.delta)
        .execute(&mut *conn)
        .await,
    )?;
    if updated.rows_affected() == 0 {
        // Cannot happen after the INSERT above; treat as a refusal rather than a silent no-op.
        return Err(LedgerError::InsufficientFunds);
    }
    Ok(())
}

fn map_check<T>(result: Result<T, sqlx::Error>) -> Result<T, LedgerError> {
    match result {
        Ok(value) => Ok(value),
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some(CHECK_VIOLATION) => {
            Err(LedgerError::InsufficientFunds)
        }
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn uid() -> Uuid {
        Uuid::from_u128(7)
    }

    #[test]
    fn balanced_postings_pass_validation() {
        let entries = [
            Entry::system("system:mint", Currency::Credits, -10),
            Entry::user(uid(), "credits", Currency::Credits, 10),
        ];
        validate(&entries).unwrap();
    }

    #[test]
    fn unbalanced_zero_and_single_entries_are_rejected() {
        let one = [Entry::user(uid(), "credits", Currency::Credits, 10)];
        assert!(matches!(validate(&one), Err(LedgerError::EmptyPosting)));
        let unbalanced = [
            Entry::system("system:mint", Currency::Credits, -10),
            Entry::user(uid(), "credits", Currency::Credits, 11),
        ];
        assert!(matches!(
            validate(&unbalanced),
            Err(LedgerError::Unbalanced("CREDITS"))
        ));
        let zero = [
            Entry::system("system:mint", Currency::Credits, 0),
            Entry::user(uid(), "credits", Currency::Credits, 0),
        ];
        assert!(matches!(validate(&zero), Err(LedgerError::ZeroDelta)));
        let cross = [
            Entry::system("system:mint", Currency::Credits, -10),
            Entry::user(uid(), "credits", Currency::Xp, 10),
        ];
        assert!(matches!(validate(&cross), Err(LedgerError::Unbalanced(_))));
        let bad = [
            Entry::user(uid(), "system:mint", Currency::Credits, -10),
            Entry::system("credits", Currency::Credits, 10),
        ];
        assert!(matches!(validate(&bad), Err(LedgerError::BadAccount)));
    }
}
