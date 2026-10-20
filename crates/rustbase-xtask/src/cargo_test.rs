//! The test gate: `cargo test`, reported as the failing tests and
//! their assertion details, or as the compile errors that stopped
//! the run.

use crate::process::{self, Captured};
use crate::text::{is_cargo_diagnostic, pair_with_locations};
use crate::{Config, Failure, TOOLING_CRATE};

/// Assertion detail lines kept per failing test.
const MAX_DETAIL_LINES: usize = 5;

/// Compile-error lines kept: the first errors are the ones to fix.
/// Failing tests are never capped: every one is named.
const MAX_COMPILE_LINES: usize = 10;

/// What a test run covers and how it reports.
///
/// Not `#[non_exhaustive]`, unlike `Config`, so callers
/// can write `Options { x, ..Options::default() }`. The cost: adding a
/// field is a breaking change.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Run only tests whose name contains this. In the condensed
    /// report a filter that matches nothing is a failure, not a pass;
    /// a verbose run shows `running 0 tests` instead.
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
        // Errors only (a warning beside them is noise here), and not
        // cargo's per-target `could not compile` summaries -- unless
        // nothing else is there, as when rustc was killed.
        let mut lines = pair_with_locations(&out.stderr, |l| {
            l.starts_with("error") && is_cargo_diagnostic(l)
        });
        lines.truncate(MAX_COMPILE_LINES);
        return Err(Failure::of_run("compilation failed", lines, out));
    }
    // libtest's `... FAILED` lines name every failing test; a details
    // section exists only for a test that captured output, so the
    // names lead and the details attach to them.
    let details = extract_failure_details(&out.stdout, &out.stderr);
    let mut names = extract_failed_names(&out.stdout);
    for d in &details {
        if !names.contains(&d.name) {
            names.push(d.name.clone());
        }
    }
    let items: Vec<String> = names
        .into_iter()
        .flat_map(|name| {
            let lines: Vec<String> = details
                .iter()
                .find(|d| d.name == name)
                .into_iter()
                .flat_map(|d| d.details.iter().take(MAX_DETAIL_LINES))
                .map(|l| format!("  {l}"))
                .collect();
            std::iter::once(name).chain(lines)
        })
        .collect();
    Err(Failure::of_run("test(s) failed", items, out))
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
            let name = l.strip_prefix("test ").unwrap_or(l);
            Some(strip_test_mode(name).to_string())
        })
        .collect()
}

/// A test name without the mode libtest appends to its result line
/// but not to its details header (`tests::sp - should panic` there,
/// `---- tests::sp stdout ----` here).
fn strip_test_mode(name: &str) -> &str {
    [" - should panic", " - compile fail", " - compile"]
        .iter()
        .find_map(|mode| name.strip_suffix(mode))
        .unwrap_or(name)
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
        // libtest's trailing `failures:` list and `test result:` line
        // end the last section; they are not that test's details.
        if trimmed == "failures:" || trimmed.starts_with("test result:") {
            failures.extend(current.take());
        } else if let Some(rest) = trimmed.strip_prefix("---- ") {
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
    use std::fmt::Write as _;

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
    fn a_compile_failure_with_no_diagnostic_carries_the_stderr_tail() {
        // A rustc killed by a signal prints no error of its own; the
        // `could not compile` summary and its cause are all there is.
        let stderr = "error: could not compile `rustbase` (lib test)\n\
                      Caused by:\n  process didn't exit successfully: \
                      `rustc` (signal: 9, SIGKILL: kill)";
        let f = judge(&captured(false, "", stderr), None).unwrap_err();
        assert_eq!(f.summary, "compilation failed");
        assert!(
            f.items.iter().any(|l| l.contains("SIGKILL")),
            "{:?}",
            f.items
        );
        assert!(
            f.items.iter().any(|l| l.contains("rustbase")),
            "{:?}",
            f.items
        );
    }

    #[test]
    fn compile_errors_are_capped() {
        let mut stderr = String::new();
        for i in 0..30 {
            let _ = writeln!(stderr, "error[E0425]: cannot find `v{i}`");
        }
        stderr.push_str("error: could not compile `a` (lib)");
        let f = judge(&captured(false, "", &stderr), None).unwrap_err();
        assert_eq!(f.items.len(), MAX_COMPILE_LINES);
    }

    #[test]
    fn every_failing_test_is_named_however_many_fail() {
        let mut stdout = String::new();
        for i in 0..20 {
            let _ = write!(
                stdout,
                "---- t{i} stdout ----\nassertion failed\n  left: 1\n right: 2\n\
                 a\nb\nc\nd\n"
            );
        }
        let f = judge(&captured(false, &stdout, ""), None).unwrap_err();
        for i in 0..20 {
            assert!(f.items.contains(&format!("t{i}")), "t{i} missing");
        }
        // Details stay capped per test.
        assert_eq!(f.items.len(), 20 * (1 + MAX_DETAIL_LINES));
    }

    #[test]
    fn a_run_that_failed_with_no_test_named_shows_the_tail() {
        // A manifest error prints no `could not compile` and no test.
        let stderr = "error: string values must be quoted\n --> Cargo.toml:3:9";
        let f = judge(&captured(false, "", stderr), None).unwrap_err();
        assert!(
            f.items.iter().any(|l| l.contains("Cargo.toml:3:9")),
            "{:?}",
            f.items
        );
    }

    #[test]
    fn a_failing_test_with_no_captured_output_is_still_named() {
        let stdout = "\
test t::noisy ... FAILED
test t::silent ... FAILED

failures:

---- t::noisy stdout ----
assertion failed

failures:
    t::noisy
    t::silent

test result: FAILED. 0 passed; 2 failed";
        let f = judge(&captured(false, stdout, ""), None).unwrap_err();
        assert_eq!(f.items, ["t::noisy", "  assertion failed", "t::silent"]);
    }

    #[test]
    fn a_test_mode_suffix_does_not_split_one_failure_into_two() {
        let stdout = "test tests::sp - should panic ... FAILED\n\nfailures:\n\n\
                      ---- tests::sp stdout ----\n\
                      note: test did not panic as expected\n\nfailures:\n    tests::sp\n";
        let f = judge(&captured(false, stdout, ""), None).unwrap_err();
        assert_eq!(
            f.items,
            ["tests::sp", "  note: test did not panic as expected"]
        );
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
error: could not compile `rustbase` (lib test) due to 1 previous error
error: could not compile `rustbase` (bin \"rustbase\" test) due to 1 previous error";
        let f = judge(&captured(false, "", stderr), None).unwrap_err();
        assert_eq!(f.summary, "compilation failed");
        // The error and its location; not cargo's per-target
        // `could not compile` summaries, nor the warning.
        assert_eq!(f.items.len(), 2, "{:?}", f.items);
        assert!(f.items[1].contains("lib.rs:10:5"));
        assert!(!f.items.iter().any(|l| l.contains("could not compile")));
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
