// The integration tests embed server/migrations with sqlx::migrate! at compile time.
// Without this, adding a migration does not rebuild the test binaries: one rebuilt
// binary applies the new version to the shared test DB and every stale one then
// fails with VersionMissing (2026-09-24, migration 0018).
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
