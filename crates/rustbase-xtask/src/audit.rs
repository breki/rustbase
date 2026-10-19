//! The security-advisory gate.
//!
//! Runs `cargo audit` (RUSTSEC) over `Cargo.lock` and, when an npm
//! project is given, `npm audit` over it, and fails on any
//! *vulnerability*. Advisory *warnings* (unsound / unmaintained /
//! yanked) are reported but do not fail the gate: they are
//! informational, not exploitable defects.
//!
//! Both tools reach the network. A vulnerability found is always
//! fatal; a check that could not finish (a tool missing, the
//! advisory DB unreachable) fails or warns by the caller's
//! [`Strictness`].

use std::path::Path;

use serde_json::Value;

use crate::process::{self, Captured};
use crate::{Config, Strictness, is_missing_subcommand};

/// Parsed `cargo audit --json` result.
#[derive(Debug, PartialEq, Eq)]
struct CargoAudit {
    /// Number of vulnerabilities (fatal).
    vulnerabilities: u64,
    /// Number of advisory warnings (informational).
    warnings: u64,
}

/// Parsed `npm audit --json` result.
#[derive(Debug, PartialEq, Eq)]
struct NpmAudit {
    /// Total vulnerabilities across all severities.
    total: u64,
    /// High-severity count (for the summary line).
    high: u64,
    /// Critical-severity count (for the summary line).
    critical: u64,
}

/// Combined audit outcome.
#[derive(Debug)]
struct AuditResult {
    /// Human-readable detail for the step line.
    detail: String,
    /// `Some` **only** on a positive vulnerability finding.
    error: Option<String>,
    /// Why a check could not finish (a tool missing, the network
    /// unreachable, an unparseable response).
    warnings: Vec<String>,
}

/// Runs the gate: the detail line on success, else the error.
/// Under [`Strictness::Warn`] a check that could not finish is
/// printed to stderr as a warning and the gate passes.
///
/// `npm_dir` is audited only when it holds a `package.json`.
///
/// # Errors
///
/// On a vulnerability, or under [`Strictness::Fail`] when a check
/// could not finish. Each error names everything that went wrong.
pub fn check(
    cfg: &Config,
    npm_dir: Option<&Path>,
    strictness: Strictness,
) -> Result<String, String> {
    let r = collect(
        npm_dir,
        || process::cargo(&cfg.root, &["audit", "--json"]),
        |dir| process::npm(dir, &["audit", "--json"]),
    );
    verdict(r, strictness)
}

/// Run both audits through the injected spawns and classify the
/// results, without deciding what a check that could not finish
/// means.
fn collect(
    npm_dir: Option<&Path>,
    cargo: impl FnOnce() -> Result<Captured, String>,
    npm: impl FnOnce(&Path) -> Result<Captured, String>,
) -> AuditResult {
    let cargo = cargo().and_then(|out| read_cargo_audit(&out));
    let npm = npm_dir
        .filter(|d| d.join("package.json").is_file())
        .map(|d| npm(d).and_then(|out| parse_npm_audit(&out.stdout)));
    classify(cargo, npm)
}

/// Read a finished `cargo audit --json`, telling "cargo-audit not
/// installed" (an actionable hint) apart from a parse failure.
fn read_cargo_audit(out: &Captured) -> Result<CargoAudit, String> {
    if out.stdout.trim().is_empty() {
        if is_missing_subcommand(&out.stderr) {
            return Err("cargo-audit is not installed -- \
                 install with: cargo install cargo-audit"
                .into());
        }
        return Err(format!("cargo audit produced no output:\n{}", out.stderr));
    }
    parse_cargo_audit(&out.stdout)
}

/// Parse the `vulnerabilities.count` and total `warnings` from
/// `cargo audit --json` stdout.
fn parse_cargo_audit(stdout: &str) -> Result<CargoAudit, String> {
    let j: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("failed to parse cargo audit JSON: {e}"))?;
    let vulnerabilities = j["vulnerabilities"]["count"]
        .as_u64()
        .ok_or("missing vulnerabilities.count in cargo audit output")?;
    // `warnings` is an object keyed by kind (unsound,
    // unmaintained, yanked, ...), each a list. Sum the lists.
    let warnings = j["warnings"].as_object().map_or(0, |o| {
        o.values()
            .map(|v| v.as_array().map_or(0, Vec::len) as u64)
            .sum()
    });
    Ok(CargoAudit {
        vulnerabilities,
        warnings,
    })
}

/// Parse `metadata.vulnerabilities` from `npm audit --json`. npm
/// exits non-zero when it finds vulnerabilities but still writes
/// the report, so the exit status is not consulted.
fn parse_npm_audit(stdout: &str) -> Result<NpmAudit, String> {
    let j: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("failed to parse npm audit JSON: {e}"))?;
    let v = &j["metadata"]["vulnerabilities"];
    let get = |k: &str| v[k].as_u64().unwrap_or(0);
    let total = v["total"]
        .as_u64()
        .ok_or("missing metadata.vulnerabilities.total in npm audit output")?;
    Ok(NpmAudit {
        total,
        high: get("high"),
        critical: get("critical"),
    })
}

/// Classify per-source outcomes: a positive vulnerability count is
/// the `error`. Any `Err`, from the spawn or from parsing its
/// output, means that check did not finish and goes in `warnings`,
/// which are not cargo-audit's advisory warnings.
fn classify(
    cargo: Result<CargoAudit, String>,
    npm: Option<Result<NpmAudit, String>>,
) -> AuditResult {
    let mut parts: Vec<String> = Vec::new();
    let mut fatal: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    match cargo {
        Ok(c) => {
            if c.vulnerabilities > 0 {
                fatal.push(format!("{} RUSTSEC", c.vulnerabilities));
            }
            parts.push(format!(
                "cargo: {} vuln, {} warn",
                c.vulnerabilities, c.warnings
            ));
        }
        Err(reason) => {
            warnings.push(format!("cargo audit unavailable: {reason}"));
            parts.push("cargo: unavailable".into());
        }
    }

    if let Some(npm) = npm {
        match npm {
            Ok(n) => {
                if n.total > 0 {
                    fatal.push(format!(
                        "{} npm ({} high, {} critical)",
                        n.total, n.high, n.critical
                    ));
                }
                parts.push(format!("npm: {} vuln", n.total));
            }
            Err(reason) => {
                warnings.push(format!("npm audit unavailable: {reason}"));
                parts.push("npm: unavailable".into());
            }
        }
    }

    AuditResult {
        detail: parts.join(", "),
        error: if fatal.is_empty() {
            None
        } else {
            Some(format!("vulnerabilities found: {}", fatal.join("; ")))
        },
        warnings,
    }
}

/// Apply `strictness` to a result: a finding always fails;
/// warnings fail only under [`Strictness::Fail`].
fn verdict(r: AuditResult, strictness: Strictness) -> Result<String, String> {
    let unfinished = r.warnings.join("; ");
    if let Some(err) = r.error {
        let also = if unfinished.is_empty() {
            String::new()
        } else {
            format!("; also: {unfinished}")
        };
        return Err(format!("{err} ({}){also}", r.detail));
    }
    if r.warnings.is_empty() {
        return Ok(r.detail);
    }
    if strictness == Strictness::Fail {
        return Err(format!("audit could not complete: {unfinished}"));
    }
    for w in &r.warnings {
        eprintln!("  warning: {w}");
    }
    Ok(r.detail)
}

#[cfg(test)]
mod tests {
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

    const CLEAN_CARGO: &str =
        r#"{"vulnerabilities":{"found":false,"count":0},"warnings":{}}"#;
    const NPM_THREE: &str = r#"{"metadata":{"vulnerabilities":
        {"high":2,"critical":1,"total":3}}}"#;

    #[test]
    fn parse_cargo_audit_clean() {
        assert_eq!(
            parse_cargo_audit(CLEAN_CARGO).unwrap(),
            CargoAudit {
                vulnerabilities: 0,
                warnings: 0
            }
        );
    }

    #[test]
    fn parse_cargo_audit_counts_warnings_across_kinds() {
        let json = r#"{"vulnerabilities":{"found":false,"count":0},
            "warnings":{"unsound":[{"a":1}],
            "unmaintained":[{"b":2},{"c":3}]}}"#;
        let r = parse_cargo_audit(json).unwrap();
        assert_eq!(r.vulnerabilities, 0);
        assert_eq!(r.warnings, 3);
    }

    #[test]
    fn parse_cargo_audit_with_vulns() {
        let json = r#"{"vulnerabilities":{"found":true,"count":2},
            "warnings":{}}"#;
        assert_eq!(parse_cargo_audit(json).unwrap().vulnerabilities, 2);
    }

    #[test]
    fn parse_cargo_audit_rejects_garbage() {
        assert!(parse_cargo_audit("not json").is_err());
        assert!(parse_cargo_audit("{}").is_err());
    }

    #[test]
    fn parse_npm_audit_clean() {
        let json = r#"{"metadata":{"vulnerabilities":
            {"info":0,"low":0,"moderate":0,"high":0,
             "critical":0,"total":0}}}"#;
        assert_eq!(
            parse_npm_audit(json).unwrap(),
            NpmAudit {
                total: 0,
                high: 0,
                critical: 0
            }
        );
    }

    #[test]
    fn parse_npm_audit_with_vulns() {
        let r = parse_npm_audit(NPM_THREE).unwrap();
        assert_eq!((r.total, r.high, r.critical), (3, 2, 1));
    }

    #[test]
    fn parse_npm_audit_rejects_missing_total_and_garbage() {
        let json = r#"{"metadata":{"vulnerabilities":{}}}"#;
        assert!(parse_npm_audit(json).is_err());
        assert!(parse_npm_audit("not json").is_err());
    }

    #[test]
    fn a_missing_cargo_audit_says_how_to_install_it() {
        let out = captured(false, "", "error: no such command: `audit`");
        let e = read_cargo_audit(&out).unwrap_err();
        assert!(e.contains("cargo install cargo-audit"), "{e}");
    }

    #[test]
    fn a_silent_cargo_audit_failure_carries_its_stderr() {
        let out = captured(false, "  ", "couldn't fetch advisory database");
        let e = read_cargo_audit(&out).unwrap_err();
        assert!(e.contains("no output"), "{e}");
        assert!(e.contains("advisory database"), "{e}");
    }

    fn clean_cargo() -> CargoAudit {
        CargoAudit {
            vulnerabilities: 0,
            warnings: 1,
        }
    }

    #[test]
    fn classify_clean_no_error_no_warnings() {
        let r = classify(Ok(clean_cargo()), None);
        assert!(r.error.is_none());
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert!(r.detail.contains("cargo: 0 vuln, 1 warn"));
    }

    #[test]
    fn classify_cargo_vuln_is_fatal() {
        let r = classify(
            Ok(CargoAudit {
                vulnerabilities: 2,
                warnings: 0,
            }),
            None,
        );
        assert!(r.error.as_ref().unwrap().contains("2 RUSTSEC"));
    }

    #[test]
    fn classify_npm_vuln_is_fatal() {
        let r = classify(
            Ok(clean_cargo()),
            Some(Ok(NpmAudit {
                total: 3,
                high: 2,
                critical: 1,
            })),
        );
        assert!(r.error.as_ref().unwrap().contains("3 npm"));
    }

    #[test]
    fn classify_unavailable_is_warning_not_fatal() {
        // A runner Err (network/tool) is not a finding: whether it
        // fails is the caller's strictness, not this function's.
        let r = classify(
            Err("network down".into()),
            Some(Err("registry unreachable".into())),
        );
        assert!(r.error.is_none());
        assert_eq!(r.warnings.len(), 2);
        assert!(r.detail.contains("cargo: unavailable"));
        assert!(r.detail.contains("npm: unavailable"));
    }

    #[test]
    fn classify_vuln_fatal_even_with_other_source_unavailable() {
        let r = classify(
            Ok(CargoAudit {
                vulnerabilities: 1,
                warnings: 0,
            }),
            Some(Err("registry unreachable".into())),
        );
        assert!(r.error.is_some());
        assert_eq!(r.warnings.len(), 1);
    }

    fn incomplete() -> AuditResult {
        AuditResult {
            detail: "cargo: unavailable".into(),
            error: None,
            warnings: vec!["cargo audit unavailable: offline".into()],
        }
    }

    #[test]
    fn an_unfinished_check_fails_only_when_strict() {
        assert_eq!(
            verdict(incomplete(), Strictness::Warn).unwrap(),
            "cargo: unavailable"
        );
        let e = verdict(incomplete(), Strictness::Fail).unwrap_err();
        assert!(e.contains("could not complete"), "{e}");
    }

    #[test]
    fn a_finding_fails_at_either_strictness() {
        for s in [Strictness::Warn, Strictness::Fail] {
            let r = AuditResult {
                detail: "cargo: 1 vuln, 0 warn".into(),
                error: Some("vulnerabilities found: 1 RUSTSEC".into()),
                warnings: Vec::new(),
            };
            assert!(verdict(r, s).unwrap_err().contains("1 RUSTSEC"));
        }
    }

    #[test]
    fn a_strict_failure_names_why_the_check_did_not_finish() {
        // The error stands alone: a caller that does not forward
        // stderr still learns the reason.
        let e = verdict(incomplete(), Strictness::Fail).unwrap_err();
        assert!(e.contains("offline"), "{e}");
    }

    #[test]
    fn a_finding_also_reports_a_source_that_did_not_finish() {
        let r = classify(
            Ok(CargoAudit {
                vulnerabilities: 1,
                warnings: 0,
            }),
            Some(Err("registry unreachable".into())),
        );
        let e = verdict(r, Strictness::Warn).unwrap_err();
        assert!(e.contains("1 RUSTSEC"), "{e}");
        assert!(e.contains("registry unreachable"), "{e}");
    }

    #[test]
    fn a_clean_run_passes_when_strict() {
        let r = classify(Ok(clean_cargo()), None);
        assert!(verdict(r, Strictness::Fail).is_ok());
    }

    #[test]
    fn npm_is_audited_only_where_a_package_json_is() {
        let dir = test_repo::empty_dir();
        let unreachable = |_: &Path| -> Result<Captured, String> {
            panic!("npm must not run without a package.json")
        };
        let cargo = || Ok(captured(true, CLEAN_CARGO, ""));
        let r = collect(Some(&dir.0), cargo, unreachable);
        assert!(!r.detail.contains("npm"), "{}", r.detail);
        let r = collect(None, cargo, unreachable);
        assert!(!r.detail.contains("npm"), "{}", r.detail);

        std::fs::write(dir.0.join("package.json"), "{}").unwrap();
        let mut ran_in = None;
        let r = collect(Some(&dir.0), cargo, |d| {
            ran_in = Some(d.to_path_buf());
            // npm exits non-zero on a finding; its report counts.
            Ok(captured(false, NPM_THREE, ""))
        });
        assert_eq!(ran_in.as_deref(), Some(dir.0.as_path()));
        assert!(r.error.unwrap().contains("3 npm"));
    }

    #[test]
    fn a_cargo_spawn_failure_is_a_warning() {
        let r = collect(
            None,
            || Err("failed to run cargo".into()),
            |_| unreachable!(),
        );
        assert!(r.error.is_none());
        assert!(r.warnings[0].contains("failed to run cargo"));
    }
}
