// Adapted from silkboard's `silkboard-updater` crate (v0.3.0): generic
// over the `App`, whose usage and profile replace silkboard's; see the
// crate README, "Provenance".

//! Operator command parsing and reports, reusable without process I/O.
//!
//! Callers must serialize operations using the deployment lock; the
//! installed entry point takes it before calling [`execute`].

use crate::{
    App, Backend, State, Version,
    outcome::{Admission, Error, Outcome, Stage},
};
use serde::Serialize;

/// A parsed operation; tags are validated before any backend call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Poll once or recover interrupted work.
    Poll,
    /// Inspect local state without network access.
    Status,
    /// Inspect the named tag without a build or activation.
    Inspect(String),
    /// Explicitly clear a failed-tag block.
    Retry(String),
    /// Display syntax without loading configuration.
    Help,
}

/// A parsed request for the app's registered adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Emit machine-readable reports instead of plain text.
    pub json: bool,
    /// Selected operation.
    pub operation: Operation,
}

/// Parse a command for app `A`. `--profile` accepts only
/// [`App::PROFILE`]; arbitrary configuration paths are forbidden.
///
/// # Errors
/// Rejects unknown profiles, malformed tags and unsupported command syntax.
pub fn parse<A: App>(arguments: &[String]) -> Result<Request, Error> {
    let mut arguments = arguments.iter().map(String::as_str).peekable();
    let json = if arguments.peek() == Some(&"--json") {
        arguments.next();
        true
    } else {
        false
    };
    if arguments.peek() == Some(&"--profile") {
        arguments.next();
        if A::PROFILE.is_none() || arguments.next() != A::PROFILE {
            return Err(Error::new(
                Stage::Configuration,
                "application profile is not registered",
            ));
        }
    }
    let arguments: Vec<_> = arguments.collect();
    let operation = match arguments.as_slice() {
        ["poll"] => Operation::Poll,
        ["status"] => Operation::Status,
        ["--help" | "-h"] => Operation::Help,
        ["inspect", tag] if Version::parse(tag).is_some() => {
            Operation::Inspect((*tag).into())
        }
        ["retry", tag] if Version::parse(tag).is_some() => {
            Operation::Retry((*tag).into())
        }
        _ => return Err(Error::new(Stage::Usage, A::USAGE)),
    };
    Ok(Request { json, operation })
}

/// A structured, versioned command result. No credentials are included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Report<P> {
    /// Completed one poll/recovery operation.
    Polled {
        /// State-machine result.
        result: Outcome,
    },
    /// Read local deployment state successfully.
    Status {
        /// Installed application/layout contract ([`App::ADAPTER`]).
        adapter: String,
        /// The admission policy the updater runs under.
        policy: P,
        /// Current pending transaction and failed-tag blocks.
        state: State,
    },
    /// Inspected eligibility without building or activating.
    Inspected {
        /// Canonical requested tag.
        tag: String,
        /// Typed eligibility, including waiting/rejection.
        admission: Admission,
    },
    /// Cleared the named block; deployment remains subject to admission.
    Retried {
        /// Canonical retried tag.
        tag: String,
    },
    /// Syntax only; no configuration or backend access.
    Help {
        /// Command syntax ([`App::USAGE`]).
        usage: String,
    },
}

/// Execute a parsed command against a trusted, serialized backend.
///
/// # Errors
/// Reports boundary failures without treating unavailable evidence as success.
pub fn execute<A: App>(
    operation: &Operation,
    backend: &mut impl Backend,
    policy: &A::Policy,
) -> Result<Report<A::Policy>, Error> {
    if let Operation::Inspect(tag) | Operation::Retry(tag) = operation
        && Version::parse(tag).is_none()
    {
        return Err(Error::new(Stage::Usage, "invalid stable tag"));
    }
    let result = match operation {
        Operation::Poll => Report::Polled {
            result: crate::poll(backend)?,
        },
        Operation::Status => Report::Status {
            adapter: A::ADAPTER.to_owned(),
            policy: policy.clone(),
            state: backend.load().map_err(|e| Error::new(Stage::State, e))?,
        },
        Operation::Inspect(tag) => {
            backend.refresh().map_err(|e| Error::new(Stage::Fetch, e))?;
            Report::Inspected {
                tag: tag.clone(),
                admission: backend
                    .inspect(tag)
                    .map_err(|e| Error::new(Stage::Admission, e))?,
            }
        }
        Operation::Retry(tag) => {
            crate::retry(backend, tag)?;
            Report::Retried { tag: tag.clone() }
        }
        Operation::Help => Report::Help {
            usage: A::USAGE.to_owned(),
        },
    };
    Ok(result)
}

/// Render the existing human interface; ineligible inspection remains a failure.
///
/// # Errors
/// Rejects ineligible inspection and reports serialization failure.
pub fn human<P>(report: &Report<P>) -> Result<String, Error> {
    Ok(match report {
        Report::Polled { result } => match result {
            Outcome::Unchanged { current, .. } => {
                format!("No newer eligible release; current {}", current.tag())
            }
            Outcome::Activated { current, .. } => {
                format!("Activated {} ({})", current.tag(), current.commit())
            }
            Outcome::Recovered { current, .. } => {
                format!("Restored {} after incomplete upgrade", current.tag())
            }
        },
        Report::Status { state, .. } => serde_json::to_string_pretty(state)
            .map_err(|e| Error::new(Stage::State, e.to_string()))?,
        Report::Inspected {
            admission: Admission::Eligible { release },
            ..
        } => format!("Eligible {} ({})", release.tag(), release.commit()),
        Report::Inspected { .. } => {
            return Err(Error::new(
                Stage::Admission,
                "release is not eligible",
            ));
        }
        Report::Retried { tag } => {
            format!("Retry enabled for {tag}; admission remains required")
        }
        Report::Help { usage } => usage.clone(),
    })
}

/// Serialize a versioned machine report, reserving stdout for this document.
///
/// # Errors
/// Returns report serialization errors.
pub fn machine<P: Serialize>(report: &Report<P>) -> Result<String, Error> {
    #[derive(Serialize)]
    struct Envelope<'a, P: Serialize> {
        schema: u32,
        #[serde(flatten)]
        report: &'a Report<P>,
    }
    serde_json::to_string(&Envelope { schema: 1, report })
        .map_err(|e| Error::new(Stage::State, e.to_string()))
}
