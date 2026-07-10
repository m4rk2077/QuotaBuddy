# Privacy

QuotaBuddy is a local Windows application. V1 has no QuotaBuddy account,
telemetry, analytics, hosted service, local HTTP server, or new provider login
flow.

## Data boundary

- The frontend receives normalized usage snapshots only. The contract has no
  token, credential, or session fields.
- For usage refresh, the native layer starts the installed Codex CLI's local
  `app-server` interface and requests account rate-limit data through its
  existing authenticated session. QuotaBuddy does not render, persist, or
  expose credential material to the webview.
- Estimated spend reads local JSONL usage records and the bundled, versioned
  pricing table. It is labelled as an estimate, never billing truth.
- User-initiated diagnostic exports contain summary data only. Raw logs,
  token values, session values, and secret values are excluded.

## Storage and network

Monitor preferences are stored locally in QuotaBuddy's app configuration
directory. Normal usage refresh can contact the provider only through the
already-installed Codex session. QuotaBuddy has no independent backend or
telemetry endpoint.

## Handling diagnostics

Share a diagnostic export only after opening it and confirming it contains no
information you do not want to disclose. Redaction reduces exposure but is not
a substitute for reviewing data before sharing it.
