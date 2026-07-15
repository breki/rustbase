//! `cargo xtask dep-age <npm|cargo> <package> [version]` --
//! a dependency-cooldown helper.
//!
//! Reports how many days ago a package version was published
//! and exits non-zero when it is younger than the cooldown
//! (default 14 days) -- the window in which a compromised or
//! malicious release is most likely still live. Security
//! fixes are the operator's judgement call to override.
//!
//! Two entry points share the fetch + verdict machinery:
//!
//! - [`dep_age`] -- the on-demand `cargo xtask dep-age
//!   <npm|cargo> <pkg> [version]` query for a single package.
//! - [`check_changed_deps`] -- the `validate` gate. It checks
//!   only the `(name, version)` pairs newly present in the
//!   working-tree lockfiles versus `HEAD`, so it costs nothing
//!   (no network) on the common commit that leaves the
//!   lockfiles untouched, and fires exactly at the moment a
//!   dependency is adopted. A *whole-tree* continuous gate is
//!   deliberately avoided -- it would flag every already-locked
//!   version on every routine update; the changed-deps scope
//!   is what makes an automatic gate tolerable. Its
//!   implementation lives in the [`gate`] submodule.
//!
//! Registry queries shell out to `curl` (avoids adding an
//! HTTP stack to xtask); the date math is dependency-free so
//! the parsing/aging logic stays unit-tested.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;
use serde_json::Value;

mod gate;

pub use gate::{check_changed_deps, dep_age_check};

/// Do not adopt a version published fewer than this many
/// days ago without a stated justification.
const COOLDOWN_DAYS: i64 = 14;

/// Package registry to query. clap renders the variants as
/// `npm` / `cargo` and rejects anything else at the CLI
/// boundary, so there is no runtime string validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Ecosystem {
    Npm,
    Cargo,
}

/// Run the `dep-age` command.
pub fn dep_age(
    ecosystem: Ecosystem,
    package: &str,
    version: Option<&str>,
) -> Result<(), String> {
    let json = fetch_registry(ecosystem, package)?;
    let (resolved, published) = match ecosystem {
        Ecosystem::Npm => npm_version_date(&json, version)?,
        Ecosystem::Cargo => cargo_version_date(&json, version)?,
    };

    let published_days = parse_iso_date(&published)?;
    let age = age_in_days(published_days, today_days());
    let msg = format!(
        "{package}@{resolved} was published {age} day(s) ago ({published})"
    );

    match cooldown_verdict(age, &msg) {
        Ok(ok) => {
            println!("{ok}");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Decide the cooldown verdict for a version of a given age.
/// Pure, so both branches and the boundary are unit-tested.
fn cooldown_verdict(age: i64, msg: &str) -> Result<String, String> {
    if age < COOLDOWN_DAYS {
        Err(format!(
            "{msg}\n  within the {COOLDOWN_DAYS}-day cooldown -- adopt only \
             with a stated justification (security fixes exempt)"
        ))
    } else {
        Ok(format!(
            "{msg}\n  older than the {COOLDOWN_DAYS}-day cooldown -- OK"
        ))
    }
}

/// Fetch the registry metadata JSON for a package via curl.
fn fetch_registry(
    ecosystem: Ecosystem,
    package: &str,
) -> Result<Value, String> {
    let url = match ecosystem {
        Ecosystem::Npm => format!("https://registry.npmjs.org/{package}"),
        Ecosystem::Cargo => {
            format!("https://crates.io/api/v1/crates/{package}")
        }
    };
    let output = Command::new("curl")
        // crates.io rejects requests without a User-Agent.
        // Bounded timeouts so a reachable-but-hanging registry
        // cannot stall the `validate` gate indefinitely: a
        // timeout is a non-zero exit -> Unavailable -> a
        // non-fatal warning, the intended offline degrade.
        .args([
            "-sSfL",
            "--connect-timeout",
            "10",
            "--max-time",
            "20",
            "-A",
            "rustbase-xtask-dep-age",
            &url,
        ])
        .output()
        .map_err(|e| format!("failed to run curl (is it installed?): {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("registry request failed:\n{stderr}"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse registry JSON: {e}"))
}

/// Resolve (version, publish-date) from an npm registry
/// document. `version` defaults to `dist-tags.latest`.
fn npm_version_date(
    json: &Value,
    version: Option<&str>,
) -> Result<(String, String), String> {
    let resolved = match version {
        Some(v) => v.to_string(),
        None => json["dist-tags"]["latest"]
            .as_str()
            .ok_or("npm: missing dist-tags.latest")?
            .to_string(),
    };
    let date = json["time"][&resolved]
        .as_str()
        .ok_or_else(|| format!("npm: no publish time for version {resolved}"))?
        .to_string();
    Ok((resolved, date))
}

/// Resolve (version, publish-date) from a crates.io crate
/// document. `version` defaults to `crate.max_stable_version`.
fn cargo_version_date(
    json: &Value,
    version: Option<&str>,
) -> Result<(String, String), String> {
    let resolved = match version {
        Some(v) => v.to_string(),
        None => json["crate"]["max_stable_version"]
            .as_str()
            .or_else(|| json["crate"]["newest_version"].as_str())
            .ok_or("cargo: missing crate.max_stable_version")?
            .to_string(),
    };
    let versions = json["versions"]
        .as_array()
        .ok_or("cargo: missing versions array")?;
    let entry = versions
        .iter()
        .find(|v| v["num"].as_str() == Some(resolved.as_str()))
        .ok_or_else(|| format!("cargo: version {resolved} not found"))?;
    let date = entry["created_at"]
        .as_str()
        .ok_or_else(|| format!("cargo: no created_at for version {resolved}"))?
        .to_string();
    Ok((resolved, date))
}

/// Extract the day-number (days since 1970-01-01) from an
/// ISO-8601 date/time string (`YYYY-MM-DD...`).
fn parse_iso_date(s: &str) -> Result<i64, String> {
    let date = s.get(..10).ok_or_else(|| format!("bad date {s:?}"))?;
    let mut it = date.split('-');
    let bad = || format!("bad date {s:?}");
    let y: i64 = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let m: u32 = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let d: u32 = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(format!("out-of-range date {s:?}"));
    }
    Ok(days_from_civil(y, m, d))
}

/// Days since the Unix epoch for a civil (proleptic
/// Gregorian) date. Howard Hinnant's `days_from_civil`.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Today as days since the Unix epoch (UTC).
fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs() / 86_400).ok())
        .unwrap_or(0)
}

/// Age in days of a publish-day relative to today (clamped
/// at 0 so a clock skew / future timestamp is not negative).
fn age_in_days(published_days: i64, today: i64) -> i64 {
    (today - published_days).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn days_from_civil_known_dates() {
        // 2000-03-01 is 11017 days after the epoch.
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        // Leap day handled.
        assert_eq!(
            days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28),
            2
        );
    }

    #[test]
    fn parse_iso_date_datetime_and_plain() {
        let a = parse_iso_date("2026-06-25T12:00:00.000Z").unwrap();
        let b = parse_iso_date("2026-06-25").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, days_from_civil(2026, 6, 25));
    }

    #[test]
    fn parse_iso_date_rejects_bad() {
        assert!(parse_iso_date("nope").is_err());
        assert!(parse_iso_date("2026-13-01").is_err());
        assert!(parse_iso_date("2026-06-40").is_err());
    }

    #[test]
    fn cooldown_verdict_boundary() {
        // Exactly the cooldown length is OK; one day inside
        // fails; well outside is OK; freshly published fails.
        assert!(cooldown_verdict(COOLDOWN_DAYS, "m").is_ok());
        assert!(cooldown_verdict(COOLDOWN_DAYS - 1, "m").is_err());
        assert!(cooldown_verdict(COOLDOWN_DAYS + 30, "m").is_ok());
        assert!(cooldown_verdict(0, "m").is_err());
    }

    #[test]
    fn age_computation_and_clamp() {
        let pub_day = days_from_civil(2026, 6, 1);
        assert_eq!(age_in_days(pub_day, days_from_civil(2026, 6, 15)), 14);
        // Future publish date clamps to 0, not negative.
        assert_eq!(age_in_days(days_from_civil(2026, 7, 1), pub_day), 0);
    }

    #[test]
    fn npm_version_date_resolves_latest() {
        let json: Value = serde_json::from_str(
            r#"{"dist-tags":{"latest":"1.2.3"},
                "time":{"1.2.3":"2026-01-10T00:00:00Z",
                        "1.0.0":"2020-01-01T00:00:00Z"}}"#,
        )
        .unwrap();
        assert_eq!(
            npm_version_date(&json, None).unwrap(),
            ("1.2.3".into(), "2026-01-10T00:00:00Z".into())
        );
        assert_eq!(
            npm_version_date(&json, Some("1.0.0")).unwrap().1,
            "2020-01-01T00:00:00Z"
        );
    }

    #[test]
    fn cargo_version_date_finds_entry() {
        let json: Value = serde_json::from_str(
            r#"{"crate":{"max_stable_version":"2.0.0"},
                "versions":[{"num":"2.0.0","created_at":"2026-02-02T00:00:00Z"},
                            {"num":"1.0.0","created_at":"2019-05-05T00:00:00Z"}]}"#,
        )
        .unwrap();
        assert_eq!(
            cargo_version_date(&json, None).unwrap(),
            ("2.0.0".into(), "2026-02-02T00:00:00Z".into())
        );
        assert_eq!(
            cargo_version_date(&json, Some("1.0.0")).unwrap().1,
            "2019-05-05T00:00:00Z"
        );
    }

    #[test]
    fn cargo_version_date_missing_version_errors() {
        let json: Value = serde_json::from_str(
            r#"{"crate":{"max_stable_version":"2.0.0"},
                "versions":[{"num":"2.0.0","created_at":"2026-02-02T00:00:00Z"}]}"#,
        )
        .unwrap();
        assert!(cargo_version_date(&json, Some("9.9.9")).is_err());
    }
}
