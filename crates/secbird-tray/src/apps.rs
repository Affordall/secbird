//! Which program owns a window: for switching profiles with the program in front.

use std::collections::BTreeSet;
use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, TRUE};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
    GW_OWNER,
};

/// Exe name (lowercase, e.g. `cs2.exe`) of the window in front.
pub fn foreground() -> Option<String> {
    exe_of(unsafe { GetForegroundWindow() })
}

/// Exe names of programs with a visible, titled top-level window, sorted. Offered as
/// suggestions in the editor so nobody has to know the exe name of their game.
pub fn running() -> Vec<String> {
    unsafe extern "system" fn each(hwnd: HWND, list: LPARAM) -> BOOL {
        let list = &mut *(list as *mut BTreeSet<String>);
        let shown = IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null();
        if shown && GetWindowTextLengthW(hwnd) > 0 {
            if let Some(exe) = exe_of(hwnd) {
                list.insert(exe);
            }
        }
        TRUE
    }
    let mut list = BTreeSet::new();
    unsafe { EnumWindows(Some(each), &mut list as *mut _ as LPARAM) };
    // Windows' own shell windows are never a game.
    for exe in ["explorer.exe", "applicationframehost.exe", "textinputhost.exe", "secbird-tray.exe"] {
        list.remove(exe);
    }
    list.into_iter().collect()
}

/// Limited query rights work on most processes, elevated games included; protected
/// processes refuse and yield `None`.
fn exe_of(hwnd: HWND) -> Option<String> {
    if hwnd.is_null() {
        return None;
    }
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        ok.then(|| secbird_core::profile::normalize_app(&String::from_utf16_lossy(&buf[..len as usize])))
    }
}
