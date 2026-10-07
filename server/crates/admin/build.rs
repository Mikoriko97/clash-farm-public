// The integration tests embed server/migrations with sqlx::migrate! at compile time.
// Without this, adding a migration does not rebuild the test binaries: a rebuilt
// binary applies the new version to the shared test DB and a stale one then fails
// with VersionMissing (same as crates/api/build.rs, 2026-09-24).
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
