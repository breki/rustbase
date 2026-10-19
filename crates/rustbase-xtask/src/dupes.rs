//! The code-duplication gate.
//!
//! Runs `code-dupes` over each workspace crate's `src/` (tests and
//! the `xtask` package left out) and fails when the duplication
//! found, pooled across crates (duplicate lines over total lines),
//! exceeds
//! [`Thresholds::dupes_workspace`](crate::Thresholds), or in any
//! one crate exceeds [`Thresholds::dupes_crate`](crate::Thresholds).
//! Each run sees only its own crate, so code copied from one crate
//! into another is not measured.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::process::{self, Captured};
use crate::{Config, TOOLING_CRATE, Thresholds};

/// A workspace member to measure.
#[derive(Debug, PartialEq)]
struct CrateSrc {
    name: String,
    /// Its `src/` directory.
    src: PathBuf,
}

/// One crate's duplication figures.
#[derive(Debug, PartialEq)]
struct CrateStats {
    krate: CrateSrc,
    total_lines: u64,
    exact_lines: u64,
}

impl CrateStats {
    fn pct(&self) -> f64 {
        percent(self.exact_lines, self.total_lines)
    }
}

/// Runs the gate in the workspace root; on success, the detail
/// line.
///
/// # Errors
///
/// When a limit is exceeded (naming the crates and how to list
/// their duplicates), when nothing was counted, or when `cargo
/// metadata` or `code-dupes` cannot run or be read.
pub fn check(cfg: &Config) -> Result<String, String> {
    collect(
        cfg,
        |args| process::cargo(&cfg.root, args),
        // `code-dupes -p <src> --exclude-tests --format json stats`,
        // captured through a pipe. An older note said code-dupes
        // missed sources when piped; `stats` reports the same counts
        // piped as on a terminal (checked with 0.2.1).
        |src| {
            let args = ["-p", "--exclude-tests", "--format", "json", "stats"]
                .map(OsStr::new);
            let mut all = vec![args[0], src.as_os_str()];
            all.extend(&args[1..]);
            process::tool(&cfg.root, "code-dupes", &all)
        },
    )
}

/// [`check`] with the `cargo` and `code-dupes` spawns injected.
fn collect(
    cfg: &Config,
    cargo: impl FnOnce(&[&str]) -> Result<Captured, String>,
    mut dupes: impl FnMut(&Path) -> Result<Captured, String>,
) -> Result<String, String> {
    let meta = cargo(&["metadata", "--format-version", "1", "--no-deps"])?;
    if !meta.ok {
        return Err(format!("cargo metadata failed:\n{}", meta.stderr));
    }
    let mut stats = Vec::new();
    for krate in crate_src_dirs(&meta.stdout)? {
        let out = dupes(&krate.src).map_err(|e| {
            format!("{e}\n  Install with: cargo install code-dupes")
        })?;
        if !out.ok {
            return Err(format!(
                "code-dupes failed for {} ({}):\n{}",
                krate.name,
                krate.src.display(),
                out.stderr
            ));
        }
        stats.push(read_stats(krate, &out.stdout)?);
    }
    judge(&stats, &cfg.thresholds)
}

/// Each workspace member with a `src/` directory, from `cargo
/// metadata` JSON, the `xtask` package left out. Finding none at all
/// is an error, not a pass.
fn crate_src_dirs(metadata: &str) -> Result<Vec<CrateSrc>, String> {
    let meta: Value = serde_json::from_str(metadata)
        .map_err(|e| format!("failed to parse cargo metadata: {e}"))?;
    let mut dirs = Vec::new();
    for pkg in meta["packages"].as_array().into_iter().flatten() {
        let name = pkg["name"].as_str().unwrap_or("");
        if name == TOOLING_CRATE {
            continue;
        }
        let manifest = Path::new(pkg["manifest_path"].as_str().unwrap_or(""));
        if let Some(src) = manifest.parent().map(|d| d.join("src"))
            && src.is_dir()
        {
            dirs.push(CrateSrc {
                name: name.to_owned(),
                src,
            });
        }
    }
    if dirs.is_empty() {
        return Err(format!(
            "no src/ directories found in workspace at {}",
            meta["workspace_root"].as_str().unwrap_or("?")
        ));
    }
    Ok(dirs)
}

/// Read `code-dupes --format json stats` output for one crate.
fn read_stats(krate: CrateSrc, stdout: &str) -> Result<CrateStats, String> {
    let name = &krate.name;
    let j: Value = serde_json::from_str(stdout).map_err(|e| {
        format!("failed to parse code-dupes JSON for {name}: {e}")
    })?;
    let field = |key: &str| {
        j[key].as_u64().ok_or_else(|| {
            format!("missing {key} in code-dupes output for {name}")
        })
    };
    Ok(CrateStats {
        total_lines: field("total_lines")?,
        exact_lines: field("exact_duplicate_lines")?,
        krate,
    })
}

/// Hold the figures to both limits.
fn judge(stats: &[CrateStats], t: &Thresholds) -> Result<String, String> {
    let total: u64 = stats.iter().map(|s| s.total_lines).sum();
    let exact: u64 = stats.iter().map(|s| s.exact_lines).sum();
    if total == 0 {
        return Err(format!(
            "code-dupes counted 0 lines across {} crates, so the gate is \
             vacuous",
            stats.len()
        ));
    }
    let workspace = percent(exact, total);
    let hot: Vec<&CrateStats> =
        stats.iter().filter(|s| s.pct() > t.dupes_crate).collect();

    let mut reasons = Vec::new();
    if workspace > t.dupes_workspace {
        reasons.push(format!(
            "all crates at {workspace:.1}% > {:.1}%",
            t.dupes_workspace
        ));
    }
    if !hot.is_empty() {
        let names: Vec<String> = hot
            .iter()
            .map(|s| format!("{} at {:.1}%", s.krate.name, s.pct()))
            .collect();
        reasons.push(format!(
            "over {:.1}% per crate: {}",
            t.dupes_crate,
            names.join(", ")
        ));
    }
    if reasons.is_empty() {
        return Ok(format!(
            "all crates {workspace:.1}% <= {:.1}%, each crate <= {:.1}%",
            t.dupes_workspace, t.dupes_crate
        ));
    }
    // Every crate when the total is over, else only the hot ones.
    let listed: Vec<&CrateStats> = if workspace > t.dupes_workspace {
        stats.iter().collect()
    } else {
        hot
    };
    let mut msg = format!("duplication exceeded: {}", reasons.join("; "));
    msg.push_str("\n  list the duplicates with:");
    for s in listed {
        let _ = write!(
            msg,
            "\n    code-dupes -p \"{}\" --exclude-tests report",
            s.krate.src.display()
        );
    }
    Err(msg)
}

/// `part` as a percentage of `whole`; zero for an empty whole.
#[allow(clippy::cast_precision_loss)] // line counts, far below 2^52
fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::test_repo;

    fn krate(name: &str) -> CrateSrc {
        CrateSrc {
            name: name.into(),
            src: PathBuf::from(format!("/w/{name}/src")),
        }
    }

    fn stats(name: &str, total_lines: u64, exact_lines: u64) -> CrateStats {
        CrateStats {
            krate: krate(name),
            total_lines,
            exact_lines,
        }
    }

    fn t() -> Thresholds {
        Thresholds::default()
    }

    #[test]
    fn a_clean_workspace_passes_with_both_limits_named() {
        let d = judge(&[stats("a", 1000, 4), stats("b", 13, 0)], &t()).unwrap();
        assert_eq!(d, "all crates 0.4% <= 6.0%, each crate <= 12.0%");
    }

    #[test]
    fn a_small_crate_over_the_workspace_limit_passes_under_its_own() {
        // 10% in a tiny crate is diluted by the big one, and under
        // the per-crate limit.
        assert!(
            judge(&[stats("big", 1000, 10), stats("tiny", 50, 5)], &t())
                .is_ok()
        );
    }

    #[test]
    fn a_hotspot_fails_even_when_the_workspace_total_holds() {
        let e = judge(&[stats("big", 1000, 10), stats("hot", 100, 15)], &t())
            .unwrap_err();
        assert!(e.contains("over 12.0% per crate: hot at 15.0%"), "{e}");
        assert!(
            e.contains("code-dupes -p \"/w/hot/src\" --exclude-tests report"),
            "{e}"
        );
        assert!(!e.contains("/w/big/src"), "only the hot crate: {e}");
        assert!(!e.contains("all crates at"), "{e}");
    }

    #[test]
    fn the_workspace_total_fails_even_when_no_crate_is_hot() {
        let e = judge(&[stats("a", 100, 10), stats("b", 100, 10)], &t())
            .unwrap_err();
        assert!(e.contains("all crates at 10.0% > 6.0%"), "{e}");
        assert!(e.contains("/w/a/src") && e.contains("/w/b/src"), "{e}");
    }

    #[test]
    fn limits_come_from_config() {
        let strict = Thresholds {
            dupes_workspace: 0.1,
            ..t()
        };
        assert!(judge(&[stats("a", 1000, 4)], &strict).is_err());
    }

    #[test]
    fn an_empty_crate_beside_a_measured_one_counts_as_zero() {
        assert_eq!(stats("a", 0, 0).pct(), 0.0);
        assert!(judge(&[stats("a", 0, 0), stats("b", 100, 0)], &t()).is_ok());
    }

    #[test]
    fn a_run_that_counted_no_lines_fails() {
        let e = judge(&[stats("a", 0, 0), stats("b", 0, 0)], &t()).unwrap_err();
        assert!(e.contains("vacuous"), "{e}");
    }

    #[test]
    fn read_stats_takes_the_line_counts() {
        let out = r#"{"total_lines": 1077, "exact_duplicate_lines": 4,
                      "exact_duplicate_percent": 0.37}"#;
        let s = read_stats(krate("a"), out).unwrap();
        assert_eq!((s.total_lines, s.exact_lines), (1077, 4));
    }

    #[test]
    fn read_stats_rejects_garbage_and_missing_fields() {
        let e = read_stats(krate("a"), "nope").unwrap_err();
        assert!(e.contains("parse code-dupes JSON for a"), "{e}");
        let e = read_stats(krate("a"), "{}").unwrap_err();
        assert!(e.contains("missing total_lines"), "{e}");
    }

    /// A workspace with crates `a` (with `src/`), `nosrc` (without)
    /// and the tooling crate, and its `cargo metadata` JSON.
    fn workspace() -> (test_repo::TempRepo, String) {
        let dir = test_repo::empty_dir();
        for c in ["a", "xtask"] {
            fs::create_dir_all(dir.0.join(c).join("src")).unwrap();
        }
        let manifest = |c: &str| {
            dir.0
                .join(c)
                .join("Cargo.toml")
                .to_string_lossy()
                .into_owned()
        };
        let meta = json!({"workspace_root": dir.0, "packages": [
            {"name": "a", "manifest_path": manifest("a")},
            {"name": "nosrc", "manifest_path": manifest("nosrc")},
            {"name": "xtask", "manifest_path": manifest("xtask")},
        ]})
        .to_string();
        (dir, meta)
    }

    #[test]
    fn crate_src_dirs_skips_the_tooling_crate_and_missing_src() {
        let (dir, meta) = workspace();
        let got = crate_src_dirs(&meta).unwrap();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].name, "a");
        assert!(got[0].src.starts_with(&dir.0));
    }

    #[test]
    fn crate_src_dirs_with_nothing_to_measure_is_an_error() {
        let e = crate_src_dirs(r#"{"workspace_root": "/w", "packages": []}"#)
            .unwrap_err();
        assert!(e.contains("no src/ directories"), "{e}");
        assert!(crate_src_dirs("nope").is_err());
    }

    fn captured(ok: bool, stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    fn cfg() -> Config {
        Config::for_project("p", "/w".into())
    }

    #[test]
    fn collect_measures_each_crate_and_judges_the_total() {
        let (_dir, meta) = workspace();
        let seen = RefCell::new(Vec::new());
        let d = collect(
            &cfg(),
            |args| {
                assert_eq!(args[0], "metadata");
                Ok(captured(true, &meta, ""))
            },
            |src| {
                seen.borrow_mut().push(src.to_path_buf());
                Ok(captured(
                    true,
                    r#"{"total_lines": 100, "exact_duplicate_lines": 1}"#,
                    "",
                ))
            },
        )
        .unwrap();
        assert!(d.starts_with("all crates 1.0%"), "{d}");
        assert_eq!(seen.into_inner().len(), 1);
    }

    #[test]
    fn collect_reports_each_way_a_tool_can_fail() {
        let (_dir, meta) = workspace();
        let e = collect(
            &cfg(),
            |_| Ok(captured(false, "", "bad toml")),
            |_| unreachable!(),
        )
        .unwrap_err();
        assert!(e.contains("cargo metadata failed") && e.contains("bad toml"));

        let e = collect(
            &cfg(),
            |_| Ok(captured(true, &meta, "")),
            |_| Err("failed to run code-dupes: not found".into()),
        )
        .unwrap_err();
        assert!(e.contains("cargo install code-dupes"), "{e}");

        let e = collect(
            &cfg(),
            |_| Ok(captured(true, &meta, "")),
            |_| Ok(captured(false, "", "parse error")),
        )
        .unwrap_err();
        assert!(e.contains("code-dupes failed for a"), "{e}");
        assert!(e.contains("parse error"), "{e}");
    }
}
