//! `cargo xtask records-check`: the shared library's integrity check
//! of the record files (the work queue, the reviewer logs and the
//! template feedback): the library's default list, unless
//! `tasks_config()` sets `record_files`.

use rustbase_xtask::records;

use crate::helpers::tasks_config;
use crate::step::Step;

/// Standalone `cargo xtask records-check`.
pub fn records_check() -> Result<(), String> {
    let detail = records::check(&tasks_config())?;
    println!("Records OK ({detail})");
    Ok(())
}

/// The `validate` step.
pub fn step() -> Step {
    Step::new("Records", "records-check", || {
        records::check(&tasks_config())
    })
}
