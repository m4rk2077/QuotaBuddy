# Windows shell backdrop and tray anchoring

QuotaBuddy keeps one persistent, borderless Tauri/WebView2 window sized at
400 x 560 DIP. Hiding the panel never destroys the WebView. Positioning derives
the live window size and contains no legacy dashboard-size constant.

## Backdrop decision

- Windows 11 build 22621 or newer uses `DWMSBT_TRANSIENTWINDOW` (Desktop
  Acrylic) only when High Contrast is off and Transparency Effects are on.
- Windows 10, earlier Windows 11 builds, High Contrast, disabled transparency,
  capability-query failures, or a failed DWM call use an opaque solid fallback.
- The capability is checked again before every show. The native DWM return value
  decides the effective mode; CSS adds tint but does not create desktop blur.
- Mica is intentionally not selected because this tray surface is transient.

The implementation does not opt Windows 10 into undocumented composition APIs.
Changes to Windows accessibility/transparency settings take effect on the next
panel opening.

## Tray geometry

Every tray click supplies the physical notification-icon rectangle. QuotaBuddy
uses its center to select the real monitor, reads that monitor's physical bounds,
work area, and scale factor, converts the fixed DIP panel size to physical pixels,
and anchors it on the taskbar side. The result is clamped to the work area and
passed through `CalculatePopupWindowPosition` before the window is shown.
Opening from the context menu first asks Windows for the current tray rectangle
and falls back to the most recent event rectangle only when that query fails.

Pure unit tests cover top, bottom, left, and right taskbars, negative monitor
coordinates, work-area clamping, auto-hide inference, and scale factors from
100% through 200%. The native wrapper falls back to the deterministic pure result
if the Windows positioning call fails.

## Dynamic tray presentation

- The tray starts in a safe unavailable state. The persistent WebView owns the
  single immediate local refresh and five-minute polling cycle; each invocation
  crosses into Rust and updates the tray, avoiding competing native RPC loops.
- Healthy, warning, critical, stale, and unavailable use five distinct
  QuotaBuddy Q/orbit silhouettes as well as cyan, amber, coral, slate, and gray.
- Tooltips contain at most two normalized remaining percentages and 120
  characters. Stale cached data adds a localized stale marker. Provider labels,
  errors, paths, account identifiers, and credentials are never reused.
- New installs pin Session and Week by default. An existing saved empty or custom
  selection remains authoritative.
- Icon and tooltip setters are deduplicated independently. A failed native setter
  is not marked as applied, so a later refresh retries it.

Left-button down records whether the panel was visible. Left-button up uses that
recorded state, so the focus-loss hide event cannot accidentally reopen a panel
the user intended to close. Right click retains the native shortcut menu. Escape,
close, and focus loss hide the persistent window according to the panel's current
view behavior.

## Scope limits

- No fixed bottom-only pointer is drawn; it would be incorrect for lateral and
  top taskbars or when work-area clamping moves the panel away from icon center.
- No Windows 10 experimental Acrylic.
