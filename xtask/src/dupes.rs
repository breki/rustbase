//! `cargo xtask dupes`: the shared library's code-duplication gate.

use rustbase_xtask::dupes;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Standalone `cargo xtask dupes`.
pub fn dupes() -> Result<(), String> {
    let detail = dupes::check(&tasks_config())?;
    println!("Duplication OK ({detail})");
    Ok(())
}

/// The `validate` step.
pub fn step() -> Step {
    Step::new("Duplication", "dupes", || dupes::check(&tasks_config()))
}
