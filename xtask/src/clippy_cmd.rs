//! `cargo xtask clippy`: the shared library's lint gate (workspace,
//! then `xtask` with every feature off and each alone).

use rustbase_xtask::clippy;

use crate::helpers::tasks_config;

/// Diagnostic lines the standalone command shows.
const MAX_WARNING_LINES: usize = 10;

/// Prints `Clippy OK`, or returns the failing pass as the one
/// report.
pub fn clippy() -> Result<(), String> {
    clippy::check(&tasks_config()).map_err(|f| f.report(MAX_WARNING_LINES))?;
    println!("Clippy OK");
    Ok(())
}
