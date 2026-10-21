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

#[cfg(feature = "frontend")]
use crate::frontend;
use crate::{
    Config, FMT_DIFF_LINES, STANDALONE_LINES, Strictness, audit, canon,
    cargo_test, changelog, check, clippy, coverage, deny, dep_age, doc, dupes,
    fmt, licenses, records, scripts, tag_signature, todo, version_sync,
};

/// The subcommands the library implements.
#[derive(Debug, Subcommand)]
#[non_exhaustive]
pub enum LibCommand {
    /// Type-check all targets (compiles tests, runs none)
    Check,
    /// Check the claims canon prose makes (paths, cross-refs,
    /// git grants, width, backlog IDs)
    CanonCheck,
    /// Check the record files: duplicate ids, unknown field labels,
    /// malformed ids, dangling Depends on / Supersedes
    RecordsCheck,
    /// Check that scripts under scripts/ with a shebang are
    /// committed executable
    ScriptsCheck,
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
    /// The four frontend gates, as `frontend-<gate>`.
    #[cfg(feature = "frontend")]
    #[command(flatten)]
    Frontend(FrontendCommand),
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
    /// Write the third-party licence file for a distributed binary
    /// (THIRD-PARTY-LICENSES): every crate that goes into building it,
    /// with its licence texts; fails when a crate ships none
    Licenses {
        /// Where to write it: a file, or a directory to put
        /// THIRD-PARTY-LICENSES in -- an existing one, or a path ending
        /// in `/`, which is created (default: cargo's target directory)
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Target triple the binary is built for (default: the host's)
        #[arg(long)]
        target: Option<String>,
        /// The workspace member whose binary ships (default: every
        /// publishable member)
        #[arg(long)]
        package: Option<String>,
        /// How many crates may ship no licence text; raise it only
        /// with a stated reason
        #[arg(long, default_value_t = 0)]
        max_missing: usize,
    },
    /// Check that a release tag carries an SSH signature from a key
    /// in `.github/release-signers` (used by `/release` when the
    /// project tracks that file)
    VerifyReleaseTag {
        /// The tag to check, for example `v1.2.0`
        tag: String,
    },
    /// Rewrite `<!-- version: X -->` sentinels (and `VERSION=` lines
    /// in the same files) in README.md and docs/ from the crate's
    /// manifest (used by `/release`)
    VersionSync,
    /// Mechanically read/edit `docs/todo.md` (used by `/todo` and
    /// `/implement`) -- list, add, or complete a captured item
    Todo {
        #[command(subcommand)]
        action: TodoAction,
    },
}

/// `changelog` subcommands.
#[derive(Debug, Subcommand)]
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
#[derive(Debug, Subcommand)]
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
            check::check(cfg).map_err(|f| f.report(STANDALONE_LINES))?;
            println!("Check OK");
        }
        LibCommand::CanonCheck => {
            println!("Canon OK ({})", canon::check(cfg)?);
        }
        LibCommand::RecordsCheck => {
            println!("Records OK ({})", records::check(cfg)?);
        }
        LibCommand::ScriptsCheck => {
            println!("Scripts OK ({})", scripts::check(cfg)?);
        }
        LibCommand::Clippy => {
            clippy::check(cfg).map_err(|f| f.report(STANDALONE_LINES))?;
            println!("Clippy OK");
        }
        LibCommand::Doc => {
            doc::check(cfg).map_err(|f| f.report(STANDALONE_LINES))?;
            println!("Doc OK");
        }
        LibCommand::Test {
            filter,
            verbose,
            ignored,
        } => run_test(filter.as_deref(), verbose, ignored, cfg)?,
        // Its output streams to the terminal, so a failure's error is
        // only that it failed.
        LibCommand::Fmt => {
            fmt::fix(cfg).map_err(|f| f.report(FMT_DIFF_LINES))?;
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
            let npm = cfg.npm_dir();
            println!(
                "Audit OK ({})",
                audit::check(cfg, npm.as_deref(), Strictness::Fail)?
            );
        }
        LibCommand::Deny => {
            println!("Deny OK ({})", deny::check(cfg)?);
        }
        #[cfg(feature = "frontend")]
        LibCommand::Frontend(c) => run_frontend(&c, cfg)?,
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
        LibCommand::Licenses {
            out,
            target,
            package,
            max_missing,
        } => {
            let opts = licenses::Options {
                out: out.as_deref(),
                target: target.as_deref(),
                package: package.as_deref(),
                max_missing,
            };
            println!("{}", licenses::write(cfg, &opts)?);
        }
        LibCommand::VerifyReleaseTag { tag } => {
            println!("{}", tag_signature::verify_release_tag(cfg, &tag)?);
        }
        LibCommand::VersionSync => println!("{}", version_sync::sync(cfg)?),
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

/// `test [filter]`. Every failing test is named, so the report is not
/// capped.
fn run_test(
    filter: Option<&str>,
    verbose: bool,
    ignored: bool,
    cfg: &Config,
) -> Result<(), String> {
    let opts = cargo_test::Options {
        filter,
        verbose,
        ignored,
    };
    cargo_test::run(cfg, opts).map_err(|f| f.to_string())?;
    if !verbose {
        println!("Test OK");
    }
    Ok(())
}

/// The frontend gates' subcommands, flattened into [`LibCommand`].
#[cfg(feature = "frontend")]
#[derive(Debug, Subcommand)]
pub enum FrontendCommand {
    /// Type-check the frontend (svelte-check); skips
    /// cleanly when there is no frontend
    FrontendCheck,
    /// Format the frontend with Prettier; skips cleanly
    /// when there is no frontend
    FrontendFmt {
        /// Check formatting read-only instead of
        /// auto-fixing in place
        #[arg(long)]
        check: bool,
    },
    /// Check frontend code duplication (requires jscpd);
    /// skips cleanly when there is no frontend
    FrontendDupes,
    /// Run the frontend unit suite (vitest); skips cleanly
    /// when there is no frontend
    FrontendTest,
}

/// `frontend-<gate>`.
#[cfg(feature = "frontend")]
fn run_frontend(c: &FrontendCommand, cfg: &Config) -> Result<(), String> {
    use frontend::{Gate, Outcome};
    let gate = match *c {
        FrontendCommand::FrontendCheck => Gate::Check,
        FrontendCommand::FrontendFmt { check: true } => Gate::FmtCheck,
        FrontendCommand::FrontendFmt { check: false } => Gate::Fmt,
        FrontendCommand::FrontendDupes => Gate::Dupes,
        FrontendCommand::FrontendTest => Gate::Test,
    };
    match frontend::run(cfg, gate).map_err(|f| f.report(STANDALONE_LINES))? {
        Outcome::Skipped => println!("{} {}", gate.name(), frontend::SKIPPED),
        Outcome::NotConfigured => {
            println!("{} {}", gate.name(), frontend::NOT_CONFIGURED);
        }
        _ => println!("{} OK", gate.name()),
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
        std::fs::create_dir_all(dir.0.join("crates/p/src")).unwrap();
        std::fs::write(
            dir.0.join("crates/p/Cargo.toml"),
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n\
             edition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(dir.0.join("crates/p/src/lib.rs"), "").unwrap();
        std::fs::write(dir.0.join("README.md"), "<!-- version: 0.1.0 -->\n")
            .unwrap();
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
            &["version-sync"],
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
        let readme = std::fs::read_to_string(dir.0.join("README.md")).unwrap();
        assert_eq!(readme, "<!-- version: 1.0.0 -->\n");
    }

    #[test]
    fn every_command_parses_and_runs_against_its_config() {
        // With no workspace and no repository each command fails fast,
        // and nothing is written.
        let dir = test_repo::outside_any_workspace();
        let cfg = Config::for_project("p", dir.0.clone());
        let ok = |args: &[&str]| run(parse(args), &cfg).is_ok();
        assert!(ok(&["records-check"]), "absent files are skipped");
        assert!(ok(&["canon-check"]), "no canon files to read");
        assert!(ok(&["scripts-check"]), "skipped outside a work tree");
        #[cfg(feature = "frontend")]
        for args in [
            &["frontend-fmt", "--check"][..],
            &["frontend-fmt"],
            &["frontend-check"],
            &["frontend-dupes"],
            &["frontend-test"],
        ] {
            assert!(ok(args), "{args:?}: no frontend configured, so skipped");
        }
        for args in [
            &["check"][..],
            &["clippy"],
            &["doc"],
            &["test", "x", "--verbose", "--ignored"],
            &["test"],
            &["fmt"],
            &["coverage"],
            &["dupes"],
            &["deny"],
            &["dep-age-check", "--base", "HEAD"],
            &["dep-preflight"],
        ] {
            assert!(!ok(args), "{args:?} passed with no workspace");
        }
        // Parsing only: these would reach a registry or the advisory
        // database.
        parse(&["audit"]);
        parse(&["dep-age", "cargo", "serde", "1.0.0"]);
        parse(&["dep-age", "npm", "x", "--latest-aged", "--within", "3"]);
    }

    #[test]
    fn every_standard_step_hint_names_a_library_command() {
        // A failed step prints `cargo xtask <cmd>`; the command must
        // exist for whatever project flattens this enum in.
        use clap::CommandFactory;
        let cli = Cli::command();
        let cfg = Config::for_project("p", "/w".into());
        let steps = crate::step::standard(&cfg, false);
        #[cfg(feature = "frontend")]
        let steps =
            steps.into_iter().chain(crate::frontend::steps(&cfg, false));
        for step in steps {
            assert!(
                cli.find_subcommand(step.cmd()).is_some(),
                "step {:?} hints `{}`, which is no library command",
                step.name(),
                step.cmd()
            );
        }
    }
}
