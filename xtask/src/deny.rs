//! `cargo xtask deny`: the shared library's offline licence,
//! bans and sources gate, configured by `deny.toml`.

use rustbase_xtask::deny;

use crate::helpers::tasks_config;

/// Standalone `cargo xtask deny`.
pub fn deny() -> Result<(), String> {
    let checks = deny::check(&tasks_config())?;
    println!("Deny OK ({checks})");
    Ok(())
}
