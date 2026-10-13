# Changelog

## [0.3.2] - 2026-10-14

### Fixed
- Cobra (wired): the device list now has its real transaction id (1F, was FF), from a user's
  `secbird report`. It worked before only because discovery falls back to the other ids.

### Changed
- Cobra (wired) is verified on real hardware: logo lighting, DPI and polling rate (issue #1).

## [0.3.1] - 2026-10-14

### Fixed
- Mousepads and docks no longer show a polling rate in the editor; it only appears where the
  device list says the device has one.

### Changed
- README lists devices users reported working (Cobra wired, Basilisk V3 Pro 35K Phantom,
  BlackWidow V3 Mini HyperSpeed, Goliathus Chroma, Thunderbolt 4 Dock Chroma), separate from
  the ones verified on our own hardware.

## [0.3.0] - 2026-10-13

### Added
- Button remapping for mice with on-board button memory, in the editor and with `secbird button`:
  mouse buttons, keys and shortcuts (captured by pressing them), turbo for keys and clicks,
  DPI up/down/cycle and sniper clutch, on-board profile switching, media keys, double click,
  disable, and a HyperShift layer. Written to the mouse's live slot; the saved slots keep
  Synapse's or the factory setting and Reset copies it back. Remaps live in profiles, so
  per-app switching changes them too. Verified on the Basilisk V3 (key and button remaps).
- Popup above the clock when the mouse's DPI button or on-board profile button is used, and
  when a per-app profile switches. Read from the mouse's own event reports: no polling.
- DPI changed with the mouse's button is saved to the profile in use and shown in the editor.

### Changed
- Redesigned editor: each device is one sheet with settings as rows and profiles as columns,
  every value visible and editable in place; the active profile is lit and the light moves when
  it changes. Games are managed in each profile's header. Works in narrow windows.

### Fixed
- A turbo setting only Synapse can make (wheel tilt) showed the wrong rate in the editor.

## [0.2.0] - 2026-10-13

### Added
- Per-app profiles: tie a profile to games or apps and it switches while that program is
  in front, then goes back to the active profile. Set it in the editor's Profile card (with
  suggestions from programs that have a window open) or with `secbird profile apps`.
  Event-driven (Windows foreground notifications, no polling); adds about 27 KB to the tray.

## [0.1.0] - 2026-10-13

First public beta. Tested on real hardware with the Basilisk V3 only; other devices are
listed from public protocol facts and need your reports (`secbird report`).

### Added
- Tray app that applies each device's profile at sign-in, on plug-in and after sleep, and
  retries devices that are slow to appear (4, 15 and 45 seconds after start or wake).
- Editor in your browser: lighting per zone, brightness, DPI, polling rate, profiles,
  per-key keyboard colors, an overview of all devices, and "Sync color everywhere".
- `secbird` command line for the same settings, plus `report`, `export` and `import`.
- Profiles per physical device, keyed by serial number, so two identical mice keep
  separate settings.
- Battery level in the tray tooltip and a low-battery alert at 15%.
- Warning when Razer Synapse is running and may overwrite settings.
- `secbird debloat`: lists Razer's background services and startup entries, turns them
  off on request (administrator, every command printed first) and can undo it.
- Log file at `%APPDATA%\Secretarybird\secbird.log`.
- Installer for the current user (no administrator needed) and a portable zip.

### Known limits
- Windows 10/11 only.
- Kraken headsets and laptop cooling pads are not supported yet.
- A wireless mouse connected by cable may show up twice.
- The installer is not code-signed, so Windows SmartScreen asks for confirmation.
