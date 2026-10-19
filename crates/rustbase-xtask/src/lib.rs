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

use std::path::PathBuf;

pub mod audit;
pub mod coverage;
pub mod deny;
pub mod dep_age;
pub mod dupes;
mod process;

/// The project's `xtask` package, left out of the coverage and
/// duplication measurements: a binary of glue around this library.
/// The name is fixed, not configurable. This library is tooling
/// too, but it is measured: its logic is what the gates run.
const TOOLING_CRATE: &str = "xtask";

/// What a gate does when it cannot finish (a tool missing, the
/// network down), as opposed to finishing and finding a problem,
/// which always fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// What differs between projects using this library.
#[derive(Clone, Debug)]
pub struct Config {
    /// The workspace root: where `Cargo.lock` lives and where
    /// `git` and `cargo` run.
    pub root: PathBuf,
    /// Name of the environment variable listing
    /// `name@version` dependencies allowed through the cooldown.
    pub dep_age_allow_env: String,
    /// User-Agent sent to package registries (crates.io refuses
    /// requests without one).
    pub user_agent: String,
    /// The coverage and duplication limits.
    pub thresholds: Thresholds,
}

impl Config {
    /// The defaults for a project called `project`:
    /// `<PROJECT>_DEP_AGE_ALLOW`, `<project>-xtask-dep-age` and
    /// the default [`Thresholds`].
    pub fn for_project(project: &str, root: PathBuf) -> Self {
        let prefix = project.to_ascii_uppercase().replace('-', "_");
        Self {
            root,
            dep_age_allow_env: format!("{prefix}_DEP_AGE_ALLOW"),
            user_agent: format!("{project}-xtask-dep-age"),
            thresholds: Thresholds::default(),
        }
    }
}

/// Percentages the coverage and duplication gates hold the
/// workspace to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Minimum line coverage of the workspace as a whole.
    pub coverage_overall: f64,
    /// Minimum line coverage of any one file, so a single
    /// uncovered file is not averaged away.
    pub coverage_file: f64,
    /// Maximum exact duplication pooled across crates: duplicate
    /// lines over total lines, each crate measured alone. A crate of
    /// 1000 lines with 10 duplicated and one of 50 with 5 pool to
    /// 15/1050 = 1.4%. Copies between crates are not measured.
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

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git(dir: &Path, args: &[&str]) {
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
        assert_eq!(cfg.user_agent, "my-app-xtask-dep-age");
        assert_eq!(cfg.root, PathBuf::from("/w"));
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
