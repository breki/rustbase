//! Changed-dependency cooldown gate -- the `validate` step.
//!
//! Checks only the `(name, version)` pairs newly present in
//! the working-tree lockfiles versus a base commit (`HEAD`
//! unless `--base` names another, as CI does), so it costs
//! nothing (no network) on the common commit that leaves the
//! lockfiles untouched, and fires exactly at the moment a
//! dependency is adopted. A *whole-tree* continuous gate is
//! deliberately avoided -- it would flag every already-locked
//! version on every routine update.
//!
//! Scope caveat: the check covers **every** newly-locked
//! registry dependency, transitive ones included. A
//! lockfile-churning update (`cargo update` / `npm update`)
//! therefore can flag many fresh transitive versions at once;
//! see `Config::dep_age_allow_env` and the supply-chain notes in
//! `CLAUDE.md` for the intended workflow.
//!
//! The shared registry-fetch and date-verdict machinery lives
//! in the parent [`super`] module; this submodule imports it.

use std::collections::HashSet;
use std::fs;

use serde_json::Value;

use super::io::{git_commit_exists, git_show};
use super::{
    COOLDOWN_DAYS, Ecosystem, age_in_days, cargo_version_date, fetch_registry,
    npm_version_date, parse_iso_date, today_days,
};
use crate::Config;

/// Combined outcome of the changed-deps gate. Both `error` and
/// `unchecked` fail it, inside `validate` and standalone; they are
/// kept apart because they call for different fixes.
pub struct DepAgeResult {
    /// Human-readable detail for the step line.
    pub detail: String,
    /// `Some` only when a changed dependency is within the
    /// cooldown.
    pub error: Option<String>,
    /// Checks that could not finish (missing baseline, unreachable
    /// registry, unparseable response).
    pub unchecked: Vec<String>,
}

/// Per-dependency check outcome.
#[derive(Debug, PartialEq, Eq)]
enum DepOutcome {
    /// Older than the cooldown -- fine.
    Aged,
    /// Within the cooldown window (fatal); carries the reason.
    Fresh(String),
    /// Could not be checked (network / tool / parse); fails the
    /// gate like `Fresh`, but is reported apart from it.
    Unavailable(String),
    /// Named in the allow file or env var -- skipped, never fetched.
    Allowed,
}

/// Fold per-dependency outcomes into the combined result.
/// Pure, so the fresh-vs-unchecked split is unit-tested.
/// `allow_hint` ends a fresh failure, saying how to waive one.
fn classify_dep_age(
    outcomes: &[(String, DepOutcome)],
    allow_hint: &str,
) -> DepAgeResult {
    if outcomes.is_empty() {
        return DepAgeResult {
            detail: "no dependency changes".into(),
            error: None,
            unchecked: Vec::new(),
        };
    }
    let mut fresh: Vec<String> = Vec::new();
    let mut unchecked: Vec<String> = Vec::new();
    let (mut aged, mut allowed) = (0usize, 0usize);
    for (label, outcome) in outcomes {
        match outcome {
            DepOutcome::Aged => aged += 1,
            DepOutcome::Allowed => allowed += 1,
            DepOutcome::Fresh(why) => fresh.push(format!("{label} ({why})")),
            DepOutcome::Unavailable(reason) => {
                unchecked.push(format!("{label}: {reason}"));
            }
        }
    }
    // `unchecked.len()` is the count of Unavailable outcomes.
    // Including it keeps the sub-counts reconciling with the
    // "N changed" total when the registry is unreachable (where
    // nothing was actually checked).
    let detail = format!(
        "{} changed: {aged} aged, {allowed} allow-listed, {} fresh, \
         {} unchecked",
        outcomes.len(),
        fresh.len(),
        unchecked.len(),
    );
    let error = if fresh.is_empty() {
        None
    } else {
        Some(format!(
            "dependencies within the {COOLDOWN_DAYS}-day cooldown: {}\n  \
             adopt only with a stated justification (security fixes \
             exempt); {allow_hint}",
            fresh.join("; ")
        ))
    };
    DepAgeResult {
        detail,
        error,
        unchecked,
    }
}

/// How a fresh dependency is waived, for the failure message.
pub(super) fn allow_hint(cfg: &Config) -> String {
    format!(
        "allow via a `name@version` line in {} committed at the base the \
         check diffs from (main, in CI), or {}=name@version[,...]",
        cfg.dep_age_allow_file, cfg.dep_age_allow_env
    )
}

/// The waived `name@version` labels: the env list, and the allow
/// file as committed at `base` (none when it is absent there).
fn allowed(cfg: &Config, base: &str, env: Option<&str>) -> HashSet<String> {
    let mut allow = parse_allow(env);
    if let Some(text) = git_show(&cfg.root, base, &cfg.dep_age_allow_file) {
        allow.extend(parse_allow_file(&text));
    }
    allow
}

/// One `name@version` per line; `#` starts a comment.
fn parse_allow_file(text: &str) -> HashSet<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parse the comma-separated allow list into a set of
/// `name@version` labels. Empty / whitespace entries dropped.
fn parse_allow(spec: Option<&str>) -> HashSet<String> {
    spec.into_iter()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Extract `(name, version)` pairs from a `Cargo.lock`,
/// **registry-sourced crates only**. A dependency-free scan of
/// the `[[package]]` blocks -- avoids adding a TOML parser for
/// a machine-generated, regular format. Lines inside a
/// `dependencies = [...]` array start with a quote, not
/// `name = `/`version = `, so they are ignored.
///
/// Only crates with a `source = "registry+..."` line are
/// returned: local workspace crates (no `source`, e.g. this
/// project's own crates on a version bump) and git deps
/// (`source = "git+..."`) have no crates.io publish date to
/// check, so including them would fail every release commit.
pub(super) fn parse_cargo_lock(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut registry = false;
    let mut flush = |name: &mut Option<String>,
                     version: &mut Option<String>,
                     registry: &mut bool| {
        if let (Some(n), Some(v)) = (name.take(), version.take())
            && *registry
        {
            out.push((n, v));
        }
        *registry = false;
    };
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            flush(&mut name, &mut version, &mut registry);
        } else if let Some(v) = line.strip_prefix("name = ") {
            name = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("version = ") {
            version = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("source = ") {
            registry = unquote(v).starts_with("registry+");
        }
    }
    flush(&mut name, &mut version, &mut registry);
    out
}

/// Strip surrounding double quotes from a TOML string value.
fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

/// Extract `(name, version)` pairs from an npm lockfile
/// (`lockfileVersion` 2/3 `packages` map), **registry-sourced
/// packages only**. The map is keyed by install path
/// (`node_modules/<name>`, `node_modules/a/node_modules/b`);
/// the name is the segment after the final `node_modules/`.
///
/// Only entries whose `resolved` is an `http(s)` tarball URL
/// are returned -- the root package (empty key), workspace
/// links (`link: true`, relative `resolved`) and `file:` deps
/// have no npm-registry publish date to check. `None` for text
/// that is not JSON or has no `packages` map (a v1 lockfile).
fn parse_npm_lock(text: &str) -> Option<Vec<(String, String)>> {
    let json = serde_json::from_str::<Value>(text).ok()?;
    let packages = json["packages"].as_object()?;
    let pairs = packages
        .iter()
        .filter(|(key, _)| !key.is_empty())
        .filter(|(_, val)| {
            val["resolved"]
                .as_str()
                .is_some_and(|r| r.starts_with("http"))
        })
        .filter_map(|(key, val)| {
            let name = key.rsplit("node_modules/").next().unwrap_or(key);
            val["version"]
                .as_str()
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect();
    Some(pairs)
}

/// `(name, version)` pairs present in `new` but not in `old`
/// -- both freshly added packages and version bumps of an
/// existing package (the bumped tuple is simply not in `old`).
pub(super) fn new_locked_versions(
    old: &[(String, String)],
    new: &[(String, String)],
) -> Vec<(String, String)> {
    let old_set: HashSet<&(String, String)> = old.iter().collect();
    new.iter()
        .filter(|pair| !old_set.contains(*pair))
        .cloned()
        .collect()
}

/// Network check of one dependency version against the
/// cooldown. Thin wrapper over the shared fetch + date +
/// verdict helpers; the branchy logic lives in those
/// (already unit-tested) pieces.
fn check_one(
    cfg: &Config,
    eco: Ecosystem,
    name: &str,
    version: &str,
) -> DepOutcome {
    let json = match fetch_registry(cfg, eco, name) {
        Ok(j) => j,
        Err(e) => return DepOutcome::Unavailable(e),
    };
    let dated = match eco {
        Ecosystem::Npm => npm_version_date(&json, Some(version)),
        Ecosystem::Cargo => cargo_version_date(&json, Some(version)),
    };
    let published = match dated {
        Ok((_, p)) => p,
        Err(e) => return DepOutcome::Unavailable(e),
    };
    match parse_iso_date(&published) {
        Ok(days) => {
            let age = age_in_days(days, today_days());
            if age < COOLDOWN_DAYS {
                DepOutcome::Fresh(format!("published {age}d ago"))
            } else {
                DepOutcome::Aged
            }
        }
        Err(e) => DepOutcome::Unavailable(e),
    }
}

/// One lockfile the gate diffs, and how to read it.
struct Lockfile {
    eco: Ecosystem,
    /// Path from the workspace root.
    rel: &'static str,
    /// Missing means "no such ecosystem" unless required.
    required: bool,
    /// `None` when the text is not a lockfile it can read.
    parser: fn(&str) -> Option<Pairs>,
}

/// `(name, version)` pairs read from a lockfile.
type Pairs = Vec<(String, String)>;

const CARGO_LOCK: Lockfile = Lockfile {
    eco: Ecosystem::Cargo,
    rel: "Cargo.lock",
    required: true,
    // Read line by line, so it never refuses a file.
    parser: |text| Some(parse_cargo_lock(text)),
};

/// The repo-root npm tree (the template's Playwright E2E harness).
const ROOT_NPM_LOCK: Lockfile = Lockfile {
    eco: Ecosystem::Npm,
    rel: "package-lock.json",
    required: false,
    parser: parse_npm_lock,
};

const NPM_LOCK: Lockfile = Lockfile {
    eco: Ecosystem::Npm,
    rel: "frontend/package-lock.json",
    required: false,
    parser: parse_npm_lock,
};

/// Diff one lockfile against `base` and append an outcome per
/// newly-adopted dependency. A missing optional lockfile means
/// the ecosystem is absent (skip); an unreadable or missing
/// required one is unchecked; so is the file when `base` has no
/// baseline to diff it against.
fn collect_changes(
    cfg: &Config,
    base: &str,
    lock: &Lockfile,
    allow: &HashSet<String>,
    outcomes: &mut Vec<(String, DepOutcome)>,
    unchecked: &mut Vec<String>,
) {
    let Lockfile {
        eco,
        rel,
        required,
        parser,
    } = *lock;
    let current_text = match fs::read_to_string(cfg.root.join(rel)) {
        Ok(text) => text,
        // An optional lockfile that does not exist means the
        // ecosystem is absent; anything else must not pass silently.
        Err(e) if !required && e.kind() == std::io::ErrorKind::NotFound => {
            return;
        }
        Err(e) => {
            unchecked.push(format!("{rel}: cannot read ({e})"));
            return;
        }
    };
    // A lockfile absent at an existing base commit is new: every
    // entry in it is newly adopted, so diff against nothing.
    let baseline_text = match git_show(&cfg.root, base, rel) {
        Some(text) => text,
        None if git_commit_exists(&cfg.root, base) => String::new(),
        None => {
            // The root-commit clause is for CI, which falls back to
            // `HEAD^` when the push names no earlier commit.
            let hint = if base == "HEAD" {
                "commit first"
            } else {
                "is that commit fetched? a shallow checkout lacks it, and \
                 a root commit has no parent"
            };
            unchecked.push(format!(
                "{rel}: no {base} baseline to diff against ({hint})"
            ));
            return;
        }
    };
    // Unchanged adopts nothing, whatever format the file is in.
    if current_text == baseline_text {
        return;
    }
    let Some(current) = parser(&current_text) else {
        unchecked.push(format!(
            "{rel}: cannot parse (not JSON, or no `packages` map) -- \
             resolve any conflict markers, or regenerate it with npm 7+"
        ));
        return;
    };
    // A baseline it cannot read counts as empty: every entry is new.
    let baseline = parser(&baseline_text).unwrap_or_default();
    let mut changed = new_locked_versions(&baseline, &current);
    changed.sort();
    for (name, version) in changed {
        let label = format!("{name}@{version}");
        let outcome = if allow.contains(&label) {
            DepOutcome::Allowed
        } else {
            check_one(cfg, eco, &name, &version)
        };
        outcomes.push((label, outcome));
    }
}

/// Run the changed-dependency cooldown gate: check only the
/// `(name, version)` pairs newly present in the working-tree
/// lockfiles versus `base` (`validate` passes `HEAD`). Free (no
/// network) when the lockfiles are unchanged.
pub fn check_changed_deps(cfg: &Config, base: &str) -> DepAgeResult {
    let env = std::env::var(&cfg.dep_age_allow_env).ok();
    let allow = allowed(cfg, base, env.as_deref());
    let mut outcomes: Vec<(String, DepOutcome)> = Vec::new();
    let mut unchecked: Vec<String> = Vec::new();

    collect_changes(
        cfg,
        base,
        &CARGO_LOCK,
        &allow,
        &mut outcomes,
        &mut unchecked,
    );
    collect_changes(
        cfg,
        base,
        &NPM_LOCK,
        &allow,
        &mut outcomes,
        &mut unchecked,
    );
    collect_changes(
        cfg,
        base,
        &ROOT_NPM_LOCK,
        &allow,
        &mut outcomes,
        &mut unchecked,
    );

    let mut result = classify_dep_age(&outcomes, &allow_hint(cfg));
    // Lockfile-level entries precede the per-dep ones the
    // classifier collected.
    unchecked.extend(result.unchecked);
    result.unchecked = unchecked;
    result
}

/// The gate's verdict, the same for `validate` and the standalone
/// command: a fresh dependency fails, and so does a check that could
/// not finish. A fresh error is reported alone; unchecked entries
/// surface once no fresh one is left. `Ok` carries the step detail.
pub(crate) fn verdict(r: DepAgeResult) -> Result<String, String> {
    if let Some(err) = r.error {
        return Err(err);
    }
    if !r.unchecked.is_empty() {
        return Err(format!(
            "dep-age could not complete: {}",
            r.unchecked.join("; ")
        ));
    }
    Ok(r.detail)
}

/// Standalone `cargo xtask dep-age-check`, with the `validate`
/// step's verdict. `base` is the commit to diff against: `HEAD`
/// locally; in CI, the commit the push or pull request started from.
pub fn dep_age_check(cfg: &Config, base: &str) -> Result<(), String> {
    check_base_ref(base)?;
    let r = check_changed_deps(cfg, base);
    // The counts, unchecked ones included, which the fresh error hides.
    if r.error.is_some() {
        eprintln!("  {}", r.detail);
    }
    println!("Dep-age OK ({})", verdict(r)?);
    Ok(())
}

/// A base ref is handed to `git show`, so one starting with `-`
/// would be read as an option; a blank one names nothing.
fn check_base_ref(base: &str) -> Result<(), String> {
    if base.trim().is_empty() || base.starts_with('-') {
        Err(format!("dep-age-check --base: '{base}' is not a git ref"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> Config {
        Config::for_project("t", std::env::temp_dir())
    }

    #[test]
    fn a_missing_required_lockfile_is_unchecked_not_a_pass() {
        // An empty outcome list reads as "no dependency changes",
        // so only the unchecked entry keeps it from passing.
        let (mut outcomes, mut unchecked) = (Vec::new(), Vec::new());
        collect_changes(
            &test_cfg(),
            "HEAD",
            &Lockfile {
                rel: "no-such-file.lock",
                ..CARGO_LOCK
            },
            &HashSet::new(),
            &mut outcomes,
            &mut unchecked,
        );
        assert!(outcomes.is_empty(), "{outcomes:?}");
        assert_eq!(unchecked.len(), 1, "{unchecked:?}");
        assert!(unchecked[0].contains("cannot read"), "{unchecked:?}");
        assert!(!unchecked[0].contains("skipping"), "{unchecked:?}");
    }

    #[test]
    fn an_unparseable_npm_lockfile_is_unchecked_not_a_pass() {
        // Conflict markers or a v1 lockfile hold no `packages` map,
        // which must not read as "no dependency changes".
        let repo = crate::test_repo::with_lock(LOCK, true);
        let cfg = Config::for_project("t", repo.0.clone());
        for text in ["<<<<<<< HEAD\n{}", r#"{"lockfileVersion": 1}"#] {
            std::fs::write(repo.0.join("package-lock.json"), text).unwrap();
            let (mut outcomes, mut unchecked) = (Vec::new(), Vec::new());
            collect_changes(
                &cfg,
                "HEAD",
                &ROOT_NPM_LOCK,
                &HashSet::new(),
                &mut outcomes,
                &mut unchecked,
            );
            assert!(outcomes.is_empty(), "{outcomes:?}");
            assert_eq!(unchecked.len(), 1, "{text}: {unchecked:?}");
            assert!(unchecked[0].contains("cannot parse"), "{unchecked:?}");
            assert!(unchecked[0].contains("npm 7"), "{unchecked:?}");
        }
    }

    #[test]
    fn an_unchanged_lockfile_passes_whatever_its_format() {
        // A committed v1 lockfile adopted nothing, so nothing is
        // left unchecked.
        let repo = crate::test_repo::with_lock(LOCK, true);
        let v1 = r#"{"lockfileVersion": 1, "dependencies": {}}"#;
        std::fs::write(repo.0.join("package-lock.json"), v1).unwrap();
        crate::test_repo::git(&repo.0, &["add", "package-lock.json"]);
        crate::test_repo::git(&repo.0, &["commit", "-q", "-m", "v1"]);
        let cfg = Config::for_project("t", repo.0.clone());
        let (mut outcomes, mut unchecked) = (Vec::new(), Vec::new());
        collect_changes(
            &cfg,
            "HEAD",
            &ROOT_NPM_LOCK,
            &HashSet::new(),
            &mut outcomes,
            &mut unchecked,
        );
        assert!(outcomes.is_empty() && unchecked.is_empty(), "{unchecked:?}");
    }

    #[test]
    fn a_missing_optional_lockfile_means_no_such_ecosystem() {
        let (mut outcomes, mut unchecked) = (Vec::new(), Vec::new());
        collect_changes(
            &test_cfg(),
            "HEAD",
            &Lockfile {
                rel: "no-such-frontend/package-lock.json",
                ..NPM_LOCK
            },
            &HashSet::new(),
            &mut outcomes,
            &mut unchecked,
        );
        assert!(outcomes.is_empty() && unchecked.is_empty(), "{unchecked:?}");
    }

    const LOCK: &str = "version = 4\n\n[[package]]\nname = \"serde\"\n\
        version = \"1.0.0\"\nsource = \"registry+https://github.com/\
        rust-lang/crates.io-index\"\n";

    #[test]
    fn an_unchanged_lockfile_passes_without_the_network() {
        let repo = crate::test_repo::with_lock(LOCK, true);
        let cfg = Config::for_project("t", repo.0.clone());
        let r = check_changed_deps(&cfg, "HEAD");
        assert!(
            r.error.is_none() && r.unchecked.is_empty(),
            "{:?}",
            r.unchecked
        );
        assert_eq!(r.detail, "no dependency changes");
        assert!(dep_age_check(&cfg, "HEAD").is_ok());
    }

    #[test]
    fn verdict_fails_when_the_check_cannot_finish() {
        let incomplete = DepAgeResult {
            detail: "1 changed: 0 aged, 1 unchecked".into(),
            error: None,
            unchecked: vec!["a@1.0.0: registry unreachable".into()],
        };
        let err = verdict(incomplete).unwrap_err();
        assert!(err.contains("registry unreachable"), "{err}");
        let done = DepAgeResult {
            detail: "no dependency changes".into(),
            error: None,
            unchecked: Vec::new(),
        };
        assert_eq!(verdict(done).unwrap(), "no dependency changes");
    }

    #[test]
    fn a_lockfile_with_no_baseline_cannot_pass_the_standalone_check() {
        let repo = crate::test_repo::with_lock(LOCK, false);
        let cfg = Config::for_project("t", repo.0.clone());
        let r = check_changed_deps(&cfg, "HEAD");
        assert_eq!(r.unchecked.len(), 1, "{:?}", r.unchecked);
        assert!(
            r.unchecked[0].contains("no HEAD baseline"),
            "{:?}",
            r.unchecked
        );
        let err = dep_age_check(&cfg, "HEAD").unwrap_err();
        assert!(err.contains("could not complete"), "{err}");
        // The error itself names the cause and, since no allow-list
        // entry can cover it, the way out for that base. `--skip` is
        // the project's `validate` option, not the library's to name.
        assert!(err.contains("no HEAD baseline"), "{err}");
        assert!(err.contains("commit first"), "{err}");
        assert!(!err.contains("skip"), "{err}");
        let err = dep_age_check(&cfg, "0123abc").unwrap_err();
        assert!(err.contains("shallow"), "{err}");
        assert!(!err.contains("commit first"), "{err}");
        assert!(dep_age_check(&cfg, "-p").is_err());
    }

    #[test]
    fn a_lockfile_new_since_the_base_checks_every_entry() {
        // The base commit exists but had no such lockfile, so
        // everything in it is newly adopted -- not "no baseline".
        let repo = crate::test_repo::with_lock(LOCK, true);
        std::fs::write(
            repo.0.join("package-lock.json"),
            r#"{"packages": {"": {}, "node_modules/left-pad":
               {"version": "1.3.0", "resolved":
               "https://registry.npmjs.org/left-pad/-/left-pad-1.3.0.tgz"}}}"#,
        )
        .unwrap();
        let cfg = Config::for_project("t", repo.0.clone());
        let allow: HashSet<String> = ["left-pad@1.3.0".to_string()].into();
        let (mut outcomes, mut unchecked) = (Vec::new(), Vec::new());
        collect_changes(
            &cfg,
            "HEAD",
            &ROOT_NPM_LOCK,
            &allow,
            &mut outcomes,
            &mut unchecked,
        );
        assert!(unchecked.is_empty(), "{unchecked:?}");
        assert_eq!(
            outcomes,
            vec![("left-pad@1.3.0".into(), DepOutcome::Allowed)]
        );
    }

    #[test]
    fn base_ref_must_not_read_as_an_option() {
        // The ref goes to `git show <ref>:<path>`; a leading `-`
        // would be parsed as an option, and blank names nothing.
        for ok in ["HEAD", "origin/main", "6528907", "HEAD~1"] {
            assert!(check_base_ref(ok).is_ok(), "{ok}");
        }
        for bad in ["", " ", "--output=/tmp/x", "-p"] {
            assert!(check_base_ref(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parse_cargo_lock_registry_only_ignoring_locals_and_deps() {
        let lock = r#"
version = 4

[[package]]
name = "aho-corasick"
version = "1.1.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
dependencies = [
 "memchr",
]

[[package]]
name = "rustbase"
version = "0.13.0"
dependencies = [
 "aho-corasick",
]
"#;
        // Registry crate kept; the local workspace crate
        // (no `source`, e.g. this project on a version bump),
        // the `version = 4` header, and the quoted dependency
        // line are all excluded.
        assert_eq!(
            parse_cargo_lock(lock),
            vec![("aho-corasick".into(), "1.1.2".into())]
        );
    }

    #[test]
    fn parse_npm_lock_registry_only_skipping_root_and_links() {
        let lock = r#"{"lockfileVersion":3,"packages":{
            "":{"name":"frontend","version":"0.0.0"},
            "node_modules/vite":{"version":"8.0.1",
                "resolved":"https://registry.npmjs.org/vite/-/vite-8.0.1.tgz"},
            "node_modules/vite/node_modules/esbuild":{"version":"0.24.0",
                "resolved":"https://registry.npmjs.org/esbuild/-/esbuild-0.24.0.tgz"},
            "node_modules/local":{"version":"1.0.0","link":true,
                "resolved":"../local"}
        }}"#;
        let mut pkgs = parse_npm_lock(lock).unwrap();
        pkgs.sort();
        // Root ("") and the local link (relative `resolved`)
        // dropped; nested name is the final path segment.
        assert_eq!(
            pkgs,
            vec![
                ("esbuild".into(), "0.24.0".into()),
                ("vite".into(), "8.0.1".into()),
            ]
        );
    }

    #[test]
    fn parse_npm_lock_refuses_bad_json_and_v1() {
        assert_eq!(parse_npm_lock("not json"), None);
        assert_eq!(parse_npm_lock("{}"), None);
        assert_eq!(parse_npm_lock(r#"{"packages": {}}"#), Some(Vec::new()));
    }

    #[test]
    fn new_locked_versions_detects_adds_and_bumps() {
        let old = vec![
            ("serde".to_string(), "1.0.100".to_string()),
            ("tokio".to_string(), "1.35.0".to_string()),
        ];
        let new = vec![
            ("serde".to_string(), "1.0.100".to_string()), // unchanged
            ("tokio".to_string(), "1.36.0".to_string()),  // bumped
            ("anyhow".to_string(), "1.0.80".to_string()), // added
        ];
        let mut got = new_locked_versions(&old, &new);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("anyhow".into(), "1.0.80".into()),
                ("tokio".into(), "1.36.0".into()),
            ]
        );
    }

    #[test]
    fn parse_allow_splits_trims_and_handles_none() {
        let a = parse_allow(Some(" serde@1.0.0 , anyhow@1.0.80 ,"));
        assert_eq!(a.len(), 2);
        assert!(a.contains("serde@1.0.0"));
        assert!(a.contains("anyhow@1.0.80"));
        let got = parse_allow(None);
        assert!(got.is_empty(), "{got:?}");
        let got = parse_allow(Some(""));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn the_allow_file_takes_one_pin_per_line_with_comments() {
        let a = parse_allow_file(
            "# security fixes still in the cooldown\n\
             ring@0.17.9   # RUSTSEC-2026-0042\n\
             \n   serde@1.0.999\n#old@1.0.0\n",
        );
        let mut a: Vec<_> = a.into_iter().collect();
        a.sort();
        assert_eq!(a, ["ring@0.17.9", "serde@1.0.999"]);
    }

    #[test]
    fn the_allow_file_counts_as_committed_at_the_base() {
        // A pin added in the working tree, or in a commit after the
        // base, does not count: in CI the base is main, so a change
        // cannot waive its own fresh dependency.
        let repo = crate::test_repo::with_lock(LOCK, true);
        let cfg = Config::for_project("t", repo.0.clone());
        let file = repo.0.join(&cfg.dep_age_allow_file);
        std::fs::write(&file, "ring@0.17.9 # fix\n").unwrap();
        crate::test_repo::git(&repo.0, &["add", "."]);
        crate::test_repo::git(&repo.0, &["commit", "-q", "-m", "waive"]);
        std::fs::write(&file, "ring@0.17.9\nserde@1.0.999\n").unwrap();
        let a = allowed(&cfg, "HEAD", Some("anyhow@1.0.80"));
        assert!(a.contains("ring@0.17.9"), "{a:?}");
        assert!(a.contains("anyhow@1.0.80"), "{a:?}");
        assert!(!a.contains("serde@1.0.999"), "{a:?}");
        // At the first commit the file did not exist: only the env.
        let a = allowed(&cfg, "HEAD~1", None);
        assert!(a.is_empty(), "{a:?}");
    }

    #[test]
    fn the_hint_names_the_file_and_the_variable() {
        let cfg = Config::for_project("my-app", "/r".into());
        let hint = allow_hint(&cfg);
        assert!(hint.contains(".dep-age-allow"), "{hint}");
        assert!(hint.contains("MY_APP_DEP_AGE_ALLOW=name@version"), "{hint}");
    }

    #[test]
    fn classify_dep_age_empty_is_clean() {
        let r = classify_dep_age(&[], "ALLOW");
        assert!(r.error.is_none());
        assert!(r.unchecked.is_empty(), "{:?}", r.unchecked);
        assert_eq!(r.detail, "no dependency changes");
    }

    #[test]
    fn classify_dep_age_fresh_is_fatal() {
        let r = classify_dep_age(
            &[
                ("a@1.0.0".into(), DepOutcome::Aged),
                (
                    "b@2.0.0".into(),
                    DepOutcome::Fresh("published 2d ago".into()),
                ),
            ],
            "allow via MY_ALLOW",
        );
        let err = r.error.unwrap();
        assert!(err.contains("b@2.0.0"));
        assert!(err.ends_with("; allow via MY_ALLOW"), "{err}");
        assert!(err.contains("published 2d ago"));
        assert!(r.detail.contains("1 fresh"));
        assert!(r.detail.contains("1 aged"));
    }

    #[test]
    fn classify_dep_age_unavailable_is_unchecked_not_fresh() {
        let r = classify_dep_age(
            &[(
                "a@1.0.0".into(),
                DepOutcome::Unavailable("network down".into()),
            )],
            "ALLOW",
        );
        assert!(r.error.is_none());
        assert_eq!(r.unchecked, vec!["a@1.0.0: network down".to_string()]);
        // The detail sub-counts reconcile with "1 changed".
        assert!(r.detail.contains("1 changed"));
        assert!(r.detail.contains("1 unchecked"));
    }

    #[test]
    fn classify_dep_age_allowed_counts_but_fresh_still_fatal() {
        let r = classify_dep_age(
            &[
                ("a@1.0.0".into(), DepOutcome::Allowed),
                (
                    "b@2.0.0".into(),
                    DepOutcome::Fresh("published 1d ago".into()),
                ),
            ],
            "ALLOW",
        );
        // The allow-listed one is not fatal; the fresh one is.
        assert!(r.error.is_some());
        assert!(r.detail.contains("1 allow-listed"));
    }
}
