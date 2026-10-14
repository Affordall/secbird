//! Is there a newer Secretarybird? Reads the latest published release of the repository
//! this build came from (GitHub's public API; drafts and pre-releases are never "latest").
//! Only reads: updating means running the new installer, which keeps settings and profiles.

use crate::error::{Error, Result};
use serde::Deserialize;

const HOST: &str = "api.github.com";
/// The version running now.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

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

fn newer_than(current: &str, text: &str) -> Result<Option<Release>> {
    let r: GithubRelease = serde_json::from_str(text).map_err(|e| Error::Db(format!("GitHub release: {e}")))?;
    let version = r.tag_name.trim_start_matches('v').to_string();
    let ours = format!("https://github.com/{}/releases/", repo());
    if r.draft || r.prerelease || !r.html_url.starts_with(&ours) || !crate::firmware::is_newer(&version, current) {
        return Ok(None);
    }
    Ok(Some(Release { version, url: r.html_url }))
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
    }

    /// Talks to api.github.com: `cargo test -p secbird-core -- --ignored`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let text = crate::net::get(HOST, &format!("/repos/{}/releases/latest", repo())).unwrap().expect("a published release");
        assert!(newer_than("0.0.1", &text).unwrap().is_some());
    }
}
