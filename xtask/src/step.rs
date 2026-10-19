//! One `validate` gate as data. Each gate module builds its own
//! with a `step()` function. Gates depend only on this type,
//! never on `validate`, which lists them without a cycle.

/// A single validation step: a display name, the subcommand to
/// re-run when it fails (the `iterate with` hint), and the
/// closure that performs it.
pub struct Step {
    name: &'static str,
    cmd: &'static str,
    run: Box<dyn FnOnce() -> Result<String, String>>,
}

impl Step {
    /// A step named `name`, re-run with `cargo xtask <cmd>`
    /// when it fails. `run` returns the detail printed after
    /// `OK` (empty for none) or the error.
    pub fn new(
        name: &'static str,
        cmd: &'static str,
        run: impl FnOnce() -> Result<String, String> + 'static,
    ) -> Self {
        Self {
            name,
            cmd,
            run: Box::new(run),
        }
    }

    /// The name printed on the step's `[N/M]` line.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The subcommand to re-run when this step fails.
    pub fn cmd(&self) -> &'static str {
        self.cmd
    }

    /// Run the step, consuming it.
    pub fn run(self) -> Result<String, String> {
        (self.run)()
    }
}
