# QuotaBuddy

Windows-first local usage monitor. Tauri 2 shell, React panel, Rust core.

Native backdrop and tray-positioning decisions: [Windows shell](docs/windows-shell.md).

## Foundation scope

This foundation starts in the notification area and keeps the window available through the tray menu or a left-click. Closing the panel hides it; **Quit** explicitly exits the app.

The core exposes normalized, privacy-limited contracts. `UsageSnapshot` contains availability, quota metrics, reset metadata, refresh metadata, error state, and staleness. The history contract contains aggregated counters by day and model, but never credentials, conversation content, session identifiers, account identifiers, or source paths.

Only detected Codex clients are emitted. Missing Codex clients are not sent to the panel.

## Local history, API equivalent, and diagnostics

QuotaBuddy can show two deliberately separate views:

- account activity returned by the authenticated Codex app-server, aggregated across clients;
- local model history derived from allowlisted token counters in Codex JSONL rollouts on this PC.

The local view includes model share, cached input, daily totals, and an API-equivalent estimate from a versioned bundled pricing table. It is a comparison against public API prices, never the actual charge for a ChatGPT subscription. User-initiated diagnostics contain summary data only: raw logs, conversation content, sessions, paths, account data, and secret values are excluded.

## Development

```powershell
npm install
npm run tauri dev
```

## Windows release

QuotaBuddy v0.2 ships a per-user NSIS installer for Windows. Build and create its
SHA-256 file locally:

```powershell
npm ci
npm run release:windows
npm run release:checksums
```

The distributable files are written to `release/` and are intentionally ignored
by Git. Verify a downloaded installer before running it:

```powershell
Get-FileHash .\release\QuotaBuddy_0.2.0_x64-setup.exe -Algorithm SHA256
Get-Content .\release\QuotaBuddy_0.2.0_x64-setup.exe.sha256
```

Do not publish a GitHub Release until the installer, checksum, and the gates
below have been reviewed. See [Windows release runbook](docs/release-windows.md),
[privacy](docs/privacy.md), [compatibility](docs/compatibility.md), and
[diagnostics](docs/diagnostics.md).

## Quality gates

```powershell
npm test
npm run build
Set-Location src-tauri
cargo fmt --check
cargo test
cargo check
```

Rust diagnostics must go through `redact_sensitive_text` / `log_redacted` before persistence or display. Tests use anonymized fixtures only.

## License and notices

QuotaBuddy is released under the [MIT License](LICENSE). Runtime dependency
attribution is in [NOTICE](NOTICE).
