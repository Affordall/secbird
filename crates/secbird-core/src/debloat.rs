//! Find and turn off Razer's background services and startup entries.
//!
//! Only Win32 services and `Run` registry entries are touched, never drivers.
//! Every change is a plain `sc.exe` / `reg.exe` command that is printed before
//! it runs, so anyone can check it or run it by hand.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartType {
    Auto,
    DelayedAuto,
    Demand,
    Disabled,
    Other,
}

impl StartType {
    /// The value for `sc.exe config <name> start= <value>`.
    fn sc_value(self) -> Option<&'static str> {
        match self {
            StartType::Auto => Some("auto"),
            StartType::DelayedAuto => Some("delayed-auto"),
            StartType::Demand => Some("demand"),
            StartType::Disabled => Some("disabled"),
            StartType::Other => None,
        }
    }
}

impl fmt::Display for StartType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StartType::Auto => "automatic",
            StartType::DelayedAuto => "automatic (delayed)",
            StartType::Demand => "manual",
            StartType::Disabled => "disabled",
            StartType::Other => "boot/system",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub display_name: String,
    pub image: String,
    pub start: StartType,
    pub running: bool,
    /// Working set in bytes; `None` when the process can't be opened (not elevated).
    pub memory: Option<u64>,
}

impl Service {
    pub fn needs_change(&self) -> bool {
        self.running || self.start != StartType::Disabled
    }
}

/// A value under a `...\CurrentVersion\Run` key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEntry {
    /// Full key path as `reg.exe` takes it, e.g. `HKCU\Software\...\Run`.
    pub key: String,
    pub name: String,
    pub data: String,
    /// `REG_EXPAND_SZ` rather than `REG_SZ`.
    #[serde(default)]
    pub expand: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub services: Vec<Service>,
    pub run_entries: Vec<RunEntry>,
}

/// One `sc.exe` / `reg.exe` invocation from `%SystemRoot%\System32`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl Cmd {
    fn new(program: &'static str, args: &[&str]) -> Self {
        Cmd { program, args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// Only per-user registry edits work without elevation.
    pub fn needs_admin(&self) -> bool {
        !(self.program == "reg.exe" && self.args.get(1).is_some_and(|k| k.starts_with("HKCU\\")))
    }

    pub fn run(&self) -> Result<()> {
        let exe = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| r"C:\Windows".into())
            .join("System32")
            .join(self.program);
        let out = std::process::Command::new(exe)
            .args(&self.args)
            .output()
            .map_err(|e| Error::System(format!("{}: {e}", self.program)))?;
        if out.status.success() {
            return Ok(());
        }
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let msg = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("failed");
        Err(Error::System(msg.to_string()))
    }
}

/// Shown exactly as Rust passes it to the process (MSVC quoting rules).
impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.program)?;
        for a in &self.args {
            write!(f, " {}", quote(a))?;
        }
        Ok(())
    }
}

fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

/// Commands that disable and stop the services and remove the startup entries.
pub fn plan(scan: &Scan) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    for s in scan.services.iter().filter(|s| s.needs_change()) {
        if s.start != StartType::Disabled {
            cmds.push(Cmd::new("sc.exe", &["config", &s.name, "start=", "disabled"]));
        }
        if s.running {
            cmds.push(Cmd::new("sc.exe", &["stop", &s.name]));
        }
    }
    for r in &scan.run_entries {
        cmds.push(Cmd::new("reg.exe", &["delete", &r.key, "/v", &r.name, "/f"]));
    }
    cmds
}

/// Original state saved before `plan` runs, so it can be undone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Backup {
    pub services: Vec<SavedService>,
    pub run_entries: Vec<RunEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedService {
    pub name: String,
    pub start: StartType,
    pub running: bool,
}

impl Backup {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map(Some)
                .map_err(|e| Error::Db(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Db(format!("{}: {e}", path.display()))),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let io = |e: std::io::Error| Error::Db(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| Error::Db(e.to_string()))?;
        std::fs::write(path, text).map_err(io)
    }

    /// Add what `plan(scan)` is about to change. Entries saved by an earlier
    /// run are kept, so the backup always holds the state before the first run.
    pub fn record(&mut self, scan: &Scan) {
        for s in scan.services.iter().filter(|s| s.needs_change()) {
            if !self.services.iter().any(|b| b.name.eq_ignore_ascii_case(&s.name)) {
                self.services.push(SavedService { name: s.name.clone(), start: s.start, running: s.running });
            }
        }
        for r in &scan.run_entries {
            if !self.run_entries.iter().any(|b| b.key == r.key && b.name == r.name) {
                self.run_entries.push(r.clone());
            }
        }
    }
}

/// Commands that put back what the backup recorded.
pub fn undo_plan(backup: &Backup) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    for s in &backup.services {
        if let Some(v) = s.start.sc_value() {
            cmds.push(Cmd::new("sc.exe", &["config", &s.name, "start=", v]));
        }
        if s.running {
            cmds.push(Cmd::new("sc.exe", &["start", &s.name]));
        }
    }
    for r in &backup.run_entries {
        let kind = if r.expand { "REG_EXPAND_SZ" } else { "REG_SZ" };
        cmds.push(Cmd::new("reg.exe", &["add", &r.key, "/v", &r.name, "/t", kind, "/d", &r.data, "/f"]));
    }
    cmds
}

fn is_razer(path: &str) -> bool {
    path.to_ascii_lowercase().contains("\\razer\\")
}

pub fn scan() -> Result<Scan> {
    Ok(Scan { services: win::services()?, run_entries: win::run_entries() })
}

pub fn is_elevated() -> bool {
    win::is_elevated()
}

/// Running Razer Synapse processes (Synapse 2, 3 or 4). Synapse keeps re-sending its
/// own settings, so it fights with Secretarybird over the same devices.
pub fn synapse_running() -> Vec<String> {
    let mut names: Vec<String> = win::process_names().into_iter().filter(|n| is_synapse(n)).collect();
    names.sort();
    names.dedup();
    names
}

fn is_synapse(exe: &str) -> bool {
    let n = exe.to_ascii_lowercase();
    n == "razerappengine.exe" || n == "rzsynapse.exe" || n.starts_with("razer synapse")
}

mod win {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::ProcessStatus::*;
    use windows_sys::Win32::System::Registry::*;
    use windows_sys::Win32::System::Services::*;
    use windows_sys::Win32::System::Threading::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }

    fn last_error(what: &str) -> Error {
        Error::System(format!("{what}: {}", std::io::Error::last_os_error()))
    }

    struct ScHandle(SC_HANDLE);
    impl Drop for ScHandle {
        fn drop(&mut self) {
            unsafe { CloseServiceHandle(self.0) };
        }
    }

    pub fn services() -> Result<Vec<Service>> {
        unsafe {
            let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE);
            if scm.is_null() {
                return Err(last_error("open service manager"));
            }
            let scm = ScHandle(scm);

            // u64 storage keeps the returned structs aligned.
            let mut buf: Vec<u64> = Vec::new();
            let mut count = 0u32;
            loop {
                let (mut needed, mut resume) = (0u32, 0u32);
                let ok = EnumServicesStatusExW(
                    scm.0,
                    SC_ENUM_PROCESS_INFO,
                    SERVICE_WIN32,
                    SERVICE_STATE_ALL,
                    buf.as_mut_ptr().cast(),
                    (buf.len() * 8) as u32,
                    &mut needed,
                    &mut count,
                    &mut resume,
                    null(),
                );
                if ok != 0 {
                    break;
                }
                if GetLastError() != ERROR_MORE_DATA {
                    return Err(last_error("list services"));
                }
                buf.resize(buf.len() + (needed as usize).div_ceil(8) + 64, 0);
            }

            let entries = std::slice::from_raw_parts(
                buf.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
                count as usize,
            );
            let mut out = Vec::new();
            for e in entries {
                let name = from_wide(e.lpServiceName);
                let Some((image, start)) = config(&scm, e.lpServiceName) else { continue };
                if !(name.to_ascii_lowercase().starts_with("razer") || is_razer(&image)) {
                    continue;
                }
                let status = &e.ServiceStatusProcess;
                out.push(Service {
                    display_name: from_wide(e.lpDisplayName),
                    name,
                    image,
                    start,
                    running: status.dwCurrentState != SERVICE_STOPPED,
                    memory: memory(status.dwProcessId),
                });
            }
            out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
            Ok(out)
        }
    }

    unsafe fn config(scm: &ScHandle, name: *const u16) -> Option<(String, StartType)> {
        let h = OpenServiceW(scm.0, name, SERVICE_QUERY_CONFIG);
        if h.is_null() {
            return None;
        }
        let h = ScHandle(h);
        let mut needed = 0u32;
        QueryServiceConfigW(h.0, null_mut(), 0, &mut needed);
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        if QueryServiceConfigW(h.0, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut needed) == 0 {
            return None;
        }
        let cfg = &*buf.as_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        let start = match cfg.dwStartType {
            SERVICE_AUTO_START => {
                let mut info = SERVICE_DELAYED_AUTO_START_INFO { fDelayedAutostart: 0 };
                let ok = QueryServiceConfig2W(
                    h.0,
                    SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                    (&mut info as *mut SERVICE_DELAYED_AUTO_START_INFO).cast(),
                    std::mem::size_of_val(&info) as u32,
                    &mut needed,
                );
                if ok != 0 && info.fDelayedAutostart != 0 {
                    StartType::DelayedAuto
                } else {
                    StartType::Auto
                }
            }
            SERVICE_DEMAND_START => StartType::Demand,
            SERVICE_DISABLED => StartType::Disabled,
            _ => StartType::Other,
        };
        Some((from_wide(cfg.lpBinaryPathName), start))
    }

    unsafe fn memory(pid: u32) -> Option<u64> {
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let ok = K32GetProcessMemoryInfo(h, &mut pmc, pmc.cb);
        CloseHandle(h);
        (ok != 0).then_some(pmc.WorkingSetSize as u64)
    }

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";

    pub fn run_entries() -> Vec<RunEntry> {
        let keys = [(HKEY_CURRENT_USER, "HKCU", RUN), (HKEY_LOCAL_MACHINE, "HKLM", RUN), (HKEY_LOCAL_MACHINE, "HKLM", RUN32)];
        let mut out = Vec::new();
        for (root, prefix, sub) in keys {
            unsafe {
                let mut key: HKEY = null_mut();
                if RegOpenKeyExW(root, wide(sub).as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut key) != 0 {
                    continue;
                }
                let mut name = vec![0u16; 16384];
                let mut data = vec![0u16; 16384];
                for index in 0.. {
                    let mut name_len = name.len() as u32;
                    let mut data_len = (data.len() * 2) as u32;
                    let mut kind = 0u32;
                    let rc = RegEnumValueW(
                        key,
                        index,
                        name.as_mut_ptr(),
                        &mut name_len,
                        null(),
                        &mut kind,
                        data.as_mut_ptr().cast(),
                        &mut data_len,
                    );
                    if rc == ERROR_NO_MORE_ITEMS {
                        break;
                    }
                    if rc != 0 || !(kind == REG_SZ || kind == REG_EXPAND_SZ) {
                        continue;
                    }
                    let value = &data[..data_len as usize / 2];
                    let value = String::from_utf16_lossy(value).trim_end_matches('\0').to_string();
                    if is_razer(&value) {
                        out.push(RunEntry {
                            key: format!("{prefix}\\{sub}"),
                            name: String::from_utf16_lossy(&name[..name_len as usize]),
                            data: value,
                            expand: kind == REG_EXPAND_SZ,
                        });
                    }
                }
                RegCloseKey(key);
            }
        }
        out
    }

    pub fn process_names() -> Vec<String> {
        use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
        let mut out = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut entry);
            while ok != 0 {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                out.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                ok = Process32NextW(snap, &mut entry);
            }
            CloseHandle(snap);
        }
        out
    }

    pub fn is_elevated() -> bool {
        unsafe {
            let mut token: HANDLE = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            );
            CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(name: &str, start: StartType, running: bool) -> Service {
        Service {
            name: name.into(),
            display_name: name.into(),
            image: format!(r"C:\Program Files\Razer\{name}.exe"),
            start,
            running,
            memory: None,
        }
    }

    fn sample() -> Scan {
        Scan {
            services: vec![
                service("Razer Game Manager Service 3", StartType::Auto, true),
                service("Razer Elevation Service", StartType::DelayedAuto, false),
                service("Razer Old", StartType::Disabled, false),
            ],
            run_entries: vec![RunEntry {
                key: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".into(),
                name: "RazerAppEngine".into(),
                data: r#""C:\Program Files\Razer\RazerAppEngine\RazerAppEngine.exe" --autoStart=1"#.into(),
                expand: false,
            }],
        }
    }

    #[test]
    fn recognizes_synapse_processes() {
        for n in ["RazerAppEngine.exe", "Razer Synapse 3.exe", "Razer Synapse Service.exe", "RzSynapse.exe"] {
            assert!(is_synapse(n), "{n}");
        }
        for n in ["secbird-tray.exe", "razer_elevation_service.exe", "GameManagerService3.exe"] {
            assert!(!is_synapse(n), "{n}");
        }
        assert!(win::process_names().iter().any(|n| n.eq_ignore_ascii_case("cargo.exe") || n.ends_with(".exe")));
    }

    #[test]
    fn quoting_matches_msvc_rules() {
        assert_eq!(quote("stop"), "stop");
        assert_eq!(quote("Razer Elevation Service"), r#""Razer Elevation Service""#);
        assert_eq!(quote(r#""C:\a b\x.exe" -y"#), r#""\"C:\a b\x.exe\" -y""#);
        assert_eq!(quote(r"C:\dir with space\"), r#""C:\dir with space\\""#);
        assert_eq!(quote(""), r#""""#);
    }

    #[test]
    fn plan_skips_already_disabled() {
        let cmds: Vec<String> = plan(&sample()).iter().map(|c| c.to_string()).collect();
        assert_eq!(
            cmds,
            [
                r#"sc.exe config "Razer Game Manager Service 3" start= disabled"#,
                r#"sc.exe stop "Razer Game Manager Service 3""#,
                r#"sc.exe config "Razer Elevation Service" start= disabled"#,
                r"reg.exe delete HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v RazerAppEngine /f",
            ]
        );
    }

    #[test]
    fn undo_restores_original_state() {
        let mut backup = Backup::default();
        backup.record(&sample());
        let cmds: Vec<String> = undo_plan(&backup).iter().map(|c| c.to_string()).collect();
        assert_eq!(cmds[0], r#"sc.exe config "Razer Game Manager Service 3" start= auto"#);
        assert_eq!(cmds[1], r#"sc.exe start "Razer Game Manager Service 3""#);
        assert_eq!(cmds[2], r#"sc.exe config "Razer Elevation Service" start= delayed-auto"#);
        assert!(cmds[3].starts_with(r"reg.exe add HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v RazerAppEngine /t REG_SZ /d "));
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn record_keeps_the_first_original_state() {
        let mut backup = Backup::default();
        backup.record(&sample());
        // A second run sees the service already disabled but running again.
        let later = Scan {
            services: vec![service("Razer Game Manager Service 3", StartType::Disabled, true)],
            run_entries: vec![],
        };
        backup.record(&later);
        assert_eq!(backup.services[0].start, StartType::Auto);
        assert_eq!(backup.services.len(), 2);
    }

    #[test]
    fn only_hkcu_edits_skip_admin() {
        let cmds = plan(&sample());
        assert!(cmds[0].needs_admin());
        assert!(!cmds[3].needs_admin());
    }
}
