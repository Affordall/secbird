# Changelog

## [0.4.0] - 2026-10-14

### Changed
- Redesigned editor, built to be quick mid-game: it opens on the live profile, with a profile
  switcher on top, settings in sections (Lighting, Sensitivity, Buttons, Macros, Power, Profile,
  Device), each shown as tiles, and changes made in a fixed side panel. Nothing scrolls on a
  desktop window, even a Naga with all its buttons; the Naga's side grid is drawn as a keypad.
  Compare shows every profile side by side, as before.
- Themes: Bridge (charcoal with the logo's red-orange; the new default dark), Roast (warm brown), Classic
  (the previous look), Light, Workshop and Neon, picked from the header; Auto follows Windows.
- Many devices or profiles: the device tabs and the profile switcher show arrows and a fade
  where more are hidden, scroll with the mouse wheel, and keep the selected one in view.
- Faster to open: buttons and macros are read from the device only when their section is opened,
  and the editor no longer re-reads the serial number on every refresh.

### Added
- Button safety: a change that would leave the mouse without any left click is refused; every
  button change offers Undo for 10 seconds; "Restore all" (editor Buttons section, tray menu
  "Restore all mouse buttons", `secbird button restore-all`) puts every remapped button back
  to what the mouse stores and forgets the remaps, so they aren't applied again.
- Naga side buttons: the 12-button grid (`grid-1` to `grid-12`) and the 6-button side plate
  (`hex-1` to `hex-7`) can be remapped on both layers (issues #4, #8). Button ids come from
  Synapse; untested on real hardware. They only appear on mice that report having them.
- DPI stages: set the 1-5 DPI values the mouse's DPI button cycles through, and which one is in
  use (`secbird stages`, editor row "DPI stages"). Stored on the mouse and in profiles. Verified
  on a Basilisk V3.
- Macros, stored on the device so they play without any software running: keys and
  shortcuts, mouse buttons, wheel and delays. Make them by typing steps
  (`tap:ctrl+c wait:50 click:left`) or by recording in the editor, and play one from any
  button: once, several times, while held, or on/off (`macro:3`, `macro:3@5`, `macro:3@hold`,
  `macro:3@toggle`). Macros made in Synapse show up too. `secbird macro`, and the editor's
  "Macros" group. Storing, reading back, playing from a button and deleting verified on a
  Basilisk V3.
- Lighting effects the device runs by itself, per zone: spectrum cycling, wave (direction and
  speed 1-5), breathing, reactive and starlight (`secbird effect logo spectrum`, editor "Effect"
  in each lighting zone). Saved in profiles. Devices only run the effects their firmware has:
  the Basilisk V3 runs spectrum and wave (verified) but not breathing, reactive or starlight,
  which Synapse animates from the PC; Secretarybird says so instead of failing silently.
- Firmware update check: `secbird firmware`, a "Check for firmware update" link in the editor,
  and an opt-in weekly check from the tray menu that notifies once per new version. It reads
  the public product page Synapse uses (apps.razer.com) and links to Razer's own updater;
  Secretarybird never installs firmware. The editor now shows the full firmware version.
- Sleep timer for wireless devices: how long the device waits before it sleeps, 1-15 minutes
  (`secbird sleep`, editor group "Power"). Devices with an auto-sleep switch allow up to 60
  minutes and can stop sleeping altogether. Untested on real hardware.

### Fixed
- Mamba Tournament Edition and Diamondback Chroma: the polling rate shows again (reported on
  Reddit). Their device-list entries had none, so v0.3.1 hid it.
- Strider Chroma and Blade 15 Advanced (Early 2021) use the transaction id their owners'
  reports show (1F).
- Mouse Dock Pro: a mouse on the dock can now be controlled (issues #3, #6). Untested on real
  hardware; built from the reporters' `secbird report` output. The dock is the
  mouse's receiver, so it now shows as a wireless mouse with DPI, polling rate (the dock's
  high-speed command), battery and button remapping. Secretarybird asks the dock which mouse is
  paired and uses that mouse's name, DPI range and lighting zones; if it can't tell, it uses
  generic settings that work for any of them.

### Changed
- Device reports without a terminal: tray menu → "Make a device report…" copies the report and
  opens the GitHub form.
- `secbird report` on Blade laptops (and devices not in the list) also reads fan, performance,
  GPU and battery-limit settings, so owners' reports show what fan and power controls need.
  Still read-only.
- Blade 15 Advanced (Early 2021): the lid logo is a lighting zone (on/off, brightness), from a
  user's report (#7). Untested; other Blades get it as reports come in.
- `secbird report` also shows which mice a receiver or dock says are paired (`00:BF`).

## [0.3.3] - 2026-10-14

### Changed
- Device reports without a terminal: tray menu → "Make a device report…" copies the report and
  opens the GitHub form; paste it in.
- `secbird report` on Razer Blade laptops (and devices not in the list) also reads fan,
  performance, GPU and battery-limit settings, so owners' reports show what fan and power
  controls need. Still read-only: a report changes nothing on the device.

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
