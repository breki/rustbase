//! A copy of a long-running command's output in `target/<name>.log`,
//! rewritten on each run, so the whole record survives when the
//! terminal closes or a caller shows only the tail (`CLAUDE.md`'s
//! **Long-running scripts** rule).

use std::cell::{Cell, RefCell};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

/// Terminal output, and a copy in `target/<name>.log` when opened with
/// [`Tee::open`]. The file is best-effort: a failed write warns once
/// and never fails the command.
#[derive(Debug)]
pub struct Tee {
    /// The log and its path; `None` for terminal-only output.
    file: Option<(PathBuf, RefCell<File>)>,
    /// The held `<name>.log.lock`, released on drop.
    lock: Option<File>,
    /// Set by the first failed write, which warns once.
    write_failed: Cell<bool>,
}

/// Why [`Tee::open`] could not open a log.
#[derive(Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Another run of the command holds this log (its path).
    Held(PathBuf),
    /// Anything else, as a message.
    Failed(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Held(p) => {
                write!(
                    f,
                    "{} is held by another run of this command",
                    p.display()
                )
            }
            Self::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<OpenError> for String {
    fn from(e: OpenError) -> Self {
        e.to_string()
    }
}

impl Tee {
    /// Open `<root>/target/<name>.log`, truncating it and creating
    /// `target/` if missing.
    ///
    /// # Errors
    ///
    /// [`OpenError::Held`] when another run holds the log;
    /// [`OpenError::Failed`] when `name` is not a single plain path
    /// component (empty, a separator, `.`, `..` or a Windows drive
    /// prefix), which keeps the log inside `target/`, or when the file
    /// cannot be created.
    pub fn open(root: &Path, name: &str) -> Result<Self, OpenError> {
        let failed = OpenError::Failed;
        let mut parts = Path::new(name).components();
        let plain = matches!(
            (parts.next(), parts.next()),
            (Some(Component::Normal(s)), None) if s == name
        );
        if !plain {
            return Err(failed(format!(
                "invalid log name {name:?}: must be a bare stem (one plain \
                 path component)"
            )));
        }
        let dir = root.join("target");
        fs::create_dir_all(&dir)
            .map_err(|e| failed(format!("create {}: {e}", dir.display())))?;
        let path = dir.join(format!("{name}.log"));
        // A sibling lock file, taken before the log is emptied, keeps a
        // second concurrent run off the first one's log. The log itself
        // is not locked: on Windows that would stop anyone reading it
        // while the run goes on. The lock is the operating system's and
        // ends when the `Tee` drops or the process exits; the empty
        // `.lock` file left behind means nothing, so a "held" error
        // means another run really is still going.
        let lock_path = dir.join(format!("{name}.log.lock"));
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| {
                failed(format!("open {}: {e}", lock_path.display()))
            })?;
        if let Err(e) = try_lock_briefly(&lock) {
            return Err(match e {
                TryLockError::WouldBlock => OpenError::Held(path),
                TryLockError::Error(e) => {
                    failed(format!("lock {}: {e}", lock_path.display()))
                }
            });
        }
        let file = File::create(&path)
            .map_err(|e| failed(format!("open {}: {e}", path.display())))?;
        let mut tee = Self::from_file(path, file);
        tee.lock = Some(lock);
        Ok(tee)
    }

    /// Terminal output only: [`Tee::record`] writes nothing.
    pub fn terminal() -> Self {
        Self {
            file: None,
            lock: None,
            write_failed: Cell::new(false),
        }
    }

    fn from_file(path: PathBuf, file: File) -> Self {
        Self {
            file: Some((path, RefCell::new(file))),
            lock: None,
            write_failed: Cell::new(false),
        }
    }

    /// The log file's path; `None` for terminal-only output.
    pub fn path(&self) -> Option<&Path> {
        self.file.as_ref().map(|(p, _)| p.as_path())
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
        let Some((path, file)) = &self.file else {
            return;
        };
        let failed = writeln!(file.borrow_mut(), "{line}").is_err();
        if failed && !self.write_failed.replace(true) {
            eprintln!(
                "warning: writing {} failed; further errors suppressed \
                 (the terminal output is complete)",
                path.display()
            );
        }
    }
}

/// `file.try_lock()`, retried for up to 200 ms. When one process drops
/// a `Tee` and opens the same name again (as the tests do) while
/// another thread starts a child, the child holds a copy of every open
/// file until it execs, which on Unix keeps the released lock held for
/// that moment. A real second run holds it for minutes.
fn try_lock_briefly(file: &File) -> Result<(), TryLockError> {
    for _ in 0..10 {
        match file.try_lock() {
            Err(TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other,
        }
    }
    file.try_lock()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    #[test]
    fn every_line_reaches_the_log_and_a_new_run_starts_it_afresh() {
        let dir = test_repo::empty_dir();
        let tee = Tee::open(&dir.0, "run").unwrap();
        assert_eq!(tee.path(), Some(dir.0.join("target/run.log").as_path()));
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
        let mut bad = vec!["", "a/b", ".", "..", "/abs"];
        if cfg!(windows) {
            // A drive prefix would replace `target/` in the join.
            bad.extend(["a\\b", "C:x"]);
        }
        for name in bad {
            let e = Tee::open(&dir.0, name).unwrap_err().to_string();
            assert!(e.contains("bare stem"), "{name:?}: {e}");
        }
        // Dots inside a stem are not a parent directory.
        assert!(Tee::open(&dir.0, "x..y").is_ok());
    }

    #[test]
    fn a_log_another_run_holds_is_refused_untouched() {
        let dir = test_repo::empty_dir();
        let first = Tee::open(&dir.0, "run").unwrap();
        first.record("first run");
        let e = Tee::open(&dir.0, "run").unwrap_err();
        assert_eq!(e, OpenError::Held(dir.0.join("target/run.log")));
        assert!(e.to_string().contains("another run"), "{e}");
        drop(first);
        let log = fs::read_to_string(dir.0.join("target/run.log")).unwrap();
        assert_eq!(log, "first run\n", "the refused open truncated it");
        assert!(Tee::open(&dir.0, "run").is_ok(), "free once dropped");
    }

    #[test]
    fn a_terminal_tee_writes_no_file() {
        let tee = Tee::terminal();
        assert_eq!(tee.path(), None);
        tee.println("shown");
        tee.record("dropped");
        assert!(!tee.write_failed.get());
    }

    #[test]
    fn a_failed_write_is_remembered_and_never_fails_the_caller() {
        let dir = test_repo::empty_dir();
        let path = dir.0.join("ro.log");
        fs::write(&path, "").unwrap();
        // Opened for reading, so every write fails.
        let tee = Tee::from_file(path.clone(), File::open(&path).unwrap());
        assert!(!tee.write_failed.get());
        tee.record("a");
        tee.record("b");
        assert!(tee.write_failed.get());
    }

    #[test]
    fn a_log_that_cannot_be_created_is_an_error() {
        let dir = test_repo::empty_dir();
        fs::write(dir.0.join("target"), "a file").unwrap();
        assert!(
            Tee::open(&dir.0, "run")
                .unwrap_err()
                .to_string()
                .contains("create")
        );
    }
}
