//! `cargo xtask test`: the shared library's test gate.

use rustbase_xtask::cargo_test;
pub use rustbase_xtask::cargo_test::Options as TestOptions;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Prints `Test OK`, or returns the failures as the one report: every
/// failing test is named, so the report is not truncated here.
pub fn test(opts: TestOptions<'_>) -> Result<(), String> {
    cargo_test::run(&tasks_config(), opts).map_err(|f| f.to_string())?;
    if !opts.verbose {
        println!("Test OK");
    }
    Ok(())
}

/// The `validate` step: xtask's own tests only, since Coverage runs
/// every other crate's.
pub fn step() -> Step {
    Step::new("Test (xtask only)", "test", || {
        cargo_test::tooling_only(&tasks_config()).map_err(|f| f.to_string())?;
        Ok(String::new())
    })
}
