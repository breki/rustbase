//! `cargo xtask dupes`: the shared library's code-duplication gate.

use rustbase_xtask::dupes;

use crate::helpers::tasks_config;

/// Standalone `cargo xtask dupes`.
pub fn dupes() -> Result<(), String> {
    let detail = dupes::check(&tasks_config())?;
    println!("Duplication OK ({detail})");
    Ok(())
}
