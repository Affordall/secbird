# Changelog

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
