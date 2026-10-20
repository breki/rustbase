//! `cargo xtask fmt`: the shared library's format gate.

use rustbase_xtask::fmt;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Diff lines a failure report shows.
const MAX_DIFF_LINES: usize = 20;

/// The `validate` step. Auto-fixes unless `check` selects the
/// read-only path; empty detail on success.
pub fn step(check: bool) -> Step {
    Step::new("Fmt", "fmt", move || {
        let cfg = tasks_config();
        let r = if check {
            fmt::check(&cfg)
        } else {
            fmt::fix(&cfg)
        };
        r.map_err(|f| f.report(MAX_DIFF_LINES))?;
        Ok(String::new())
    })
}

/// Run `cargo fmt --all`. Its output streams to the terminal, so a
/// failure's error is only that it failed; the cap does not apply.
pub fn fmt() -> Result<(), String> {
    fmt::fix(&tasks_config()).map_err(|f| f.report(MAX_DIFF_LINES))
}
