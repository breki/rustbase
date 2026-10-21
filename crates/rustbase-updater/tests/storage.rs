//! State binds to the app's policy and adapter; each app decides
//! whether state from before the envelope existed is accepted.

use rustbase_updater::{
    App, Release, State, Transaction,
    storage::{StateError, decode, encode},
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
        Err(StateError::MigrationRequired)
    );
    let other = Policy {
        signers: "/tmp/other".into(),
    };
    assert_eq!(
        decode::<Strict>(&bytes, &other),
        Err(StateError::MigrationRequired)
    );
}

#[test]
fn unversioned_state_is_accepted_only_where_the_app_allows_it() {
    let bytes = serde_json::to_vec(&pending()).unwrap();
    assert_eq!(
        decode::<Strict>(&bytes, &Policy::canonical()),
        Err(StateError::Unversioned)
    );
    assert_eq!(
        decode::<Legacy>(&bytes, &Policy::canonical()).unwrap(),
        pending()
    );
    let other = Policy {
        signers: "/tmp/other".into(),
    };
    assert_eq!(
        decode::<Legacy>(&bytes, &other),
        Err(StateError::Unversioned)
    );
}

#[test]
fn future_unknown_and_malformed_state_is_not_silently_reset() {
    let policy = Policy::canonical();
    let original: Value =
        serde_json::from_slice(&encode::<Strict>(&pending(), &policy).unwrap())
            .unwrap();
    let mut future = original.clone();
    future["schema"] = json!(2);
    let bytes = serde_json::to_vec(&future).unwrap();
    assert_eq!(
        decode::<Strict>(&bytes, &policy),
        Err(StateError::MigrationRequired)
    );
    let mut extra = original.clone();
    extra["extra"] = json!(true);
    let bytes = serde_json::to_vec(&extra).unwrap();
    assert!(invalid(&decode::<Strict>(&bytes, &policy)));
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
        assert!(invalid(&decode::<Strict>(&bytes, &policy)), "{state}");
        // The legacy path validates the same way.
        let bytes = serde_json::to_vec(&state).unwrap();
        assert!(invalid(&decode::<Legacy>(&bytes, &policy)), "{state}");
    }
    assert!(invalid(&decode::<Strict>(b"bad-json", &policy)));
}

fn invalid(result: &Result<State, StateError>) -> bool {
    matches!(result, Err(StateError::Invalid(_)))
}

/// The next release of `Strict`, whose policy gained a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WiderPolicy {
    signers: String,
    workflow_id: u64,
}
struct StrictNext;
impl App for StrictNext {
    type Policy = WiderPolicy;
    const ADAPTER: &'static str = Strict::ADAPTER;
    const USAGE: &'static str = Strict::USAGE;
}

#[test]
fn a_policy_of_another_shape_needs_migration_not_repair() {
    let old = encode::<Strict>(&pending(), &Policy::canonical()).unwrap();
    let wider = WiderPolicy {
        signers: Policy::canonical().signers,
        workflow_id: 7,
    };
    assert_eq!(
        decode::<StrictNext>(&old, &wider),
        Err(StateError::MigrationRequired)
    );
    let new = encode::<StrictNext>(&pending(), &wider).unwrap();
    assert_eq!(
        decode::<Strict>(&new, &Policy::canonical()),
        Err(StateError::MigrationRequired)
    );
}

#[test]
fn a_future_schema_needs_migration_whatever_fields_it_adds() {
    let policy = Policy::canonical();
    let original: Value =
        serde_json::from_slice(&encode::<Strict>(&pending(), &policy).unwrap())
            .unwrap();
    let mut top = original.clone();
    top["schema"] = json!(2);
    top["history"] = json!([]);
    let mut inner = original.clone();
    inner["schema"] = json!(2);
    inner["state"]["attempts"] = json!(3);
    for future in [top, inner] {
        let bytes = serde_json::to_vec(&future).unwrap();
        assert_eq!(
            decode::<Strict>(&bytes, &policy),
            Err(StateError::MigrationRequired),
            "{future}"
        );
    }
}

#[test]
fn a_file_that_is_not_a_state_document_is_invalid_for_every_app() {
    let policy = Policy::canonical();
    let original: Value =
        serde_json::from_slice(&encode::<Strict>(&pending(), &policy).unwrap())
            .unwrap();
    let mut no_adapter = original.clone();
    no_adapter.as_object_mut().unwrap().remove("adapter");
    let mut no_policy = original.clone();
    no_policy.as_object_mut().unwrap().remove("policy");
    let mut text_schema = original.clone();
    text_schema["schema"] = json!("1");
    for value in [
        json!(null),
        json!([]),
        json!(0),
        json!("x"),
        no_adapter,
        no_policy,
        text_schema,
    ] {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(invalid(&decode::<Strict>(&bytes, &policy)), "{value}");
        assert!(invalid(&decode::<Legacy>(&bytes, &policy)), "{value}");
    }
}

/// The refusal texts are what an operator reads; the migration one is
/// also the text jutro's updater printed.
#[test]
fn refusals_print_their_reason() {
    assert_eq!(
        StateError::MigrationRequired.to_string(),
        "state schema, policy or adapter differs; operator migration required"
    );
    assert_eq!(
        StateError::Unversioned.to_string(),
        "state has no schema; \
         unversioned state is not accepted under this application and policy"
    );
    assert_eq!(StateError::Invalid("bad tag".into()).to_string(), "bad tag");
}
