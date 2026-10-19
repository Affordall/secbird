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
- macOS release workspace tests: 94 unit tests and 1 doctest passed; 2 existing
  live-network tests ignored.
- Browser lifecycle/profile-race tests: 20 passed using Node 22.
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

## Final review

A fresh-context reviewer identified two important issues. Both were fixed with
regression tests: identical cameras now have consistent distinguishing labels
and inspectable local identities; profile operations capture their target before
awaiting, so late responses cannot overwrite a newly selected device. Multi-step
create/delete requests retain the original target, and superseded responses are
ignored. Browser smoke checks confirmed twin labels, About identity visibility,
and a camera profile response arriving after selection moved to a mouse.

Additional tests cover failure of the second FOV write without saving settings,
and imported profiles containing both camera and HID settings. Release suites
and cross-target clippy were rerun after these changes.

Deferred minor review findings:

- Native extension-node discovery reports a generic failure instead of preserving
  every candidate node's HRESULT and malformed-metadata details.
- After Stop/Start, the preview format dropdown may retain the previous selection
  while capture starts with browser defaults. The **Actual preview** label reports
  the newly negotiated format; use it rather than the dropdown as readback.
- Direct setting-write errors retain native HRESULTs but omit the requested
  setting/FOV-stage context (profile replay already adds setting context).

Physical control effects, firmware compatibility, concurrent browser/native
access, and Windows runtime behavior remain unverified rather than waived.

## Execution rulings and remaining risks

1. The harness worktree tool failed, so an explicitly approved Git worktree was
   used and the session moved to it. Risk: the harness may not list that worktree;
   Git still tracks branch `feat/kiyo-pro` and main remains unchanged.
2. The user authorized global Rust installation. Official rustup was installed
   without modifying shell profiles. Risk: the installation remains on the Mac
   until explicitly uninstalled.
3. On macOS, native Windows tests were replaced locally by cross-compilation and
   clippy, not treated as runtime evidence. Risk: Windows/driver failures may still
   surface; native CI and hardware testing remain required.
4. Camera JSON/dispatch receives Store and foreground context explicitly. Risk:
   this is an internal interface adjustment from the written plan; it avoids
   global-read coupling and targets the effective profile.
5. Portable engine/API tests run on the host; native server/KS tests remain
   Windows-only. Risk: those tests are compiled here but still need CI execution.
6. Preview presets are honestly labeled candidates because the browser does not
   enumerate all valid tuples. Risk: formats outside the preset list may only be
   available via browser defaults or another application. The final review
   accepted this trade-off without treating it as exhaustive format support.
7. Existing upstream formatting was not swept. Risk: whole-repository rustfmt
   remains red; new modules pass. The final review kept this baseline issue
   separate from feature defects.
8. The final review could not validate physical image effects, firmware
   compatibility, concurrent control/capture access, or Windows runtime behavior.
   These remain unverified, not waived; native testing may require adjustments.

## Native-format diagnostic follow-up

Approved bounded addition: `secbird -d 0E05 camera formats` lists native
driver-advertised output formats without selecting formats or starting capture.
It reports pixel subtypes, resolutions, nominal intervals, advertised interval
bounds, and explicit discrete rates when available. Capability bounds are not
expanded into invented supported combinations. Enumeration runs on the existing
COM worker and does not require vendor extension-unit control support.

- [x] Failing parser/CLI tests written and observed before implementation.
- [x] Implement read-only native enumeration and bounded driver-data parsing.
- [x] Run full regression tests and cross-target checks; review changes.
- [x] Obtain Windows diagnostic output from the physical camera.

Follow-up verification: 101 host unit tests and 1 doctest pass, with 2 existing
live-network tests ignored; 20 JavaScript tests pass. Release clippy with warnings
denied passes on macOS and for Windows GNU workspace/all targets and Linux GNU
core/CLI/all targets. Windows ABI/subtype tests compile in the native target but
cannot execute on this Mac.

The read-only review found that checking the video capability layout before the
media major type would reject a legitimate audio pin. Fixed with bounded generic
buffer allocation followed by media classification and video-specific layout
validation; mixed video/audio and malformed buffer regression tests pass. The
diagnostic is also documented outside the CLI's saved-settings help category.
At implementation time, physical enumeration and driver runtime behavior were
unverified. Busy/privacy behavior and hot-unplug still need physical testing.

User-supplied Windows output now confirms native format enumeration reports
1920x1080 at discrete 60 FPS for H264, MJPG, NV12, and YUY2. This verifies the
reported enumeration result, not actual capture, saved defaults, or persistence.

## Experimental native-format read/set follow-up

Approved bounded addition: `camera format [--pin N]` reads the opened handle;
`camera format WIDTHxHEIGHT FPS FOURCC [--pin N]` explicitly requests an
advertised mode using SetFormat and checks same-handle readback. No graph is
connected or run, and neither profiles nor Windows shared-camera defaults are
written. Failed writes/readbacks attempt a verified rollback; failures are
reported rather than silently retried.

Persistence must be checked in a new process after the setter exits and all other
camera apps/tray processes are closed. An unconnected pin may retain a format
only in its own memory; success does not prove a global effect, and resetting on
reopen does not disprove other device/driver persistence mechanisms.

- [x] Request validation, discrete/range selection, CLI dispatch, and rollback tests
  written first and observed failing before implementation.
- [x] Run regression/cross-target checks and review changes.
- [ ] Test native read/set and separate-process retention on the user's camera.

Verification: 110 host unit tests plus 1 doctest pass, with 2 existing live-network
tests ignored; 20 JavaScript tests pass. The host release build, host/Windows GNU
release workspace all-targets clippy, Linux GNU release core/CLI all-targets
clippy, new-module formatting, and whitespace checks pass.

The read-only review found no critical or important issues. Its minor restoration
precision finding was fixed with a failing-then-passing regression test: handle
readback now includes an exact FPS ratio, so restoration need not use the rounded
three-decimal display. Native acceptance, physical capture effects, busy/privacy
and hot-unplug behavior, and cross-process retention still require Windows and
hardware testing; same-handle success is not treated as evidence of persistence.
