# QuotaBuddy

Windows-first local usage monitor. Tauri 2 shell, React panel, Rust core.

## Foundation scope

This foundation starts in the notification area and keeps the window available through the tray menu or a left-click. Closing the panel hides it; **Quit** explicitly exits the app.

The core exposes one Rust-to-frontend contract: `UsageSnapshot`. It contains normalized availability, metrics, reset metadata, last successful refresh metadata, error state, and staleness. It intentionally has no credential, session, or token fields.

Only detected clients are emitted. Missing Codex, Claude Code, and Cursor clients are not sent to the panel. Provider adapters, alerts, estimated spend calculation, packaging, and release automation are intentionally outside this ticket.

## Development

```powershell
npm install
npm run tauri dev
```

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
