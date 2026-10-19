// The `#[cfg(feature)]` groups are the optional command groups
// declared in `Cargo.toml`.
mod audit;
#[cfg(feature = "template-tools")]
mod backfeed;
mod canon;
mod changelog;
mod check;
#[cfg(feature = "clean-cache")]
mod clean_cache;
mod clippy_cmd;
mod coverage;
mod deny;
mod dep_age_gate;
#[cfg(feature = "deploy")]
mod deploy;
#[cfg(feature = "deploy")]
mod deploy_config;
#[cfg(feature = "deploy")]
mod deploy_guard;
#[cfg(feature = "deploy")]
mod deploy_remote;
#[cfg(feature = "deploy")]
mod deploy_setup;
mod doc_cmd;
mod dupes;
#[cfg(feature = "template-tools")]
mod feedback;
mod fmt_cmd;
#[cfg(feature = "frontend")]
mod frontend;
#[cfg(feature = "frontend")]
mod frontend_check;
#[cfg(feature = "frontend")]
mod frontend_dupes;
#[cfg(feature = "frontend")]
mod frontend_fmt;
#[cfg(feature = "frontend")]
mod frontend_test;
mod helpers;
mod step;
#[cfg(feature = "template-tools")]
mod sync;
mod test_cmd;
mod todo;
mod validate;

use clap::{Parser, Subcommand};
use rustbase_xtask::dep_age;

use helpers::tasks_config;

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Type-check all targets (compiles tests, runs none)
    Check,
    /// Check the claims canon prose makes (paths, cross-refs,
    /// git grants, width, backlog IDs)
    CanonCheck,
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
    /// Run fmt + clippy + tests + coverage + duplication
    Validate {
        /// Check formatting read-only (`fmt --check`)
        /// instead of auto-fixing it in place. Use in CI
        /// or before partial staging.
        #[arg(long)]
        check: bool,
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
        ecosystem: rustbase_xtask::dep_age::Ecosystem,
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
    /// Type-check the frontend (svelte-check); skips
    /// cleanly when there is no frontend
    #[cfg(feature = "frontend")]
    FrontendCheck,
    /// Format the frontend with Prettier; skips cleanly
    /// when there is no frontend
    #[cfg(feature = "frontend")]
    FrontendFmt {
        /// Check formatting read-only instead of
        /// auto-fixing in place
        #[arg(long)]
        check: bool,
    },
    /// Check frontend code duplication (requires jscpd);
    /// skips cleanly when there is no frontend
    #[cfg(feature = "frontend")]
    FrontendDupes,
    /// Run the frontend unit suite (vitest); skips cleanly
    /// when there is no frontend
    #[cfg(feature = "frontend")]
    FrontendTest,
    /// Deploy to a remote Linux host (build frontend, sync,
    /// build on remote, restart service)
    #[cfg(feature = "deploy")]
    Deploy,
    /// One-time remote provisioning (user, dirs, service)
    #[cfg(feature = "deploy")]
    DeploySetup,
    /// Empty `target/{debug,release}/incremental/` while
    /// keeping the dirs themselves (manual invocation only)
    #[cfg(feature = "clean-cache")]
    CleanCache,
    /// Print a downstream's template-feedback entries on or
    /// after its backfeed-ledger watermark (the `/template-
    /// backfeed` delta; requires the downstream path)
    #[cfg(feature = "template-tools")]
    BackfeedDiff {
        /// Path to the downstream rustbase-derived project
        downstream_path: String,
        /// Ledger key for this downstream (default: the final
        /// path component). Use for worktree layouts like
        /// `../ledgerstone/main` whose basename is a branch.
        #[arg(long)]
        name: Option<String>,
    },
    /// Advance the backfeed-ledger watermark for a downstream
    /// after evaluating a batch of its feedback
    #[cfg(feature = "template-tools")]
    BackfeedRecord {
        /// Path to the downstream rustbase-derived project
        downstream_path: String,
        /// Newest feedback-entry date evaluated (`YYYY-MM-DD`)
        #[arg(long)]
        watermark: String,
        /// Downstream commit SHA (default: read from its `.git`)
        #[arg(long)]
        head: Option<String>,
        /// Ledger key for this downstream (default: the final
        /// path component). Use for worktree layouts like
        /// `../ledgerstone/main` whose basename is a branch.
        #[arg(long)]
        name: Option<String>,
    },
    /// Append an entry to `docs/developer/template-feedback.md`
    /// with a minted `tf-<date>-<slug>` ID (body from
    /// `--body-file` or stdin)
    #[cfg(feature = "template-tools")]
    FeedbackAdd {
        /// Which lifecycle section to add the entry to
        #[arg(long, value_enum)]
        section: feedback::FeedbackSection,
        /// Entry title (also drives the ID slug)
        #[arg(long)]
        title: String,
        /// Read the entry body from this file (default: stdin)
        #[arg(long)]
        body_file: Option<String>,
    },
    /// Print the categorized `/template-sync` file delta from
    /// `<last-synced>` to `template/main`, minus the never-sync
    /// bookkeeping set (requires git + a fetched `template/main`)
    #[cfg(feature = "template-tools")]
    SyncCandidates {
        /// The `last-synced` SHA from `.template-sync.toml`
        last_synced: String,
    },
    /// Mechanically edit `CHANGELOG.md` (used by `/commit`) --
    /// insert a bullet under the right `[Unreleased]` subsection
    Changelog {
        #[command(subcommand)]
        action: changelog::ChangelogAction,
    },
    /// Mechanically read/edit `docs/todo.md` (used by `/todo` and
    /// `/implement`) -- list, add, or complete a captured item
    Todo {
        #[command(subcommand)]
        action: todo::TodoAction,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Check => check::check(),
        XCommand::CanonCheck => canon::canon_check(),
        XCommand::Clippy => clippy_cmd::clippy(),
        XCommand::Test {
            filter,
            verbose,
            ignored,
        } => test_cmd::test(test_cmd::TestOptions {
            filter: filter.as_deref(),
            verbose,
            ignored,
        }),
        XCommand::Validate { check } => validate::validate(check),
        XCommand::Fmt => fmt_cmd::fmt(),
        XCommand::Doc => doc_cmd::doc(),
        XCommand::Coverage => coverage::coverage(),
        XCommand::Dupes => dupes::dupes(),
        XCommand::Audit => audit::audit(),
        XCommand::Deny => deny::deny(),
        XCommand::DepAge {
            ecosystem,
            package,
            version,
            latest_aged,
            within,
        } => {
            let cfg = tasks_config();
            if latest_aged {
                dep_age::dep_age_latest(
                    &cfg,
                    ecosystem,
                    &package,
                    within.as_deref(),
                )
            } else {
                dep_age::dep_age(&cfg, ecosystem, &package, version.as_deref())
            }
        }
        XCommand::DepAgeCheck { base } => {
            dep_age::dep_age_check(&tasks_config(), &base)
        }
        XCommand::DepPreflight => dep_age::dep_preflight(&tasks_config()),
        #[cfg(feature = "frontend")]
        XCommand::FrontendCheck => frontend_check::frontend_check_cmd(),
        #[cfg(feature = "frontend")]
        XCommand::FrontendFmt { check } => {
            frontend_fmt::frontend_fmt_cmd(check)
        }
        #[cfg(feature = "frontend")]
        XCommand::FrontendDupes => frontend_dupes::frontend_dupes_cmd(),
        #[cfg(feature = "frontend")]
        XCommand::FrontendTest => frontend_test::frontend_test_cmd(),
        #[cfg(feature = "deploy")]
        XCommand::Deploy => deploy::deploy(),
        #[cfg(feature = "deploy")]
        XCommand::DeploySetup => deploy_setup::deploy_setup(),
        #[cfg(feature = "clean-cache")]
        XCommand::CleanCache => clean_cache::clean_cache(),
        #[cfg(feature = "template-tools")]
        XCommand::BackfeedDiff {
            downstream_path,
            name,
        } => backfeed::backfeed_diff(&downstream_path, name.as_deref()),
        #[cfg(feature = "template-tools")]
        XCommand::BackfeedRecord {
            downstream_path,
            watermark,
            head,
            name,
        } => backfeed::backfeed_record(
            &downstream_path,
            &watermark,
            head.as_deref(),
            name.as_deref(),
        ),
        #[cfg(feature = "template-tools")]
        XCommand::FeedbackAdd {
            section,
            title,
            body_file,
        } => feedback::feedback_add(section, &title, body_file.as_deref()),
        #[cfg(feature = "template-tools")]
        XCommand::SyncCandidates { last_synced } => {
            sync::sync_candidates(&last_synced)
        }
        XCommand::Changelog { action } => changelog::changelog(action),
        XCommand::Todo { action } => todo::todo(action),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
