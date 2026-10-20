//! The executable-scripts gate: every file under `scripts/` that
//! opens with a `#!` line must be committed executable (git mode
//! `100755`), since the docs run them by bare path, and one committed
//! `100644` exits 126 without running.
//!
//! The git index is the subject, not the working tree's permission
//! bits: the index is what other clones get, and it means the same on
//! Windows. A script is found by its shebang, not a `.sh` suffix, so
//! an extensionless one is covered; a file with no shebang (a sourced
//! library) is exempt.

use std::ffi::OsStr;
use std::path::Path;

use crate::Config;
use crate::process::{self, Captured};

/// The directory the gate reads, relative to the root.
const SCRIPTS_DIR: &str = "scripts";

/// One `git ls-files --stage` record.
#[derive(Debug, PartialEq, Eq)]
struct StageEntry<'a> {
    /// `100644`, `100755`, `120000` (symlink), `160000` (gitlink).
    mode: &'a str,
    /// `0` outside a conflict; a conflicted path appears once per
    /// side, as stages 1, 2 and 3.
    stage: &'a str,
    path: &'a str,
}

/// Runs the gate in `cfg.root`; on success, the detail line.
///
/// # Errors
///
/// When a script with a shebang is not committed executable, or when
/// git or a tracked file cannot be read.
pub fn check(cfg: &Config) -> Result<String, String> {
    if !cfg.root.join(".git").exists() {
        // A source tarball has no index to read.
        return Ok("skipped: not a git work tree".into());
    }
    collect(&cfg.root, || {
        // Raw, NUL-separated paths: see `parse_stage_entries`.
        let args = [
            "-c",
            "core.quotePath=false",
            "ls-files",
            "--stage",
            "-z",
            "--",
            SCRIPTS_DIR,
        ]
        .map(OsStr::new);
        process::tool(&cfg.root, "git", &args)
    })
}

/// [`check`] with the `git ls-files` spawn injected.
fn collect(
    root: &Path,
    ls_files: impl FnOnce() -> Result<Captured, String>,
) -> Result<String, String> {
    let out = ls_files()?;
    if !out.ok {
        return Err(format!("git ls-files failed:\n{}", out.stderr.trim()));
    }
    let mut scripts = 0;
    let mut offenders = Vec::new();
    for e in parse_stage_entries(&out.stdout)? {
        // Stage 0 only: mid-merge each side has its own mode. Regular
        // files only: a symlink cannot be made 100755.
        if e.stage != "0" || !matches!(e.mode, "100644" | "100755") {
            continue;
        }
        // Bytes: a non-UTF-8 script must not become an exemption.
        let head = std::fs::read(root.join(e.path))
            .map_err(|err| format!("cannot read tracked {}: {err}", e.path))?;
        if !head.starts_with(b"#!") {
            continue;
        }
        scripts += 1;
        if e.mode != "100755" {
            offenders.push(format!("{} ({})", e.path, e.mode));
        }
    }
    if offenders.is_empty() {
        return Ok(format!("{scripts} scripts executable"));
    }
    Err(format!(
        "these scripts have a shebang but are not committed executable, \
         so running them by bare path exits 126:\n  {}\n  fix: git \
         update-index --chmod=+x <path>",
        offenders.join("\n  ")
    ))
}

/// The records of `git ls-files --stage -z`: `<mode> <sha>
/// <stage>\t<path>`, NUL-separated. `-z` matters: without it git
/// quotes a path holding non-ASCII bytes, and the quotes would end up
/// in the path. A malformed record is an error, never skipped.
fn parse_stage_entries(listing: &str) -> Result<Vec<StageEntry<'_>>, String> {
    let mut out = Vec::new();
    for record in listing.split('\0').filter(|r| !r.is_empty()) {
        let Some((meta, path)) = record.split_once('\t') else {
            return Err(format!("no tab in ls-files record: {record:?}"));
        };
        let mut fields = meta.split_whitespace();
        let (Some(mode), Some(_sha), Some(stage), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("malformed ls-files metadata: {meta:?}"));
        };
        out.push(StageEntry { mode, stage, path });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_repo;

    fn listing(out: &str) -> Captured {
        Captured {
            ok: true,
            status: String::new(),
            stdout: out.into(),
            stderr: String::new(),
        }
    }

    /// A temp dir holding `scripts/<name>` with `body` for each pair.
    fn tree(files: &[(&str, &str)]) -> test_repo::TempRepo {
        let dir = test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("scripts/lib")).unwrap();
        for (name, body) in files {
            fs::write(dir.0.join("scripts").join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn a_shebang_script_committed_without_the_bit_fails() {
        let dir =
            tree(&[("a.sh", "#!/bin/sh\n"), ("b", "#!/usr/bin/env bash\n")]);
        let ls = "100644 x 0\tscripts/a.sh\x00100755 y 0\tscripts/b\0";
        let e = collect(&dir.0, || Ok(listing(ls))).unwrap_err();
        assert!(e.contains("scripts/a.sh (100644)"), "{e}");
        assert!(!e.contains("scripts/b"), "{e}");
        assert!(e.contains("git update-index --chmod=+x"), "{e}");
    }

    #[test]
    fn sourced_files_symlinks_and_conflict_sides_are_exempt() {
        let dir =
            tree(&[("lib/util.sh", "f() { :; }\n"), ("ok.sh", "#!/bin/sh\n")]);
        let ls = "100644 a 0\tscripts/lib/util.sh\x00\
                  120000 b 0\tscripts/link.sh\x00\
                  100644 c 1\tscripts/ok.sh\x00\
                  100755 d 0\tscripts/ok.sh\0";
        assert_eq!(
            collect(&dir.0, || Ok(listing(ls))),
            Ok("1 scripts executable".into())
        );
    }

    #[test]
    fn a_tracked_file_that_cannot_be_read_fails() {
        let dir = tree(&[]);
        let ls = "100644 a 0\tscripts/gone.sh\0";
        let e = collect(&dir.0, || Ok(listing(ls))).unwrap_err();
        assert!(e.contains("cannot read tracked scripts/gone.sh"), "{e}");
        let failed = Captured {
            ok: false,
            stderr: "fatal: oops".into(),
            ..listing("")
        };
        let e = collect(&dir.0, || Ok(failed)).unwrap_err();
        assert!(e.contains("fatal: oops"), "{e}");
    }

    #[test]
    fn check_reads_the_real_index_and_skips_outside_a_work_tree() {
        let dir = tree(&[("run.sh", "#!/bin/sh\n")]);
        let cfg = Config::for_project("p", dir.0.clone());
        assert_eq!(check(&cfg), Ok("skipped: not a git work tree".into()));
        test_repo::git(&dir.0, &["init", "-q"]);
        test_repo::git(&dir.0, &["add", "scripts/run.sh"]);
        test_repo::git(
            &dir.0,
            &["update-index", "--chmod=-x", "scripts/run.sh"],
        );
        assert!(check(&cfg).unwrap_err().contains("scripts/run.sh (100644)"));
        test_repo::git(
            &dir.0,
            &["update-index", "--chmod=+x", "scripts/run.sh"],
        );
        assert_eq!(check(&cfg), Ok("1 scripts executable".into()));
    }

    #[test]
    fn parse_stage_entries_reads_mode_stage_and_raw_paths() {
        let ls = "100755 abc 0\tscripts/two words.sh\x00100644 def 2\tscripts/café.sh\0";
        assert_eq!(
            parse_stage_entries(ls).unwrap(),
            [
                StageEntry {
                    mode: "100755",
                    stage: "0",
                    path: "scripts/two words.sh"
                },
                StageEntry {
                    mode: "100644",
                    stage: "2",
                    path: "scripts/café.sh"
                },
            ]
        );
        assert!(parse_stage_entries("100644 abc 0 no-tab\0").is_err());
        assert!(parse_stage_entries("100644 abc\tshort.sh\0").is_err());
    }
}
