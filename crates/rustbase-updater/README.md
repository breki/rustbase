# rustbase-updater

The self-upgrade engine for an application that updates itself from
signed release tags. One poll selects the highest newer stable tag
the backend admits, records a durable transaction before building,
activates the candidate, checks its health, and commits. Any failure
restores the previous release and blocks the failed tag until an
operator retries it. A poll that finds an interrupted transaction
recovers it first, before any network access.

The crate holds the state machine and the layers around it:

| Module | What it does |
|-|-|
| `engine` | `Version`, `Release`, `State`, the `Backend` trait, `poll` and `retry` |
| `outcome` | typed results and failures, with the stage each failure came from |
| `storage` | the versioned state file, bound to the app's policy and adapter |
| `cli` | command parsing and plain-text or JSON reports, without process I/O |

The application supplies everything that touches the system: a
`Backend` (fetching tags, checking signatures and its admission
policy, building, activating, health checks) and the entry point that
takes the deployment lock. Backends run with the operator's authority
and are trusted code.

## Declaring an application

What differs between applications is declared once, in an `App`
implementation:

```rust
use rustbase_updater::App;

struct Jutro;
impl App for Jutro {
    type Policy = Policy; // the app's admission policy
    const ADAPTER: &'static str = "jutro-linux-v1";
    const USAGE: &'static str =
        "usage: jutro-updater [--json] poll | status | retry TAG | inspect TAG";
    // `PROFILE` defaults to `None`: `--profile` is refused.
    // `accepts_legacy` defaults to false: unversioned state is refused.
}

// let state = storage::decode::<Jutro>(&bytes, &policy)?;
// let request = cli::parse::<Jutro>(&args)?;
// let report = cli::execute::<Jutro>(&request.operation, &mut backend, &policy)?;
```

The adapter id and the policy are written into the state file, and a
state file written under a different one is refused with
`storage::MIGRATION_REQUIRED` rather than reset. An app whose earlier
updater wrote state without the versioned envelope says so in
`accepts_legacy`; every other app refuses such state with
`storage::UNVERSIONED`.

A trait was chosen over passing the adapter, usage and legacy rule to
each call (breki/rustbase#58): the app's identity is declared once,
so a call cannot pair one app's policy with another's adapter.

## Provenance

The engine comes from silkboard's `silkboard-updater` crate (v0.3.0,
MIT, same owner), which jutro's `jutro-updater` had copied unchanged.
Moving it here replaces those hand-synced copies with one crate.

- `engine.rs`, `outcome.rs`: unchanged, except that the
  `Backend::healthy` doc no longer names silkboard's two health
  endpoints.
- `storage.rs`: generic over the `App`; jutro's named refusals
  (`MIGRATION_REQUIRED`, `UNVERSIONED`) replace silkboard's inline
  messages; silkboard's legacy-state rule becomes `accepts_legacy`.
- `cli.rs`: generic over the `App`, whose `USAGE` and `PROFILE`
  replace silkboard's hard-coded ones; `Report` is generic over the
  policy type.
- `tests/recovery.rs`, `tests/identity.rs`: unchanged apart from the
  crate name. `tests/toolkit.rs` and `tests/storage.rs` use fixture
  apps in place of silkboard's policy.

Left in the applications: silkboard's GitHub Actions evidence gate
(`evidence`, `profile`), and each app's Linux backend (accounts,
paths, build, activation, health).

## Status

Not published to crates.io yet, and neither application uses it yet:
each switches over in its own repository. Publishing follows the
manual procedure `rustbase-xtask` uses (`docs/xtask-library.md`,
"Distribution and versioning"), with a `rustbase-updater-v` tag
prefix.
