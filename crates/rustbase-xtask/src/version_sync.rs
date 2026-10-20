//! Rewrites version sentinels in the docs from the crate's manifest,
//! the script `CLAUDE.md`'s **Version source of truth** section calls
//! for. A doc that quotes the version (an install snippet, say) drifts
//! once a release bumps `Cargo.toml` without it; this reads the
//! version from `Config::version_manifest` and, in every markdown file
//! carrying a `<!-- version: X -->` sentinel, rewrites that sentinel
//! and any `VERSION=<semver>` line in the same file. `/release` runs
//! it during the bump.
//!
//! The sentinel is the opt-in: a file without it is never touched, so
//! an unrelated `VERSION=` line elsewhere is safe.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Config;

/// How a sentinel line starts.
const SENTINEL: &str = "<!-- version:";

/// Whether `line`, trimmed, is a whole sentinel. A file opts in only
/// with such a line, so a doc that quotes the sentinel in a sentence
/// is not rewritten.
fn is_sentinel(line: &str) -> bool {
    let t = line.trim();
    t.starts_with(SENTINEL) && t.ends_with("-->")
}

/// Sync every sentinel to the manifest's version, in place; on
/// success, the line to print.
///
/// # Errors
///
/// When the manifest cannot be read or has no `[package]` version, or
/// when a file to rewrite cannot be written.
pub fn sync(cfg: &Config) -> Result<String, String> {
    let manifest = cfg.root.join(&cfg.version_manifest);
    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let version = parse_package_version(&text).ok_or_else(|| {
        format!("no [package] version in {}", manifest.display())
    })?;
    let mut changed = Vec::new();
    let mut opted_in = 0;
    for path in candidate_files(&cfg.root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if text.lines().any(is_sentinel) {
            opted_in += 1;
        }
        if let Some(new) = rewrite_text(&text, &version) {
            fs::write(&path, new)
                .map_err(|e| format!("write {}: {e}", path.display()))?;
            let rel = path.strip_prefix(&cfg.root).unwrap_or(&path);
            changed.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    if opted_in == 0 {
        return Ok("No version sentinels in README.md or docs/".into());
    }
    if changed.is_empty() {
        return Ok(format!("Version sentinels already at {version}"));
    }
    changed.sort();
    Ok(format!(
        "Version-sync: set {version} in {}",
        changed.join(", ")
    ))
}

/// The `version` under `[package]` in a Cargo manifest. Reads only the
/// `[package]` table, so a dependency's `version =` below it is never
/// taken for the crate's.
fn parse_package_version(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let t = line.trim();
        if let Some(section) = t.strip_prefix('[') {
            in_package = section.starts_with("package]");
            continue;
        }
        if in_package
            && let Some(rest) = t.strip_prefix("version")
            && let Some(val) = rest.trim_start().strip_prefix('=')
        {
            return Some(val.trim().trim_matches('"').to_string());
        }
    }
    None
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

/// One file's text with its sentinels rewritten, or `None` when it
/// carries no `<!-- version: -->` sentinel or is already in sync.
fn rewrite_text(text: &str, version: &str) -> Option<String> {
    if !text.lines().any(is_sentinel) {
        return None;
    }
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        let rewritten = if is_sentinel(trimmed) {
            Some(format!("{indent}<!-- version: {version} -->"))
        } else if trimmed.starts_with("VERSION=") {
            Some(format!("{indent}VERSION={version}"))
        } else {
            None
        };
        match rewritten {
            Some(want) => {
                changed |= want != line;
                out.push_str(&want);
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    // `lines()` drops a trailing newline; restore the original end.
    if !text.ends_with('\n') {
        out.pop();
    }
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    #[test]
    fn parses_the_package_version_ignoring_others() {
        let toml = "[package]\nname = \"p\"\nversion = \"0.6.0\"\n\n\
                    [dependencies]\nserde = { version = \"1\" }\n";
        assert_eq!(parse_package_version(toml).as_deref(), Some("0.6.0"));
        assert_eq!(parse_package_version("[dependencies]\nx = \"1\"\n"), None);
        // A root manifest's shared version is not the crate's.
        let root = "[workspace.package]\nversion = \"9.9.9\"\n\n\
                    [package]\nversion = \"1.2.3\"\n";
        assert_eq!(parse_package_version(root).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn rewrites_a_drifted_sentinel_and_version_line() {
        let text = "## Install\n\n<!-- version: 0.1.0 -->\n\n```bash\n  \
                    VERSION=0.1.0\n```\n";
        let out = rewrite_text(text, "0.6.0").expect("should change");
        assert!(out.contains("<!-- version: 0.6.0 -->"), "{out}");
        assert!(out.contains("  VERSION=0.6.0"), "{out}");
        assert!(!out.contains("0.1.0"), "{out}");
    }

    #[test]
    fn a_file_in_sync_or_without_a_sentinel_is_left_alone() {
        let synced = "<!-- version: 0.6.0 -->\nVERSION=0.6.0\n";
        assert_eq!(rewrite_text(synced, "0.6.0"), None);
        // A bare VERSION= is rewritten only where a sentinel opts in.
        assert_eq!(rewrite_text("VERSION=0.1.0\n", "0.6.0"), None);
    }

    #[test]
    fn a_sentinel_quoted_in_prose_opts_nothing_in() {
        // Only a line that is a sentinel opts the file in; a doc
        // describing the convention keeps its own VERSION= lines.
        let doc = "Write `<!-- version: X -->` above it.\nVERSION=0.1.0\n";
        assert_eq!(rewrite_text(doc, "0.6.0"), None);
    }

    #[test]
    fn preserves_a_missing_trailing_newline() {
        let out = rewrite_text("<!-- version: 0.1.0 -->", "0.6.0").unwrap();
        assert_eq!(out, "<!-- version: 0.6.0 -->");
    }

    #[test]
    fn sync_rewrites_the_opted_in_docs_from_the_manifest() {
        let dir = test_repo::empty_dir();
        let root = &dir.0;
        fs::create_dir_all(root.join("crates/p")).unwrap();
        fs::create_dir_all(root.join("docs/guide")).unwrap();
        fs::write(
            root.join("crates/p/Cargo.toml"),
            "[package]\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let stale = "<!-- version: 0.1.0 -->\nVERSION=0.1.0\n";
        fs::write(root.join("README.md"), stale).unwrap();
        fs::write(root.join("docs/guide/install.md"), stale).unwrap();
        fs::write(root.join("docs/other.md"), "VERSION=0.1.0\n").unwrap();
        let cfg = Config::for_project("p", root.clone());
        assert_eq!(
            sync(&cfg),
            Ok(
                "Version-sync: set 1.2.3 in README.md, docs/guide/install.md"
                    .into()
            )
        );
        let got = fs::read_to_string(root.join("README.md")).unwrap();
        assert_eq!(got, "<!-- version: 1.2.3 -->\nVERSION=1.2.3\n");
        let other = fs::read_to_string(root.join("docs/other.md")).unwrap();
        assert_eq!(other, "VERSION=0.1.0\n");
        assert_eq!(sync(&cfg), Ok("Version sentinels already at 1.2.3".into()));
    }

    #[test]
    fn no_sentinel_anywhere_says_so() {
        let dir = test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("crates/p")).unwrap();
        fs::write(
            dir.0.join("crates/p/Cargo.toml"),
            "[package]\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(dir.0.join("README.md"), "no sentinel\n").unwrap();
        let cfg = Config::for_project("p", dir.0.clone());
        assert_eq!(
            sync(&cfg),
            Ok("No version sentinels in README.md or docs/".into())
        );
    }

    #[test]
    fn a_missing_or_versionless_manifest_is_an_error() {
        let dir = test_repo::empty_dir();
        let cfg = Config::for_project("p", dir.0.clone());
        assert!(sync(&cfg).unwrap_err().contains("read "));
        fs::create_dir_all(dir.0.join("crates/p")).unwrap();
        fs::write(dir.0.join("crates/p/Cargo.toml"), "[workspace]\n").unwrap();
        assert!(sync(&cfg).unwrap_err().contains("no [package] version"));
    }
}
