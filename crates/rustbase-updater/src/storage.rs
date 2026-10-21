// Adapted from silkboard's `silkboard-updater` crate at silkboard tag
// v0.3.0: generic over the `App`, with typed refusals; see the crate
// README, "Provenance".

//! Versioned, policy-bound persistent state; no filesystem side effects.

use crate::{App, Release, State, Version};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Why state could not be encoded or loaded. A refused file is never
/// rewritten, so a pending transaction in it survives for the
/// operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// Written under another schema, policy or adapter.
    MigrationRequired,
    /// No `schema` field, and [`App::accepts_legacy`] said no.
    Unversioned,
    /// Malformed JSON, unknown fields, an invalid tag or transaction,
    /// or a serialization failure in `encode`; the message says which.
    Invalid(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationRequired => f.write_str(
                "state schema, policy or adapter differs; \
                 operator migration required",
            ),
            Self::Unversioned => f.write_str(
                "state has no schema; unversioned state is not \
                 accepted under this application and policy",
            ),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for StateError {}

fn invalid(error: impl fmt::Display) -> StateError {
    StateError::Invalid(error.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document<P> {
    schema: u32,
    adapter: String,
    policy: P,
    state: State,
}

/// Encode state with the exact policy and the app's adapter identity.
///
/// # Errors
/// Returns [`StateError::Invalid`] if serialization fails; durable
/// persistence belongs to the backend.
pub fn encode<A: App>(
    state: &State,
    policy: &A::Policy,
) -> Result<Vec<u8>, StateError> {
    serde_json::to_vec(&Document {
        schema: 1,
        adapter: A::ADAPTER.to_owned(),
        policy: policy.clone(),
        state: state.clone(),
    })
    .map_err(invalid)
}

/// Decode without changing bytes or discarding incompatible transactions.
/// Unversioned state loads only where [`App::accepts_legacy`] allows it.
///
/// # Errors
/// [`StateError::MigrationRequired`] for another schema, policy or
/// adapter, [`StateError::Unversioned`] for refused legacy state, and
/// [`StateError::Invalid`] for anything malformed.
pub fn decode<A: App>(
    bytes: &[u8],
    policy: &A::Policy,
) -> Result<State, StateError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(invalid)?;
    let Some(object) = value.as_object() else {
        return Err(invalid("state is not a JSON object"));
    };
    let state = if let Some(schema) = object.get("schema") {
        // Classify before the strict parse: a file from another schema,
        // adapter or policy shape needs migrating, not repair.
        let schema = schema.as_u64().ok_or_else(|| invalid("bad schema"))?;
        if schema != 1 {
            return Err(StateError::MigrationRequired);
        }
        let adapter = object
            .get("adapter")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("missing adapter"))?;
        let recorded = object
            .get("policy")
            .cloned()
            .ok_or_else(|| invalid("missing policy"))?;
        if adapter != A::ADAPTER
            || serde_json::from_value::<A::Policy>(recorded).ok().as_ref()
                != Some(policy)
        {
            return Err(StateError::MigrationRequired);
        }
        let document: Document<A::Policy> =
            serde_json::from_value(value).map_err(invalid)?;
        document.state
    } else {
        if !A::accepts_legacy(policy) {
            return Err(StateError::Unversioned);
        }
        serde_json::from_value(value).map_err(invalid)?
    };
    for tag in &state.blocked {
        if Version::parse(tag).is_none() {
            return Err(invalid("invalid blocked tag"));
        }
    }
    if let Some(pending) = &state.pending {
        for release in [&pending.previous, &pending.candidate] {
            Release::new(release.tag(), release.commit()).map_err(invalid)?;
        }
        if !state.blocked.contains(pending.candidate.tag())
            || Version::parse(pending.previous.tag())
                >= Version::parse(pending.candidate.tag())
        {
            return Err(invalid("invalid recovery transaction"));
        }
    }
    Ok(state)
}
