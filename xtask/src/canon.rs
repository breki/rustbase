//! `cargo xtask canon-check`: the shared library's check of the
//! claims canon prose makes (paths, cross-references, `git`
//! grants, width, backlog IDs). This project's exemptions are in
//! `tasks_config()`.

use rustbase_xtask::canon;

use crate::helpers::tasks_config;

/// Standalone `cargo xtask canon-check`.
pub fn canon_check() -> Result<(), String> {
    let detail = canon::check(&tasks_config())?;
    println!("Canon OK ({detail})");
    Ok(())
}
