# rustbase-xtask

Shared `cargo xtask` tooling for projects derived from the
[rustbase](https://github.com/breki/rustbase) template: the quality
gates (format, Clippy, tests, coverage, duplication, docs, licences,
security audit, dependency cooldown, canon and record checks) and the
helper commands (changelog, todo, version sync, third-party licence
file, release-tag check).

A project's own `xtask` crate depends on this library and keeps only
its CLI and its project-specific commands, so a fix to a gate reaches
every project as a dependency bump rather than a hand merge.

The defaults assume the rustbase layout (`docs/todo.md`, `.claude/`,
`CHANGELOG.md` in Keep a Changelog form). It is not meant as a
general-purpose xtask library.

## Use

```toml
# xtask/Cargo.toml
[dependencies]
rustbase-xtask = { version = "0.1", features = ["clap"] }
```

```rust,ignore
use rustbase_xtask::{Config, cli, step};

// What differs between projects: names, paths, thresholds.
let cfg = Config::for_project("my-app", workspace_root);

// The library's subcommands, flattened into your own clap CLI and
// run with `cli::run(cmd, &cfg)`.
// `validate`: the standard gates in order, plus your own steps
// placed with `step::insert_after`, run by `step::run_all`.
let steps = step::standard(&cfg, check_only);
```

The `clap` feature enables `cli`; leave it out if you write your own
CLI. The template's
[`xtask/`](https://github.com/breki/rustbase/tree/main/xtask) is the
reference wiring.

## Licence

MIT. See `LICENSE`.
