//! State binds to the app's policy and adapter; each app decides
//! whether state from before the envelope existed is accepted.

use rustbase_updater::{
    App, Release, State, Transaction,
    storage::{MIGRATION_REQUIRED, UNVERSIONED, decode, encode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    signers: String,
}

impl Policy {
    fn canonical() -> Self {
        Self {
            signers: "/etc/demo/release-signers".into(),
        }
    }
}

/// Never wrote unversioned state, so accepts none.
struct Strict;
impl App for Strict {
    type Policy = Policy;
    const ADAPTER: &'static str = "strict-linux-v1";
    const USAGE: &'static str = "usage: strict-updater poll";
}

/// Accepts unversioned state only under its canonical policy.
struct Legacy;
impl App for Legacy {
    type Policy = Policy;
    const ADAPTER: &'static str = "legacy-linux-v1";
    const USAGE: &'static str = "usage: legacy-updater poll";
    fn accepts_legacy(policy: &Policy) -> bool {
        *policy == Policy::canonical()
    }
}

fn pending() -> State {
    State {
        blocked: ["v0.2.0".into()].into(),
        pending: Some(Transaction {
            previous: Release::new("v0.1.1", &"a".repeat(40)).unwrap(),
            candidate: Release::new("v0.2.0", &"b".repeat(40)).unwrap(),
        }),
    }
}

#[test]
fn versioned_state_round_trips_only_under_its_own_policy_and_adapter() {
    let policy = Policy::canonical();
    let bytes = encode::<Strict>(&pending(), &policy).unwrap();
    assert_eq!(decode::<Strict>(&bytes, &policy).unwrap(), pending());
    // Same policy type, other adapter.
    assert_eq!(
        decode::<Legacy>(&bytes, &policy),
        Err(MIGRATION_REQUIRED.into())
    );
    let other = Policy {
        signers: "/tmp/other".into(),
    };
    assert_eq!(
        decode::<Strict>(&bytes, &other),
        Err(MIGRATION_REQUIRED.into())
    );
}

#[test]
fn unversioned_state_is_accepted_only_where_the_app_allows_it() {
    let bytes = serde_json::to_vec(&pending()).unwrap();
    assert_eq!(
        decode::<Strict>(&bytes, &Policy::canonical()),
        Err(UNVERSIONED.into())
    );
    assert_eq!(
        decode::<Legacy>(&bytes, &Policy::canonical()).unwrap(),
        pending()
    );
    let other = Policy {
        signers: "/tmp/other".into(),
    };
    assert_eq!(decode::<Legacy>(&bytes, &other), Err(UNVERSIONED.into()));
}

#[test]
fn future_unknown_and_malformed_state_is_not_silently_reset() {
    let policy = Policy::canonical();
    let original: Value =
        serde_json::from_slice(&encode::<Strict>(&pending(), &policy).unwrap())
            .unwrap();
    for (key, value) in [("schema", json!(2)), ("extra", json!(true))] {
        let mut bad = original.clone();
        bad[key] = value;
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(decode::<Strict>(&bytes, &policy).is_err(), "{key}");
    }
    let release =
        |tag: &str, commit: &str| json!({"tag": tag, "commit": commit});
    let (a, b) = ("a".repeat(40), "b".repeat(40));
    for state in [
        json!({"blocked": ["../bad"], "pending": null}),
        // The candidate of a pending transaction must be blocked.
        json!({"blocked": [], "pending": {
            "previous": release("v0.1.1", &a),
            "candidate": release("v0.2.0", &b)}}),
        // The previous release must be older than the candidate.
        json!({"blocked": ["v0.2.0"], "pending": {
            "previous": release("v0.3.0", &a),
            "candidate": release("v0.2.0", &b)}}),
        json!({"blocked": ["v0.2.0"], "pending": {
            "previous": release("v0.1.1", "bad"),
            "candidate": release("v0.2.0", &b)}}),
        json!({"blocked": [], "pending": null, "extra": true}),
    ] {
        let mut bad = original.clone();
        bad["state"] = state.clone();
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(decode::<Strict>(&bytes, &policy).is_err(), "{state}");
        // The legacy path validates the same way.
        let bytes = serde_json::to_vec(&state).unwrap();
        assert!(decode::<Legacy>(&bytes, &policy).is_err(), "{state}");
    }
    assert!(decode::<Strict>(b"bad-json", &policy).is_err());
}
