# QuotaBuddy

Windows-first local usage monitor. Tauri 2 shell, React panel, Rust core.

## Foundation scope

This foundation starts in the notification area and keeps the window available through the tray menu or a left-click. Closing the panel hides it; **Quit** explicitly exits the app.

The core exposes one Rust-to-frontend contract: `UsageSnapshot`. It contains normalized availability, metrics, reset metadata, last successful refresh metadata, error state, and staleness. It intentionally has no credential, session, or token fields.

Only detected clients are emitted. Missing Codex clients are not sent to the panel. Provider alerts, packaging, and release automation are intentionally outside this ticket.

## Local estimated spend and diagnostics

Codex estimated spend uses only local JSONL log records with a model plus input/output token counts. Pricing is read from the versioned bundled table at `src-tauri/fixtures/pricing_table_2026-07-10.json`. It is always an estimate, never provider billing. User-initiated diagnostics contain summary data only: raw logs, tokens, sessions, and secret values are excluded.

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
