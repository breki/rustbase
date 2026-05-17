# Deployment

`rustbase-web` ships an opinionated deploy flow for a
single remote Linux host running systemd. It was
developed against a Raspberry Pi 4 on Debian 13 (arm64),
but works on any Linux target that has `sudo` and
`systemctl`. The build runs natively on the remote -- no
cross-compilation toolchain required on the dev machine.

## Architecture

```
Dev machine                       Remote host
─────────────                     ───────────
npm run build ──► frontend/dist
                     │
tar Rust source ─────┼──► ~/rustbase-build/
                     │        cargo build --release
                     │              │
              scp dist ──────► /opt/rustbase/frontend-dist
                              /opt/rustbase/rustbase-web
                                    │
                              systemd: rustbase-web.service
                              http://<host>:9200
```

**Deploy path**: `/opt/rustbase`
**Service user**: `rustbase` (dedicated system account)
**Port**: 9200 (configurable in the service unit)

## Prerequisites

On the dev machine (Windows/Linux/macOS):
- Rust toolchain (`cargo`)
- Node.js + npm (for frontend build)
- Native `ssh`, `scp`, and `tar` on `PATH`
  (Windows 10+ ships all three)
- SSH access to the remote (`ssh <host>` must work)

On the remote:
- Rust toolchain (`rustup`) -- install via
  `curl --proto '=https' --tlsv1.2 -sSf \
       https://sh.rustup.rs | sh -s -- -y`
- `sudo` access for the deploy user

## First-time setup

1. Copy the deploy config template:

   ```bash
   cp .deploy.sample .deploy
   ```

2. Edit `.deploy` with your remote details:

   ```
   rpi_host=raspberrypi.local
   rpi_user=pi
   deploy_path=/opt/rustbase
   ```

3. Run the one-time setup:

   ```powershell
   cargo xtask deploy-setup
   # or: .\build.ps1 deploy-setup
   ```

   This SSHes into the remote and:
   - Creates a `rustbase` system user (nologin shell)
   - Creates `/opt/rustbase/frontend-dist`
   - Installs and enables the systemd service

## Deploying

```powershell
cargo xtask deploy
# or: .\build.ps1 deploy
```

Both invocations run the same `xtask` binary, which:

1. Builds the frontend locally (`npm run build`)
2. Tars the Rust source and `scp`s it to the remote
3. Builds the release binary on the remote
4. Stops the service, installs binary + frontend
   (atomic swap)
5. Restarts and verifies the service (3 retries)

The first build takes ~4 minutes (downloading +
compiling all dependencies). Incremental builds take
30--60 seconds since the build cache persists in
`~/rustbase-build/target/` on the remote.

## Verifying

```bash
# Health check (if rustbase-web exposes one)
curl http://<host>:9200/

# Service status
ssh <host> "systemctl status rustbase-web"

# Recent logs
ssh <host> "journalctl -u rustbase-web -n 30"
```

## File layout on the remote

```
/opt/rustbase/
├── rustbase-web              # binary
└── frontend-dist/            # static frontend assets
```

## Deploy artifacts

| File | Purpose |
|------|---------|
| `.deploy.sample` | Config template (committed) |
| `.deploy` | Actual config (gitignored) |
| `xtask/src/deploy_config.rs` | Shared config loader + input validation |
| `xtask/src/deploy_remote.rs` | `ssh`/`scp` wrappers (no shell in between) |
| `xtask/src/deploy_setup.rs` | One-time provisioning |
| `xtask/src/deploy.rs` | Repeatable deploy (sync + build + restart) |
| `deploy/rustbase-web.service` | systemd unit file |

## Security notes

- **HTTP only**: rustbase-web serves over plain HTTP.
  Cookies traverse the LAN in cleartext. This is an
  accepted default for a trusted home network. To add
  TLS, place a reverse proxy (nginx or caddy) in front.

- **Sandboxed service**: the systemd unit uses
  `ProtectSystem=strict`, `NoNewPrivileges`,
  `CapabilityBoundingSet=` (empty),
  `RestrictAddressFamilies`, and `SystemCallFilter` to
  limit the attack surface. The service has no
  writable paths by default; if your app needs to
  write somewhere, add a `ReadWritePaths=` directive.

- **Resource limits**: `MemoryMax=256M` and
  `CPUQuota=80%` prevent runaway processes from
  affecting the host.

- **Config validation**: deploy config values are
  validated against a strict character allowlist to
  prevent shell injection via SSH command strings.

## Troubleshooting

**Service won't start**:
```bash
ssh <host> "journalctl -u rustbase-web -n 50 --no-pager"
```

**Permission denied errors**:
```bash
ssh <host> "sudo ls -la /opt/rustbase/"
```

The `rustbase` user owns everything under
`/opt/rustbase/`. The deploy user (configured via
`rpi_user`) needs `sudo` to copy files into the deploy
path.

**Port conflict**:
```bash
ssh <host> "ss -tlnp | grep 9200"
```

**Stale build cache**:
If builds produce unexpected results, clear the cache:
```bash
ssh <host> "rm -rf ~/rustbase-build/target"
```

**Windows path handling**:
The Rust deploy runs `ssh.exe`/`scp.exe`/`tar.exe` via
`std::process::Command` with explicit arg vectors -- no
shell parsing, no `MSYS_NO_PATHCONV` dance. If an
invocation unexpectedly sees a `C:/...` path on the
remote side, check that the relevant `scp_to` call
passes a relative filename and sets `current_dir` to
the right local directory (see
`xtask/src/deploy_remote.rs`).
