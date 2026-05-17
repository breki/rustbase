mod check;
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
    },
    /// Run fmt + clippy + tests + coverage + duplication
    Validate,
    /// Format code
    Fmt,
    /// Run coverage check (requires cargo-llvm-cov)
    Coverage,
    /// Run code duplication check (requires code-dupes)
    Dupes,
    /// Deploy to a remote Linux host (build frontend, sync,
    /// build on remote, restart service)
    Deploy,
    /// One-time remote provisioning (user, dirs, service)
    DeploySetup,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Check => check::check(),
        XCommand::Clippy => clippy_cmd::clippy(),
        XCommand::Test { filter, verbose } => {
            test_cmd::test(filter.as_deref(), verbose)
        }
        XCommand::Validate => validate::validate(),
        XCommand::Fmt => fmt_cmd::fmt(),
        XCommand::Coverage => coverage::coverage(),
        XCommand::Dupes => dupes::dupes(),
        XCommand::Deploy => deploy::deploy(),
        XCommand::DeploySetup => deploy_setup::deploy_setup(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
