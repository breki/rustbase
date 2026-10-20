//! Keeps version numbers in the docs current, the job `CLAUDE.md`'s
//! **Version source of truth** section describes. A doc that quotes
//! the version (an install snippet, say) drifts once a release bumps
//! `Cargo.toml` without it; `/release` runs this during the bump.
//!
//! The rules are exact, since a loose match would rewrite prose:
//!
//! - The files read are `README.md` at the root and every `.md` under
//!   `docs/`, recursively; nothing else.
//! - The version is the package's, as `cargo metadata` reports it for
//!   `Config::version_manifest`, so every way a manifest can state it
//!   (workspace inheritance included) is cargo's to resolve.
//! - A file opts in with a line that is exactly a sentinel,
//!   `<!-- version: X -->` and nothing else, outside a fenced code
//!   block (one opened by three or more backticks or tildes). A
//!   sentinel quoted in a sentence or shown in a fenced example does
//!   not opt the file in; an indented code block is not recognized.
//! - In an opted-in file the sentinel line is rewritten, and so is the
//!   number in every line, fenced or not, that starts (after
//!   indentation) with `VERSION=` followed by digits, optionally
//!   quoted; the rest of that line is kept. Other forms
//!   (`export VERSION=`, `VERSION=v1.0`) are not touched.
//! - Every doc is read before any is written, so a read failure
//!   writes nothing. A write failure names its file; the files written
//!   before it stay rewritten.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::Config;
use crate::process::{self, Captured};

/// How a sentinel line starts.
const SENTINEL: &str = "<!-- version:";

/// What a sync found and did. Paths are relative to the root,
/// `/`-separated and sorted.
#[derive(Debug, PartialEq, Eq)]
pub struct SyncReport {
    /// The package's version.
    pub version: String,
    /// Every file carrying a sentinel, changed or not: what a release
    /// stages, so a re-run after a partial release stages them too.
    pub opted_in: Vec<String>,
    /// The files this run rewrote.
    pub changed: Vec<String>,
}

/// The summary line, then the files to stage when there are any.
impl fmt::Display for SyncReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = &self.version;
        if self.opted_in.is_empty() {
            return f.write_str("No version sentinels in README.md or docs/");
        }
        if self.changed.is_empty() {
            write!(f, "Version sentinels already at {v}")?;
        } else {
            write!(f, "Version-sync: set {v} in {}", self.changed.join(", "))?;
        }
        write!(f, "\nFiles with a sentinel: {}", self.opted_in.join(", "))
    }
}

/// Sync every sentinel to the package's version, in place.
///
/// # Errors
///
/// When `cargo metadata` fails or reports no package for the manifest,
/// or when a doc cannot be read or written.
pub fn sync(cfg: &Config) -> Result<SyncReport, String> {
    collect(cfg, |args| process::cargo(&cfg.root, args))
}

/// [`sync`] with the `cargo` spawn injected: it reads every doc, then
/// writes the ones that change.
fn collect(
    cfg: &Config,
    cargo: impl FnOnce(&[&str]) -> Result<Captured, String>,
) -> Result<SyncReport, String> {
    let manifest = cfg.root.join(&cfg.version_manifest);
    let manifest_arg = manifest.to_string_lossy();
    let out = cargo(&[
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        // With `--no-deps` cargo needs nothing from the network; this
        // stops a needless index fetch during a release.
        "--offline",
        "--manifest-path",
        &manifest_arg,
    ])?;
    if !out.ok {
        return Err(format!("cargo metadata failed:\n{}", out.stderr.trim()));
    }
    let version = package_version(&out.stdout, &manifest)?;
    // Read everything first, so a failure writes nothing.
    let mut writes = Vec::new();
    let mut report = SyncReport {
        version,
        opted_in: Vec::new(),
        changed: Vec::new(),
    };
    for path in candidate_files(&cfg.root) {
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let rel = path
            .strip_prefix(&cfg.root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        match rewrite_text(&text, &report.version) {
            FileSync::NotOptedIn => {}
            FileSync::InSync => report.opted_in.push(rel),
            FileSync::Rewritten(new) => {
                report.opted_in.push(rel.clone());
                report.changed.push(rel);
                writes.push((path, new));
            }
        }
    }
    for (path, new) in writes {
        fs::write(&path, new)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    report.opted_in.sort();
    report.changed.sort();
    Ok(report)
}

/// The version of the package `cargo metadata` reports at `manifest`.
/// In a workspace it lists every member, so the manifest picks one.
fn package_version(metadata: &str, manifest: &Path) -> Result<String, String> {
    let meta: Value = serde_json::from_str(metadata)
        .map_err(|e| format!("cannot read cargo metadata: {e}"))?;
    let want = fs::canonicalize(manifest).unwrap_or_else(|_| manifest.into());
    meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| {
            let at = Path::new(p["manifest_path"].as_str().unwrap_or(""));
            fs::canonicalize(at).unwrap_or_else(|_| at.into()) == want
        })
        .and_then(|p| p["version"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "cargo metadata names no package at {}; point \
                 Config::version_manifest at the crate whose version the \
                 project ships",
                manifest.display()
            )
        })
}

/// Files a sentinel might live in: `README.md` and every markdown file
/// under `docs/`.
fn candidate_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join("README.md")];
    collect_md(&root.join("docs"), &mut out);
    out.retain(|p| p.is_file());
    out
}

/// Append every `.md` file under `dir`, recursively.
fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_md(&path, out);
        } else if path.extension().is_some_and(|x| x == "md") {
            out.push(path);
        }
    }
}

/// What syncing one file comes to.
#[derive(Debug, PartialEq, Eq)]
enum FileSync {
    /// No sentinel line outside a code block.
    NotOptedIn,
    /// Opted in, and already at the version.
    InSync,
    /// Opted in; the new text.
    Rewritten(String),
}

/// Whether `line` is a sentinel and nothing else: one token between
/// `<!-- version:` and `-->`, so a rewrite drops nothing.
fn is_sentinel(line: &str) -> bool {
    line.strip_prefix(SENTINEL)
        .and_then(|rest| rest.strip_suffix("-->"))
        .is_some_and(|v| {
            let v = v.trim();
            !v.is_empty()
                && !v.contains(char::is_whitespace)
                && !v.contains("--")
        })
}

/// A code fence's opening: its character and how many of it.
fn fence(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    let c = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

/// One file's text with its sentinels and `VERSION=` numbers set to
/// `version`, each line keeping its own ending.
fn rewrite_text(text: &str, version: &str) -> FileSync {
    let mut opted_in = false;
    // The open code block's fence; it closes only on the same
    // character, at least as many, and nothing else on the line.
    let mut open: Option<(char, usize)> = None;
    let mut out = String::with_capacity(text.len());
    for chunk in text.split_inclusive('\n') {
        let body = chunk.trim_end_matches(['\r', '\n']);
        let ending = &chunk[body.len()..];
        let trimmed = body.trim();
        let mut new = None;
        match (open, fence(body)) {
            (None, Some(f)) => open = Some(f),
            (Some((c, n)), Some((c2, n2)))
                if c2 == c && n2 >= n && trimmed.chars().all(|x| x == c) =>
            {
                open = None;
            }
            (None, None) if is_sentinel(trimmed) => {
                opted_in = true;
                let indent = &body[..body.len() - body.trim_start().len()];
                new = Some(format!("{indent}<!-- version: {version} -->"));
            }
            _ => {}
        }
        // A `VERSION=` inside a code block counts: it is the install
        // snippet the sentinel keeps current.
        let line = new.or_else(|| set_assignment(body, version));
        out.push_str(line.as_deref().unwrap_or(body));
        out.push_str(ending);
    }
    if !opted_in {
        FileSync::NotOptedIn
    } else if out == text {
        FileSync::InSync
    } else {
        FileSync::Rewritten(out)
    }
}

/// `line` with the number after a leading `VERSION=` replaced, the
/// rest of the line (quotes, a command after it) kept; `None` unless
/// the value starts with a digit.
fn set_assignment(line: &str, version: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("VERSION=")?;
    let indent = &line[..line.len() - trimmed.len()];
    let (open, value) = match rest.chars().next() {
        Some(q @ ('"' | '\'')) => (q.to_string(), &rest[1..]),
        _ => (String::new(), rest),
    };
    let len = value
        .find(|c: char| !(c.is_ascii_alphanumeric() || ".+-".contains(c)))
        .unwrap_or(value.len());
    let token = &value[..len];
    if !token.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let tail = &value[len..];
    if !open.is_empty() && !tail.starts_with(open.as_str()) {
        return None;
    }
    Some(format!("{indent}VERSION={open}{version}{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    fn rewritten(text: &str) -> String {
        match rewrite_text(text, "0.6.0") {
            FileSync::Rewritten(s) => s,
            other => panic!("not rewritten: {other:?}"),
        }
    }

    #[test]
    fn rewrites_a_drifted_sentinel_and_version_line() {
        let text = "## Install\n\n<!-- version: 0.1.0 -->\n\n```bash\n  \
                    VERSION=0.1.0\n```\n";
        let out = rewritten(text);
        assert!(out.contains("<!-- version: 0.6.0 -->"), "{out}");
        assert!(out.contains("\n  VERSION=0.6.0\n"), "{out}");
        assert!(!out.contains("0.1.0"), "{out}");
    }

    #[test]
    fn only_the_number_in_an_exact_assignment_changes() {
        let text = "<!-- version: 0.1.0 -->\nVERSION=0.1.0 ./install.sh\n\
                    VERSION=\"0.1.0\"\nVERSION=$(curl x)\nVERSION=v0.1.0\n\
                    export VERSION=0.1.0\n";
        assert_eq!(
            rewritten(text),
            "<!-- version: 0.6.0 -->\nVERSION=0.6.0 ./install.sh\n\
             VERSION=\"0.6.0\"\nVERSION=$(curl x)\nVERSION=v0.1.0\n\
             export VERSION=0.1.0\n"
        );
    }

    #[test]
    fn a_file_in_sync_or_not_opted_in_is_left_alone() {
        let synced = "<!-- version: 0.6.0 -->\nVERSION=0.6.0\n";
        assert_eq!(rewrite_text(synced, "0.6.0"), FileSync::InSync);
        let bare = "VERSION=0.1.0\n";
        assert_eq!(rewrite_text(bare, "0.6.0"), FileSync::NotOptedIn);
        let prose = "Write `<!-- version: X -->` above it.\nVERSION=0.1.0\n";
        assert_eq!(rewrite_text(prose, "0.6.0"), FileSync::NotOptedIn);
        let fenced = "```md\n<!-- version: 0.5.0 -->\n```\nVERSION=0.1.0\n";
        assert_eq!(rewrite_text(fenced, "0.6.0"), FileSync::NotOptedIn);
    }

    #[test]
    fn a_sentinel_line_holds_the_sentinel_alone() {
        // More on the line would be dropped by the rewrite.
        let doc = "<!-- version: 0.1.0 --> <!-- toc -->\nVERSION=0.1.0\n";
        assert_eq!(rewrite_text(doc, "0.6.0"), FileSync::NotOptedIn);
        let empty = "<!-- version: -->\nVERSION=0.1.0\n";
        assert_eq!(rewrite_text(empty, "0.6.0"), FileSync::NotOptedIn);
    }

    #[test]
    fn a_code_block_closes_only_on_its_own_fence() {
        // A shorter or different fence inside is content: the example
        // sentinel after it is still inside the block.
        for doc in [
            "````md\n```bash\nx\n```\n<!-- version: 0.5.0 -->\n````\n",
            "```md\n~~~\n<!-- version: 0.5.0 -->\n```\n",
            "```md\n``` not a close\n<!-- version: 0.5.0 -->\n```\n",
        ] {
            assert_eq!(rewrite_text(doc, "0.6.0"), FileSync::NotOptedIn);
        }
        let after = "```\ncode\n```\n<!-- version: 0.1.0 -->\n";
        assert_eq!(
            rewritten(after),
            "```\ncode\n```\n<!-- version: 0.6.0 -->\n"
        );
    }

    #[test]
    fn line_endings_are_kept() {
        assert_eq!(
            rewritten("<!-- version: 0.1.0 -->\r\nkeep\r\nVERSION=0.1.0\r\n"),
            "<!-- version: 0.6.0 -->\r\nkeep\r\nVERSION=0.6.0\r\n"
        );
        assert_eq!(
            rewritten("<!-- version: 0.1.0 -->"),
            "<!-- version: 0.6.0 -->"
        );
    }

    #[test]
    fn the_version_is_the_package_at_the_manifest() {
        let meta = r#"{"packages": [
            {"name": "a", "version": "9.9.9", "manifest_path": "/w/a/Cargo.toml"},
            {"name": "p", "version": "1.2.3", "manifest_path": "/w/p/Cargo.toml"}
        ]}"#;
        let got = package_version(meta, Path::new("/w/p/Cargo.toml"));
        assert_eq!(got, Ok("1.2.3".into()));
        let e =
            package_version(meta, Path::new("/w/x/Cargo.toml")).unwrap_err();
        assert!(e.contains("Config::version_manifest"), "{e}");
        assert!(package_version("nope", Path::new("/w")).is_err());
    }

    /// A temp crate at `1.2.3`, its `Config`, and empty `docs/guide`.
    fn project() -> (test_repo::TempRepo, Config) {
        let dir = test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("crates/p/src")).unwrap();
        fs::create_dir_all(dir.0.join("docs/guide")).unwrap();
        fs::write(
            dir.0.join("crates/p/Cargo.toml"),
            "[package]\nname = \"p\"\nversion = \"1.2.3\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(dir.0.join("crates/p/src/lib.rs"), "").unwrap();
        let cfg = Config::for_project("p", dir.0.clone());
        (dir, cfg)
    }

    #[test]
    fn sync_asks_cargo_and_rewrites_the_opted_in_docs() {
        let (dir, cfg) = project();
        let stale = "<!-- version: 0.1.0 -->\nVERSION=0.1.0\n";
        fs::write(dir.0.join("README.md"), stale).unwrap();
        fs::write(dir.0.join("docs/guide/install.md"), stale).unwrap();
        fs::write(dir.0.join("docs/other.md"), "VERSION=0.1.0\n").unwrap();
        let report = sync(&cfg).unwrap();
        assert_eq!(report.changed, ["README.md", "docs/guide/install.md"]);
        assert_eq!(report.opted_in, report.changed);
        assert_eq!(
            report.to_string(),
            "Version-sync: set 1.2.3 in README.md, docs/guide/install.md\n\
             Files with a sentinel: README.md, docs/guide/install.md"
        );
        let got = fs::read_to_string(dir.0.join("README.md")).unwrap();
        assert_eq!(got, "<!-- version: 1.2.3 -->\nVERSION=1.2.3\n");
        let other = fs::read_to_string(dir.0.join("docs/other.md")).unwrap();
        assert_eq!(other, "VERSION=0.1.0\n");
        // A re-run changes nothing but still names the files to stage.
        let again = sync(&cfg).unwrap();
        assert_eq!(again.changed, Vec::<String>::new());
        assert_eq!(again.opted_in, ["README.md", "docs/guide/install.md"]);
        assert!(
            again
                .to_string()
                .starts_with("Version sentinels already at")
        );
    }

    #[test]
    fn no_sentinel_anywhere_says_so() {
        let (dir, cfg) = project();
        fs::write(dir.0.join("README.md"), "no sentinel\n").unwrap();
        assert_eq!(
            sync(&cfg).unwrap().to_string(),
            "No version sentinels in README.md or docs/"
        );
    }

    #[test]
    fn a_doc_that_cannot_be_read_stops_the_run_before_any_write() {
        let (dir, cfg) = project();
        let stale = "<!-- version: 0.1.0 -->\n";
        fs::write(dir.0.join("README.md"), stale).unwrap();
        fs::write(dir.0.join("docs/bad.md"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(sync(&cfg).unwrap_err().contains("bad.md"));
        let readme = fs::read_to_string(dir.0.join("README.md")).unwrap();
        assert_eq!(readme, stale, "README.md was written before the failure");
    }

    #[test]
    fn a_failed_cargo_metadata_is_an_error() {
        let (_dir, cfg) = project();
        let failed = Captured {
            ok: false,
            status: String::new(),
            stdout: String::new(),
            stderr: "error: bad manifest".into(),
        };
        let e = collect(&cfg, |_| Ok(failed)).unwrap_err();
        assert!(e.contains("bad manifest"), "{e}");
    }
}
