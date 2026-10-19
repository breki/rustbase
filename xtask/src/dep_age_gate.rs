//! The `validate` step for the shared library's cooldown gate.

use rustbase_xtask::dep_age;

use crate::helpers::tasks_config;
use crate::validate::Step;

/// Dep-age step -- cooldown-checks only dependencies added or
/// bumped in the working tree versus HEAD. A within-cooldown
/// dependency fails the gate. A check that could not finish
/// (missing baseline, unreachable registry, unreadable
/// `Cargo.lock`) warns, so an offline run is not blocked --
/// unless `dep_age_strict` says to fail.
pub fn step() -> Step {
    Step::new("Dep-age", "dep-age-check", || {
        let strict = dep_age_strict(|k| std::env::var(k).ok());
        let cfg = tasks_config();
        dep_age_verdict(dep_age::check_changed_deps(&cfg, "HEAD"), strict)
    })
}

/// The step's result: a fresh dependency always fails; a check
/// that could not finish fails only when `strict`.
fn dep_age_verdict(
    r: dep_age::DepAgeResult,
    strict: bool,
) -> Result<String, String> {
    if let Some(err) = r.error {
        return Err(err);
    }
    if strict && !r.warnings.is_empty() {
        return Err(format!(
            "dep-age could not complete: {}",
            r.warnings.join("; ")
        ));
    }
    for w in &r.warnings {
        eprintln!("  warning: {w}");
    }
    Ok(r.detail)
}

/// Strict when `RUSTBASE_DEP_AGE_STRICT=1` or under CI
/// (`CI=true`, which GitHub Actions sets). `env` is injected so
/// the tests need no process environment.
fn dep_age_strict(env: impl Fn(&str) -> Option<String>) -> bool {
    env("RUSTBASE_DEP_AGE_STRICT").as_deref() == Some("1")
        || env("CI").as_deref() == Some("true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_age_warnings_fail_only_when_strict() {
        let incomplete = || dep_age::DepAgeResult {
            detail: "0 changed".into(),
            error: None,
            warnings: vec!["registry unreachable".into()],
        };
        assert!(dep_age_verdict(incomplete(), false).is_ok());
        let err = dep_age_verdict(incomplete(), true).unwrap_err();
        assert!(err.contains("registry unreachable"), "{err}");
    }

    #[test]
    fn dep_age_strictness_comes_from_ci_or_opt_in() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| (*v).into())
            }
        };
        assert!(!dep_age_strict(env(&[])));
        assert!(dep_age_strict(env(&[("CI", "true")])));
        assert!(dep_age_strict(env(&[("RUSTBASE_DEP_AGE_STRICT", "1")])));
        assert!(!dep_age_strict(env(&[("RUSTBASE_DEP_AGE_STRICT", "0")])));
        assert!(!dep_age_strict(env(&[("CI", "false")])));
    }
}
