//! The library's `cargo xtask` subcommands, for a project's CLI to
//! take whole: flatten [`LibCommand`] into the project's own
//! subcommand enum and hand a parsed one to [`run`]. A command the
//! library gains then reaches the project as a dependency bump.
//!
//! ```ignore
//! #[derive(clap::Subcommand)]
//! enum XCommand {
//!     #[command(flatten)]
//!     Lib(rustbase_xtask::cli::LibCommand),
//!     Validate { #[arg(long)] check: bool },  // the project's own
//! }
//! // XCommand::Lib(c) => rustbase_xtask::cli::run(c, &tasks_config()),
//! ```

use clap::Subcommand;

use crate::{
    Config, Strictness, audit, canon, cargo_test, changelog, check, clippy,
    coverage, deny, dep_age, doc, dupes, fmt, records, todo,
};

/// Error lines `check` shows.
const MAX_ERROR_LINES: usize = 10;

/// Diagnostic lines `clippy` and `doc` show.
const MAX_DIAGNOSTIC_LINES: usize = 10;

/// Diff lines `fmt` shows.
const MAX_DIFF_LINES: usize = 20;

/// The subcommands the library implements.
#[derive(Subcommand)]
pub enum LibCommand {
    /// Type-check all targets (compiles tests, runs none)
    Check,
    /// Check the claims canon prose makes (paths, cross-refs,
    /// git grants, width, backlog IDs)
    CanonCheck,
    /// Check the record files: duplicate ids, unknown field labels,
    /// malformed ids, dangling Depends on / Supersedes
    RecordsCheck,
    /// Run clippy (deny warnings)
    Clippy,
    /// Check doc comments build and doc links resolve
    Doc,
    /// Run all tests
    Test {
        /// Optional test filter
        filter: Option<String>,
        /// Show raw cargo test output
        #[arg(long)]
        verbose: bool,
        /// Run `#[ignore]`-tagged tests (e.g. manual
        /// tools). Off by default; not run by
        /// `validate`.
        #[arg(long)]
        ignored: bool,
    },
    /// Format code
    Fmt,
    /// Run coverage check (requires cargo-llvm-cov)
    Coverage,
    /// Run code duplication check (requires code-dupes)
    Dupes,
    /// Security-advisory audit (RUSTSEC + npm); requires
    /// cargo-audit
    Audit,
    /// Licence, bans and sources gate (offline); requires
    /// cargo-deny
    Deny,
    /// Report a dependency version's age and flag it if
    /// within the publish cooldown (on-demand; requires curl)
    DepAge {
        /// Registry to query
        #[arg(value_enum)]
        ecosystem: dep_age::Ecosystem,
        /// Package name
        package: String,
        /// Version to check (default: latest)
        version: Option<String>,
        /// Instead of checking a version's age, print the
        /// newest version published before the cooldown (the
        /// pin target for `/update-deps`). Ignores `version`.
        #[arg(long)]
        latest_aged: bool,
        /// With `--latest-aged`, stay on one release line: `3`,
        /// `3.x` or `0.4` (whole version components).
        #[arg(long, requires = "latest_aged")]
        within: Option<String>,
    },
    /// Cooldown-check only the dependencies added or bumped in
    /// the working tree versus HEAD (the changed-deps gate that
    /// `validate` runs; requires curl + git)
    DepAgeCheck {
        /// Commit to diff the lockfiles against. CI passes the
        /// commit the push or pull request started from.
        #[arg(long, default_value = "HEAD")]
        base: String,
    },
    /// Pin any changed Rust dependency still within the cooldown
    /// down to its newest aged version, before compiling
    /// (front-door remediation; requires curl + git + cargo)
    DepPreflight,
    /// Mechanically edit `CHANGELOG.md` (used by `/commit`) --
    /// insert a bullet under the right `[Unreleased]` subsection
    Changelog {
        #[command(subcommand)]
        action: ChangelogAction,
    },
    /// Mechanically read/edit `docs/todo.md` (used by `/todo` and
    /// `/implement`) -- list, add, or complete a captured item
    Todo {
        #[command(subcommand)]
        action: TodoAction,
    },
}

/// `changelog` subcommands.
#[derive(Subcommand)]
pub enum ChangelogAction {
    /// Add a bullet under the `[Unreleased]` section.
    Add {
        /// Which subsection the change belongs to.
        #[arg(long, value_enum)]
        kind: changelog::Kind,
        /// Mark as a breaking change: prefixes the bullet with
        /// `**BREAKING:**`, which `infer-bump` reads as a major bump.
        #[arg(long)]
        breaking: bool,
        /// The change description (one entry, wrapped to 80
        /// columns).
        text: String,
    },
    /// Print the `SemVer` bump (`major` / `minor` / `patch`) the
    /// `[Unreleased]` bullets call for: a top-level `- **BREAKING:**`
    /// entry or a non-empty `### Removed` is major, a non-empty
    /// `### Added` minor, else patch. Fails when there is nothing to
    /// release.
    InferBump,
    /// Rename `[Unreleased]` to `[<version>] - <date>` and open a
    /// fresh empty `[Unreleased]` above it.
    Promote {
        /// The release version, `X.Y.Z`.
        #[arg(long)]
        version: String,
        /// The release date, `YYYY-MM-DD`.
        #[arg(long)]
        date: String,
    },
}

/// `todo` subcommands.
#[derive(Subcommand)]
pub enum TodoAction {
    /// List queued items as `slug -- summary`, one per line.
    List,
    /// Append a new item to the queue.
    Add {
        /// Short kebab-case topic slug (must be unique).
        #[arg(long)]
        slug: String,
        /// One-line summary. It must fit on one `**Summary:**` line
        /// within 80 columns, or the command errors and says the
        /// budget -- put detail in --body.
        #[arg(long)]
        summary: String,
        /// Optional longer body, wrapped as prose under the summary.
        #[arg(long)]
        body: Option<String>,
    },
    /// Remove a completed item from the queue.
    Done {
        /// The slug to remove.
        slug: String,
    },
}

/// Run `cmd` for the project `cfg` describes, printing its outcome.
///
/// # Errors
///
/// The command's failure, as the one report to print.
pub fn run(cmd: LibCommand, cfg: &Config) -> Result<(), String> {
    match cmd {
        LibCommand::Check => {
            check::check(cfg).map_err(|f| f.report(MAX_ERROR_LINES))?;
            println!("Check OK");
        }
        LibCommand::CanonCheck => {
            println!("Canon OK ({})", canon::check(cfg)?);
        }
        LibCommand::RecordsCheck => {
            println!("Records OK ({})", records::check(cfg)?);
        }
        LibCommand::Clippy => {
            clippy::check(cfg).map_err(|f| f.report(MAX_DIAGNOSTIC_LINES))?;
            println!("Clippy OK");
        }
        LibCommand::Doc => {
            doc::check(cfg).map_err(|f| f.report(MAX_DIAGNOSTIC_LINES))?;
            println!("Doc OK");
        }
        LibCommand::Test {
            filter,
            verbose,
            ignored,
        } => {
            // Every failing test is named, so the report is not capped.
            let opts = cargo_test::Options {
                filter: filter.as_deref(),
                verbose,
                ignored,
            };
            cargo_test::run(cfg, opts).map_err(|f| f.to_string())?;
            if !verbose {
                println!("Test OK");
            }
        }
        // Its output streams to the terminal, so a failure's error is
        // only that it failed.
        LibCommand::Fmt => {
            fmt::fix(cfg).map_err(|f| f.report(MAX_DIFF_LINES))?;
        }
        LibCommand::Coverage => {
            let r = coverage::check(cfg)?;
            println!("  lines: {}/{} ({:.1}%)", r.covered, r.total, r.line_pct);
            println!("  coverage OK ({r})");
        }
        LibCommand::Dupes => {
            println!("Duplication OK ({})", dupes::check(cfg)?);
        }
        // Run by hand, so an audit that could not finish fails.
        LibCommand::Audit => {
            let npm = cfg.audit_npm_dir.as_deref();
            println!(
                "Audit OK ({})",
                audit::check(cfg, npm, Strictness::Fail)?
            );
        }
        LibCommand::Deny => {
            println!("Deny OK ({})", deny::check(cfg)?);
        }
        LibCommand::DepAge {
            ecosystem,
            package,
            version,
            latest_aged,
            within,
        } => {
            if latest_aged {
                dep_age::dep_age_latest(
                    cfg,
                    ecosystem,
                    &package,
                    within.as_deref(),
                )?;
            } else {
                dep_age::dep_age(cfg, ecosystem, &package, version.as_deref())?;
            }
        }
        LibCommand::DepAgeCheck { base } => dep_age::dep_age_check(cfg, &base)?,
        LibCommand::DepPreflight => dep_age::dep_preflight(cfg)?,
        LibCommand::Changelog { action } => run_changelog(action, cfg)?,
        LibCommand::Todo { action } => run_todo(action, cfg)?,
    }
    Ok(())
}

/// `changelog <action>`.
fn run_changelog(action: ChangelogAction, cfg: &Config) -> Result<(), String> {
    match action {
        ChangelogAction::Add {
            kind,
            breaking,
            text,
        } => {
            changelog::add(cfg, kind, breaking, &text)?;
            println!(
                "Added to CHANGELOG [Unreleased] under {}.",
                kind.heading()
            );
        }
        ChangelogAction::InferBump => {
            println!("{}", changelog::infer_bump(cfg)?);
        }
        ChangelogAction::Promote { version, date } => {
            changelog::promote(cfg, &version, &date)?;
            println!("Promoted [Unreleased] to [{version}] - {date}.");
        }
    }
    Ok(())
}

/// `todo <action>`.
fn run_todo(action: TodoAction, cfg: &Config) -> Result<(), String> {
    match action {
        TodoAction::List => {
            for item in todo::list(cfg)? {
                println!("{item}");
            }
        }
        TodoAction::Add {
            slug,
            summary,
            body,
        } => {
            todo::add(cfg, &slug, &summary, body.as_deref())?;
            println!("Added todo '{slug}'.");
        }
        TodoAction::Done { slug } => {
            todo::done(cfg, &slug)?;
            println!("Removed '{slug}' from the queue.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::test_repo;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: LibCommand,
    }

    fn parse(args: &[&str]) -> LibCommand {
        Cli::try_parse_from(
            std::iter::once("xtask").chain(args.iter().copied()),
        )
        .unwrap_or_else(|e| panic!("{args:?}: {e}"))
        .cmd
    }

    #[test]
    fn the_queue_and_changelog_commands_edit_their_files() {
        let dir = test_repo::empty_dir();
        let cfg = Config::for_project("p", dir.0.clone());
        std::fs::create_dir_all(dir.0.join("docs")).unwrap();
        std::fs::write(dir.0.join("docs/todo.md"), "# TODO\n").unwrap();
        std::fs::write(
            dir.0.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n",
        )
        .unwrap();
        for args in [
            &["todo", "add", "--slug", "a-b", "--summary", "s"][..],
            &["todo", "list"],
            &["todo", "done", "a-b"],
            &["changelog", "add", "--kind", "fixed", "a fix"],
            &["changelog", "infer-bump"],
            &[
                "changelog",
                "promote",
                "--version",
                "1.0.0",
                "--date",
                "2026-10-20",
            ],
        ] {
            run(parse(args), &cfg).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        }
        let log = std::fs::read_to_string(dir.0.join("CHANGELOG.md")).unwrap();
        assert!(log.contains("## [1.0.0] - 2026-10-20"), "{log}");
        assert!(log.contains("- a fix"), "{log}");
        let queue =
            std::fs::read_to_string(dir.0.join("docs/todo.md")).unwrap();
        assert!(!queue.contains("a-b"), "{queue}");
    }

    #[test]
    fn every_command_parses_and_runs_against_its_config() {
        // In an empty directory the tools fail fast (no Cargo.toml
        // above a temp dir) and nothing is written but `fmt`'s no-op.
        let dir = test_repo::empty_dir();
        let cfg = Config::for_project("p", dir.0.clone());
        let ok = |args: &[&str]| run(parse(args), &cfg).is_ok();
        assert!(ok(&["records-check"]), "absent files are skipped");
        for args in [
            &["check"][..],
            &["canon-check"],
            &["clippy"],
            &["doc"],
            &["test", "x", "--verbose", "--ignored"],
            &["test"],
            &["fmt"],
            &["coverage"],
            &["dupes"],
            &["audit"],
            &["deny"],
            &["dep-age-check", "--base", "HEAD"],
            &["dep-preflight"],
        ] {
            let _ = ok(args);
        }
        // Parsing only: these would reach a package registry.
        parse(&["dep-age", "cargo", "serde", "1.0.0"]);
        parse(&["dep-age", "npm", "x", "--latest-aged", "--within", "3"]);
    }
}
