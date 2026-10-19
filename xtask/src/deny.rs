//! `cargo xtask deny`: the shared library's offline licence,
//! bans and sources gate, configured by `deny.toml`.

use rustbase_xtask::deny;

use crate::helpers::tasks_config;
use crate::step::Step;

/// What the gate checks, for the success line.
const CHECKS: &str = "licenses, bans, sources";

/// Standalone `cargo xtask deny`.
pub fn deny() -> Result<(), String> {
    deny::check(&tasks_config())?;
    println!("Deny OK ({CHECKS})");
    Ok(())
}

/// The `validate` step. Unlike Audit it needs no network, so a
/// missing `cargo-deny` is a hard error rather than a warning.
pub fn step() -> Step {
    Step::new("Deny", "deny", || {
        deny::check(&tasks_config())?;
        Ok(CHECKS.into())
    })
}
