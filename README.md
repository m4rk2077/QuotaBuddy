# QuotaBuddy

Windows-first local usage monitor. Tauri 2 shell, React panel, Rust core.

Native backdrop and tray-positioning decisions: [Windows shell](docs/windows-shell.md).

## Foundation scope

This foundation starts in the notification area and keeps the window available through the tray menu or a left-click. Closing the panel hides it; **Quit** explicitly exits the app.

The core exposes one Rust-to-frontend contract: `UsageSnapshot`. It contains normalized availability, metrics, reset metadata, last successful refresh metadata, error state, and staleness. It intentionally has no credential, session, or token fields.

Only detected Codex clients are emitted. Missing Codex clients are not sent to the panel.

## Local estimated spend and diagnostics

Codex estimated spend uses only local JSONL log records with a model plus input/output token counts. Pricing is read from the versioned bundled table at `src-tauri/fixtures/pricing_table_2026-07-10.json`. It is always an estimate, never provider billing. User-initiated diagnostics contain summary data only: raw logs, tokens, sessions, and secret values are excluded.

## Development

```powershell
npm install
npm run tauri dev
```

## Windows release

QuotaBuddy V1 ships a per-user NSIS installer for Windows. Build and create its
SHA-256 file locally:

```powershell
npm ci
npm run release:windows
npm run release:checksums
```

The distributable files are written to `release/` and are intentionally ignored
by Git. Verify a downloaded installer before running it:

```powershell
Get-FileHash .\release\QuotaBuddy_0.1.1_x64-setup.exe -Algorithm SHA256
Get-Content .\release\QuotaBuddy_0.1.1_x64-setup.exe.sha256
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
