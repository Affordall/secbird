//! Is there newer firmware for a device? Asks Razer's public product manifest, the same file
//! Synapse reads (`apps.razer.com/synapse/products/<pid>/mw/manifest.json`, field
//! `latestFirmwareInfo`). Only reads: Secretarybird never flashes firmware, it links to
//! Razer's own updater.
//!
//! Only runs when asked (`secbird firmware`, the editor's button, or the tray's weekly check
//! once turned on).

use crate::commands as cmd;
use crate::device::Device;
use crate::error::{Error, Result};
use serde::Deserialize;

const HOST: &str = "apps.razer.com";

/// Newest firmware Razer offers for a product.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Latest {
    pub version: String,
    /// Razer's download page or updater for it.
    pub url: String,
    /// Razer's install guide.
    #[serde(default)]
    pub guide: Option<String>,
    /// Milliseconds since 1970.
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<u64>,
    /// The update only applies while one of these products is connected too (e.g. the
    /// receiver it pairs with).
    #[serde(default, rename = "requiredConnectedDevices")]
    pub required_connected: Vec<u16>,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default, rename = "latestFirmwareInfo")]
    latest: Option<Latest>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    #[serde(rename = "productId")]
    product: u16,
    /// A number, or a list when a product has several receivers.
    #[serde(default, rename = "dongleId")]
    dongle: serde_json::Value,
    #[serde(default, rename = "bleId")]
    ble: serde_json::Value,
}

impl CatalogEntry {
    fn answers_for(&self, pid: u16) -> bool {
        let has = |v: &serde_json::Value| match v {
            serde_json::Value::Number(n) => n.as_u64() == Some(u64::from(pid)),
            serde_json::Value::Array(list) => list.iter().any(|n| n.as_u64() == Some(u64::from(pid))),
            _ => false,
        };
        self.product == pid || has(&self.dongle) || has(&self.ble)
    }
}

/// What a check found for one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    UpToDate { current: String },
    Update { current: String, latest: Latest },
    /// Razer publishes no firmware for this product.
    NoneListed { current: String },
}

impl Device {
    /// Full firmware version, `major.minor.internal.reserved` (falls back to the two-part
    /// version on devices without the long form).
    pub fn firmware_version(&self) -> Result<String> {
        match self.get(cmd::FIRMWARE_EXT, cmd::FIRMWARE_EXT_SIZE, &[]) {
            Ok(r) => {
                let a = r.args();
                Ok(format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3]))
            }
            Err(_) => self.firmware().map(|(major, minor)| format!("{major}.{minor}.0.0")),
        }
    }
}

/// `a.b.c.d` as numbers, for comparing. Missing parts count as 0.
fn parts(v: &str) -> [u32; 4] {
    let mut out = [0; 4];
    for (slot, p) in out.iter_mut().zip(v.trim().split('.')) {
        *slot = p.trim().parse().unwrap_or(0);
    }
    out
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    parts(latest) > parts(current)
}

/// Compare a device's firmware with the manifest entry. `connected` are the product ids
/// plugged in right now, for updates that need a second device.
pub fn status(current: String, latest: Option<Latest>, connected: &[u16]) -> Status {
    match latest {
        Some(l) if l.required_connected.is_empty() || l.required_connected.iter().any(|p| connected.contains(p)) => {
            if is_newer(&l.version, &current) {
                Status::Update { current, latest: l }
            } else {
                Status::UpToDate { current }
            }
        }
        _ => Status::NoneListed { current },
    }
}

/// Check one device: its firmware, then Razer's manifest. Uses the network.
pub fn check(dev: &Device, connected: &[u16]) -> Result<Status> {
    let current = dev.firmware_version()?;
    Ok(status(current, latest_for(dev.pid)?, connected))
}

/// The manifest entry for a product id. Receivers and Bluetooth ids have no manifest of
/// their own, so those are looked up in Razer's device list first.
pub fn latest_for(pid: u16) -> Result<Option<Latest>> {
    if let Some(text) = get(&format!("/synapse/products/{pid}/mw/manifest.json"))? {
        return parse_manifest(&text);
    }
    let Some(list) = get("/synapse/dashboard/AvailableDevices.json")? else { return Ok(None) };
    match product_of(pid, &list)? {
        Some(product) if product != pid => match get(&format!("/synapse/products/{product}/mw/manifest.json"))? {
            Some(text) => parse_manifest(&text),
            None => Ok(None),
        },
        _ => Ok(None),
    }
}

fn parse_manifest(text: &str) -> Result<Option<Latest>> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| Error::Db(format!("firmware manifest: {e}")))?;
    Ok(m.latest.filter(|l| !l.version.is_empty() && l.url.starts_with("https://")))
}

fn product_of(pid: u16, catalog: &str) -> Result<Option<u16>> {
    let list: Vec<CatalogEntry> =
        serde_json::from_str(catalog).map_err(|e| Error::Db(format!("Razer device list: {e}")))?;
    Ok(list.iter().find(|e| e.answers_for(pid)).map(|e| e.product))
}

/// HTTPS GET from apps.razer.com. `None` on 404 (no manifest for that product).
fn get(path: &str) -> Result<Option<String>> {
    crate::net::get(HOST, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{"name":"170","latestFirmwareInfo":{"version":"2.50.3.0","url":"https://rzr.to/4fkm21",
        "guide":"https://rzr.to/4fkm21","severity":"warning","requiredConnectedDevices":[207]},"other":1}"#;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("2.50.3.0", "2.50.2.9"));
        assert!(is_newer("1.10.5.0", "1.9.0.0"), "10 > 9, not text order");
        assert!(!is_newer("1.2.0.0", "1.2.0.0"));
        assert!(!is_newer("1.2", "1.2.0.0"));
        assert!(is_newer("2.0.1.0", "1.255.0.0"));
    }

    #[test]
    fn manifest_entry_is_read_and_checked() {
        let latest = parse_manifest(MANIFEST).unwrap().unwrap();
        assert_eq!(latest.version, "2.50.3.0");
        assert_eq!(latest.required_connected, [207]);
        assert_eq!(parse_manifest(r#"{"name":"153"}"#).unwrap(), None);
        assert_eq!(parse_manifest(r#"{"latestFirmwareInfo":{"version":"1.0","url":"http://x"}}"#).unwrap(), None, "https only");

        // Needs product 207 connected.
        assert_eq!(status("2.50.1.0".into(), Some(latest.clone()), &[170]), Status::NoneListed { current: "2.50.1.0".into() });
        assert!(matches!(status("2.50.1.0".into(), Some(latest.clone()), &[170, 207]), Status::Update { .. }));
        assert_eq!(status("2.50.3.0".into(), Some(latest), &[207]), Status::UpToDate { current: "2.50.3.0".into() });
    }

    /// Talks to apps.razer.com: `cargo test -p secbird-core -- --ignored`.
    #[test]
    #[ignore]
    fn live_manifests() {
        let pro = latest_for(170).unwrap().expect("Basilisk V3 Pro lists firmware");
        assert!(pro.url.starts_with("https://"));
        assert_eq!(latest_for(171).unwrap(), Some(pro), "its receiver maps to it");
        assert_eq!(latest_for(0xFFF1).unwrap(), None, "unknown product");
    }

    #[test]
    fn receivers_map_to_their_product() {
        let list = r#"[{"productId":153},{"productId":170,"dongleId":171,"bleId":172},{"productId":300,"dongleId":[301,302]}]"#;
        assert_eq!(product_of(171, list).unwrap(), Some(170));
        assert_eq!(product_of(172, list).unwrap(), Some(170));
        assert_eq!(product_of(153, list).unwrap(), Some(153));
        assert_eq!(product_of(302, list).unwrap(), Some(300), "one of several receivers");
        assert_eq!(product_of(999, list).unwrap(), None);
    }
}
