//! The test gate: `cargo test`, reported as the failing tests and
//! their assertion details, or as the compile errors that stopped
//! the run.

use crate::process::{self, Captured};
use crate::text::pair_with_locations;
use crate::{Config, Failure, TOOLING_CRATE};

/// Assertion detail lines kept per failing test.
const MAX_DETAIL_LINES: usize = 5;

/// What a test run covers and how it reports.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Run only tests whose name contains this; a filter that
    /// matches nothing is a failure, not a pass.
    pub filter: Option<&'a str>,
    /// Stream cargo's own output instead of the condensed report.
    pub verbose: bool,
    /// Run the `#[ignore]`-tagged tests instead of the default set.
    pub ignored: bool,
}

/// Test invocation scope.
#[derive(Clone, Copy)]
enum Scope {
    /// `--workspace`: every crate.
    Workspace,
    /// `-p xtask`: only the build-tooling crate, whose tests the
    /// coverage run (which excludes it) does not execute.
    ToolingOnly,
}

/// Runs the workspace's tests in its root.
///
/// # Errors
///
/// The failing tests, the compile errors, a filter that matched no
/// test, or why cargo could not run. A verbose run streams cargo's
/// output and reports only that it failed.
pub fn run(cfg: &Config, opts: Options<'_>) -> Result<(), Failure> {
    let args = build_args(Scope::Workspace, opts.filter, opts.ignored)?;
    if opts.verbose {
        // The stream shows `running 0 tests` to the person watching,
        // so the zero-match guard below is for the condensed report.
        return Ok(process::cargo_stream(&cfg.root, &args)?);
    }
    judge(&process::cargo(&cfg.root, &args)?, opts.filter)
}

/// Runs only the `xtask` crate's tests, for `validate`: the coverage
/// run executes every other crate's.
///
/// # Errors
///
/// As [`run`].
pub fn tooling_only(cfg: &Config) -> Result<(), Failure> {
    let args = build_args(Scope::ToolingOnly, None, false)?;
    judge(&process::cargo(&cfg.root, &args)?, None)
}

/// Turn a finished `cargo test` into the gate's result.
fn judge(out: &Captured, filter: Option<&str>) -> Result<(), Failure> {
    if out.ok {
        // `cargo test <filter>` exits 0 when the filter matches
        // nothing, which would be a green run that ran nothing.
        if let Some(f) = filter
            && count_tests_run(&out.stdout) == 0
        {
            return Err(format!(
                "filter {f:?} matched zero tests -- nothing ran"
            )
            .into());
        }
        return Ok(());
    }
    if out.stderr.contains("could not compile") {
        let lines =
            pair_with_locations(&out.stderr, |l| l.starts_with("error"));
        return Err(Failure::new(
            "compilation failed",
            lines.into_iter().map(String::from).collect(),
        ));
    }
    let failures = extract_failure_details(&out.stdout, &out.stderr);
    let items = if failures.is_empty() {
        extract_failed_names(&out.stdout)
    } else {
        failures
            .into_iter()
            .flat_map(|f| {
                let details = f.details.into_iter().take(MAX_DETAIL_LINES);
                std::iter::once(f.name).chain(details.map(|d| format!("  {d}")))
            })
            .collect()
    };
    Err(Failure::new("test(s) failed", items))
}

/// Sum the `running N tests` counts cargo prints, one per test
/// binary.
fn count_tests_run(stdout: &str) -> u64 {
    stdout
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("running ")?;
            let n = rest
                .strip_suffix(" tests")
                .or_else(|| rest.strip_suffix(" test"))?;
            n.trim().parse::<u64>().ok()
        })
        .sum()
}

/// The cargo test argument list, for both scopes.
fn build_args(
    scope: Scope,
    filter: Option<&str>,
    ignored: bool,
) -> Result<Vec<&str>, String> {
    let mut args = vec!["test"];
    match scope {
        Scope::Workspace => args.push("--workspace"),
        Scope::ToolingOnly => args.extend(["-p", TOOLING_CRATE]),
    }
    // Both `--ignored` and a positional filter go to the test
    // harness, after `--`.
    if filter.is_some() || ignored {
        args.push("--");
    }
    if let Some(f) = filter {
        if f.is_empty() {
            return Err("test filter must not be empty".into());
        }
        args.push(f);
    }
    if ignored {
        args.push("--ignored");
    }
    Ok(args)
}

/// Test names from `test foo ... FAILED` lines.
fn extract_failed_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_suffix(" ... FAILED")?;
            Some(l.strip_prefix("test ").unwrap_or(l).to_string())
        })
        .collect()
}

/// A single test failure with detail lines.
struct FailureDetail {
    /// Fully qualified test name.
    name: String,
    /// Assertion detail lines (panic message, etc.).
    details: Vec<String>,
}

/// Failing test details from `---- name stdout ----` sections.
fn extract_failure_details(stdout: &str, stderr: &str) -> Vec<FailureDetail> {
    let mut failures = Vec::new();
    let combined = format!("{stdout}\n{stderr}");
    let mut current: Option<FailureDetail> = None;
    for line in combined.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("---- ") {
            failures.extend(current.take());
            current =
                rest.strip_suffix(" stdout ----").map(|name| FailureDetail {
                    name: name.to_string(),
                    details: Vec::new(),
                });
        } else if let Some(f) = current.as_mut()
            && !trimmed.is_empty()
            && !trimmed.starts_with("thread '")
            && !trimmed.starts_with("note: run with")
        {
            f.details.push(trimmed.to_string());
        }
    }
    failures.extend(current);
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(ok: bool, stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn extract_failed_names_from_output() {
        let stdout = "\
test foo::bar ... ok
test baz::qux ... FAILED
test third::fail ... FAILED";
        assert_eq!(extract_failed_names(stdout), ["baz::qux", "third::fail"]);
        let none = extract_failed_names("test a ... ok");
        assert!(none.is_empty(), "{none:?}");
    }

    const FAILING: &str = "\
test api::tests::my_test ... FAILED

failures:

---- api::tests::my_test stdout ----
thread 'api::tests::my_test' panicked at 'msg'
assertion `left == right` failed
  left: 1
 right: 2
note: run with RUST_BACKTRACE=1

failures:
    api::tests::my_test
";

    #[test]
    fn a_failing_test_is_reported_with_its_assertion_details() {
        let f = judge(&captured(false, FAILING, ""), None).unwrap_err();
        assert_eq!(f.summary, "test(s) failed");
        assert_eq!(f.items[0], "api::tests::my_test");
        assert!(
            f.items.iter().any(|d| d.contains("assertion")),
            "{:?}",
            f.items
        );
        assert!(!f.items.iter().any(|d| d.contains("thread '")));
    }

    #[test]
    fn a_failure_without_details_lists_the_names() {
        let stdout = "test a::b ... FAILED\ntest result: FAILED";
        let f = judge(&captured(false, stdout, ""), None).unwrap_err();
        assert_eq!(f.items, ["a::b"]);
    }

    #[test]
    fn a_compile_error_is_reported_with_its_location() {
        let stderr = "\
error[E0425]: cannot find value `foo`
  --> crates/rustbase/src/lib.rs:10:5
warning: unused variable: `x`
  --> crates/rustbase/src/lib.rs:99:9
error: could not compile `rustbase` (lib) due to 1 previous error";
        let f = judge(&captured(false, "", stderr), None).unwrap_err();
        assert_eq!(f.summary, "compilation failed");
        assert_eq!(f.items.len(), 3, "{:?}", f.items);
        assert!(f.items[1].contains("lib.rs:10:5"));
        assert!(!f.items.iter().any(|l| l.contains("99:9")));
    }

    #[test]
    fn a_filter_that_matched_nothing_fails() {
        let out = captured(true, "running 0 tests\ntest result: ok", "");
        let f = judge(&out, Some("typo")).unwrap_err();
        assert!(f.summary.contains("matched zero tests"), "{}", f.summary);
        assert_eq!(judge(&out, None), Ok(()));
        let ran = captured(true, "running 1 test\ntest x ... ok", "");
        assert_eq!(judge(&ran, Some("x")), Ok(()));
    }

    #[test]
    fn build_args_cover_both_scopes_filters_and_ignored() {
        let args = |s, f, i| build_args(s, f, i).unwrap();
        assert_eq!(
            args(Scope::Workspace, None, false),
            ["test", "--workspace"]
        );
        assert_eq!(
            args(Scope::ToolingOnly, None, false),
            ["test", "-p", "xtask"]
        );
        assert_eq!(
            args(Scope::Workspace, Some("foo"), true),
            ["test", "--workspace", "--", "foo", "--ignored"]
        );
        assert_eq!(
            args(Scope::Workspace, None, true),
            ["test", "--workspace", "--", "--ignored"]
        );
        let e = build_args(Scope::Workspace, Some(""), false).unwrap_err();
        assert!(e.contains("must not be empty"));
    }

    #[test]
    fn count_tests_run_sums_binaries_singular_and_plural() {
        let stdout = "running 3 tests\nrunning 1 test\nrunning 0 tests";
        assert_eq!(count_tests_run(stdout), 4);
        assert_eq!(count_tests_run("running 12 tests"), 12);
        assert_eq!(count_tests_run("test result: ok. 0 passed"), 0);
    }
}
