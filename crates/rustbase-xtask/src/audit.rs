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
use std::time::Duration;

use serde_json::Value;

use crate::process::{self, Captured};
use crate::{Config, Strictness, is_missing_subcommand};

/// Parsed `cargo audit --json` result.
#[derive(Debug, PartialEq, Eq)]
struct CargoAudit {
    /// Number of vulnerabilities (fatal).
    vulnerabilities: u64,
    /// Number of cargo-audit's advisory warnings (unsound,
    /// unmaintained, yanked: informational, never fatal).
    warnings: u64,
    /// One line per RUSTSEC vulnerability, for the failure message.
    advisories: Vec<String>,
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
    /// One line per vulnerable npm package (a line can name several
    /// advisories), for the failure message.
    advisories: Vec<String>,
}

/// Combined audit outcome.
#[derive(Debug)]
struct AuditResult {
    /// Human-readable detail for the step line.
    detail: String,
    /// `Some` **only** on a positive vulnerability finding.
    error: Option<String>,
    /// Why a check could not finish (a tool missing, the network
    /// unreachable, an unparseable response) -- not cargo-audit's
    /// advisory warnings, which only reach `detail` as a count.
    warnings: Vec<String>,
    /// The cargo and npm advisory lines, listed under the error.
    advisories: Vec<String>,
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
        || process::cargo_within(&cfg.root, &["audit", "--json"], DEADLINE),
        |dir| process::npm_within(dir, &["audit", "--json"], DEADLINE),
    );
    verdict(r, strictness)
}

/// How long each audit may run. Both reach the network, and a host
/// that accepts the connection and never answers would otherwise hang
/// the gate; a run past this is a check that could not finish.
const DEADLINE: Duration = Duration::from_secs(180);

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
/// `cargo audit --json` stdout, and turn each entry of
/// `vulnerabilities.list` into one line (`rustsec_line`).
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
    let advisories = j["vulnerabilities"]["list"]
        .as_array()
        .map_or_else(Vec::new, |list| list.iter().map(rustsec_line).collect());
    Ok(CargoAudit {
        vulnerabilities,
        warnings,
        advisories,
    })
}

/// A JSON string field, or `?` when the tool left it out, made safe
/// for the terminal by `visible`.
fn text(v: &Value) -> String {
    visible(v.as_str().unwrap_or("?"))
}

/// Printable ASCII (space to `~`) passes through; every other
/// character becomes its escape (`\u{e9}`, `\n`). Advisory text comes
/// from outside and reaches the terminal and `target/validate.log`,
/// so no escape sequence, line break or bidi mark may move the
/// cursor, reorder a line or add one. An allow-list holds by
/// construction, where a list of risky characters fails on the one
/// it forgets.
fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if (' '..='~').contains(&c) {
            out.push(c);
        } else {
            out.extend(c.escape_default());
        }
    }
    out
}

/// `<advisory id> <crate> <version>: <title> (patched: <ranges>)`,
/// the id as cargo-audit gives it (`RUSTSEC-2021-0003`).
fn rustsec_line(v: &Value) -> String {
    let patched: Vec<String> = v["versions"]["patched"]
        .as_array()
        .map_or_else(Vec::new, |a| a.iter().map(text).collect());
    let patched = if patched.is_empty() {
        "no patched version".to_owned()
    } else {
        format!("patched: {}", patched.join("; "))
    };
    format!(
        "{} {} {}: {} ({patched})",
        text(&v["advisory"]["id"]),
        text(&v["package"]["name"]),
        text(&v["package"]["version"]),
        text(&v["advisory"]["title"]),
    )
}

/// The severity keys of npm's `metadata.vulnerabilities`.
const NPM_SEVERITIES: [&str; 5] =
    ["info", "low", "moderate", "high", "critical"];

/// Parse the counts in `metadata.vulnerabilities` from `npm audit
/// --json`, and turn each entry of the top-level `vulnerabilities`
/// map into one line (`npm_line`). npm exits non-zero when it finds
/// vulnerabilities but still writes the report, so the exit status
/// is not consulted.
fn parse_npm_audit(stdout: &str) -> Result<NpmAudit, String> {
    let j: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("failed to parse npm audit JSON: {e}"))?;
    let v = &j["metadata"]["vulnerabilities"];
    let get = |k: &str| v[k].as_u64().unwrap_or(0);
    // The larger of npm's total and its own severity counts, so a
    // report whose `total` is missing or 0 beside non-zero counts
    // still fails.
    let counted = NPM_SEVERITIES.iter().map(|k| get(k)).sum::<u64>();
    let has_counts = NPM_SEVERITIES.iter().any(|k| v[*k].is_u64());
    let total = match v["total"].as_u64() {
        Some(t) => t.max(counted),
        None if has_counts => counted,
        None => {
            return Err(
                "missing metadata.vulnerabilities counts in npm audit output"
                    .into(),
            );
        }
    };
    let advisories =
        j["vulnerabilities"].as_object().map_or_else(Vec::new, |o| {
            o.iter().map(|(name, e)| npm_line(name, e)).collect()
        });
    Ok(NpmAudit {
        total,
        high: get("high"),
        critical: get("critical"),
        advisories,
    })
}

/// `npm <package> <range> <severity>: <advisories> (<fix>)`. A `via`
/// entry is an advisory (its GHSA id and title) or, for a package that
/// is vulnerable only through a dependency, that dependency's name.
/// npm's advisory object has no id field: the GHSA id is the last
/// segment of its `url` (`.../advisories/GHSA-xvch-5gv4-984h`).
fn npm_line(name: &str, e: &Value) -> String {
    let name = visible(name);
    let via: Vec<String> = e["via"].as_array().map_or_else(Vec::new, |a| {
        a.iter()
            .map(|v| {
                if let Some(dep) = v.as_str() {
                    return format!("via {}", visible(dep));
                }
                let url = text(&v["url"]);
                let id =
                    url.rsplit_once('/').map_or(url.as_str(), |(_, id)| id);
                format!("{id} {}", text(&v["title"]))
            })
            .collect()
    });
    let fix = match &e["fixAvailable"] {
        Value::Bool(true) => "fix available".to_owned(),
        Value::Object(f) => format!(
            "fix: {}@{}{}",
            f.get("name").map_or_else(|| name.clone(), text),
            f.get("version").map_or_else(|| "?".to_owned(), text),
            if f.get("isSemVerMajor") == Some(&Value::Bool(true)) {
                ", a major bump"
            } else {
                ""
            }
        ),
        _ => "no fix".to_owned(),
    };
    let via = if via.is_empty() {
        String::new()
    } else {
        format!(": {}", via.join("; "))
    };
    format!(
        "npm {name} {} {}{via} ({fix})",
        text(&e["range"]),
        text(&e["severity"]),
    )
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
    let mut advisories: Vec<String> = Vec::new();

    match cargo {
        Ok(c) => {
            advisories.extend(c.advisories);
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
                advisories.extend(n.advisories);
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
        advisories,
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
        let mut msg = format!("{err} ({}){also}", r.detail);
        // Every advisory, uncapped, unlike other gates' diagnostics: on
        // a security finding each line is something to act on.
        for line in &r.advisories {
            msg.push_str("\n  ");
            msg.push_str(line);
        }
        return Err(msg);
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

    /// Trimmed from a real `cargo audit --json` run on smallvec 1.6.0.
    const CARGO_SMALLVEC: &str = r#"{"vulnerabilities":{"found":true,
        "count":1,"list":[{"advisory":{"id":"RUSTSEC-2021-0003",
        "package":"smallvec","title":"Buffer overflow in SmallVec::insert_many"},
        "versions":{"patched":[">=0.6.14, <1.0.0",">=1.6.1"]},
        "package":{"name":"smallvec","version":"1.6.0"}}]},"warnings":{}}"#;

    /// Trimmed from a real `npm audit --json` run on minimist 1.2.5;
    /// `optimist` follows the documented shape of a dependent that is
    /// vulnerable only through another package.
    const NPM_MINIMIST: &str = r#"{"vulnerabilities":{
        "minimist":{"name":"minimist","severity":"critical",
          "range":"1.0.0 - 1.2.5","fixAvailable":{"name":"minimist",
          "version":"1.2.8","isSemVerMajor":false},
          "via":[{"source":1097678,"title":"Prototype Pollution in minimist",
          "url":"https://github.com/advisories/GHSA-xvch-5gv4-984h",
          "severity":"critical","range":">=1.0.0 <1.2.6"}]},
        "optimist":{"name":"optimist","severity":"critical","range":"*",
          "fixAvailable":false,"via":["minimist"]}},
        "metadata":{"vulnerabilities":{"high":0,"critical":2,"total":2}}}"#;

    #[test]
    fn each_rustsec_advisory_becomes_one_line() {
        let a = parse_cargo_audit(CARGO_SMALLVEC).unwrap();
        assert_eq!(
            a.advisories,
            ["RUSTSEC-2021-0003 smallvec 1.6.0: Buffer overflow in \
              SmallVec::insert_many (patched: >=0.6.14, <1.0.0; >=1.6.1)"]
        );
    }

    #[test]
    fn each_npm_advisory_becomes_one_line() {
        let a = parse_npm_audit(NPM_MINIMIST).unwrap();
        assert_eq!(
            a.advisories,
            [
                "npm minimist 1.0.0 - 1.2.5 critical: GHSA-xvch-5gv4-984h \
                 Prototype Pollution in minimist (fix: minimist@1.2.8)",
                "npm optimist * critical: via minimist (no fix)",
            ]
        );
    }

    #[test]
    fn an_advisory_with_no_fix_says_so() {
        let v: Value = serde_json::from_str(
            r#"{"advisory":{"id":"RUSTSEC-1","title":"t"},
                "versions":{"patched":[]},
                "package":{"name":"c","version":"1.0.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            rustsec_line(&v),
            "RUSTSEC-1 c 1.0.0: t (no patched version)"
        );
    }

    #[test]
    fn every_npm_fix_and_via_shape_has_its_wording() {
        let line = |json: &str| {
            npm_line("p", &serde_json::from_str::<Value>(json).unwrap())
        };
        let base = r#""range":"<2","severity":"high""#;
        assert_eq!(
            line(&format!(r#"{{{base},"via":["q"],"fixAvailable":true}}"#)),
            "npm p <2 high: via q (fix available)"
        );
        assert_eq!(
            line(&format!(
                r#"{{{base},"via":["q"],"fixAvailable":{{"name":"p",
                "version":"2.0.0","isSemVerMajor":true}}}}"#
            )),
            "npm p <2 high: via q (fix: p@2.0.0, a major bump)"
        );
        // A `via` advisory with no URL still shows its title.
        assert_eq!(
            line(&format!(r#"{{{base},"via":[{{"title":"t"}}]}}"#)),
            "npm p <2 high: ? t (no fix)"
        );
        // No `via` at all leaves no stray space before the fix.
        assert_eq!(
            line(&format!(r#"{{{base},"via":[]}}"#)),
            "npm p <2 high (no fix)"
        );
    }

    #[test]
    fn control_characters_in_advisory_text_stay_on_one_visible_line() {
        // External text must not move the cursor, recolour, or add a
        // fake line to the listing the operator acts on.
        let v: Value = serde_json::from_str(
            r#"{"advisory":{"id":"RUSTSEC-1",
                "title":"x\u001b[2K\r  RUSTSEC-0 ok\nmore"},
                "versions":{"patched":[]},
                "package":{"name":"c","version":"1"}}"#,
        )
        .unwrap();
        let line = rustsec_line(&v);
        assert!(!line.chars().any(char::is_control), "{line:?}");
        assert!(line.contains(r"x\u{1b}[2K\r  RUSTSEC-0 ok\nmore"), "{line}");
        let e: Value = serde_json::from_str(
            r#"{"range":"*","severity":"high","via":["d\u001bep"],
                "fixAvailable":{"name":"p\nq","version":"2"}}"#,
        )
        .unwrap();
        let line = npm_line("n\ra", &e);
        assert!(!line.chars().any(char::is_control), "{line:?}");
        // A bidi override would reverse what follows on the line (the
        // patched range); a line separator breaks it in some editors.
        let line = visible("a\u{202e}b\u{2028}c\u{2066}d");
        assert_eq!(line, r"a\u{202e}b\u{2028}c\u{2066}d");
        // Only printable ASCII passes, so no list of risky characters
        // can miss one (U+061C, the Arabic letter mark, is one a list
        // of risky characters easily leaves out).
        assert_eq!(visible("caf\u{e9}\u{61c}1.2"), r"caf\u{e9}\u{61c}1.2");
        assert_eq!(
            visible(r#">=1.6.1, <2 "q" \x ~"#),
            r#">=1.6.1, <2 "q" \x ~"#
        );
    }

    #[test]
    fn a_finding_lists_every_advisory_in_full() {
        let r = classify(
            parse_cargo_audit(CARGO_SMALLVEC),
            Some(parse_npm_audit(NPM_MINIMIST)),
        );
        let err = verdict(r, Strictness::Fail).unwrap_err();
        let lines: Vec<&str> = err.lines().collect();
        assert!(lines[0].starts_with("vulnerabilities found:"), "{err}");
        assert_eq!(lines.len(), 4, "{err}");
        assert!(
            lines[1].starts_with("  RUSTSEC-2021-0003 smallvec"),
            "{err}"
        );
        assert!(lines[3].starts_with("  npm optimist"), "{err}");
    }

    #[test]
    fn parse_cargo_audit_clean() {
        assert_eq!(
            parse_cargo_audit(CLEAN_CARGO).unwrap(),
            CargoAudit {
                vulnerabilities: 0,
                warnings: 0,
                advisories: Vec::new(),
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
                critical: 0,
                advisories: Vec::new(),
            }
        );
    }

    #[test]
    fn parse_npm_audit_with_vulns() {
        let r = parse_npm_audit(NPM_THREE).unwrap();
        assert_eq!((r.total, r.high, r.critical), (3, 2, 1));
    }

    #[test]
    fn a_zero_npm_total_with_severity_counts_is_not_clean() {
        // The verdict must not rest on one field: a report whose
        // `total` disagrees with its own severity counts fails.
        let json = r#"{"metadata":{"vulnerabilities":
            {"low":1,"moderate":1,"high":1,"critical":0,"total":0}}}"#;
        assert_eq!(parse_npm_audit(json).unwrap().total, 3);
    }

    #[test]
    fn a_missing_npm_total_is_the_sum_of_the_severities() {
        let json = r#"{"metadata":{"vulnerabilities":{"high":2}}}"#;
        assert_eq!(parse_npm_audit(json).unwrap().total, 2);
    }

    #[test]
    fn parse_npm_audit_rejects_no_counts_and_garbage() {
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
            advisories: Vec::new(),
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
                advisories: Vec::new(),
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
                advisories: Vec::new(),
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
                advisories: Vec::new(),
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
            advisories: Vec::new(),
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
                advisories: Vec::new(),
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
                advisories: Vec::new(),
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
