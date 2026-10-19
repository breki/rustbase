//! `cargo xtask doc`: the shared library's doc gate (rustdoc with
//! `-D warnings`, public and private passes). `validate` runs it
//! as its Doc step.

use rustbase_xtask::doc::{self, STDERR_TAIL_LINES};

use crate::helpers::tasks_config;
use crate::step::Step;

/// Maximum number of diagnostic lines the standalone command shows.
const MAX_DIAGNOSTIC_LINES: usize = 10;

/// Prints `Doc OK` on success, or `FAILED` with the offending lines.
pub fn doc() -> Result<(), String> {
    let Some(failure) = doc::check(&tasks_config())? else {
        println!("Doc OK");
        return Ok(());
    };
    eprintln!("FAILED: {}\n", failure.summary);
    for line in failure.items.iter().take(MAX_DIAGNOSTIC_LINES) {
        eprintln!("  {line}");
    }
    if failure.items.len() > MAX_DIAGNOSTIC_LINES {
        eprintln!(
            "  ... and {} more",
            failure.items.len() - MAX_DIAGNOSTIC_LINES
        );
    }
    Err(failure.summary)
}

/// The `validate` step; empty detail on success.
pub fn step() -> Step {
    Step::new("Doc", "doc", || match doc::check(&tasks_config())? {
        None => Ok(String::new()),
        Some(failure) => {
            for line in failure.items.iter().take(STDERR_TAIL_LINES) {
                eprintln!("  {line}");
            }
            Err(failure.summary)
        }
    })
}
