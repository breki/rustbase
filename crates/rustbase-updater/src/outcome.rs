// Copied from silkboard's `silkboard-updater` crate (v0.3.0, MIT, same
// owner); see the crate README, "Provenance".

//! Structured library outcomes, independent of terminal output.

use crate::Release;
use serde::Serialize;
use std::{collections::BTreeSet, fmt};

/// Candidate eligibility; rejected and waiting tags may be reconsidered later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "admission", rename_all = "snake_case")]
pub enum Admission {
    /// Both independently installed trust gates succeeded.
    Eligible {
        /// Exact admitted source identity.
        release: Release,
    },
    /// Required release evidence is not complete yet.
    Waiting {
        /// Non-secret explanation.
        reason: String,
    },
    /// Evidence or signature failed the installed policy.
    Rejected {
        /// Non-secret explanation.
        reason: String,
    },
    /// A compatibility backend supplied no classification.
    NotAdmitted,
}

impl Admission {
    /// Return identity only for an eligible candidate.
    #[must_use]
    pub fn release(&self) -> Option<&Release> {
        match self {
            Self::Eligible { release } => Some(release),
            _ => None,
        }
    }
}

/// A considered tag that was not eligible during this poll.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// Canonical stable tag.
    pub tag: String,
    /// Waiting, rejected or unspecified eligibility.
    pub admission: Admission,
}

/// One serialized engine operation's successful result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The current release remains active; no candidate was eligible.
    Unchanged {
        /// Running identity checked before selection.
        current: Release,
        /// Why newer candidates were not deployed.
        not_admitted: Vec<Candidate>,
    },
    /// A new release passed health and its transaction was durably committed.
    Activated {
        /// Retained previous release.
        previous: Release,
        /// Newly active identity.
        current: Release,
    },
    /// A persisted transaction recovered before any network access.
    Recovered {
        /// Restored release.
        current: Release,
        /// Candidates still requiring manual retry.
        blocked: BTreeSet<String>,
    },
}

/// Boundary at which a deployment operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Invalid or incompatible operator policy.
    Configuration,
    /// Invalid command syntax.
    Usage,
    /// Persistent state read, validation or durable write.
    State,
    /// Current release identity.
    Current,
    /// Running application health or identity.
    Health,
    /// Remote reference refresh.
    Fetch,
    /// Signature or release-suite admission.
    Admission,
    /// Unprivileged build or protected staging.
    Build,
    /// Atomic activation or service restart.
    Activation,
    /// Restoration of the previous release.
    Recovery,
    /// Invalid manual retry request.
    Retry,
}

/// A classified failure, including the separately reported recovery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Error {
    /// Original failed boundary.
    pub stage: Stage,
    /// Non-secret boundary diagnostic.
    pub message: String,
    /// Candidate blocked during an attempted upgrade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<Box<Release>>,
    /// Previous release confirmed healthy after the failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored: Option<Box<Release>>,
    /// Recovery itself failed; the durable transaction remains pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Box<Self>>,
}

impl Error {
    /// Construct a boundary failure before recovery is attempted.
    #[must_use]
    pub fn new(stage: Stage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            candidate: None,
            restored: None,
            recovery: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.stage, self.message)?;
        if let Some(release) = &self.restored {
            write!(f, "; restored {}", release.tag())?;
        }
        if let Some(error) = &self.recovery {
            write!(f, "; recovery failed: {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
