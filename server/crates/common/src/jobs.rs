//! Names and cadence of the cf-api background jobs, and the Redis keys where the runner
//! leaves their traces (heartbeat, failures). Shared with cf-admin («Система») and the
//! `alerts` job, so a job that stops or keeps failing is noticed without log access.

/// (job name, interval in seconds): the schedule `cf_api::jobs::spawn_all` runs.
pub const SCHEDULE: &[(&str, u64)] = &[
    ("expire_quotes", 60),
    ("retention", 6 * 3600),
    ("reconcile_ledger", 24 * 3600),
    ("finalize_burns", 120),
    ("regrow_resources", 60),
    ("alerts", 60),
];

pub fn every_secs(name: &str) -> Option<u64> {
    SCHEDULE.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Unix seconds of the last completed run (success or failure); kept a week so a
/// restart does not look like a stopped job.
pub fn heartbeat_key(name: &str) -> String {
    format!("jobs:heartbeat:{name}")
}

/// Failures counted over a sliding day.
pub fn failures_key(name: &str) -> String {
    format!("jobs:failures:{name}")
}

/// The last error's text (≤ 200 chars), kept a day.
pub fn last_error_key(name: &str) -> String {
    format!("jobs:last_error:{name}")
}

pub const HEARTBEAT_TTL_SECS: u64 = 7 * 24 * 3600;
pub const FAILURES_TTL_SECS: u64 = 24 * 3600;

/// "щохвилини" / "кожні 2 хв" / "кожні 6 год" for messages and the admin.
pub fn every_text(secs: u64) -> String {
    match secs {
        0..=60 => "щохвилини".to_owned(),
        61..=3599 => format!("кожні {} хв", secs / 60),
        _ => format!("кожні {} год", secs / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_names_are_unique_and_known() {
        for (name, secs) in SCHEDULE {
            assert_eq!(every_secs(name), Some(*secs));
            assert_eq!(SCHEDULE.iter().filter(|(n, _)| n == name).count(), 1, "{name}");
        }
        assert_eq!(every_secs("nope"), None);
        assert_eq!(every_text(60), "щохвилини");
        assert_eq!(every_text(120), "кожні 2 хв");
        assert_eq!(every_text(6 * 3600), "кожні 6 год");
        assert_eq!(heartbeat_key("alerts"), "jobs:heartbeat:alerts");
    }
}
