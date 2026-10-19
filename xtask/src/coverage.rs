//! `cargo xtask coverage`: the shared library's line-coverage gate.

use rustbase_xtask::coverage;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Standalone `cargo xtask coverage`.
pub fn coverage() -> Result<(), String> {
    let cfg = tasks_config();
    let r = coverage::check(&cfg)?;
    println!("  lines: {}/{} ({:.1}%)", r.covered, r.total, r.line_pct);
    let min = cfg.thresholds.coverage_overall;
    println!("  coverage OK ({:.1}% >= {min}%)", r.line_pct);
    Ok(())
}

/// The `validate` step.
pub fn step() -> Step {
    Step::new("Coverage", "coverage", || {
        let cfg = tasks_config();
        let r = coverage::check(&cfg)?;
        Ok(format!(
            "{:.1}% >= {}%",
            r.line_pct, cfg.thresholds.coverage_overall
        ))
    })
}
