# Kiyo Pro support

Goal: support the Razer Kiyo Pro RZ1900364 on Windows 10/11 in the existing
Secretarybird Rust core, CLI, and tray/editor, suitable for an upstream PR.

- [x] Inspect fork and upstream discovery, transport, and tray integration.
- [x] Confirm Windows target and upstream architecture requirement.
- [x] Agree on approach and feature scope in chat.
- [x] Write and self-review architectural design specification.
- [x] Obtain user approval of the written specification.
- [ ] Write implementation plan and agree on execution method.
- [ ] Implement with regression tests.
- [ ] Verify builds/tests and document required Windows hardware checks.

Findings: discovery currently enumerates HID interfaces and requires a successful
Protocol 2.5 serial query. Kiyo Pro uses UVC camera controls (USB 1532:0E05),
including vendor extension-unit commands for HDR, field of view, and autofocus
mode. A database-only addition cannot provide camera support. The HID Transport
trait explicitly models feature reports; camera commands must not be disguised
as HID reports.

No product code has been changed.

Design: `specs/2026-10-18-kiyo-pro-design.md`. Firmware work is explicitly
excluded at the user's request. Awaiting review of the written specification.

Scope amendment: include opt-in editor video preview and resolution/FPS choices
for that preview session only. Photo capture/video recording remain excluded.
The specification includes camera-access conflicts, resource cleanup, and
preview-specific acceptance checks.

User approved the amended specification. Implementation plan written and
self-reviewed at `plans/2026-10-18-kiyo-pro.md`; awaiting plan review and execution
method selection. Product code is unchanged. Preview implementation is planned
through browser-native capture inside the existing local editor.
