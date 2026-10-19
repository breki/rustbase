//! `cargo xtask test`: the shared library's test gate.

use rustbase_xtask::cargo_test;
pub use rustbase_xtask::cargo_test::Options as TestOptions;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Lines a failure report shows: failing tests with their details,
/// or compile errors.
const MAX_REPORT_LINES: usize = 40;

/// Prints `Test OK`, or returns the failures as the one report.
pub fn test(opts: TestOptions<'_>) -> Result<(), String> {
    cargo_test::run(&tasks_config(), opts)
        .map_err(|f| f.report(MAX_REPORT_LINES))?;
    if !opts.verbose {
        println!("Test OK");
    }
    Ok(())
}

/// The `validate` step: xtask's own tests only, since Coverage runs
/// every other crate's.
pub fn step() -> Step {
    Step::new("Test (xtask only)", "test", || {
        cargo_test::tooling_only(&tasks_config())
            .map_err(|f| f.report(MAX_REPORT_LINES))?;
        Ok(String::new())
    })
}
