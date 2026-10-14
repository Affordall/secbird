//! The little network use there is: HTTPS GETs for the firmware check (apps.razer.com) and
//! the new-version check (api.github.com). Nothing is ever sent but the request itself.

use crate::error::{Error, Result};

/// HTTPS GET. `None` on 403 or 404 (nothing there, or a rate limit).
#[cfg(windows)]
pub fn get(host: &str, path: &str) -> Result<Option<String>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Networking::WinHttp::*;
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    struct Handle(*mut core::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
    let fail = |what: &str| Error::Unsupported(format!("couldn't reach {host} ({what}); check the internet connection"));
    unsafe {
        let agent = wide(concat!("Secretarybird/", env!("CARGO_PKG_VERSION")));
        let session = Handle(WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0));
        if session.0.is_null() {
            return Err(fail("open"));
        }
        // Short: the editor waits on firmware checks, and a failed check is harmless.
        WinHttpSetTimeouts(session.0, 5_000, 5_000, 5_000, 8_000);
        let connect = Handle(WinHttpConnect(session.0, wide(host).as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if connect.0.is_null() {
            return Err(fail("connect"));
        }
        let request = Handle(WinHttpOpenRequest(
            connect.0,
            wide("GET").as_ptr(),
            wide(path).as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null()
            || WinHttpSendRequest(request.0, null(), 0, null(), 0, 0, 0) == 0
            || WinHttpReceiveResponse(request.0, null_mut()) == 0
        {
            return Err(fail("request"));
        }
        let mut status: u32 = 0;
        let mut len = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&mut status as *mut u32).cast(),
            &mut len,
            null_mut(),
        );
        match status {
            200 => {}
            403 | 404 => return Ok(None),
            other => return Err(fail(&format!("HTTP {other}"))),
        }
        let mut body = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            let mut read = 0u32;
            if WinHttpReadData(request.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read) == 0 {
                return Err(fail("read"));
            }
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
            if body.len() > 4 << 20 {
                return Err(fail("reply too large"));
            }
        }
        Ok(Some(String::from_utf8_lossy(&body).into_owned()))
    }
}

#[cfg(not(windows))]
pub fn get(_host: &str, _path: &str) -> Result<Option<String>> {
    Err(Error::Unsupported("network checks are only built for Windows".into()))
}
