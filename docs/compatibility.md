# Compatibility

## Supported platform

- Windows 10 version 22H2 or newer.
- Windows 11.
- 64-bit Windows installer (`x64`).

Version 0.2 does not support macOS or Linux. It has no auto-update and is not code
signed, so Windows may show a trust prompt during installation.

On Windows 11 build 22621 or newer, Desktop Acrylic is used only when High
Contrast is off and Transparency Effects are on. Windows 10, older Windows 11
builds, disabled transparency, High Contrast, or a failed capability check use
the opaque graphite fallback. Experimental Windows 10 Acrylic APIs are not
used.

## Runtime requirements

- Microsoft Edge WebView2 Runtime, normally present on supported Windows
  installations. Install the Microsoft runtime if the app opens without its
  interface.
- An installed and authenticated Codex CLI that supports `app-server` usage
  requests. QuotaBuddy detects the client; it does not offer a login flow.

Codex quota and local model history are supported. Hermes can be identified only
through safe provider metadata; when it uses the same Codex account/workspace,
its activity can affect the shared quota but cannot be attributed separately.

Personal Claude Code quota has no documented standalone polling interface; a
future opt-in status-line bridge is possible without replacing an existing user
configuration. Personal Cursor usage has no supported public API. QuotaBuddy
does not scrape either product's private storage or dashboard. Team/Enterprise
admin integrations remain separate future opt-ins.

## Installer behavior

The NSIS package installs for the current Windows user. Administrator rights
are not required. A previous installation should be closed before installing a
new build. Starting QuotaBuddy again reuses and focuses the existing tray
instance instead of creating a second process.
