mod audit;
mod check;
mod clean_cache;
mod clippy_cmd;
mod coverage;
mod dep_age;
mod deploy;
mod deploy_config;
mod deploy_remote;
mod deploy_setup;
mod dupes;
mod fmt_cmd;
mod frontend;
mod frontend_check;
mod frontend_dupes;
mod frontend_fmt;
mod frontend_test;
mod helpers;
mod test_cmd;
mod validate;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Fast compilation check (no tests)
    Check,
    /// Run clippy (deny warnings)
    Clippy,
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
    },
    /// Cooldown-check only the dependencies added or bumped in
    /// the working tree versus HEAD (the changed-deps gate that
    /// `validate` runs; requires curl + git)
    DepAgeCheck,
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
    /// Deploy to a remote Linux host (build frontend, sync,
    /// build on remote, restart service)
    Deploy,
    /// One-time remote provisioning (user, dirs, service)
    DeploySetup,
    /// Empty `target/{debug,release}/incremental/` while
    /// keeping the dirs themselves (manual invocation only)
    CleanCache,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Check => check::check(),
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
        XCommand::Coverage => coverage::coverage(),
        XCommand::Dupes => dupes::dupes(),
        XCommand::Audit => audit::audit(),
        XCommand::DepAge {
            ecosystem,
            package,
            version,
        } => dep_age::dep_age(ecosystem, &package, version.as_deref()),
        XCommand::DepAgeCheck => dep_age::dep_age_check(),
        XCommand::FrontendCheck => frontend_check::frontend_check_cmd(),
        XCommand::FrontendFmt { check } => {
            frontend_fmt::frontend_fmt_cmd(check)
        }
        XCommand::FrontendDupes => frontend_dupes::frontend_dupes_cmd(),
        XCommand::FrontendTest => frontend_test::frontend_test_cmd(),
        XCommand::Deploy => deploy::deploy(),
        XCommand::DeploySetup => deploy_setup::deploy_setup(),
        XCommand::CleanCache => clean_cache::clean_cache(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
