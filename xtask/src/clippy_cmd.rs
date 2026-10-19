//! `cargo xtask clippy`: the shared library's lint gate (workspace,
//! then `xtask` with every feature off and each alone).

use rustbase_xtask::clippy;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Diagnostic lines the standalone command shows.
const MAX_WARNING_LINES: usize = 10;

/// Lines the `validate` step shows, fewer because they sit among
/// other steps.
const STEP_WARNING_LINES: usize = 5;

/// Prints `Clippy OK`, or returns the failing pass as the one
/// report.
pub fn clippy() -> Result<(), String> {
    clippy::check(&tasks_config()).map_err(|f| f.report(MAX_WARNING_LINES))?;
    println!("Clippy OK");
    Ok(())
}

/// The `validate` step; empty detail on success.
pub fn step() -> Step {
    Step::new("Clippy", "clippy", || {
        clippy::check(&tasks_config())
            .map_err(|f| f.report(STEP_WARNING_LINES))?;
        Ok(String::new())
    })
}
