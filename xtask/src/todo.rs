//! `cargo xtask todo`: CLI over the shared library's `docs/todo.md`
//! queue (one `### <slug>` entry per item; completing one removes
//! it).

use clap::Subcommand;
use rustbase_xtask::todo;

use crate::helpers::tasks_config;

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

/// Entry point for `cargo xtask todo <action>`.
pub fn todo(action: TodoAction) -> Result<(), String> {
    let cfg = tasks_config();
    match action {
        TodoAction::List => {
            for item in todo::list(&cfg)? {
                println!("{item}");
            }
        }
        TodoAction::Add {
            slug,
            summary,
            body,
        } => {
            todo::add(&cfg, &slug, &summary, body.as_deref())?;
            println!("Added todo '{slug}'.");
        }
        TodoAction::Done { slug } => {
            todo::done(&cfg, &slug)?;
            println!("Removed '{slug}' from the queue.");
        }
    }
    Ok(())
}
