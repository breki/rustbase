//! `cargo xtask coverage`: the shared library's line-coverage gate.

use rustbase_xtask::coverage;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Standalone `cargo xtask coverage`.
pub fn coverage() -> Result<(), String> {
    let r = coverage::check(&tasks_config())?;
    println!("  lines: {}/{} ({:.1}%)", r.covered, r.total, r.line_pct);
    println!("  coverage OK ({r})");
    Ok(())
}

/// The `validate` step.
pub fn step() -> Step {
    Step::new("Coverage", "coverage", || {
        Ok(coverage::check(&tasks_config())?.to_string())
    })
}
