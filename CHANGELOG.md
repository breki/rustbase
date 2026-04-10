# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial project template with workspace structure
- xtask build automation (validate, test, clippy, fmt,
  coverage)
- Claude Code configuration with Stop hook, commit
  skill with Red Team + Artisan code review
- GitHub Actions CI (multi-platform) and release
  workflow (5 targets)
- Development diary and code review finding logs
- Optional web app: Axum backend + Svelte 5/Vite
  frontend with dev proxy, SPA routing, health/status
  API endpoints
- PowerShell build script (`build.ps1`)
- Integration test scaffold with `assert_cmd`
- Playwright E2E test scaffold with auto-server start
- `.ports` config pattern for port management
- `.mise.toml` for Node.js version management
- `llms.txt` AI-agent reference (llmstxt.org)
- `/architect` and `/web-dev` Claude Code skills
- CI frontend build job; release packages both
  binaries with frontend dist
- Code duplication check (`cargo xtask dupes`) using
  `code-dupes` with 6% threshold
- `/template-improve` slash command for logging
  template feedback
- TDD (red/green/refactor) guidance in `CLAUDE.md`

### Fixed

- Coverage no longer fails out of the box by excluding
  `xtask` crate and binary `main.rs` entry points
- Clarified `anyhow` vs `thiserror` dependency split
  in `Cargo.toml` comments
- Enforced that all commits must use `/commit` skill
