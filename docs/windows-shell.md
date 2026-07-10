# Windows shell backdrop and tray anchoring

Issue #18 keeps one persistent Tauri/WebView2 window and changes only its native
shell behavior. The current 960 x 680 DIP content remains unchanged pending the
separate visual redesign.

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

Pure unit tests cover top, bottom, left, and right taskbars, negative monitor
coordinates, work-area clamping, and mixed DPI. The native wrapper falls back to
the deterministic pure result if the Windows positioning call fails.

## Scope limits

- No visual redesign from #20.
- No dynamic tray icon or tray presentation reducer from #21.
- No Windows 10 experimental Acrylic.
