//! Is there a newer Secretarybird? Reads the latest published release of the repository
//! this build came from (GitHub's public API; drafts and pre-releases are never "latest").
//! Updating, when the user asks: download that release's installer, check it was signed with
//! the maintainer's key, and run it silently. The installer keeps settings and profiles,
//! closes the tray and starts the new one.

use crate::error::{Error, Result};
use serde::Deserialize;

const HOST: &str = "api.github.com";
/// The version running now.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// The public half of the key each installer is signed with (tools/sign-release.ps1, which
/// keeps the private half off GitHub): P-256, X then Y. A GitHub account taken over can
/// replace a release's files, but can't sign them.
const SIGNING_KEY: &str = "59c20fc1f6cab033661b93ac0fdac277aa51f3f2f721a82ff127920e221a1b2d006dca701090e953e34cbf24990b3a2d967c72f9a7fbccb0a81fd0e2abeec37b";

/// A newer release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    /// The release page, with the installer and zip.
    pub url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// `owner/repo` from the repository URL in Cargo.toml.
fn repo() -> &'static str {
    env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/").trim_end_matches('/')
}

/// The latest release when it is newer than this build; `None` when this is the latest
/// (or GitHub has nothing to say, e.g. rate-limited).
pub fn check() -> Result<Option<Release>> {
    match crate::net::get(HOST, &format!("/repos/{}/releases/latest", repo()))? {
        Some(text) => newer_than(CURRENT, &text),
        None => Ok(None),
    }
}

/// A release page of this project: where this build came from, or where the repository
/// lives now or may move to. GitHub redirects a moved repository's old address, and the
/// release then links to the new one.
fn ours(url: &str) -> bool {
    [repo(), "infinityInfinite/secbird", "betterslops-inc/secbird"]
        .iter()
        .any(|r| url.starts_with(&format!("https://github.com/{r}/releases/")))
}

fn newer_than(current: &str, text: &str) -> Result<Option<Release>> {
    let r: GithubRelease = serde_json::from_str(text).map_err(|e| Error::Db(format!("GitHub release: {e}")))?;
    let version = r.tag_name.trim_start_matches('v').to_string();
    if r.draft || r.prerelease || !ours(&r.html_url) || !crate::firmware::is_newer(&version, current) {
        return Ok(None);
    }
    Ok(Some(Release { version, url: r.html_url }))
}

/// Whether this copy was set up by the installer, so it can update itself. An unzipped
/// portable copy has no uninstaller next to it; it gets the release page instead.
pub fn installed() -> bool {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("unins000.exe").is_file())).unwrap_or(false)
}

fn installer_name(version: &str) -> String {
    format!("Secretarybird-Setup-{version}.exe")
}

/// Download `version`'s installer and its signature from the release, check the signature,
/// and save the installer in the temp folder. Errors say what to do instead.
pub fn download_installer(version: &str) -> Result<std::path::PathBuf> {
    if version.is_empty() || version.len() > 20 || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(Error::InvalidArg(format!("'{version}' is not a release version")));
    }
    let name = installer_name(version);
    let base = format!("/{}/releases/download/v{version}/{name}", repo());
    let missing = |what: &str| {
        Error::Unsupported(format!("Secretarybird {version} has no {what} to update from; download it from the release page instead"))
    };
    let sig = crate::net::download("github.com", &format!("{base}.sig"))?.ok_or_else(|| missing("signature"))?;
    let exe = crate::net::download("github.com", &base)?.ok_or_else(|| missing("installer"))?;
    verify(&name, &exe, &String::from_utf8_lossy(&sig))?;
    let save = |e: std::io::Error| Error::System(format!("couldn't save the update: {e}"));
    let dir = std::env::temp_dir().join("Secretarybird-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(save)?;
    let path = dir.join(&name);
    std::fs::write(&path, &exe).map_err(save)?;
    Ok(path)
}

/// Run a downloaded installer with no wizard. It closes the tray, installs over this copy
/// and starts the tray again; `/UPDATE=yes` keeps it from turning Start with Windows back on.
pub fn run_installer(path: &std::path::Path) -> Result<()> {
    std::process::Command::new(path)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-", "/UPDATE=yes"])
        .spawn()
        .map(drop)
        .map_err(|e| Error::System(format!("couldn't start the installer: {e}")))
}

/// The signature covers the installer's file name (so one version's signed installer can't
/// pass as another's) and its bytes.
fn verify(name: &str, exe: &[u8], sig_hex: &str) -> Result<()> {
    let bad = || {
        Error::Unsupported(
            "the downloaded installer isn't signed with Secretarybird's key, so it was not run. Download it from the release page if you want it anyway".into(),
        )
    };
    let sig = from_hex(sig_hex.trim()).filter(|s| s.len() == 64).ok_or_else(bad)?;
    let mut msg = Vec::with_capacity(name.len() + 1 + exe.len());
    msg.extend_from_slice(name.as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(exe);
    if verify_p256(&from_hex(SIGNING_KEY).unwrap_or_default(), &msg, &sig) {
        Ok(())
    } else {
        Err(bad())
    }
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect()
}

/// ECDSA P-256 with SHA-256 through Windows' own crypto (bcrypt.dll). `key` is X then Y,
/// `sig` is r then s.
#[cfg(windows)]
fn verify_p256(key: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Security::Cryptography::*;
    if key.len() != 64 {
        return false;
    }
    unsafe {
        let mut sha: BCRYPT_ALG_HANDLE = null_mut();
        if BCryptOpenAlgorithmProvider(&mut sha, BCRYPT_SHA256_ALGORITHM, null(), 0) != 0 {
            return false;
        }
        let mut hash = [0u8; 32];
        let hashed = BCryptHash(sha, null(), 0, msg.as_ptr(), msg.len() as u32, hash.as_mut_ptr(), 32) == 0;
        BCryptCloseAlgorithmProvider(sha, 0);
        if !hashed {
            return false;
        }
        let mut ecdsa: BCRYPT_ALG_HANDLE = null_mut();
        if BCryptOpenAlgorithmProvider(&mut ecdsa, BCRYPT_ECDSA_P256_ALGORITHM, null(), 0) != 0 {
            return false;
        }
        let mut blob = Vec::with_capacity(72);
        blob.extend_from_slice(&BCRYPT_ECDSA_PUBLIC_P256_MAGIC.to_le_bytes());
        blob.extend_from_slice(&32u32.to_le_bytes());
        blob.extend_from_slice(key);
        let mut handle: BCRYPT_KEY_HANDLE = null_mut();
        let ok = BCryptImportKeyPair(ecdsa, null_mut(), BCRYPT_ECCPUBLIC_BLOB, &mut handle, blob.as_ptr(), blob.len() as u32, 0) == 0
            && BCryptVerifySignature(handle, null(), hash.as_ptr(), 32, sig.as_ptr(), sig.len() as u32, 0) == 0;
        if !handle.is_null() {
            BCryptDestroyKey(handle);
        }
        BCryptCloseAlgorithmProvider(ecdsa, 0);
        ok
    }
}

#[cfg(not(windows))]
fn verify_p256(_key: &[u8], _msg: &[u8], _sig: &[u8]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(tag: &str) -> String {
        format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/{}/releases/tag/{tag}","draft":false,"prerelease":false,"assets":[]}}"#, repo())
    }

    #[test]
    fn only_a_newer_release_counts() {
        let r = newer_than("0.4.0", &reply("v0.4.1")).unwrap().unwrap();
        assert_eq!(r.version, "0.4.1");
        assert!(r.url.ends_with("/releases/tag/v0.4.1"));
        assert_eq!(newer_than("0.4.0", &reply("v0.4.0")).unwrap(), None);
        assert_eq!(newer_than("0.4.0", &reply("v0.3.9")).unwrap(), None);
        assert!(newer_than("0.9.0", &reply("v0.10.0")).unwrap().is_some(), "10 > 9, not text order");
        let draft = reply("v0.5.0").replace(r#""draft":false"#, r#""draft":true"#);
        assert_eq!(newer_than("0.4.0", &draft).unwrap(), None);
        let elsewhere = r#"{"tag_name":"v9.0.0","html_url":"https://evil.example/x"}"#;
        assert_eq!(newer_than("0.4.0", elsewhere).unwrap(), None, "only links to our own releases");
        let moved = r#"{"tag_name":"v0.5.0","html_url":"https://github.com/betterslops-inc/secbird/releases/tag/v0.5.0"}"#;
        assert!(newer_than("0.4.0", moved).unwrap().is_some(), "the organization the repository moves to");
    }

    /// Signed with the release key, the way tools/sign-release.ps1 signs.
    #[test]
    #[cfg(windows)]
    fn only_a_signed_installer_passes() {
        let sig = "6b37f8d9acc178cbe15a39292f10c64e2c481a4f253acb79e05a617f538f43112e59f590e38026e5d09cf40f3cc8a2eb4e156b0e9e892574847f93f987fe8437";
        let name = installer_name("0.0.1");
        assert!(verify(&name, b"test installer", sig).is_ok());
        assert!(verify(&name, b"test installer", &format!("{sig}\n")).is_ok(), "the .sig file ends in a newline");
        assert!(verify(&name, b"test installeR", sig).is_err(), "changed installer");
        assert!(verify(&installer_name("0.0.2"), b"test installer", sig).is_err(), "signed for another version");
        assert!(verify(&name, b"test installer", &sig.replace('6', "7")).is_err(), "changed signature");
        assert!(verify(&name, b"test installer", "").is_err());
        assert!(verify(&name, b"test installer", "zz").is_err());
    }

    #[test]
    fn odd_versions_are_refused_before_any_download() {
        for v in ["", "0.4.4/../../x", "0.4.4-rc1", "v0.4.4"] {
            assert!(matches!(download_installer(v), Err(Error::InvalidArg(_))), "{v}");
        }
    }

    /// Talks to api.github.com: `cargo test -p secbird-core -- --ignored`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let text = crate::net::get(HOST, &format!("/repos/{}/releases/latest", repo())).unwrap().expect("a published release");
        assert!(newer_than("0.0.1", &text).unwrap().is_some());
    }
}
