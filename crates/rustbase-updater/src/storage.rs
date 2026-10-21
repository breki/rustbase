// Adapted from silkboard's `silkboard-updater` crate (v0.3.0): generic
// over the `App`, with jutro's named refusals; see the crate README,
// "Provenance".

//! Versioned, policy-bound persistent state; no filesystem side effects.

use crate::{App, Release, State, Version};
use serde::{Deserialize, Serialize};

/// Refusal for state written under another schema, policy or adapter.
pub const MIGRATION_REQUIRED: &str =
    "state schema, policy or adapter differs; operator migration required";

/// Refusal for state without a `schema` field when the app accepts
/// no state from before the versioned envelope.
pub const UNVERSIONED: &str =
    "state has no schema, and this policy accepts no unversioned state";

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
/// Returns serialization errors; durable persistence belongs to the backend.
pub fn encode<A: App>(
    state: &State,
    policy: &A::Policy,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&Document {
        schema: 1,
        adapter: A::ADAPTER.to_owned(),
        policy: policy.clone(),
        state: state.clone(),
    })
    .map_err(|e| e.to_string())
}

/// Decode without changing bytes or discarding incompatible transactions.
/// Unversioned state loads only where [`App::accepts_legacy`] allows it.
///
/// # Errors
/// Rejects unknown schemas/fields, context mismatch and invalid transactions.
pub fn decode<A: App>(
    bytes: &[u8],
    policy: &A::Policy,
) -> Result<State, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let state = if value.get("schema").is_some() {
        let document: Document<A::Policy> =
            serde_json::from_value(value).map_err(|e| e.to_string())?;
        if document.schema != 1
            || document.policy != *policy
            || document.adapter != A::ADAPTER
        {
            return Err(MIGRATION_REQUIRED.into());
        }
        document.state
    } else {
        if !A::accepts_legacy(policy) {
            return Err(UNVERSIONED.into());
        }
        serde_json::from_value(value).map_err(|e| e.to_string())?
    };
    for tag in &state.blocked {
        if Version::parse(tag).is_none() {
            return Err("invalid blocked tag".into());
        }
    }
    if let Some(pending) = &state.pending {
        for release in [&pending.previous, &pending.candidate] {
            Release::new(release.tag(), release.commit())?;
        }
        if !state.blocked.contains(pending.candidate.tag())
            || Version::parse(pending.previous.tag())
                >= Version::parse(pending.candidate.tag())
        {
            return Err("invalid recovery transaction".into());
        }
    }
    Ok(state)
}
