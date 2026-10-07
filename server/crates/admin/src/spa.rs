//! The admin app shell (ADR-022): `web/dist-admin` built by Vite, served from
//! `CF_ADMIN_STATIC_DIR`. `admin.html` needs the admin session (no session → the login
//! page); hashed files under `/assets` are public but carry no data. Without a built
//! app the server still starts and says how to build it, so the JSON API stays usable.

use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};

use crate::error::AdminError;
use crate::html;
use crate::session::Admin;
use crate::state::AdminState;

const MISSING: &str = "<p>Збірку адмін-застосунку не знайдено.</p><p>Збери її: <code>npm run build:admin</code> у <code>web/</code> \
і задай <code>CF_ADMIN_STATIC_DIR=web/dist-admin</code> (docs/RUNBOOK.md §7). JSON API під <code>/api/</code> працює.</p>";

/// `GET /` and every app route: the shell for a signed-in admin, `/login` otherwise.
pub async fn index(State(state): State<AdminState>, _admin: Admin) -> Result<Response, AdminError> {
    let Some(dir) = state.config.static_dir.as_ref() else {
        return Ok(html::page("Адмінка", MISSING).into_response());
    };
    // Vite keeps the entry's name: `admin.html` is the app shell.
    match tokio::fs::read(dir.join("admin.html")).await {
        Ok(bytes) => Ok((
            [
                (CONTENT_TYPE, "text/html; charset=utf-8"),
                (CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response()),
        Err(err) => {
            tracing::error!(error = %err, dir = %dir.display(), "admin static dir unreadable");
            Ok(html::page("Адмінка", MISSING).into_response())
        }
    }
}
