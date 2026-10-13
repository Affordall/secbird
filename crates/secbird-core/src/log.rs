//! A small append-only log in the config folder (`secbird.log`), so failures that
//! happen in the background can be looked up later instead of vanishing.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotated to `secbird.log.old` beyond this size, so it never grows without bound.
const MAX_BYTES: u64 = 256 * 1024;

pub fn path() -> PathBuf {
    crate::config_dir().join("secbird.log")
}

pub fn append(msg: &str) {
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "{} {msg}", utc(secs));
    }
}

/// `YYYY-MM-DD HH:MM:SSZ` for a Unix time (civil-from-days, proleptic Gregorian).
fn utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_formats() {
        assert_eq!(super::utc(0), "1970-01-01 00:00:00Z");
        assert_eq!(super::utc(1_700_000_000), "2023-11-14 22:13:20Z");
        assert_eq!(super::utc(951_782_400), "2000-02-29 00:00:00Z");
    }
}
