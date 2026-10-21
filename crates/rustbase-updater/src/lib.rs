//! Self-upgrade engine for an application that updates itself from
//! signed release tags: release selection, a durable upgrade
//! transaction, recovery and retry, plus the state file and command
//! layer around them.
//!
//! The application supplies the rest. Its [`Backend`] does every
//! boundary operation (fetch, signature and policy checks, build,
//! activation, health), and its [`App`] names what differs between
//! applications. Backends are trusted operator code; identity types
//! are not authorization.

pub mod cli;
pub mod engine;
pub mod outcome;
pub mod storage;

use serde::{Serialize, de::DeserializeOwned};

pub use engine::{Backend, Release, State, Transaction, Version, poll, retry};

/// What one application declares once: its admission policy type,
/// the identity its state is bound to, and its command syntax.
pub trait App {
    /// The operator-installed admission policy. Persisted state
    /// records it and loads only under an equal one.
    type Policy: Clone + PartialEq + Serialize + DeserializeOwned;

    /// The installed adapter and layout contract, recorded in state.
    /// State written under another adapter is refused.
    const ADAPTER: &'static str;

    /// The command syntax, returned for `--help` and for a request
    /// that does not parse.
    const USAGE: &'static str;

    /// The one profile name `--profile` accepts. `None` refuses the
    /// option.
    const PROFILE: Option<&'static str> = None;

    /// Whether state written before the versioned envelope may load
    /// under `policy`. Only an application whose earlier updater
    /// wrote such state should say yes.
    #[must_use]
    fn accepts_legacy(policy: &Self::Policy) -> bool {
        let _ = policy;
        false
    }
}
