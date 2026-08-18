# rustbase

Core library and CLI binary for a rustbase-derived
project.

This crate is the package that gets published to
crates.io, so this file is what crates.io renders on the
crate page. It is deliberately short -- the full project
documentation lives in the [workspace
README](https://github.com/breki/rustbase#readme).

## Usage

As a binary:

```bash
cargo run -p rustbase -- --help
```

As a library, add it as a dependency and use the public
API (every public item is documented; build the docs with
`cargo doc --open`).

## Why a crate-level README

`package.readme` must point at a file **inside** the
package directory: `cargo package` only includes files
under the crate root, so a manifest pointing at
`../../README.md` ships a `.crate` whose readme is
missing. Clippy's `cargo` lint group also requires
`package.readme` to be set, so the key cannot simply be
dropped. Keeping a small readme here satisfies both, and
avoids duplicating (and drifting from) the workspace
README.

## License

MIT. See the workspace `LICENSE`.
