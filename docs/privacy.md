# Privacy

QuotaBuddy is a local Windows application. Version 0.2 has no QuotaBuddy account,
telemetry, analytics, hosted service, local HTTP server, or new provider login
flow.

## Data boundary

- The frontend receives normalized quota snapshots and aggregated history.
  History may include token counters by date and model, but never credentials,
  conversation content, session IDs, account IDs, source paths, or raw events.
- For usage refresh, the native layer starts the installed Codex CLI's local
  `app-server` interface and requests account rate-limit data through its
  existing authenticated session. QuotaBuddy does not render, persist, or
  expose credential material to the webview.
- Local history reads only allowlisted model/timestamp/token-counter fields from
  Codex JSONL rollouts. Prompts, responses, tool calls, working directories, and
  identifiers are ignored.
- API-equivalent cost uses the bundled, versioned pricing table. It is labelled
  as an estimate and is never presented as subscription or provider billing.
- Account activity uses the optional `account/usage/read` response. Email and
  account/workspace identifiers are discarded before data reaches the webview.
- Hermes detection is metadata-only. QuotaBuddy never compares, copies, logs,
  or returns OAuth tokens and cannot attribute a share of account usage to a
  specific external client.
- User-initiated diagnostic exports contain summary data only. Raw logs,
  token values, session values, and secret values are excluded.

## Storage and network

Monitor preferences are stored locally in QuotaBuddy's app configuration
directory. The native in-memory history cache uses local source paths only as
lookup keys and retains only allowlisted model, timestamp, and token counters;
it is not persisted or sent to the webview. Normal usage refresh can contact the
provider only through the already-installed Codex session. QuotaBuddy has no
independent backend or telemetry endpoint.

## Handling diagnostics

Share a diagnostic export only after opening it and confirming it contains no
information you do not want to disclose. Redaction reduces exposure but is not
a substitute for reviewing data before sharing it.
