# Artisan Findings -- Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-008

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-007 -- `main.rs` uses format! + parse for SocketAddr

- **Date:** 2026-04-10
- **Category:** API Design (Low)
- **Commit context:** v0.1.2 template feedback fixes
- **Description:** `crates/rustbase-web/src/main.rs:36-38`
  builds a `SocketAddr` via `format!` + `.parse()` +
  `.expect()`. Could parse `cli.bind` as `IpAddr` via
  clap and construct `SocketAddr::new` directly.

### AQ-006 -- `create_router` accepts `&str` not `&Path`

- **Date:** 2026-04-10
- **Category:** API Design (Low)
- **Commit context:** v0.1.2 template feedback fixes
- **Description:** `crates/rustbase-web/src/api/mod.rs:16`
  accepts `&str` for a filesystem path. Should use
  `&Path` or `impl AsRef<Path>` for type safety.
