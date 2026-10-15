//! The tray's own settings (`tray.json`), starting with Windows, and the settings folder:
//! read by the tray and changed from its menu or the editor's Settings page.

use secbird_core::config_dir;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const APP_NAME: &str = "Secretarybird";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tray settings in `tray.json`.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct TraySettings {
    /// Check Razer's product pages for newer firmware once a week. Off until turned on.
    #[serde(default)]
    pub firmware_weekly: bool,
    /// Unix time of the last weekly check.
    #[serde(default)]
    pub firmware_checked: u64,
    /// Device key -> firmware version already announced, so each is announced once.
    #[serde(default)]
    pub firmware_notified: std::collections::BTreeMap<String, String>,
    /// Don't check GitHub for a newer Secretarybird. The check is on unless turned off.
    #[serde(default)]
    pub updates_off: bool,
    /// Unix time of the last daily new-version check.
    #[serde(default)]
    pub update_checked: u64,
    /// The newest version already announced, so each is announced once.
    #[serde(default)]
    pub update_notified: String,
    /// No small popups above the clock for DPI, on-board profile and game profile changes.
    #[serde(default)]
    pub popups_off: bool,
    /// No notification when a wireless device runs low.
    #[serde(default)]
    pub battery_warning_off: bool,
    /// Don't switch profiles when a program tied to one comes to the front.
    #[serde(default)]
    pub auto_switch_off: bool,
}

impl TraySettings {
    pub fn path() -> std::path::PathBuf {
        config_dir().join("tray.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::create_dir_all(config_dir());
            let _ = std::fs::write(Self::path(), text);
        }
    }
}

pub fn autostart_enabled() -> bool {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
            return false;
        }
        let found =
            RegQueryValueExW(key, wide(APP_NAME).as_ptr(), null(), null_mut(), null_mut(), null_mut()) == ERROR_SUCCESS;
        RegCloseKey(key);
        found
    }
}

pub fn set_autostart(enable: bool) {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) != ERROR_SUCCESS {
            return;
        }
        if enable {
            if let Ok(exe) = std::env::current_exe() {
                let value = wide(&format!("\"{}\"", exe.display()));
                RegSetValueExW(
                    key,
                    wide(APP_NAME).as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr() as *const u8,
                    (value.len() * 2) as u32,
                );
            }
        } else {
            RegDeleteValueW(key, wide(APP_NAME).as_ptr());
        }
        RegCloseKey(key);
    }
}

pub fn open_config_folder() {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    unsafe {
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(&dir.display().to_string()).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}
