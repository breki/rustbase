//! A copy of a long-running command's output in `target/<name>.log`,
//! rewritten on each run, so the whole record survives when the
//! terminal closes or a caller shows only the tail (`CLAUDE.md`'s
//! **Long-running scripts** rule). Taken from jutro's `xtask_log`.

use std::cell::{Cell, RefCell};
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// An open `target/<name>.log`. Each line goes to the terminal and the
/// file; the file is best-effort, so a failed write never fails the
/// command.
#[derive(Debug)]
pub struct Tee {
    path: PathBuf,
    file: RefCell<File>,
    /// Set by the first failed write, which warns once.
    write_failed: Cell<bool>,
}

impl Tee {
    /// Open `<root>/target/<name>.log`, truncating it and creating
    /// `target/` if missing.
    ///
    /// # Errors
    ///
    /// When `name` is not a bare file stem (empty, a separator or
    /// `..`), which keeps the log inside `target/`, or when the file
    /// cannot be created.
    pub fn open(root: &Path, name: &str) -> Result<Self, String> {
        if name.is_empty() || name.contains(['/', '\\']) || name.contains("..")
        {
            return Err(format!(
                "invalid log name {name:?}: must be a bare stem (no \
                 separators, no `..`)"
            ));
        }
        let dir = root.join("target");
        fs::create_dir_all(&dir)
            .map_err(|e| format!("create {}: {e}", dir.display()))?;
        let path = dir.join(format!("{name}.log"));
        let file = File::create(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        Ok(Self::from_file(path, file))
    }

    fn from_file(path: PathBuf, file: File) -> Self {
        Self {
            path,
            file: RefCell::new(file),
            write_failed: Cell::new(false),
        }
    }

    /// The log file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `line` to stdout and the log.
    pub fn println(&self, line: &str) {
        println!("{line}");
        self.record(line);
    }

    /// `line` to stderr and the log.
    pub fn eprintln(&self, line: &str) {
        eprintln!("{line}");
        self.record(line);
    }

    /// `line` to the log only, for text the caller prints itself.
    pub fn record(&self, line: &str) {
        let failed = writeln!(self.file.borrow_mut(), "{line}").is_err();
        if failed && !self.write_failed.replace(true) {
            eprintln!(
                "warning: writing {} failed; further errors suppressed \
                 (the terminal output is complete)",
                self.path.display()
            );
        }
    }

    /// Whether a write to the log has failed.
    pub fn write_failed(&self) -> bool {
        self.write_failed.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    #[test]
    fn every_line_reaches_the_log_and_a_new_run_starts_it_afresh() {
        let dir = test_repo::empty_dir();
        let tee = Tee::open(&dir.0, "run").unwrap();
        assert_eq!(tee.path(), dir.0.join("target/run.log"));
        tee.println("out");
        tee.eprintln("err");
        tee.record("only logged");
        drop(tee);
        let log = fs::read_to_string(dir.0.join("target/run.log")).unwrap();
        assert_eq!(log, "out\nerr\nonly logged\n");
        let tee = Tee::open(&dir.0, "run").unwrap();
        tee.record("second");
        drop(tee);
        let log = fs::read_to_string(dir.0.join("target/run.log")).unwrap();
        assert_eq!(log, "second\n");
    }

    #[test]
    fn a_name_that_could_leave_target_is_refused() {
        let dir = test_repo::empty_dir();
        for bad in ["", "a/b", "a\\b", "..", "x..y"] {
            let e = Tee::open(&dir.0, bad).unwrap_err();
            assert!(e.contains("bare stem"), "{bad:?}: {e}");
        }
    }

    #[test]
    fn a_failed_write_is_remembered_and_never_fails_the_caller() {
        let dir = test_repo::empty_dir();
        let path = dir.0.join("ro.log");
        fs::write(&path, "").unwrap();
        // Opened for reading, so every write fails.
        let tee = Tee::from_file(path.clone(), File::open(&path).unwrap());
        assert!(!tee.write_failed());
        tee.record("a");
        tee.record("b");
        assert!(tee.write_failed());
    }

    #[test]
    fn a_log_that_cannot_be_created_is_an_error() {
        let dir = test_repo::empty_dir();
        fs::write(dir.0.join("target"), "a file").unwrap();
        assert!(Tee::open(&dir.0, "run").unwrap_err().contains("create"));
    }
}
