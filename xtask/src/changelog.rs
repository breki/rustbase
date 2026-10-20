//! `cargo xtask changelog`: CLI over the shared library's
//! `CHANGELOG.md` writer.

use clap::Subcommand;
use rustbase_xtask::changelog::{self, Kind};

use crate::helpers::tasks_config;

/// `changelog` subcommands.
#[derive(Subcommand)]
pub enum ChangelogAction {
    /// Add a bullet under the `[Unreleased]` section.
    Add {
        /// Which subsection the change belongs to.
        #[arg(long, value_enum)]
        kind: Kind,
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

/// Entry point for `cargo xtask changelog <action>`.
pub fn changelog(action: ChangelogAction) -> Result<(), String> {
    let cfg = tasks_config();
    match action {
        ChangelogAction::Add {
            kind,
            breaking,
            text,
        } => {
            changelog::add(&cfg, kind, breaking, &text)?;
            println!(
                "Added to CHANGELOG [Unreleased] under {}.",
                kind.heading()
            );
        }
        ChangelogAction::InferBump => {
            println!("{}", changelog::infer_bump(&cfg)?);
        }
        ChangelogAction::Promote { version, date } => {
            changelog::promote(&cfg, &version, &date)?;
            println!("Promoted [Unreleased] to [{version}] - {date}.");
        }
    }
    Ok(())
}
