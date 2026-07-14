mod check;
mod clean_cache;
mod clippy_cmd;
mod coverage;
mod deploy;
mod deploy_config;
mod deploy_remote;
mod deploy_setup;
mod dupes;
mod fmt_cmd;
mod frontend_check;
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
    /// Type-check the frontend (svelte-check); skips
    /// cleanly when there is no frontend
    FrontendCheck,
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
        XCommand::FrontendCheck => frontend_check::frontend_check_cmd(),
        XCommand::Deploy => deploy::deploy(),
        XCommand::DeploySetup => deploy_setup::deploy_setup(),
        XCommand::CleanCache => clean_cache::clean_cache(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
