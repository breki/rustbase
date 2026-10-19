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
//! see `RUSTBASE_DEP_AGE_ALLOW` and the supply-chain notes in
//! `CLAUDE.md` for the intended workflow.
//!
//! The shared registry-fetch and date-verdict machinery lives
//! in the parent [`super`] module; this submodule imports it.

use std::collections::HashSet;
use std::fs;

use serde_json::Value;

use super::io::git_show;
use super::{
    COOLDOWN_DAYS, Ecosystem, age_in_days, cargo_version_date, fetch_registry,
    npm_version_date, parse_iso_date, today_days,
};
use crate::Config;

/// Combined outcome of the changed-deps gate, shaped like the
/// template's `AuditResult` so `validate` treats the two
/// network gates the same way (fatal error vs non-fatal
/// warnings that degrade an offline run rather than block it).
pub struct DepAgeResult {
    /// Human-readable detail for the step line.
    pub detail: String,
    /// `Some` only when a changed dependency is within the
    /// cooldown (always fatal).
    pub error: Option<String>,
    /// Non-fatal problems (missing baseline, unreachable
    /// registry, unparseable response).
    pub warnings: Vec<String>,
}

/// Per-dependency check outcome.
#[derive(Debug, PartialEq, Eq)]
enum DepOutcome {
    /// Older than the cooldown -- fine.
    Aged,
    /// Within the cooldown window (fatal); carries the reason.
    Fresh(String),
    /// Could not be checked (network / tool / parse) -- a
    /// non-fatal warning, mirroring the audit gate offline.
    Unavailable(String),
    /// Named in the allow env var -- skipped, never fetched.
    Allowed,
}

/// Fold per-dependency outcomes into the combined result.
/// Pure, so the fatal-vs-warning boundary is unit-tested.
fn classify_dep_age(
    outcomes: &[(String, DepOutcome)],
    allow_env: &str,
) -> DepAgeResult {
    if outcomes.is_empty() {
        return DepAgeResult {
            detail: "no dependency changes".into(),
            error: None,
            warnings: Vec::new(),
        };
    }
    let mut fresh: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let (mut aged, mut allowed) = (0usize, 0usize);
    for (label, outcome) in outcomes {
        match outcome {
            DepOutcome::Aged => aged += 1,
            DepOutcome::Allowed => allowed += 1,
            DepOutcome::Fresh(why) => fresh.push(format!("{label} ({why})")),
            DepOutcome::Unavailable(reason) => {
                warnings.push(format!("{label}: {reason}"));
            }
        }
    }
    // `warnings.len()` is the count of Unavailable outcomes.
    // Including it keeps the sub-counts reconciling with the
    // "N changed" total in the degraded-network case (where
    // nothing was actually checked).
    let detail = format!(
        "{} changed: {aged} aged, {allowed} allow-listed, {} fresh, \
         {} unchecked",
        outcomes.len(),
        fresh.len(),
        warnings.len(),
    );
    let error = if fresh.is_empty() {
        None
    } else {
        Some(format!(
            "dependencies within the {COOLDOWN_DAYS}-day cooldown: {}\n  \
             adopt only with a stated justification (security fixes \
             exempt); allow via {allow_env}=name@version[,...]",
            fresh.join("; ")
        ))
    };
    DepAgeResult {
        detail,
        error,
        warnings,
    }
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
/// check, so including them would spuriously warn on every
/// release commit.
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
/// have no npm-registry publish date to check. An unparseable
/// / older-format lockfile yields an empty list (nothing to
/// check).
fn parse_npm_lock(text: &str) -> Vec<(String, String)> {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(packages) = json["packages"].as_object() else {
        return Vec::new();
    };
    packages
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
        .collect()
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
    parser: fn(&str) -> Vec<(String, String)>,
}

const CARGO_LOCK: Lockfile = Lockfile {
    eco: Ecosystem::Cargo,
    rel: "Cargo.lock",
    required: true,
    parser: parse_cargo_lock,
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
/// required one is a warning; a missing `base` baseline with
/// the file present is surfaced as a warning (can't diff).
fn collect_changes(
    cfg: &Config,
    base: &str,
    lock: &Lockfile,
    allow: &HashSet<String>,
    outcomes: &mut Vec<(String, DepOutcome)>,
    warnings: &mut Vec<String>,
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
            warnings.push(format!(
                "{rel}: cannot read ({e}) -- skipping the cooldown check"
            ));
            return;
        }
    };
    let Some(baseline_text) = git_show(&cfg.root, base, rel) else {
        warnings.push(format!(
            "{rel}: no {base} baseline to diff against -- skipping \
             cooldown check for this lockfile"
        ));
        return;
    };
    let mut changed =
        new_locked_versions(&parser(&baseline_text), &parser(&current_text));
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
    let allow =
        parse_allow(std::env::var(&cfg.dep_age_allow_env).ok().as_deref());
    let mut outcomes: Vec<(String, DepOutcome)> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    collect_changes(
        cfg,
        base,
        &CARGO_LOCK,
        &allow,
        &mut outcomes,
        &mut warnings,
    );
    collect_changes(cfg, base, &NPM_LOCK, &allow, &mut outcomes, &mut warnings);

    let mut result = classify_dep_age(&outcomes, &cfg.dep_age_allow_env);
    // Baseline/parse warnings precede the per-dep warnings the
    // classifier collected.
    warnings.extend(result.warnings);
    result.warnings = warnings;
    result
}

/// Standalone `cargo xtask dep-age-check`. Like the audit
/// command, this errors when the check could not complete
/// (a human who ran it explicitly wants to know), whereas the
/// `validate` step degrades that to a warning. `base` is the
/// commit to diff against: `HEAD` locally; in CI, the commit the
/// push or pull request started from.
pub fn dep_age_check(cfg: &Config, base: &str) -> Result<(), String> {
    check_base_ref(base)?;
    let r = check_changed_deps(cfg, base);
    if let Some(err) = r.error {
        eprintln!("  {}", r.detail);
        return Err(err);
    }
    for w in &r.warnings {
        eprintln!("  warning: {w}");
    }
    if r.warnings.is_empty() {
        println!("Dep-age OK ({})", r.detail);
        Ok(())
    } else {
        Err("dep-age check could not complete (see warnings above)".into())
    }
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
    fn a_missing_required_lockfile_is_a_warning_not_a_pass() {
        // An empty outcome list reads as "no dependency changes",
        // so only the warning keeps "could not check" from passing.
        let (mut outcomes, mut warnings) = (Vec::new(), Vec::new());
        collect_changes(
            &test_cfg(),
            "HEAD",
            &Lockfile {
                rel: "no-such-file.lock",
                ..CARGO_LOCK
            },
            &HashSet::new(),
            &mut outcomes,
            &mut warnings,
        );
        assert!(outcomes.is_empty(), "{outcomes:?}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("cannot read"), "{warnings:?}");
    }

    #[test]
    fn a_missing_optional_lockfile_means_no_such_ecosystem() {
        let (mut outcomes, mut warnings) = (Vec::new(), Vec::new());
        collect_changes(
            &test_cfg(),
            "HEAD",
            &Lockfile {
                rel: "no-such-frontend/package-lock.json",
                ..NPM_LOCK
            },
            &HashSet::new(),
            &mut outcomes,
            &mut warnings,
        );
        assert!(outcomes.is_empty() && warnings.is_empty(), "{warnings:?}");
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
            r.error.is_none() && r.warnings.is_empty(),
            "{:?}",
            r.warnings
        );
        assert_eq!(r.detail, "no dependency changes");
        assert!(dep_age_check(&cfg, "HEAD").is_ok());
    }

    #[test]
    fn a_lockfile_with_no_baseline_cannot_pass_the_standalone_check() {
        let repo = crate::test_repo::with_lock(LOCK, false);
        let cfg = Config::for_project("t", repo.0.clone());
        let r = check_changed_deps(&cfg, "HEAD");
        assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
        assert!(
            r.warnings[0].contains("no HEAD baseline"),
            "{:?}",
            r.warnings
        );
        let err = dep_age_check(&cfg, "HEAD").unwrap_err();
        assert!(err.contains("could not complete"), "{err}");
        assert!(dep_age_check(&cfg, "-p").is_err());
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
        let mut pkgs = parse_npm_lock(lock);
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
    fn parse_npm_lock_bad_json_is_empty() {
        let got = parse_npm_lock("not json");
        assert!(got.is_empty(), "{got:?}");
        let got = parse_npm_lock("{}");
        assert!(got.is_empty(), "{got:?}");
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
    fn classify_dep_age_empty_is_clean() {
        let r = classify_dep_age(&[], "ALLOW");
        assert!(r.error.is_none());
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
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
            "MY_ALLOW",
        );
        let err = r.error.unwrap();
        assert!(err.contains("b@2.0.0"));
        assert!(err.contains("MY_ALLOW=name@version"), "{err}");
        assert!(err.contains("published 2d ago"));
        assert!(r.detail.contains("1 fresh"));
        assert!(r.detail.contains("1 aged"));
    }

    #[test]
    fn classify_dep_age_unavailable_is_warning_not_fatal() {
        let r = classify_dep_age(
            &[(
                "a@1.0.0".into(),
                DepOutcome::Unavailable("network down".into()),
            )],
            "ALLOW",
        );
        assert!(r.error.is_none());
        assert_eq!(r.warnings, vec!["a@1.0.0: network down".to_string()]);
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
