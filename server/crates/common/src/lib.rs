//! Shared, dependency-light pieces: process configuration from `CF_*` env vars and
//! small hashing helpers. No axum, no sqlx here (docs/ARCHITECTURE.md §3).
#![forbid(unsafe_code)]

pub mod config;
pub mod hashing;
pub mod jobs;

pub use config::{AlertsConfig, Config, ConfigError, Env, PaymentsConfig, Secret};
