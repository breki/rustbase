//! Shared `cargo xtask` tooling for projects derived from the
//! rustbase template.
//!
//! A project's own `xtask` crate depends on this library and
//! keeps only its CLI and project-specific commands; the
//! gates themselves are versioned here, so a fix reaches every
//! project as a dependency bump rather than a hand merge.
//!
//! Each gate takes a [`Config`] carrying what differs between
//! projects.
//!
//! The `markdown` and `text` modules are public only for the
//! template's own commands; they are hidden from the docs, and
//! semantic versioning does not cover them: they may change or go
//! away in any release. Most public functions return errors as text
//! (`Result<_, String>`); the gate entry points return `Failure`.

use std::fmt::Write as _;
use std::path::PathBuf;

pub mod audit;
pub mod canon;
pub mod cargo_test;
pub mod changelog;
pub mod check;
#[cfg(feature = "clap")]
pub mod cli;
pub mod clippy;
pub mod coverage;
pub mod deny;
pub mod dep_age;
pub mod doc;
pub mod dupes;
pub mod fmt;
pub mod licenses;
#[doc(hidden)]
pub mod markdown;
mod process;
pub mod records;
pub mod scripts;
pub mod step;
pub mod tag_signature;
pub mod tee;
#[doc(hidden)]
pub mod text;
pub mod todo;
pub mod version_sync;

/// The project's `xtask` package, left out of the coverage and
/// duplication measurements: a binary of glue around this library.
/// The name is fixed, not configurable. This library is tooling
/// too, but it is measured: its logic is what the gates run.
const TOOLING_CRATE: &str = "xtask";

/// Output lines a failed run shows when its gate found no
/// diagnostic in it.
const RUN_TAIL_LINES: usize = 10;

/// Report lines a command run on its own shows. A `validate` step
/// shows `STEP_LINES`, fewer because it sits among the other steps.
#[cfg(feature = "clap")]
const STANDALONE_LINES: usize = 10;

/// Report lines a `validate` step shows.
const STEP_LINES: usize = 5;

/// Diff lines `fmt` shows, run on its own or as a step.
const FMT_DIFF_LINES: usize = 20;

/// Why a gate failed: a one-line summary and the diagnostic lines
/// behind it (empty when there are none, as when a tool could not
/// be run at all).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// One line naming what failed, e.g. `3 compilation error(s)`.
    pub summary: String,
    /// The diagnostics, each with its source location when known.
    pub items: Vec<String>,
}

impl Failure {
    /// A failure with diagnostics.
    pub fn new(
        summary: impl Into<String>,
        items: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            summary: summary.into(),
            items: items.into_iter().map(Into::into).collect(),
        }
    }

    /// A failed tool run, reported by `found` -- the diagnostics the
    /// calling gate pulled out of the output, such as the lines
    /// `check::extract_error_lines` keeps -- or, when it found none,
    /// by the tail of the run's stderr, then stdout, then just its
    /// exit status. A failure never reports nothing, whatever the tool
    /// printed.
    pub(crate) fn of_run(
        summary: impl Into<String>,
        found: impl IntoIterator<Item = impl Into<String>>,
        out: &process::Captured,
    ) -> Self {
        let mut f = Self::new(summary, found);
        if f.items.is_empty() {
            let mut items = text::stderr_tail(&out.stderr, RUN_TAIL_LINES);
            if items.is_empty() {
                items = text::stderr_tail(&out.stdout, RUN_TAIL_LINES);
            }
            f.items = items.into_iter().map(String::from).collect();
        }
        if f.items.is_empty() {
            f.items.push(format!("{}, and no output", out.status));
        }
        f
    }

    /// The summary, then at most `max` items indented beneath it and
    /// a count of the rest: the whole report in one string, so the
    /// caller prints it once.
    pub fn report(&self, max: usize) -> String {
        let mut out = self.summary.clone();
        for item in self.items.iter().take(max) {
            out.push_str("\n  ");
            out.push_str(item);
        }
        if self.items.len() > max {
            let _ = write!(out, "\n  ... and {} more", self.items.len() - max);
        }
        out
    }
}

impl From<String> for Failure {
    /// A failure with no diagnostics, such as a tool that could not
    /// run.
    fn from(summary: String) -> Self {
        Self::new(summary, Vec::<String>::new())
    }
}

/// The whole report, every item; [`Failure::report`] caps it.
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.report(usize::MAX))
    }
}

impl std::error::Error for Failure {}

/// What a gate does when it cannot finish (a tool missing, the
/// network down), as opposed to finishing and finding a problem,
/// which always fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Strictness {
    /// Fail: the caller wants to know the check never ran.
    Fail,
    /// Warn and pass, so an offline run is not blocked.
    Warn,
}

/// Whether `stderr` (or any of cargo's captured output) is cargo
/// reporting an absent subcommand, i.e. the backing tool is not
/// installed. Only cargo's two actual
/// spellings: a looser phrase like "not installed" would match a
/// real gate failure that happens to say it.
fn is_missing_subcommand(stderr: &str) -> bool {
    stderr.contains("no such command") || stderr.contains("no such subcommand")
}

/// What differs between projects using this library. Its paths are
/// relative to `root` (an absolute one is taken as it is).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Config {
    /// The project's name, as the third-party licence file's header
    /// gives it.
    pub name: String,
    /// The workspace root: where `Cargo.lock` lives and where
    /// `git` and `cargo` run.
    pub root: PathBuf,
    /// Name of the environment variable listing comma-separated
    /// `name@version` dependencies allowed through the cooldown.
    /// `for_project("hoard", ...)` names it `HOARD_DEP_AGE_ALLOW`.
    pub dep_age_allow_env: String,
    /// Name of the environment variable that, set to `1`, makes the
    /// cooldown step fail when it cannot finish instead of warning
    /// (as `CI=true` does).
    pub dep_age_strict_env: String,
    /// User-Agent sent to package registries (crates.io refuses
    /// requests without one).
    pub user_agent: String,
    /// The coverage and duplication limits.
    pub thresholds: Thresholds,
    /// The canon check's exemptions and limits.
    pub canon: canon::Rules,
    /// The record files the records check reads, relative to
    /// `root`.
    pub record_files: Vec<String>,
    /// The npm project the audit step also audits, when it holds a
    /// `package.json`; `None` audits Rust only.
    pub audit_npm_dir: Option<PathBuf>,
    /// The manifest of the package whose version, as cargo resolves it
    /// (inherited from the workspace or not), is the project's; the
    /// one `version_sync` copies into the docs.
    pub version_manifest: PathBuf,
}

impl Config {
    /// `audit_npm_dir` resolved against `root`.
    pub(crate) fn npm_dir(&self) -> Option<PathBuf> {
        self.audit_npm_dir.as_ref().map(|d| self.root.join(d))
    }

    /// The defaults for a project called `project`: `project` as
    /// `name`, `<PROJECT>_DEP_AGE_ALLOW` and `<PROJECT>_DEP_AGE_STRICT`,
    /// `<project>-xtask-dep-age`, the default [`Thresholds`], canon
    /// [`Rules`](canon::Rules) and [record files](records::DEFAULT_FILES),
    /// no npm project to audit, and `crates/<project>/Cargo.toml` as
    /// the version manifest.
    pub fn for_project(project: &str, root: PathBuf) -> Self {
        let prefix = project.to_ascii_uppercase().replace('-', "_");
        Self {
            name: project.to_owned(),
            root,
            dep_age_allow_env: format!("{prefix}_DEP_AGE_ALLOW"),
            dep_age_strict_env: format!("{prefix}_DEP_AGE_STRICT"),
            user_agent: format!("{project}-xtask-dep-age"),
            thresholds: Thresholds::default(),
            canon: canon::Rules::default(),
            record_files: records::DEFAULT_FILES
                .iter()
                .map(|f| (*f).to_owned())
                .collect(),
            audit_npm_dir: None,
            version_manifest: PathBuf::from(format!(
                "crates/{project}/Cargo.toml"
            )),
        }
    }
}

/// Percentages the coverage and duplication gates hold the
/// workspace to.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Thresholds {
    /// Minimum line coverage of the workspace as a whole.
    pub coverage_overall: f64,
    /// Minimum line coverage of any one file, so a single
    /// uncovered file is not averaged away.
    pub coverage_file: f64,
    /// Maximum exact duplication pooled across crates: duplicate
    /// lines over total lines, each crate measured alone. A crate of
    /// 1000 lines with 10 duplicated and one of 50 with 5 pool to
    /// 15/1050 = 1.4%. Copies between crates are not measured, by
    /// design.
    pub dupes_workspace: f64,
    /// Maximum exact duplication in any one crate. Wider than
    /// the workspace limit: one copied block is a large share of
    /// a small crate.
    pub dupes_crate: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            coverage_overall: 90.0,
            coverage_file: 85.0,
            dupes_workspace: 6.0,
            dupes_crate: 12.0,
        }
    }
}

/// A throwaway git repository for tests that exercise the real
/// `git` calls without touching the network.
#[cfg(test)]
pub(crate) mod test_repo {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A temp directory removed when dropped.
    pub(crate) struct TempRepo(pub PathBuf);

    /// An empty temp directory, not a repository.
    pub(crate) fn empty_dir() -> TempRepo {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rustbase-xtask-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempRepo(dir)
    }

    /// An empty temp directory with no `Cargo.toml` or `.git` in any
    /// directory above it, so a cargo command run there finds no
    /// workspace and fails fast, and it lies in no git work tree.
    /// Panics, naming the entry, when that does not hold.
    pub(crate) fn outside_any_workspace() -> TempRepo {
        let dir = empty_dir();
        // A `.git` above would put the dir in a work tree, which the
        // scripts gate reads instead of skipping.
        if let Some(m) = dir
            .0
            .ancestors()
            .flat_map(|a| [a.join("Cargo.toml"), a.join(".git")])
            .find(|m| m.exists())
        {
            panic!(
                "{} lies above the temp dir, so cargo or git would find \
                 a project there; point TMPDIR elsewhere",
                m.display()
            );
        }
        dir
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .is_ok_and(|o| o.status.success());
        assert!(ok, "git {args:?} failed in {}", dir.display());
    }

    /// A repository whose first commit holds `Cargo.lock` with
    /// `lock` as its text; `commit = false` leaves it untracked.
    pub(crate) fn with_lock(lock: &str, commit: bool) -> TempRepo {
        let repo = empty_dir();
        git(&repo.0, &["init", "-q"]);
        std::fs::write(repo.0.join("Cargo.lock"), lock).expect("write lock");
        if commit {
            git(&repo.0, &["add", "Cargo.lock"]);
            git(&repo.0, &["commit", "-q", "-m", "lock"]);
        }
        repo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_defaults_derive_the_env_var_and_user_agent() {
        let cfg = Config::for_project("my-app", PathBuf::from("/w"));
        assert_eq!(cfg.dep_age_allow_env, "MY_APP_DEP_AGE_ALLOW");
        assert_eq!(cfg.dep_age_strict_env, "MY_APP_DEP_AGE_STRICT");
        assert_eq!(cfg.audit_npm_dir, None);
        let mut npm = Config::for_project("p", PathBuf::from("/w"));
        npm.audit_npm_dir = Some("frontend".into());
        assert_eq!(npm.npm_dir(), Some(PathBuf::from("/w/frontend")));
        assert_eq!(
            cfg.version_manifest,
            PathBuf::from("crates/my-app/Cargo.toml")
        );
        assert_eq!(cfg.user_agent, "my-app-xtask-dep-age");
        assert_eq!(cfg.root, PathBuf::from("/w"));
        assert_eq!(cfg.name, "my-app");
    }

    #[test]
    fn a_failed_run_never_reports_nothing() {
        let run = |stdout: &str, stderr: &str| process::Captured {
            ok: false,
            status: "exit status: 101".into(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        };
        let none: [&str; 0] = [];
        let found = Failure::of_run("s", ["a diagnostic"], &run("", "noise"));
        assert_eq!(found.items, ["a diagnostic"]);
        let tail = Failure::of_run("s", none, &run("out", "err line"));
        assert_eq!(tail.items, ["err line"]);
        let out = Failure::of_run("s", none, &run("only stdout", " "));
        assert_eq!(out.items, ["only stdout"]);
        let bare = Failure::of_run("s", none, &run("", ""));
        assert_eq!(bare.items, ["exit status: 101, and no output"]);
    }

    #[test]
    fn a_failure_reports_its_summary_and_the_first_items_once() {
        let f = Failure::new("2 error(s)", ["a", "b"]);
        assert_eq!(f.report(5), "2 error(s)\n  a\n  b");
        assert_eq!(f.report(1), "2 error(s)\n  a\n  ... and 1 more");
        assert_eq!(f.to_string(), f.report(usize::MAX));
        assert_eq!(Failure::from("no cargo".to_string()).report(5), "no cargo");
        // Usable behind `?` into a boxed error, as `anyhow` needs.
        let boxed: Box<dyn std::error::Error> = Box::new(f);
        assert!(boxed.to_string().starts_with("2 error(s)"));
    }

    #[test]
    fn is_missing_subcommand_matches_cargo_wordings() {
        assert!(is_missing_subcommand("error: no such command: `deny`"));
        assert!(is_missing_subcommand("error: no such subcommand: `deny`"));
        // A real gate failure must not read as a missing tool, even
        // when it says "not installed".
        assert!(!is_missing_subcommand(
            "error[L001]: failed to satisfy license; some dep is not installed"
        ));
        assert!(!is_missing_subcommand(""));
    }
}
