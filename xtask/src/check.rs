//! `cargo xtask check`: the shared library's type-check gate.

use rustbase_xtask::check;

use crate::helpers::tasks_config;

/// Error lines a failure report shows.
const MAX_ERROR_LINES: usize = 10;

/// Prints `Check OK`, or returns the compilation errors as the one
/// report.
pub fn check() -> Result<(), String> {
    check::check(&tasks_config()).map_err(|f| f.report(MAX_ERROR_LINES))?;
    println!("Check OK");
    Ok(())
}
