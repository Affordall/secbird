# Kiyo Pro setup and hardware verification

Support for Razer Kiyo Pro RZ1900364 (`1532:0E05`) is implemented for Windows
10/11 but has not yet been verified on physical hardware. Automated tests and
cross-compilation do not prove that a camera accepts a control or changes its
image. Keep this distinction in the PR and device-support list.

## Build and use

On Windows, install stable Rust with the MSVC build tools, then run:

```powershell
cargo build --release
.\target\release\secbird.exe list
.\target\release\secbird.exe -d 0E05 camera
.\target\release\secbird-tray.exe
```

Close Synapse to prevent competing writes. Use the tray's **Open editor**, select
the Kiyo Pro, then choose **Camera**. No administrator rights or replacement UVC
driver are intended. Allow camera access for desktop apps in Windows privacy
settings if discovery/control access fails.

The controls are HDR on/off, HDR dark/bright mode, wide/medium/narrow field of
view, and responsive/passive autofocus strategy. The autofocus strategy is not
an autofocus enable switch or a manual-focus slider. The exact image/focus
behavior of these vendor modes still needs confirmation on hardware.

Unset profile controls leave the camera unchanged. Values shown in the editor
and CLI are saved/requested values, not reliable current-camera readbacks. A
successful write is not evidence that the visible result matches the request.
Profiles reapply explicitly stored controls on startup, reconnect, wake, and
profile changes. Firmware operations and on-device persistent-save commands are
not part of this implementation.

## Preview

Preview is optional and starts only after a user action. Click **Enable preview /
list cameras** to grant video permission, select the source explicitly, then
click **Start preview**. The browser may briefly open its default camera while
obtaining permission to reveal device labels; that temporary stream is released
before source selection. No audio permission is requested.

Browser camera IDs cannot be reliably matched to Windows device paths. Confirm
the physical camera yourself, especially if multiple devices have the same name.
Changing device/page, stopping preview, hiding/closing the editor, or losing the
video track releases the preview. Reconnecting does not restart it automatically.

Resolution/FPS choices apply only to Secretarybird's preview. They do not set
capture formats in OBS, Teams, or other applications. The browser reports ranges
rather than every valid combination, so preset tuples are labeled **try
combination** until accepted. Unsupported requests retain the last working format
or stop preview if it cannot be restored. Check **Actual preview** for the
negotiated format, including after changing HDR.

Preview can contend with other camera apps. If it is busy, stop preview in the
other app or stop Secretarybird's preview. Configuration controls remain
available without video preview, subject to the driver's access rules.

## Multiple cameras and diagnostics

To inspect native video formats without changing settings or starting a stream:

```powershell
.\target\release\secbird.exe -d 0E05 camera formats
```

Close other camera applications and the Windows camera-settings preview first
if the driver reports that the camera is busy. The diagnostic works independently
of Kiyo Pro extension-unit control support; an unavailable vendor control does
not by itself prevent format enumeration.

Each row identifies a filter output pin, resolution, and pixel format (FourCC
such as MJPG/YUY2, or a subtype GUID). **Listed FPS** comes from that advertised
media type, not the currently running camera. **Advertised range** comes from
the driver's interval bounds; it does not guarantee every intermediate rate.
**Discrete FPS** appears only when the driver supplies an explicit rate list.
If no discrete list appears, no discrete rates were inferred from the range.

This command does not call SetFormat, save profiles, enable Windows multi-app
mode, or apply a preferred camera format. Native advertised formats can differ
from the formats Windows exposes in its shared-camera settings. The output
cannot establish what format Discord currently captures or transmits.

Use `secbird list` to find full camera identities and quote them when passing
`-d`. A bare `0E05` is rejected when more than one camera matches. Profiles are
separate per device path; changing USB ports can change that identity.

`secbird report` and `secbird report --json` include read-only camera diagnostics
with anonymous IDs, not full device paths. Avoid pasting the full `list` output
publicly without checking it: camera identities can contain serial information.

## Experimental native-format retention test

This tests DirectShow's native handle configuration, not a documented global
Kiyo Pro setting. On an unconnected pin, `SetFormat` can store a choice only in
the filter's memory. A matching immediate readback does not prove the physical
camera is capturing at that rate. A reset after reopening does not rule out
other persistence mechanisms used by Synapse.

1. Exit Secretarybird's tray app completely. Close Discord, other camera apps,
   browser previews, and Windows' camera settings page so they do not retain
   capture handles during the test.
2. Read and save the initial handle format:

   ```powershell
   .\target\release\secbird.exe -d 0E05 camera format
   ```

   Save the **exact FPS** ratio printed alongside the rounded FPS. If FPS is
   unknown, stop and share that output rather than changing the format yet.

3. Explicitly request a different advertised mode, for example:

   ```powershell
   .\target\release\secbird.exe -d 0E05 camera format 1280x720 30 MJPG
   ```

   The command prints the previous format and the same-handle readback. It
   rejects unadvertised resolution/subtype combinations and unlisted discrete
   rates. If only interval bounds are available, the driver decides whether the
   requested in-range rate is valid. A write or verification failure triggers
   an attempt to restore the original handle format; rollback failure is
   explicitly reported. No command retries writes automatically.

4. Wait until that command exits, then run a **separate invocation**:

   ```powershell
   .\target\release\secbird.exe -d 0E05 camera format
   ```

   Each invocation opens a handle; the prior process closes its COM objects on
   exit. Report all three outputs. Do not infer persistence from the setter's
   immediate readback or from another app's output resolution.

5. Restore the initial mode explicitly using the original dimensions, FPS, and
   pixel format from step 2. Prefer its exact FPS ratio for restoration rather
   than a rounded decimal. For example, **only if these match the initial values**:

   ```powershell
   .\target\release\secbird.exe -d 0E05 camera format 1920x1080 10000000/166667 MJPG
   ```

Pin 0 is used by default. Append `--pin N` to either read or set another filter
output pin shown by `camera formats`. FPS can be a positive decimal or a ratio,
such as `60000/1001`; pixel format accepts the reported FourCC or subtype GUID.
Nothing is stored in Secretarybird profiles or Windows' preferred-media-type
configuration, and no stream is started. Actual driver behavior and cross-process
retention remain experimental until tested on the physical camera.

## Automated verification

```text
cargo test --release
cargo clippy --release --all-targets -- -D warnings
node --test crates/secbird-tray/src/*.test.cjs
```

Windows CI must execute the native-path tests, including the KS metadata and
authenticated server-route tests. Linux CI continues checking core/CLI support.
The JavaScript tests exercise video lifecycle and negotiation without a webcam.
A browser smoke check using a synthetic stream exercises rendering but is not
hardware verification.

## Physical test record

Fill this out on Windows before changing the support status to verified. Leave
unperformed checks unchecked and attach failures instead of inferring success.

| Environment | Result |
| --- | --- |
| Commit/build tested | Not tested |
| Windows version/build | Not tested |
| Camera firmware | Not tested; use an independently known value if available |
| Browser/version | Not tested |
| USB connection, hub, port | Not tested; keep serial-bearing paths private |
| Synapse closed | Not tested |

- [ ] Exactly one entry per physical Kiyo Pro; no duplicate HID failure entry.
- [ ] Report/discovery leave the image and persistent settings unchanged.
- [ ] HDR off/on produces the expected visible result.
- [ ] HDR dark/bright modes behave as expected; document the observed difference.
- [ ] Wide/medium/narrow FOV changes framing correctly in both directions.
- [ ] Responsive/passive focus strategies are accepted; document observed behavior.
- [ ] Saved profiles switch/reapply correctly, including foreground-app profiles.
- [ ] Restart, reconnect and wake reapply only explicitly stored controls.
- [ ] An existing mouse/keyboard still works alongside the camera.
- [ ] Preview shows the selected physical camera; verify twins if available.
- [ ] Actual preview formats match accepted requests; record tuples below.
- [ ] Unsupported tuples show an error and restore the previous format safely.
- [ ] HDR-related frame-rate restrictions are reported accurately.
- [ ] Stop preview releases camera access for another app.
- [ ] Closing/hiding the editor and switching devices/pages release capture.
- [ ] Stop/switch while a permission request is pending releases any late stream.
- [ ] Disconnect stops preview; reconnect requires explicit preview restart.
- [ ] Access conflict with OBS/Teams gives a clear error and recovers after rescan.
- [ ] Permission denial is actionable and does not prevent HID devices working.

| Requested preview tuple | Actual tuple | HDR state | Result |
| --- | --- | --- | --- |
| Browser default | Not tested | Not tested | Not tested |

If a control is accepted but does not visibly work, report that separately from
discovery or permission failures. Do not mark the camera verified based only on
saved UI values, successful tests, or a successful build.
