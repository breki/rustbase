//! Canonical release and signed tag header boundaries.
use rustbase_updater::{Release, Version};

#[test]
fn canonical_stable_versions_order_numerically() {
    for tag in [
        "",
        "1.2.3",
        "v1.2",
        "v1.2.3.4",
        "v01.2.3",
        "v1.02.3",
        "v1.2.03",
        "v1.2.3-rc.1",
        "v1.2.3+meta",
        "v1.2.3\n",
        "v1..3",
        "v1.2.9999999999999999999999999999",
        "v../..",
        "v١.2.3",
    ] {
        assert!(Version::parse(tag).is_none(), "{tag}");
        assert!(Release::new(tag, &"a".repeat(40)).is_err());
    }
    assert!(Version::parse("v0.10.0") > Version::parse("v0.9.999"));
    for commit in ["", "abc", &"A".repeat(40), &"z".repeat(40), &"a".repeat(41)]
    {
        assert!(Release::new("v1.2.3", commit).is_err());
    }
}

#[test]
fn signed_object_must_bind_exact_name_and_direct_commit() {
    let object = format!(
        "object {}\ntype commit\ntag v1.2.3\ntagger Test\n\nrelease\n-----BEGIN SSH SIGNATURE-----\nbody\n-----END SSH SIGNATURE-----\n",
        "a".repeat(40)
    );
    assert!(Release::from_tag("v1.2.3", &object).is_ok());
    for bad in [
        String::new(),
        object.replace("object ", "other "),
        object.replace("type commit", "type tag"),
        object.replace("tag v1.2.3", "tag other"),
        object.replace("SSH SIGNATURE", "PGP SIGNATURE"),
        object.trim_end().into(),
        object.replace(&"a".repeat(40), "invalid"),
    ] {
        assert!(Release::from_tag("v1.2.3", &bad).is_err());
    }
}

#[test]
fn deserialization_cannot_bypass_release_identity_validation() {
    for identity in [
        serde_json::json!({"tag":"../bad","commit":"a".repeat(40)}),
        serde_json::json!({"tag":"v0.2.0","commit":"bad"}),
        serde_json::json!({"tag":"v0.2.0","commit":"a".repeat(40),"unknown":true}),
    ] {
        assert!(serde_json::from_value::<Release>(identity).is_err());
    }
}
