//! `cargo xtask audit`: the shared library's security-advisory
//! gate, over `Cargo.lock` and the npm project `tasks_config()`
//! names (the frontend).

use rustbase_xtask::{Strictness, audit};

use crate::helpers::tasks_config;

/// Standalone `cargo xtask audit`. Errors when the audit could not
/// complete: a human who ran it explicitly wants to know.
pub fn audit() -> Result<(), String> {
    let cfg = tasks_config();
    let detail =
        audit::check(&cfg, cfg.audit_npm_dir.as_deref(), Strictness::Fail)?;
    println!("Audit OK ({detail})");
    Ok(())
}
