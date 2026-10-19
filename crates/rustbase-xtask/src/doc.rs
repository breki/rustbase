//! The doc gate: rustdoc with `-D warnings`, in a public and a
//! private pass, so a broken doc link fails instead of building
//! clean.

use crate::Config;
use crate::process::{self, Captured};
use crate::text::pair_with_locations;

/// How many lines of a failure a short report shows (the `validate`
/// step prints this many). When no diagnostic is located, the stderr
/// fallback keeps no more than this, so its last line, the cause, is
/// always among those shown.
pub const STDERR_TAIL_LINES: usize = 5;

/// Cargo's progress verbs, which say nothing about a failure.
const PROGRESS: &[&str] = &[
    "Blocking",
    "Checking",
    "Compiling",
    "Documenting",
    "Downloaded",
    "Downloading",
    "Finished",
    "Locking",
    "Updating",
];

/// One rustdoc pass, and what it is able to see.
///
/// Neither pass catches every broken link, so both run:
///
/// - A link inside a *private* module never reaches the public
///   pass, which does not render private modules.
/// - A *public* page linking a private item is an error in the
///   public pass and legal in the private one.
struct Pass {
    /// What this pass is looking after, for the failure message.
    what: &'static str,
    /// Whether to document private items.
    private: bool,
}

/// The passes, in the order they run.
const PASSES: &[Pass] = &[
    Pass {
        what: "the published API",
        private: false,
    },
    Pass {
        what: "internal cross-references",
        private: true,
    },
];

/// A failing rustdoc pass.
#[derive(Debug)]
pub struct DocFailure {
    /// One-line description naming which pass failed.
    pub summary: String,
    /// The diagnostic lines, with their source locations.
    pub items: Vec<String>,
}

/// Runs both passes in the workspace root, returning the first
/// failure.
///
/// `RUSTDOCFLAGS=-D warnings` is what makes this a gate: rustdoc
/// reports a broken link as a warning, which builds clean.
///
/// # Errors
///
/// Only when rustdoc could not be run at all. A pass that ran and
/// found problems is `Ok(Some(..))`.
pub fn check(cfg: &Config) -> Result<Option<DocFailure>, String> {
    run_passes(|args| {
        process::cargo_env(&cfg.root, args, &[("RUSTDOCFLAGS", "-D warnings")])
    })
}

/// [`check`] with the cargo spawn injected.
fn run_passes(
    mut cargo: impl FnMut(&[&str]) -> Result<Captured, String>,
) -> Result<Option<DocFailure>, String> {
    for pass in PASSES {
        let mut args = vec!["doc", "--no-deps", "--workspace"];
        if pass.private {
            args.push("--document-private-items");
        }
        let out = cargo(&args)?;
        if out.ok {
            continue;
        }
        return Ok(Some(DocFailure {
            summary: format!("cargo doc failed ({})", pass.what),
            items: failure_items(&out.stderr),
        }));
    }
    Ok(None)
}

/// The lines worth showing for a failed pass: the diagnostics,
/// or, when none carries a `-->` location (a bad manifest, a
/// rustdoc crash), the last lines of stderr. Cargo's progress lines
/// are dropped first: on a cold build they would fill the tail and
/// push the cause out.
fn failure_items(stderr: &str) -> Vec<String> {
    let items = pair_with_locations(stderr, is_diagnostic_line);
    if !items.iter().any(|l| l.trim_start().starts_with("-->")) {
        let lines: Vec<&str> = stderr
            .lines()
            .filter(|l| {
                let word = l.split_whitespace().next().unwrap_or_default();
                !word.is_empty() && !PROGRESS.contains(&word)
            })
            .collect();
        let from = lines.len().saturating_sub(STDERR_TAIL_LINES);
        return lines[from..].iter().map(|l| (*l).to_owned()).collect();
    }
    items.into_iter().map(String::from).collect()
}

/// True for a real rustdoc diagnostic, false for the cargo
/// summary lines that share the same prefix.
fn is_diagnostic_line(line: &str) -> bool {
    // `could not document` is the consequence, once per crate,
    // and would bury the cause.
    let is_summary = line.contains("could not document")
        || line.contains("aborting due to")
        || line.contains("build failed")
        || line.contains(") generated ")
        || line.contains(" warning emitted")
        || line.contains(" warnings emitted");
    if is_summary {
        return false;
    }
    line.starts_with("warning:")
        || line.starts_with("error[")
        || line.starts_with("error:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn both_passes_run_and_differ_only_in_privacy() {
        // Each pass sees a class of broken link the other cannot.
        assert_eq!(PASSES.len(), 2);
        assert!(!PASSES[0].private);
        assert!(PASSES[1].private);
    }

    #[test]
    fn extracts_the_diagnostic_and_its_location() {
        let stderr = "\
error: public documentation for `config` links to private item
  --> crates/rustbase/src/config.rs:12:7
error: could not document `rustbase`
warning: build failed, waiting for other jobs to finish...";
        let lines = pair_with_locations(stderr, is_diagnostic_line);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("links to private item"));
        assert!(lines[1].contains("config.rs:12:7"));
    }

    #[test]
    fn drops_the_cargo_summary_but_keeps_a_real_warning() {
        assert!(!is_diagnostic_line("error: could not document `rustbase`"));
        assert!(!is_diagnostic_line(
            "warning: build failed, waiting for other jobs"
        ));
        assert!(is_diagnostic_line("warning: unresolved link to `Foo`"));
        assert!(is_diagnostic_line("error[E0433]: failed to resolve"));
    }

    #[test]
    fn the_fallback_shows_the_cause_not_the_progress_lines() {
        // Cold cache: progress lines first, the cause last. The
        // printers show the head of the list (validate: 5 lines),
        // so the cause must be inside it.
        let mut stderr = String::new();
        for i in 0..17 {
            writeln!(stderr, "    Checking dep{i} v1.0.0").unwrap();
        }
        stderr.push_str(" Documenting xtask v0.1.0\n");
        stderr.push_str("error: couldn't generate documentation: No space\n");
        let items = failure_items(&stderr);
        assert!(items.len() <= 5, "{items:?}");
        assert!(items.iter().any(|l| l.contains("No space")), "{items:?}");
        assert!(!items.iter().any(|l| l.contains("Checking")), "{items:?}");
    }

    #[test]
    fn drops_the_rustdoc_generated_warnings_summary() {
        assert!(!is_diagnostic_line(
            "warning: `xtask` (bin \"xtask\" doc) generated 1 warning"
        ));
    }

    #[test]
    fn a_failure_with_no_diagnostic_falls_back_to_the_stderr_tail() {
        // A bad manifest or a rustdoc crash prints no diagnostic
        // line; the operator still needs to see why it failed.
        let stderr = "error: failed to parse manifest at `Cargo.toml`\n\
                      Caused by:\n  invalid table header\n";
        let items = failure_items(stderr);
        assert!(
            items.iter().any(|l| l.contains("invalid table header")),
            "{items:?}"
        );
    }

    fn captured(ok: bool, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn both_passes_pass_and_the_private_one_documents_private_items() {
        let mut seen = Vec::new();
        let r = run_passes(|args| {
            seen.push(args.join(" "));
            Ok(captured(true, ""))
        });
        assert!(r.unwrap().is_none());
        assert_eq!(
            seen,
            [
                "doc --no-deps --workspace",
                "doc --no-deps --workspace --document-private-items"
            ]
        );
    }

    #[test]
    fn the_first_failing_pass_is_reported_with_its_diagnostics() {
        let mut n = 0;
        let f = run_passes(|_| {
            n += 1;
            Ok(captured(
                n == 1,
                "error: unresolved link to `Foo`\n  --> src/lib.rs:3:5\n",
            ))
        })
        .unwrap()
        .expect("the private pass failed");
        assert_eq!(f.summary, "cargo doc failed (internal cross-references)");
        assert_eq!(f.items.len(), 2, "{:?}", f.items);
    }

    #[test]
    fn a_rustdoc_that_cannot_run_is_an_error() {
        assert!(run_passes(|_| Err("no cargo".into())).is_err());
    }
}
