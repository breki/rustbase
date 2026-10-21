//! Self-upgrade engine for an application that updates itself from
//! signed release tags: release selection, a durable upgrade
//! transaction, recovery and retry, plus the state file and command
//! layer around them.
//!
//! The application supplies the rest. Its [`Backend`] does every
//! boundary operation (fetch, admission, build, activation, health,
//! loading and saving state), and its [`App`] names what differs
//! between applications. Backends are trusted operator code; identity
//! types are not authorization. The crate takes no lock: the caller
//! must run `poll`, `retry` and `cli::execute` one at a time. The
//! crate README defines admission, policy, adapter and profile.

pub mod cli;
pub mod engine;
pub mod outcome;
pub mod storage;

use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

pub use engine::{Backend, Release, State, Transaction, Version, poll, retry};

/// What one application declares once: its admission policy type,
/// the identity its state is bound to, and its command syntax.
pub trait App {
    /// The operator-installed configuration of the backend's
    /// admission checks. The engine never reads it; persisted state
    /// records it and loads only under an equal one.
    type Policy: Clone + Debug + Eq + Serialize + DeserializeOwned;

    /// Names the app's backend with the paths, accounts and service
    /// it manages; recorded in state, and state under another id is
    /// refused. Change it exactly when the new backend can no longer
    /// act on state the old one wrote.
    const ADAPTER: &'static str;

    /// The command syntax, returned for `--help` and for a request
    /// that does not parse.
    const USAGE: &'static str;

    /// The one profile name `--profile` accepts. With `None` the
    /// option is not part of the syntax.
    const PROFILE: Option<&'static str> = None;

    /// Whether unversioned state -- a bare `{blocked, pending}` object
    /// with no `schema`, written before the `{schema, adapter, policy,
    /// state}` envelope -- may load under `policy`. Only an application
    /// whose earlier updater wrote such state should say yes.
    #[must_use]
    fn accepts_legacy(policy: &Self::Policy) -> bool {
        let _ = policy;
        false
    }
}
