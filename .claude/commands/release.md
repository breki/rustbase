---
description: Cut a SemVer release from accumulated [Unreleased] CHANGELOG entries
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git add:*), Bash(git commit:*), Bash(git tag:*), Bash(git describe:*), Bash(cargo xtask validate*), Bash(cargo -q xtask validate*), Bash(cargo xtask audit*), Bash(cargo -q xtask audit*), Bash(cargo clippy:*), Bash(rustc -vV), Bash(rustup target list:*), Bash(rustup target add:*), Bash(cargo update:*), Bash(cargo xtask changelog*), Bash(cargo -q xtask changelog*), Bash(cargo xtask version-sync*), Bash(cargo -q xtask version-sync*), Bash(cargo xtask verify-release-tag*), Bash(cargo -q xtask verify-release-tag*), Bash(git ls-files:*), Read, Edit, AskUserQuestion
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
   Refuse if there are any unstaged, uncommitted or
   untracked changes: the later steps dirty
   `crates/rustbase/Cargo.toml`, `Cargo.lock`,
   `CHANGELOG.md` and the docs `version-sync` rewrites, and
   nothing else may ride along. A retry after a release that
   failed before step 9's commit starts from a clean tree
   too: ask the operator to restore those files, then begin
   again here. A failure after that commit is finished by
   hand instead (step 10). To tell which
   dirty docs are `version-sync`'s leftovers, run
   `cargo -q xtask version-sync` before anything is
   restored: with the bumped `Cargo.toml` still in place it
   changes nothing and prints its "Files with a sentinel:"
   line. Reason:
   a release commit should contain only the release
   bookkeeping; mixing it with substantive code makes
   the tag dishonest about what was reviewed.

2. **Check `[Unreleased]` has content** -- run
   `cargo -q xtask changelog infer-bump`, even when a level was
   passed: it fails with "nothing to release" when
   `[Unreleased]` holds no bullet, and then abort. Reason: a
   release with no user-visible changes is a deploy in
   disguise; if the user wants to ship the same version with
   no changes, they should just `cargo xtask deploy` (which
   will pass the tag check on the existing tag).

   One exception, checked only when the abort happens: if
   `HEAD` is a `release: vX.Y.Z` commit (`git log -1`) that
   `git describe --exact-match --match 'v*' HEAD` finds no
   tag on, an earlier run stopped at step 10. Say instead
   that the release commit has no tag, so deploy refuses it,
   and point to step 10's hand finish.

3. **Determine the bump:**
   - If the user passed `major` / `minor` / `patch`, use
     that. Skip to step 4.
   - Otherwise run `cargo -q xtask changelog infer-bump`, which
     prints the level the `[Unreleased]` bullets call for:
     - Any top-level entry starting `- **BREAKING:**` (at
       column 0; a marker quoted inside an entry does not
       count) -> **major**
     - Any non-empty `### Removed` section -> **major**
     - Any non-empty `### Added` section -> **minor**
     - Otherwise -> **patch**
   - Show the user the inference with the bullets that drove
     it -- the command prints only the level, so find them by
     reading `[Unreleased]` against the rules above -- and ask via `AskUserQuestion` whether to
     accept the inferred bump or override it. Options:
     the inferred bump (recommended), the other two
     levels, and "Abort".

4. **Compute the new version:**
   - Read the current version from
     `crates/rustbase/Cargo.toml`.
   - Apply the bump (major: `X+1.0.0`; minor: `X.Y+1.0`;
     patch: `X.Y.Z+1`).
   - Today's date in ISO format (`YYYY-MM-DD`) -- the date in
     your session context; ask the operator if there is none.
     Do not hardcode it.

5. **Edit `crates/rustbase/Cargo.toml`** -- update the
   `version = "X.Y.Z"` line.

6. **Sync `Cargo.lock` and the docs' version** -- run
   `cargo update -p rustbase`
   (updates only that package's entry). Do **not** use
   `cargo generate-lockfile` -- it refreshes every
   transitive dependency, folding an unrelated
   workspace-wide bump into the release commit and
   tripping the `dep-age-check` cooldown gate on freshly
   published transitive versions. Then run
   `cargo -q xtask version-sync`, which sets every
   `<!-- version: X -->` sentinel line in `README.md` and
   `docs/` (and the number in a `VERSION=<number>` line in
   the same files) to the new version. When any file has a
   sentinel it ends with a "Files with a sentinel:" line;
   note those files, changed this run or not. A read failure
   writes nothing; a write failure names the file and leaves
   the files before it rewritten. Fix the cause and run it
   again.

7. **Rewrite `CHANGELOG.md`** with
   `cargo -q xtask changelog promote --version X.Y.Z --date
   YYYY-MM-DD`. It refuses a malformed version or date. It
   renames the existing `## [Unreleased]` heading to
   `## [X.Y.Z] - YYYY-MM-DD` and inserts this skeleton above
   it:
     ```
     ## [Unreleased]

     ### Added

     ### Changed

     ### Fixed

     ### Removed

     ## [X.Y.Z] - YYYY-MM-DD
     ```

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
     `CHANGELOG.md` and the files `version-sync` listed as
     having a sentinel (and nothing else). If `git status`
     shows one of those as untracked, stop and ask the
     operator: an untracked draft is not release
     bookkeeping.
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

10. **Tag** -- the tag must be **annotated**, never
    lightweight (`git tag vX.Y.Z`): the deploy guard runs
    `git describe --exact-match --match 'v*' HEAD`, which
    only sees annotated tags by default. Whether it is also
    signed depends on one file. `git ls-files
    .github/release-signers` prints that path when the
    project tracks a list of keys allowed to sign releases,
    and nothing when it does not.
    - **No key list:** `git tag -a vX.Y.Z -m "Release
      vX.Y.Z"`.
    - **Key list:** `git tag -s vX.Y.Z -m "Release vX.Y.Z"`
      (a signed tag is annotated too), then **always**
      `cargo -q xtask verify-release-tag vX.Y.Z`, whatever
      `git tag -s` printed or returned. git can exit 0 and
      leave an unsigned tag (a passphrase-protected key with
      no ssh-agent does this), and the deploy guard accepts
      any annotated tag, so only the check decides.
    - **When either command fails**, keep the release commit
      and report the reason. Do not rerun `/release`: its
      `[Unreleased]` block is already promoted, so step 2
      would stop. Once the cause is fixed, the operator
      finishes by hand with the same `git tag -s` and
      `verify-release-tag` commands. This command has no
      `git config` grant, so git settings are the
      operator's to change.

      **The tag:** delete it (`git tag -d vX.Y.Z`) only when
      `git tag -s` exited 0 and the check then refused it.
      Only then did this run create it: `git tag -s` refuses
      a name that already exists. Otherwise delete nothing.

      **The cause**, by what failed:
      - `git tag -s` failed: name the error. If the tag
        already exists, report the commit it points at
        (`git log -1 vX.Y.Z`) and let the operator decide.
        With no signing key, ask the operator to set
        `gpg.format` to `ssh` and `user.signingkey` to
        their key.
      - "carries no signature": signing failed silently;
        a passphrase-protected key needs ssh-agent.
      - "not SSH-signed": git signed with GPG; ask the
        operator to set `gpg.format` to `ssh` and
        `user.signingkey` to their key.
      - "not trusted": the key is not in the list (or is
        listed for a namespace other than `git`). Fixing
        it takes a commit, which moves `HEAD` off the
        release commit, so there is no hand finish; ask the
        operator.

    Do not push; the user pushes when ready.

11. **Tell the user what to do next** -- print:
    - The new version and tag name, and whether the tag is
      signed and verified or only annotated
    - The CHANGELOG bullets that were released
    - The targets the cross-platform clippy linted, so a
      platform that was never checked is visible
    - "Next: `cargo xtask deploy` will now accept this
      HEAD." (or "Push with `git push && git push
      --tags`" if appropriate). After a step-10 failure,
      say instead that the release commit has no tag yet,
      so deploy refuses it, and how to finish (step 10).

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
  bump, leave `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`
  and any file `version-sync` rewrote modified on disk so
  the user can see the broken state, and do not commit or
  tag.
