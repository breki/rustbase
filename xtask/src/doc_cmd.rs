//! `cargo xtask doc`: the shared library's doc gate (rustdoc with
//! `-D warnings`, public and private passes). `validate` runs it
//! as its Doc step.

use rustbase_xtask::doc;

use crate::helpers::tasks_config;

/// Diagnostic lines the standalone command shows.
const MAX_DIAGNOSTIC_LINES: usize = 10;

/// Prints `Doc OK`, or returns the failing pass as the one report.
pub fn doc() -> Result<(), String> {
    doc::check(&tasks_config()).map_err(|f| f.report(MAX_DIAGNOSTIC_LINES))?;
    println!("Doc OK");
    Ok(())
}
