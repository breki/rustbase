# rustbase-updater

The self-upgrade engine for an application that updates itself from
signed release tags. One poll selects the highest newer stable tag
the backend admits, records a durable transaction before building,
activates the candidate, checks its health, and commits. A failed
upgrade restores the previous release and blocks the failed tag until
an operator retries it. If the restore or the final commit fails, the
transaction stays pending, and the next poll recovers it first,
before any network access.

The crate holds the state machine and the layers around it:

| Module | What it does |
|-|-|
| `engine` | `Version`, `Release`, `State`, the `Backend` trait, `poll` and `retry` |
| `outcome` | typed results and failures, with the stage each failure came from |
| `storage` | the versioned state file, bound to the app's policy and adapter |
| `cli` | command parsing and plain-text or JSON reports, without process I/O |

The application supplies everything that touches the system: a
`Backend` (fetching tags, admission, building, activating, health
checks, and loading and saving state) and the entry point. Backends
run with the operator's authority and are trusted code.

## Terms

- **Admission**: the backend's decision that a signed tag may be
  installed. The tag's signature must verify, and any further release
  checks the app requires must pass (silkboard also requires a
  finished CI run). What those checks read is the **evidence**: an
  error saying evidence was unavailable never counts as admission. The engine asks through `Backend::inspect`, which
  answers eligible, waiting, rejected, or "no reason given"
  (`NotAdmitted`); its default derives the answer from
  `Backend::admit` for a backend that cannot tell waiting from
  rejected.
- **Policy**: the operator-installed configuration of those checks
  (trusted signers, required CI jobs). The engine never reads it; the
  state file records it, so state written under one policy does not
  load under another.
- **Adapter**: the app's backend together with the paths, accounts
  and service it manages, named by an id such as `jutro-linux-v1`.
  Change the id exactly when state written by the old backend can no
  longer be acted on by the new one; state carrying the old id is
  then refused instead of acted on.
- **Profile**: the name an installed command line may pass as
  `--profile <name>`. The name is checked against the app's one
  registered name and not used further: it keeps a command line that
  names its target explicitly (silkboard's `deployctl`) from
  silently reaching another app.
- **Versioned envelope**: the state file as `encode` writes it,
  `{schema, adapter, policy, state}`. **Unversioned** (legacy) state
  is a bare `{blocked, pending}` object with no `schema`, which
  silkboard's first updater wrote.
- **Lock**: the crate takes none. The app's entry point must hold an
  exclusive lock across each `poll`, `retry` or `cli::execute` call,
  so only one runs at a time.

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
    // `PROFILE` defaults to `None`: `--profile` is a usage error.
    // `accepts_legacy` defaults to false: unversioned state is refused.
}

// In the backend, state goes through `storage`:
//   fn load(&mut self) -> Result<State, String> {
//       storage::decode::<Jutro>(&read_file()?, &self.policy)
//           .map_err(|e| e.to_string())
//   }
//   `save` writes `storage::encode::<Jutro>(state, &self.policy)`.
//
// In the entry point, holding the lock:
//   let request = cli::parse::<Jutro>(&args)?;
//   let report = cli::execute::<Jutro>(&request.operation, &mut backend, &policy)?;
```

The adapter id and the policy are written into the state file, and a
state file written under a different one -- another adapter, another
schema, or a policy that differs in value or in shape -- is refused
with `StateError::MigrationRequired` rather than reset. An app whose
earlier updater wrote state without the versioned envelope says so in
`accepts_legacy`; every other app refuses such state with
`StateError::Unversioned`. Anything malformed is
`StateError::Invalid`, so a caller can branch on the reason without
matching message text.

A trait was chosen over passing the adapter, usage and legacy rule to
each call (breki/rustbase#58): the app's identity is declared once,
so a call cannot pair one app's policy with another's adapter.

## Provenance

The engine comes from silkboard's `silkboard-updater` crate at
silkboard tag v0.3.0 (MIT, same owner), which jutro's `jutro-updater`
had copied unchanged.
Moving it here replaces those hand-synced copies with one crate.

- `engine.rs`, `outcome.rs`: code unchanged. Five doc comments
  describe this crate rather than silkboard or its history:
  `Backend::healthy` (silkboard's two health endpoints),
  `Backend::inspect` (its default), `Admission::Eligible`
  (silkboard's two trust gates), `Admission::NotAdmitted`
  ("compatibility backend") and `Outcome` ("serialized").
- `storage.rs`: generic over the `App`; errors are a `StateError`
  enum where both sources returned strings. The migration refusal
  keeps jutro's text; the unversioned one names no application, since
  it covers both jutro's refuse-all rule and silkboard's
  canonical-policy rule, which becomes `accepts_legacy`.
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

Published on crates.io; 0.1.0 went out on 2026-10-21, tagged
`rustbase-updater-v0.1.0`. Neither application uses it yet: each
switches over in its own repository. A release follows the manual
procedure `rustbase-xtask` uses (`docs/xtask-library.md`,
"Distribution and versioning"), with a `rustbase-updater-v` tag
prefix.
