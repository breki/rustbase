//! `cargo xtask audit`: the shared library's security-advisory
//! gate, over `Cargo.lock` and the frontend.

use std::path::PathBuf;

use rustbase_xtask::{Strictness, audit};

use crate::helpers::tasks_config;
use crate::step::Step;

/// The npm project to audit. Absolute, so the result does not
/// depend on the shell's working directory; the library skips it
/// when it holds no `package.json`.
fn frontend_dir(cfg: &rustbase_xtask::Config) -> PathBuf {
    cfg.root.join("frontend")
}

/// Standalone `cargo xtask audit`. Errors when the audit could not
/// complete: a human who ran it explicitly wants to know.
pub fn audit() -> Result<(), String> {
    let cfg = tasks_config();
    let detail =
        audit::check(&cfg, Some(&frontend_dir(&cfg)), Strictness::Fail)?;
    println!("Audit OK ({detail})");
    Ok(())
}

/// The `validate` step. Fails on any vulnerability, but only warns
/// when the audit could not complete, so an offline run is not
/// blocked.
pub fn step() -> Step {
    Step::new("Audit", "audit", || {
        let cfg = tasks_config();
        audit::check(&cfg, Some(&frontend_dir(&cfg)), Strictness::Warn)
    })
}
