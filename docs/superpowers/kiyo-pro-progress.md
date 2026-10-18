# Kiyo Pro support

Goal: support the Razer Kiyo Pro RZ1900364 on Windows 10/11 in the existing
Secretarybird Rust core, CLI, and tray/editor, suitable for an upstream PR.

- [x] Inspect fork and upstream discovery, transport, and tray integration.
- [x] Confirm Windows target and upstream architecture requirement.
- [x] Agree on approach and feature scope in chat.
- [x] Write and self-review architectural design specification.
- [x] Obtain user approval of the written specification.
- [x] Write implementation plan and agree on execution method (native).
- [x] Implement with regression tests.
- [x] Verify local builds/tests and document required Windows hardware checks.

Findings: discovery currently enumerates HID interfaces and requires a successful
Protocol 2.5 serial query. Kiyo Pro uses UVC camera controls (USB 1532:0E05),
including vendor extension-unit commands for HDR, field of view, and autofocus
mode. A database-only addition cannot provide camera support. The HID Transport
trait explicitly models feature reports; camera commands must not be disguised
as HID reports.

Product implementation is on branch `feat/kiyo-pro` in an isolated worktree.

Design: `specs/2026-10-18-kiyo-pro-design.md`. Firmware work is explicitly
excluded at the user's request. The amended specification was approved in chat.

Scope amendment: include opt-in editor video preview and resolution/FPS choices
for that preview session only. Photo capture/video recording remain excluded.
The specification includes camera-access conflicts, resource cleanup, and
preview-specific acceptance checks.

User approved the amended specification. Implementation plan written and
self-reviewed at `plans/2026-10-18-kiyo-pro.md`; native execution selected.
Preview uses browser-native capture inside the existing local editor. Portable
tests and Windows cross-compilation are available on this Mac; Windows runtime
and physical-camera validation remain outstanding. See
`../kiyo-pro-verification.md` for the hardware checklist.

## Verification so far

- macOS release workspace build: passed.
- macOS release workspace tests: 91 unit tests and 1 doctest passed; 2 existing
  live-network tests ignored.
- Preview lifecycle/negotiation tests: 14 passed using Node 22.
- Release clippy with warnings denied: passed on macOS and cross-checked for
  Windows GNU (workspace/all targets) and Linux GNU (core/CLI/all targets).
- Browser smoke test with a synthetic stream: preview played, a control edit
  preserved playback, and navigating away released tracks. Not a hardware test.
- New Rust module formatting and git whitespace checks: passed. Repository-wide
  rustfmt checks flag baseline upstream formatting; no unrelated formatting sweep.
- Windows/Linux runtime execution, Windows release executable linking, and the
  physical-camera checklist have not been run here. CI and hardware checks remain
  required before claiming native runtime or hardware verification.

## Implementation decisions

- Browser-native preview avoids a video server, frame queue, and codec dependency.
- Preview format presets are labeled as candidates until exact browser negotiation
  succeeds; they are not an exhaustive list of the camera's native formats.
- Camera JSON receives the store and foreground context explicitly, so edits and
  displayed values target the effective profile rather than its manual fallback.
- Portable engine/API tests run on the host; native route/KS tests compile in the
  Windows target and still require Windows execution.
