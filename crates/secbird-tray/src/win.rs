use crate::engine::{ApplyReport, Battery, Engine};
use secbird_core::log;
use std::collections::HashSet;
use crate::server;
use secbird_core::config_dir;
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const APP_NAME: &str = "Secretarybird";
const WM_TRAY: u32 = WM_APP + 1;
/// Sent by a second launch: open the editor of the instance already running.
const WM_OPEN_EDITOR: u32 = WM_APP + 2;
const CLASS_NAME: &str = "SecretarybirdTray";
const TIMER_RESCAN: usize = 1;
/// Extra rescans after start and wake: devices often answer late at boot.
const TIMER_RETRY: usize = 2;
const RETRY_DELAYS_MS: [u32; 3] = [4_000, 15_000, 45_000];
/// Battery check for wireless devices (cheap: two short USB requests per device).
const TIMER_BATTERY: usize = 3;
const BATTERY_MS: u32 = 5 * 60 * 1000;
const LOW_BATTERY: u8 = 15;
/// Devices need a moment after arrival or resume before they answer reliably.
const SETTLE_MS: u32 = 1500;

const ID_REAPPLY: u32 = 1;
const ID_FOLDER: u32 = 2;
const ID_AUTOSTART: u32 = 3;
const ID_EXIT: u32 = 4;
const ID_EDITOR: u32 = 5;
const ID_PROFILE_BASE: u32 = 1000;

const GUID_DEVINTERFACE_HID: GUID = GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// "TaskbarCreated" is broadcast when Explorer restarts; the icon must be re-added.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct App {
    hwnd: HWND,
    icon: HICON,
    /// Shared with the editor's server thread.
    engine: Arc<Mutex<Engine>>,
    /// Started on first use; holds the tokenized URL.
    editor_url: Option<String>,
    /// Menu id offset -> (device key, profile name).
    profile_items: Vec<(String, String)>,
    /// Apply to every device on the next rescan, not just newly arrived ones.
    apply_all_pending: bool,
    /// Index into RETRY_DELAYS_MS of the next retry.
    retry_step: usize,
    /// Warned about Synapse since it last started.
    synapse_warned: bool,
    /// Devices already warned about low battery (cleared once charging or recovered).
    low_battery: HashSet<String>,
    batteries: Vec<Battery>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` on the app state. Returns `None` while the state is borrowed, which only
/// happens if a message arrives re-entrantly.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn copy_wide(dst: &mut [u16], s: &str) {
    let src: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..src.len()].copy_from_slice(&src);
    dst[src.len()] = 0;
}

pub fn run() {
    unsafe {
        let _mutex = CreateMutexW(null(), 0, wide("Local\\Secretarybird.Tray").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let running = FindWindowW(wide(CLASS_NAME).as_ptr(), null());
            if std::env::var_os("SECBIRD_SIM").is_some() {
                let text = wide(
                    "Secretarybird is already running, so SECBIRD_SIM has no effect.

                     Quit it from the tray menu (Exit), then start it again from this terminal.",
                );
                MessageBoxW(null_mut(), text.as_ptr(), wide(APP_NAME).as_ptr(), MB_ICONINFORMATION);
            }
            if !running.is_null() {
                // Let the running instance bring the browser to the front.
                AllowSetForegroundWindow(ASFW_ANY);
                PostMessageW(running, WM_OPEN_EDITOR, 0, 0);
            }
            return;
        }

        let engine = match Engine::new() {
            Ok(e) => e,
            Err(e) => {
                let text = wide(&format!("Could not start HID access: {e}"));
                MessageBoxW(null_mut(), text.as_ptr(), wide(APP_NAME).as_ptr(), MB_ICONERROR);
                return;
            }
        };

        let hinstance = GetModuleHandleW(null());
        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        RegisterClassW(&wc);
        // A hidden top-level window: message-only windows miss power broadcasts.
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(APP_NAME).as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Ordering::Relaxed);
        register_hid_notifications(hwnd);

        let icon = load_icon();
        add_icon(hwnd, icon);
        APP.with(|cell| {
            *cell.borrow_mut() = Some(App {
                hwnd,
                icon,
                engine: Arc::new(Mutex::new(engine)),
                editor_url: None,
                profile_items: Vec::new(),
                apply_all_pending: true,
                retry_step: 0,
                synapse_warned: false,
                low_battery: HashSet::new(),
                batteries: Vec::new(),
            })
        });
        log::append("tray started");
        with_app(|app| app.rescan());
        start_retries(hwnd);
        SetTimer(hwnd, TIMER_BATTERY, BATTERY_MS, None);
        if std::env::args().any(|a| a == "--editor") {
            with_app(|app| app.open_editor());
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = (lparam as u32) & 0xFFFF;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                show_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                with_app(|app| app.open_editor());
            }
            0
        }
        WM_OPEN_EDITOR => {
            with_app(|app| app.open_editor());
            0
        }
        WM_DEVICECHANGE => {
            let event = wparam as u32;
            if (event == DBT_DEVICEARRIVAL || event == DBT_DEVICEREMOVECOMPLETE) && is_razer(lparam) {
                schedule_rescan(hwnd, false);
            }
            TRUE as LRESULT
        }
        WM_POWERBROADCAST => {
            if wparam as u32 == PBT_APMRESUMEAUTOMATIC {
                log::append("resumed from sleep");
                schedule_rescan(hwnd, true);
                start_retries(hwnd);
            }
            TRUE as LRESULT
        }
        WM_TIMER if wparam == TIMER_RESCAN => {
            KillTimer(hwnd, TIMER_RESCAN);
            if with_app(|app| app.rescan()).is_none() {
                SetTimer(hwnd, TIMER_RESCAN, SETTLE_MS, None);
            }
            0
        }
        WM_TIMER if wparam == TIMER_RETRY => {
            KillTimer(hwnd, TIMER_RETRY);
            let next = with_app(|app| {
                app.rescan();
                app.retry_step += 1;
                RETRY_DELAYS_MS.get(app.retry_step).copied()
            });
            if let Some(Some(ms)) = next {
                SetTimer(hwnd, TIMER_RETRY, ms, None);
            }
            0
        }
        WM_TIMER if wparam == TIMER_BATTERY => {
            with_app(|app| app.check_battery());
            0
        }
        WM_DESTROY => {
            with_app(|app| remove_icon(app.hwnd));
            PostQuitMessage(0);
            0
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            with_app(|app| {
                add_icon(app.hwnd, app.icon);
                app.update_tooltip();
            });
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Rescan a few more times over the next minute (new devices get their profile).
fn start_retries(hwnd: HWND) {
    with_app(|app| app.retry_step = 0);
    unsafe { SetTimer(hwnd, TIMER_RETRY, RETRY_DELAYS_MS[0], None) };
}

/// Debounce: a device arrival fires one notification per HID interface.
fn schedule_rescan(hwnd: HWND, apply_all: bool) {
    with_app(|app| app.apply_all_pending |= apply_all);
    unsafe { SetTimer(hwnd, TIMER_RESCAN, SETTLE_MS, None) };
}

impl App {
    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.engine.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn rescan(&mut self) {
        let apply_all = std::mem::take(&mut self.apply_all_pending);
        let (reports, new_names, synapse) = {
            let mut engine = self.engine();
            let new = engine.rescan();
            let names: Vec<String> =
                engine.devices().iter().filter(|d| new.contains(&d.key())).map(|d| engine.label(d)).collect();
            let targets = if apply_all { engine.all_keys() } else { new };
            (engine.apply(&targets), names, engine.synapse().to_vec())
        };
        if !new_names.is_empty() {
            log::append(&format!("found: {}", new_names.join(", ")));
        }
        self.notify_failures(&reports);
        self.warn_synapse(&synapse);
        self.check_battery();
    }

    /// Synapse re-sends its own settings and holds devices open; say so once per run of it.
    fn warn_synapse(&mut self, running: &[String]) {
        if running.is_empty() {
            if self.synapse_warned {
                log::append("Synapse closed");
            }
            self.synapse_warned = false;
        } else if !self.synapse_warned {
            self.synapse_warned = true;
            log::append(&format!("Synapse running: {}", running.join(", ")));
            self.balloon(
                "Razer Synapse is running",
                "It can overwrite Secretarybird's settings. Close it, or run `secbird debloat` to stop it starting with Windows.",
            );
        }
    }

    fn check_battery(&mut self) {
        let batteries = self.engine().batteries();
        for b in &batteries {
            if b.charging || b.percent >= LOW_BATTERY + 10 {
                self.low_battery.remove(&b.key);
            } else if b.percent <= LOW_BATTERY && self.low_battery.insert(b.key.clone()) {
                log::append(&format!("low battery: {} {}%", b.name, b.percent));
                self.balloon("Battery low", &format!("{}: {}%", b.name, b.percent));
            }
        }
        self.batteries = batteries;
        self.update_tooltip();
    }

    /// Start the local editor server on first use, then open it in the default browser.
    fn open_editor(&mut self) {
        if self.editor_url.is_none() {
            match server::start(self.engine.clone()) {
                Ok(editor) => self.editor_url = Some(editor.url),
                Err(e) => return self.balloon("Could not start the editor", &e.to_string()),
            }
        }
        if let Some(url) = &self.editor_url {
            #[cfg(debug_assertions)]
            eprintln!("editor: {url}");
            unsafe {
                ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(url).as_ptr(), null(), null(), SW_SHOWNORMAL);
            }
        }
    }

    fn update_tooltip(&self) {
        let mut text = format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
        let engine = self.engine();
        if engine.devices().is_empty() {
            text.push_str("\nNo Razer devices");
        }
        for dev in engine.devices() {
            let (_, active) = Engine::profiles(dev);
            let battery = match self.batteries.iter().find(|b| b.key == dev.key()) {
                Some(b) if b.charging => format!(" · {}% charging", b.percent),
                Some(b) => format!(" · {}%", b.percent),
                None => String::new(),
            };
            text.push_str(&format!("\n{}: {active}{battery}", engine.label(dev)));
        }
        if !engine.synapse().is_empty() {
            text.push_str("\nSynapse is running");
        }
        unsafe {
            let mut nid = icon_data(self.hwnd, self.icon);
            nid.uFlags = NIF_TIP;
            copy_wide(&mut nid.szTip, &text);
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    fn notify_failures(&self, reports: &[ApplyReport]) {
        let lines: Vec<String> = reports
            .iter()
            .filter(|r| !r.failures.is_empty())
            .map(|r| format!("{} ({}): {}", r.device, r.profile, r.failures.join("; ")))
            .collect();
        if !lines.is_empty() {
            for line in &lines {
                log::append(&format!("apply failed: {line}"));
            }
            self.balloon("Some settings could not be applied", &lines.join("\n"));
        }
    }

    fn balloon(&self, title: &str, body: &str) {
        unsafe {
            let mut nid = icon_data(self.hwnd, self.icon);
            nid.uFlags = NIF_INFO;
            nid.dwInfoFlags = NIIF_WARNING;
            copy_wide(&mut nid.szInfoTitle, title);
            copy_wide(&mut nid.szInfo, body);
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Build the popup menu and remember which item maps to which profile.
    unsafe fn build_menu(&mut self) -> HMENU {
        let menu = CreatePopupMenu();
        self.profile_items.clear();
        append(menu, MF_STRING, ID_EDITOR as usize, "Open editor");
        SetMenuDefaultItem(menu, ID_EDITOR, 0);
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        let engine = self.engine.clone();
        let engine = engine.lock().unwrap_or_else(|p| p.into_inner());
        if engine.devices().is_empty() {
            append(menu, MF_STRING | MF_GRAYED, 0, "No Razer devices found");
        }
        for dev in engine.devices() {
            append(menu, MF_STRING | MF_GRAYED, 0, &engine.label(dev));
            let (names, active) = Engine::profiles(dev);
            for name in names {
                let id = ID_PROFILE_BASE + self.profile_items.len() as u32;
                let check = if name == active { MF_CHECKED } else { MF_UNCHECKED };
                append(menu, MF_STRING | check, id as usize, &format!("    {name}"));
                self.profile_items.push((dev.key(), name));
            }
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        append(menu, MF_STRING, ID_REAPPLY as usize, "Re-apply profiles");
        append(menu, MF_STRING, ID_FOLDER as usize, "Open settings folder");
        let auto = if autostart_enabled() { MF_CHECKED } else { MF_UNCHECKED };
        append(menu, MF_STRING | auto, ID_AUTOSTART as usize, "Start with Windows");
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        append(menu, MF_STRING, ID_EXIT as usize, "Exit");
        menu
    }

    fn handle_command(&mut self, id: u32) {
        match id {
            ID_REAPPLY => {
                self.apply_all_pending = true;
                self.rescan();
            }
            ID_EDITOR => self.open_editor(),
            ID_FOLDER => open_config_folder(),
            ID_AUTOSTART => set_autostart(!autostart_enabled()),
            ID_EXIT => unsafe {
                DestroyWindow(self.hwnd);
            },
            id if id >= ID_PROFILE_BASE => {
                let idx = (id - ID_PROFILE_BASE) as usize;
                if let Some((key, name)) = self.profile_items.get(idx).cloned() {
                    let result = self.engine().switch_profile(&key, &name);
                    match result {
                        Ok(reports) => self.notify_failures(&reports),
                        Err(e) => self.balloon("Could not switch profile", &e.to_string()),
                    }
                    self.update_tooltip();
                }
            }
            _ => {}
        }
    }
}

unsafe fn append(menu: HMENU, flags: MENU_ITEM_FLAGS, id: usize, text: &str) {
    AppendMenuW(menu, flags, id, wide(text).as_ptr());
}

/// TrackPopupMenu runs a modal loop that re-enters the window procedure, so the app
/// state is not borrowed while the menu is open.
unsafe fn show_menu(hwnd: HWND) {
    let Some(menu) = with_app(|app| app.build_menu()) else { return };
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
    if cmd > 0 {
        with_app(|app| app.handle_command(cmd as u32));
    }
}

unsafe fn icon_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    nid
}

unsafe fn add_icon(hwnd: HWND, icon: HICON) {
    let mut nid = icon_data(hwnd, icon);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    copy_wide(&mut nid.szTip, APP_NAME);
    Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_icon(hwnd: HWND) {
    let nid = icon_data(hwnd, null_mut());
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn register_hid_notifications(hwnd: HWND) {
    let mut filter: DEV_BROADCAST_DEVICEINTERFACE_W = zeroed();
    filter.dbcc_size = size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32;
    filter.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
    filter.dbcc_classguid = GUID_DEVINTERFACE_HID;
    RegisterDeviceNotificationW(hwnd, &filter as *const _ as *const c_void, DEVICE_NOTIFY_WINDOW_HANDLE);
}

/// True when a device-change notification names a Razer (VID 1532) interface.
unsafe fn is_razer(lparam: LPARAM) -> bool {
    let hdr = lparam as *const DEV_BROADCAST_HDR;
    if hdr.is_null() || (*hdr).dbch_devicetype != DBT_DEVTYP_DEVICEINTERFACE {
        return false;
    }
    let iface = lparam as *const DEV_BROADCAST_DEVICEINTERFACE_W;
    let name_ptr = std::ptr::addr_of!((*iface).dbcc_name) as *const u16;
    let mut len = 0;
    while *name_ptr.add(len) != 0 && len < 512 {
        len += 1;
    }
    let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, len));
    name.to_ascii_uppercase().contains("VID_1532")
}

fn autostart_enabled() -> bool {
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

fn set_autostart(enable: bool) {
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

fn open_config_folder() {
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

/// The tray variant of the logo (white background, resource id 2 from build.rs), at the
/// small-icon size Windows uses for the notification area. Resource 1 is the app icon.
unsafe fn load_icon() -> HICON {
    let icon = LoadImageW(
        GetModuleHandleW(null()),
        2 as _,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        LR_DEFAULTCOLOR,
    );
    if icon.is_null() {
        LoadIconW(null_mut(), IDI_APPLICATION)
    } else {
        icon as HICON
    }
}
