---
description: Cut a SemVer release from accumulated [Unreleased] CHANGELOG entries
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git add:*), Bash(git commit:*), Bash(git tag:*), Bash(git describe:*), Bash(cargo xtask validate*), Bash(cargo -q xtask validate*), Bash(cargo xtask audit*), Bash(cargo -q xtask audit*), Bash(cargo clippy:*), Bash(rustc -vV), Bash(rustup target list:*), Bash(rustup target add:*), Bash(cargo update:*), Bash(cargo xtask changelog*), Bash(cargo -q xtask changelog*), Read, Edit, AskUserQuestion
---

Cut a SemVer release: bump the version, promote the
`[Unreleased]` block in `CHANGELOG.md` to a dated release
section, validate, commit, and tag.

`/release` is the prerequisite for `cargo xtask deploy` --
the deploy task refuses to ship a `HEAD` that is not on a
`vX.Y.Z` tag matching `Cargo.toml`.

## Usage

```
/release                # infer bump from CHANGELOG, then ask
/release patch          # force patch bump
/release minor          # force minor bump
/release major          # force major bump
```

## Instructions

1. **Check working tree is clean** -- Run `git status`.
   Refuse if there are unstaged or uncommitted changes
   other than ones this skill is about to introduce
   (`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`). Reason:
   a release commit should contain only the release
   bookkeeping; mixing it with substantive code makes
   the tag dishonest about what was reviewed.

2. **Check `[Unreleased]` has content** -- Read
   `CHANGELOG.md`. If the `[Unreleased]` section is empty
   or contains only empty subheadings, abort with
   "nothing to release". Reason: a release with no
   user-visible changes is a deploy in disguise; if the
   user wants to ship the same version with no changes,
   they should just `cargo xtask deploy` (which will pass
   the tag check on the existing tag).

3. **Determine the bump:**
   - If the user passed `major` / `minor` / `patch`, use
     that. Skip to step 4.
   - Otherwise run `cargo -q xtask changelog infer-bump`, which
     prints the level the `[Unreleased]` bullets call for:
     - Any bullet starting with `**BREAKING:**` -> **major**
     - Any non-empty `### Removed` section -> **major**
     - Any non-empty `### Added` section -> **minor**
     - Otherwise -> **patch**

     It fails when `[Unreleased]` has no bullet, which is step
     2's "nothing to release".
   - Show the user the inference (with the bullets that
     drove it) and ask via `AskUserQuestion` whether to
     accept the inferred bump or override it. Options:
     the inferred bump (recommended), the other two
     levels, and "Abort".

4. **Compute the new version:**
   - Read the current version from
     `crates/rustbase/Cargo.toml`.
   - Apply the bump (major: `X+1.0.0`; minor: `X.Y+1.0`;
     patch: `X.Y.Z+1`).
   - Today's date in ISO format (`YYYY-MM-DD`) -- use
     the date the system provides; do not hardcode.

5. **Edit `crates/rustbase/Cargo.toml`** -- update the
   `version = "X.Y.Z"` line.

6. **Sync `Cargo.lock`** -- run `cargo update -p rustbase`
   (updates only that package's entry). Do **not** use
   `cargo generate-lockfile` -- it refreshes every
   transitive dependency, folding an unrelated
   workspace-wide bump into the release commit and
   tripping the `dep-age-check` cooldown gate on freshly
   published transitive versions.

7. **Rewrite `CHANGELOG.md`** with
   `cargo -q xtask changelog promote --version X.Y.Z --date
   YYYY-MM-DD`. It renames the existing `## [Unreleased]`
   heading to `## [X.Y.Z] - YYYY-MM-DD` and inserts a fresh
   empty `[Unreleased]` skeleton above it, refusing a
   malformed version or date:
     ```
     ## [Unreleased]

     ### Added

     ### Changed

     ### Fixed

     ### Removed

     ## [X.Y.Z] - YYYY-MM-DD
     ```
     (Subheadings with no bullets can be left in place;
     they signal the categories for future entries.)

8. **Run the release gates** -- three checks, all before
   the tag, because the tag is what `cargo xtask deploy`
   trusts:
   - **`cargo -q xtask validate`** -- the umbrella gate. If it
     fails, abort and tell the user what failed; do not
     commit a broken release.
   - **Cross-platform clippy.** `validate` lints only the
     platform you are on, but the template claims Windows,
     Linux and macOS as first-class. A release cut after a
     green local `validate` can still fail CI on the two
     platforms nobody checked -- so lint the claimed
     platforms **other than the host** before the tag, not
     after. The claimed set is `x86_64-pc-windows-msvc`,
     `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`;
     do not iterate over every installed target (a `wasm32`
     or embedded target installed for another project would
     fail on this workspace). `rustc -vV` prints the host on
     its `host:` line, and `rustup target list --installed`
     shows which claimed targets are present. For each:
     ```bash
     cargo clippy --workspace --all-targets \
       --target <target> -- -D warnings
     ```
     Linting the host's own target repeats what `validate`
     just did and checks no other platform. If a claimed
     non-host target is missing, add it with
     `rustup target add <target>` rather than skip it.
     It must be **clippy**, not `cargo check --target`:
     `check --target` compiles and runs no lints, so it
     proves the build and says nothing about the lints, so a
     green cross-target check can still hide a lint failure.
     No linker for the other
     platform is needed -- only the cfg analysis and lints
     run -- so this costs seconds on a warm target dir.
   - **Standalone `cargo -q xtask audit`.** `validate`'s audit
     step degrades to a *warning* on a missing tool or an
     unreachable advisory DB, so "Validate OK" does **not**
     by itself mean the dependencies were audited. The
     standalone command errors on both, and a release is
     exactly when that answer must be trustworthy -- an
     advisory is almost always filed against already-shipped
     code, so no changed-deps check (`dep-age-check`
     included) can catch it. Abort if it fails.

9. **Stage and commit:**
   - Stage `crates/rustbase/Cargo.toml`, `Cargo.lock`,
     `CHANGELOG.md` (and nothing else).
   - Commit directly with `git commit` (do **not** route
     through `/commit` -- a release commit is a
     single-purpose bookkeeping commit; this
     is the documented exception to the "all commits go
     through `/commit`" rule).
   - Use this message format (HEREDOC):
     ```bash
     git commit -m "$(cat <<'EOF'
     release: vX.Y.Z

     <one-line summary derived from the [Unreleased]
     bullets being released>

     AI-Generated: Claude Code (<ModelName> <YYYY-MM-DD>)
     EOF
     )"
     ```

10. **Tag** -- create an **annotated** tag:
    `git tag -a vX.Y.Z -m "Release vX.Y.Z"`. Do **not**
    use a lightweight tag (`git tag vX.Y.Z`) -- the
    deploy guard runs `git describe --exact-match
    --match 'v*' HEAD`, which only sees annotated tags
    by default. Do not push; the user pushes when ready.

11. **Tell the user what to do next** -- print:
    - The new version and tag name
    - The CHANGELOG bullets that were released
    - The targets the cross-platform clippy linted, so a
      platform that was never checked is visible
    - "Next: `cargo xtask deploy` will now accept this
      HEAD." (or "Push with `git push && git push
      --tags`" if appropriate)

    This command has no `git push` grant, deliberately: the
    operator pushes.

## Rules

- One release per commit. Never bundle a release with
  unrelated code changes.
- Never push tags automatically.
- Never edit closed (already-dated) release sections of
  `CHANGELOG.md`; only the `[Unreleased]` block is
  mutable.
- If `cargo xtask validate` fails after the version
  bump, leave `Cargo.toml`, `Cargo.lock`, and
  `CHANGELOG.md` modified on disk so the user can see
  the broken state, and do not commit or tag.
