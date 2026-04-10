# Red Team Findings -- Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-012

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

### RT-011 -- `sha256sum *` glob fragile in release

- **Date:** 2026-04-10
- **Category:** CI/CD (Low)
- **Commit context:** v0.1.2 template feedback fixes
- **Description:** `release.yml:160` uses `sha256sum *`
  which could fail or skip files if subdirectories
  exist in the artifacts directory.
- **Suggested fix:** Use explicit glob
  `sha256sum *.tar.gz *.zip` or `find -type f`.

### RT-010 -- Empty release notes don't block release

- **Date:** 2026-04-10
- **Category:** CI/CD (Medium)
- **Commit context:** v0.1.2 template feedback fixes
- **Description:** When CHANGELOG extraction produces
  empty notes, the workflow emits a `::warning` but
  still creates a release with a blank description.
- **Suggested fix:** Change to `::error` + `exit 1`
  or add a fallback body.

### RT-009 -- Inline `${{ }}` in release run blocks

- **Date:** 2026-04-10
- **Category:** CI/CD (Medium)
- **Commit context:** v0.1.2 template feedback fixes
- **Description:** `release.yml:96,108` use
  `${{ steps.archive.outputs.name }}` and
  `${{ matrix.target }}` directly in `run:` blocks.
  While currently safe due to tag regex validation,
  this is a latent injection vector if the regex is
  ever relaxed.
- **Suggested fix:** Pass values through `env:` block
  instead of inline interpolation.
