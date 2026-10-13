//! A small popup above the clock ("1600 DPI", "Gaming profile") that disappears after a
//! moment. It never takes focus and lets clicks through, so it does not get in the way
//! of a game.

use std::cell::{Cell, RefCell};
use std::mem::zeroed;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: &str = "SecretarybirdOsd";
const TIMER_HIDE: usize = 1;
const SHOW_MS: u32 = 1300;
const BACKGROUND: COLORREF = 0x0022_1D1D; // BGR: #1D1D22
const FOREGROUND: COLORREF = 0x00F1_ECEC;

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
    static TEXT: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static FONT: Cell<HFONT> = const { Cell::new(null_mut()) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Show `text` (replacing what is shown), then hide it after a moment.
pub fn show(text: &str) {
    unsafe {
        let hwnd = window();
        if hwnd.is_null() {
            return;
        }
        TEXT.with(|t| *t.borrow_mut() = text.encode_utf16().collect());

        let screen = GetDC(null_mut());
        let scale = GetDeviceCaps(screen, LOGPIXELSY as _) as f32 / 96.0;
        let font = font(scale);
        let old = SelectObject(screen, font as _);
        let mut size: SIZE = zeroed();
        TEXT.with(|t| GetTextExtentPoint32W(screen, t.borrow().as_ptr(), t.borrow().len() as i32, &mut size));
        SelectObject(screen, old);
        ReleaseDC(null_mut(), screen);

        let px = |v: f32| (v * scale).round() as i32;
        let (w, h) = (size.cx + px(36.0), px(44.0));
        let mut work: RECT = zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as _, 0);
        let (x, y) = (work.right - w - px(16.0), work.bottom - h - px(16.0));
        SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, w + 1, h + 1, px(12.0), px(12.0)), 0);
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(hwnd, null(), 1);
        SetTimer(hwnd, TIMER_HIDE, SHOW_MS, None);
    }
}

unsafe fn font(scale: f32) -> HFONT {
    if FONT.with(Cell::get).is_null() {
        let f = CreateFontW(
            -(19.0 * scale).round() as i32,
            0,
            0,
            0,
            FW_SEMIBOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            wide("Segoe UI").as_ptr(),
        );
        FONT.with(|c| c.set(f));
    }
    FONT.with(Cell::get)
}

/// Created on first use: most sessions never show a popup.
unsafe fn window() -> HWND {
    if WINDOW.with(Cell::get).is_null() {
        let hinstance = GetModuleHandleW(null());
        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            class.as_ptr(),
            wide("Secretarybird").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        SetLayeredWindowAttributes(hwnd, 0, 235, LWA_ALPHA);
        WINDOW.with(|c| c.set(hwnd));
    }
    WINDOW.with(Cell::get)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut rc: RECT = zeroed();
            GetClientRect(hwnd, &mut rc);
            let brush = CreateSolidBrush(BACKGROUND);
            FillRect(dc, &rc, brush);
            DeleteObject(brush as _);
            let old = SelectObject(dc, FONT.with(Cell::get) as _);
            SetBkMode(dc, TRANSPARENT as _);
            SetTextColor(dc, FOREGROUND);
            TEXT.with(|t| {
                let mut text = t.borrow().clone();
                DrawTextW(dc, text.as_mut_ptr(), text.len() as i32, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            });
            SelectObject(dc, old);
            EndPaint(hwnd, &ps);
            0
        }
        WM_TIMER if wparam == TIMER_HIDE => {
            KillTimer(hwnd, TIMER_HIDE);
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
