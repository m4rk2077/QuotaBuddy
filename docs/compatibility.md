# Compatibility

## Supported platform

- Windows 10 version 22H2 or newer.
- Windows 11.
- 64-bit Windows installer (`x64`).

V1 does not support macOS or Linux. It has no auto-update and is not code
signed, so Windows may show a trust prompt during installation.

## Runtime requirements

- Microsoft Edge WebView2 Runtime, normally present on supported Windows
  installations. Install the Microsoft runtime if the app opens without its
  interface.
- An installed and authenticated Codex CLI that supports `app-server` usage
  requests. QuotaBuddy detects the client; it does not offer a login flow.

Claude Code and Cursor are out of scope for the Codex-only V1 release.

## Installer behavior

The NSIS package installs for the current Windows user. Administrator rights
are not required. A previous installation should be closed before installing a
new build.
