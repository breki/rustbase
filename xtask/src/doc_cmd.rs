use crate::helpers::{pair_with_locations, run_cargo_capture_env};

/// Maximum number of diagnostic lines to display.
const MAX_DIAGNOSTIC_LINES: usize = 10;

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

/// Check that every doc comment builds and every doc link
/// resolves.
///
/// Prints `Doc OK` on success, or `FAILED` with the offending
/// lines.
pub fn doc() -> Result<(), String> {
    match doc_check()? {
        None => {
            println!("Doc OK");
            Ok(())
        }
        Some(failure) => {
            eprintln!("FAILED: {}\n", failure.summary);
            for line in failure.items.iter().take(MAX_DIAGNOSTIC_LINES) {
                eprintln!("  {line}");
            }
            if failure.items.len() > MAX_DIAGNOSTIC_LINES {
                eprintln!(
                    "  ... and {} more",
                    failure.items.len() - MAX_DIAGNOSTIC_LINES
                );
            }
            Err(failure.summary)
        }
    }
}

/// A failing rustdoc pass.
pub struct DocFailure {
    /// One-line description naming which pass failed.
    pub summary: String,
    /// The diagnostic lines, with their source locations.
    pub items: Vec<String>,
}

/// Runs both passes, returning the first failure.
///
/// `RUSTDOCFLAGS=-D warnings` is what makes this a gate: rustdoc
/// reports a broken link as a warning, which builds clean.
///
/// # Errors
///
/// Returns `Err` only when rustdoc could not be run at all. A
/// pass that ran and found problems is `Ok(Some(..))`.
pub fn doc_check() -> Result<Option<DocFailure>, String> {
    for pass in PASSES {
        let mut args = vec!["doc", "--no-deps", "--workspace"];
        if pass.private {
            args.push("--document-private-items");
        }
        let output =
            run_cargo_capture_env(&args, &[("RUSTDOCFLAGS", "-D warnings")])?;
        if output.status.success() {
            continue;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(Some(DocFailure {
            summary: format!("broken documentation in {}", pass.what),
            items: pair_with_locations(&stderr, is_diagnostic_line)
                .into_iter()
                .map(String::from)
                .collect(),
        }));
    }
    Ok(None)
}

/// True for a real rustdoc diagnostic, false for the cargo
/// summary lines that share the same prefix.
fn is_diagnostic_line(line: &str) -> bool {
    // `could not document` is the consequence, once per crate,
    // and would bury the cause.
    let is_summary = line.contains("could not document")
        || line.contains("aborting due to")
        || line.contains("build failed")
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
}
