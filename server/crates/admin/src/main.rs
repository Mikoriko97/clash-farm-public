//! `cf-admin` binary: config from env, connect, serve on loopback with graceful shutdown.
//! `cf-admin totp-new [sol:<wallet>]` prints a fresh second-factor secret (ADR-023).
#![forbid(unsafe_code)]

use cf_admin::totp::TotpSecret;
use cf_admin::{build_app, AdminConfig, AdminState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("totp-new") {
        totp_new(args.next().as_deref());
        return Ok(());
    }
    let config = AdminConfig::from_env()?;
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let bind = config.bind;
    let admins = config.admin_wallets.len();
    let totp = match (config.totp_required, config.totp_secrets.len()) {
        (true, _) => "required".to_owned(),
        (false, 0) => "off".to_owned(),
        (false, n) => format!("optional ({n} of {admins} wallets)"),
    };
    let state = AdminState::connect(config).await?;
    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, admins, totp, "cf-admin listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Shown once on the operator's terminal, never logged: the operator puts the entry into
/// the environment and the URI into an authenticator app (RUNBOOK §7).
fn totp_new(wallet: Option<&str>) {
    let secret = TotpSecret::generate();
    let account = wallet.unwrap_or("sol:<wallet>");
    println!("CF_ADMIN_TOTP_SECRETS entry (append with a comma for several admins):");
    println!("  {account}={}", secret.to_base32());
    println!();
    println!(
        "otpauth URI for Aegis / Google Authenticator / 1Password (enter manually or make a QR locally):"
    );
    println!("  {}", secret.otpauth_uri(account));
    println!();
    println!("Shown once. Store it only in the environment or the secret manager, never in git or chat.");
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::warn!("ctrl_c handler unavailable; running until killed");
        std::future::pending::<()>().await;
    }
    tracing::info!("shutdown signal received");
}
