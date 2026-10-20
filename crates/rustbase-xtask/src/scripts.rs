//! The executable-scripts gate: every file under `scripts/` that
//! opens with a `#!` line must be staged executable (git mode
//! `100755`), since canon runs them by bare path (`/commit` runs
//! `scripts/e2e.sh`), and the shell refuses a `100644` one with
//! "permission denied" (exit 126).
//!
//! The git index is the subject -- both a file's mode and the bytes
//! the shebang is read from -- not the working tree: the index is what
//! the next commit records, so a staged fix passes before it is
//! committed. A Windows working tree has no executable bit, so the
//! index is the one place every platform records it. A script is
//! found by its shebang, not a `.sh` suffix, so an extensionless one
//! is covered; a file with no shebang (a sourced library) is exempt.

use std::ffi::OsStr;
use std::path::Path;

use crate::Config;
use crate::process::{self, Captured};

/// The directory the gate reads, relative to the root.
const SCRIPTS_DIR: &str = "scripts";

/// One `git ls-files --stage` record.
#[derive(Debug, PartialEq, Eq)]
struct StageEntry<'a> {
    /// `100644`, `100755`, `120000` (symlink), `160000` (gitlink: a
    /// submodule).
    mode: &'a str,
    /// The blob the index holds for the path.
    sha: &'a str,
    /// `0` outside a conflict; a conflicted path has one record per
    /// side instead (1 the common base, 2 ours, 3 theirs).
    stage: &'a str,
    path: &'a str,
}

/// Runs the gate in `cfg.root`; on success, the detail line.
///
/// # Errors
///
/// When a script with a shebang is not staged executable, or when
/// git cannot list the index or read a blob from it.
pub fn check(cfg: &Config) -> Result<String, String> {
    if !in_work_tree(&cfg.root) {
        // A source tarball: nothing to read, and git may be absent.
        return Ok("skipped: not in a git work tree".into());
    }
    let git = |args: &[&str]| {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        process::tool(&cfg.root, "git", &args)
    };
    collect(
        &cfg.root,
        || {
            // Unquoted, NUL-separated paths: see `parse_stage_entries`.
            // `-z` alone already prints paths verbatim; the
            // `core.quotePath` override is belt and braces.
            git(&[
                "-c",
                "core.quotePath=false",
                "ls-files",
                "--stage",
                "-z",
                "--",
                SCRIPTS_DIR,
            ])
        },
        |sha| git(&["cat-file", "blob", sha]),
    )
}

/// Whether `root` lies in a git work tree: a `.git` entry (a directory,
/// or the file a worktree or submodule has) in it or any directory
/// above. Decided from the files, without git, so a refusal from git
/// inside a real work tree surfaces as an error instead of a skip.
fn in_work_tree(root: &Path) -> bool {
    root.ancestors().any(|d| d.join(".git").exists())
}

/// [`check`] with the `git ls-files` and `git cat-file blob <sha>`
/// spawns injected; the paths it prints are relative to `root`.
fn collect(
    root: &Path,
    ls_files: impl FnOnce() -> Result<Captured, String>,
    mut blob: impl FnMut(&str) -> Result<Captured, String>,
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
        let content = blob(e.sha)?;
        if !content.ok {
            return Err(format!(
                "git cat-file could not read {} from the index:\n{}",
                e.path,
                content.stderr.trim()
            ));
        }
        // The lossy decoding keeps an ASCII `#!` prefix intact.
        if !content.stdout.starts_with("#!") {
            continue;
        }
        scripts += 1;
        if e.mode != "100755" {
            offenders.push(format!("{} ({})", e.path, e.mode));
        }
    }
    if offenders.is_empty() {
        let noun = if scripts == 1 { "script" } else { "scripts" };
        return Ok(format!("{scripts} {noun} executable"));
    }
    Err(format!(
        "these scripts have a shebang but are not committed executable, \
         so running them by bare path exits 126 (paths relative to \
         {}):\n  {}\n  fix: chmod +x <path> && git add <path>\n  (on \
         Windows, or with core.fileMode=false: git add --chmod=+x <path>)",
        root.display(),
        offenders.join("\n  ")
    ))
}

/// The records of `git ls-files --stage -z`: `<mode> <sha>
/// <stage>\t<path>`, NUL-separated. `-z` matters: without it git
/// quotes a path holding non-ASCII bytes, and the quotes would end up
/// in the path. A path is only shown, never opened, so one that is not
/// UTF-8 merely displays with a replacement character. A malformed
/// record is an error, never skipped.
fn parse_stage_entries(listing: &str) -> Result<Vec<StageEntry<'_>>, String> {
    let mut out = Vec::new();
    for record in listing.split('\0').filter(|r| !r.is_empty()) {
        let Some((meta, path)) = record.split_once('\t') else {
            return Err(format!("no tab in ls-files record: {record:?}"));
        };
        let mut fields = meta.split_whitespace();
        let (Some(mode), Some(sha), Some(stage), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("malformed ls-files metadata: {meta:?}"));
        };
        out.push(StageEntry {
            mode,
            sha,
            stage,
            path,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_repo;

    fn captured(ok: bool, stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// A `git cat-file blob` stand-in serving `blobs` by sha.
    fn index<'a>(
        blobs: &'a [(&str, &str)],
    ) -> impl FnMut(&str) -> Result<Captured, String> + 'a {
        move |sha| {
            let found = blobs.iter().find(|(s, _)| *s == sha);
            Ok(found.map_or_else(
                || captured(false, "", "fatal: Not a valid object name"),
                |(_, body)| captured(true, body, ""),
            ))
        }
    }

    /// A temp dir holding `scripts/<name>` with `body` for each pair.
    fn tree(files: &[(&str, &str)]) -> test_repo::TempRepo {
        let dir = test_repo::outside_any_workspace();
        fs::create_dir_all(dir.0.join("scripts")).unwrap();
        for (name, body) in files {
            fs::write(dir.0.join("scripts").join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn a_shebang_script_committed_without_the_bit_fails() {
        let ls = "100644 x 0\tscripts/a.sh\x00100755 y 0\tscripts/b\0";
        let blobs = [("x", "#!/bin/sh\n"), ("y", "#!/usr/bin/env bash\n")];
        let e = collect(
            Path::new("/w"),
            || Ok(captured(true, ls, "")),
            index(&blobs),
        )
        .unwrap_err();
        assert!(e.contains("scripts/a.sh (100644)"), "{e}");
        assert!(!e.contains("scripts/b"), "{e}");
        // The printed fix sets the bit in the working tree and the
        // index: `git update-index --chmod=+x` alone leaves a Unix file
        // 0644, and the next `git add` records 100644 again.
        assert!(e.contains("chmod +x <path> && git add <path>"), "{e}");
        assert!(e.contains("git add --chmod=+x <path>"), "{e}");
    }

    #[test]
    fn a_project_inside_a_larger_repository_is_checked() {
        let outer = test_repo::outside_any_workspace();
        let root = outer.0.join("tools/proj");
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/run.sh"), "#!/bin/sh\n").unwrap();
        test_repo::git(&outer.0, &["init", "-q"]);
        test_repo::git(&outer.0, &["add", "tools"]);
        test_repo::git(
            &outer.0,
            &["update-index", "--chmod=-x", "tools/proj/scripts/run.sh"],
        );
        let e = check(&Config::for_project("p", root.clone())).unwrap_err();
        // Relative to the project root, which the message names, so the
        // printed fix works from there.
        assert!(e.contains("\n  scripts/run.sh (100644)"), "{e}");
        assert!(!e.contains("tools/proj/scripts"), "{e}");
        assert!(
            e.contains(&format!("relative to {}", root.display())),
            "{e}"
        );
    }

    #[test]
    fn the_work_tree_is_found_by_a_git_entry_at_or_above_the_root() {
        let outer = test_repo::outside_any_workspace();
        let root = outer.0.join("tools/proj");
        fs::create_dir_all(&root).unwrap();
        assert!(!in_work_tree(&root), "a bare temp dir is no work tree");
        fs::write(outer.0.join(".git"), "gitdir: elsewhere\n").unwrap();
        assert!(in_work_tree(&root));
    }

    #[test]
    fn a_work_tree_git_refuses_fails_rather_than_skipping() {
        // A broken `.git` stands for any refusal (another owner, a
        // corrupt repository): the gate must say so, not pass.
        let dir = tree(&[("run.sh", "#!/bin/sh\n")]);
        fs::write(dir.0.join(".git"), "not a gitdir line\n").unwrap();
        let e = check(&Config::for_project("p", dir.0.clone())).unwrap_err();
        assert!(e.contains("git ls-files failed"), "{e}");
    }

    #[test]
    fn sourced_files_symlinks_and_conflict_sides_are_exempt() {
        let ls = "100644 a 0\tscripts/lib/util.sh\x00\
                  120000 b 0\tscripts/link.sh\x00\
                  100644 c 1\tscripts/ok.sh\x00\
                  100755 d 0\tscripts/ok.sh\0";
        // Only the stage-0 regular files are read.
        let blobs = [("a", "f() { :; }\n"), ("d", "#!/bin/sh\n")];
        assert_eq!(
            collect(
                Path::new("/w"),
                || Ok(captured(true, ls, "")),
                index(&blobs)
            ),
            Ok("1 script executable".into())
        );
        assert_eq!(
            collect(Path::new("/w"), || Ok(captured(true, "", "")), index(&[])),
            Ok("0 scripts executable".into())
        );
    }

    #[test]
    fn a_blob_git_cannot_read_fails() {
        let ls = "100644 a 0\tscripts/x.sh\0";
        let e =
            collect(Path::new("/w"), || Ok(captured(true, ls, "")), index(&[]))
                .unwrap_err();
        assert!(e.contains("could not read scripts/x.sh"), "{e}");
        assert!(e.contains("Not a valid object name"), "{e}");
    }

    #[test]
    fn a_failed_ls_files_reports_its_stderr() {
        let e = collect(
            Path::new("/w"),
            || Ok(captured(false, "", "fatal: oops")),
            index(&[]),
        )
        .unwrap_err();
        assert!(e.contains("git ls-files failed") && e.contains("oops"));
    }

    #[test]
    fn check_reads_the_real_index_and_skips_outside_a_work_tree() {
        let dir = tree(&[("run.sh", "#!/bin/sh\n")]);
        let cfg = Config::for_project("p", dir.0.clone());
        assert_eq!(check(&cfg), Ok("skipped: not in a git work tree".into()));
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
        assert_eq!(check(&cfg), Ok("1 script executable".into()));
    }

    #[test]
    fn the_shebang_is_read_from_the_index_not_the_working_tree() {
        let dir = tree(&[("gone.sh", "#!/bin/sh\n"), ("plain.sh", "echo\n")]);
        let cfg = Config::for_project("p", dir.0.clone());
        test_repo::git(&dir.0, &["init", "-q"]);
        test_repo::git(&dir.0, &["add", "scripts"]);
        test_repo::git(
            &dir.0,
            &["update-index", "--chmod=-x", "scripts/gone.sh"],
        );
        // Deleted but not `git rm`-ed: the index still holds it.
        fs::remove_file(dir.0.join("scripts/gone.sh")).unwrap();
        // A shebang added but not staged is not what the next commit
        // records.
        fs::write(dir.0.join("scripts/plain.sh"), "#!/bin/sh\n").unwrap();
        let e = check(&cfg).unwrap_err();
        assert!(e.contains("scripts/gone.sh (100644)"), "{e}");
        assert!(!e.contains("plain.sh"), "{e}");
    }

    #[test]
    fn parse_stage_entries_reads_mode_sha_stage_and_raw_paths() {
        let ls = "100755 abc 0\tscripts/two words.sh\x00100644 def 2\tscripts/café.sh\0";
        assert_eq!(
            parse_stage_entries(ls).unwrap(),
            [
                StageEntry {
                    mode: "100755",
                    sha: "abc",
                    stage: "0",
                    path: "scripts/two words.sh"
                },
                StageEntry {
                    mode: "100644",
                    sha: "def",
                    stage: "2",
                    path: "scripts/café.sh"
                },
            ]
        );
        assert!(parse_stage_entries("100644 abc 0 no-tab\0").is_err());
        assert!(parse_stage_entries("100644 abc\tshort.sh\0").is_err());
    }
}
