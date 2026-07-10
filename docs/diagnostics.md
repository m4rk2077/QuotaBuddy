# Diagnostics

## Safe checks

1. Confirm the installed Codex CLI is authenticated by using Codex directly.
2. Open QuotaBuddy and use Refresh.
3. If the panel reports an unavailable session, reauthenticate with Codex and
   refresh again.
4. If an estimated-spend value is missing, treat it as unavailable rather than
   zero; only recognized local JSONL records with a priced model are counted.

QuotaBuddy keeps the last successful usage snapshot when a later refresh
fails, and marks the value stale. An estimate is not an invoice.

## Diagnostic export

Exports are user initiated. They include the pricing-table version, count of
recognized local records, estimated-spend summary, and explanatory notes. They
exclude raw local logs, token fields, session fields, and detected secret
fields. Review every export before sharing it.

## Do not share

Never send your Codex profile, raw JSONL logs, cookies, tokens, API keys,
passwords, or screenshots containing them. QuotaBuddy's CI also uses only
anonymized fixtures and never runs a provider refresh.
