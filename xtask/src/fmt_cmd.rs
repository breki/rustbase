//! `cargo xtask fmt`: the shared library's format gate.

use rustbase_xtask::fmt;

use crate::helpers::tasks_config;

/// Diff lines a failure report shows.
const MAX_DIFF_LINES: usize = 20;

/// Run `cargo fmt --all`. Its output streams to the terminal, so a
/// failure's error is only that it failed; the cap does not apply.
pub fn fmt() -> Result<(), String> {
    fmt::fix(&tasks_config()).map_err(|f| f.report(MAX_DIFF_LINES))
}
