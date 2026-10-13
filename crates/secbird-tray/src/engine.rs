//! Device and profile handling for the tray, kept free of Win32 code.

use secbird_core::{config_dir, discover, hidapi::HidApi, Db, Device, Result, Store};
use std::collections::HashSet;

pub struct Engine {
    api: HidApi,
    devices: Vec<Device>,
    unsupported: Vec<Unsupported>,
    /// Synapse processes seen at the last rescan.
    synapse: Vec<String>,
}

/// Battery state of one wireless device.
pub struct Battery {
    pub key: String,
    pub name: String,
    pub percent: u8,
    pub charging: bool,
}

/// A connected Razer product that can't be controlled (yet), shown so it isn't silently missing.
pub struct Unsupported {
    pub pid: u16,
    pub name: String,
    pub reason: &'static str,
}

/// What happened when a device's active profile was sent to it.
pub struct ApplyReport {
    pub device: String,
    pub profile: String,
    pub failures: Vec<String>,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Self { api: HidApi::new()?, devices: Vec::new(), unsupported: Vec::new(), synapse: Vec::new() })
    }

    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn unsupported(&self) -> &[Unsupported] {
        &self.unsupported
    }

    pub fn synapse(&self) -> &[String] {
        &self.synapse
    }

    /// Battery of every wireless device that answers.
    pub fn batteries(&self) -> Vec<Battery> {
        self.devices
            .iter()
            .filter_map(|d| {
                let percent = d.battery().ok().flatten()?;
                Some(Battery { key: d.key(), name: d.name(), percent, charging: d.charging().unwrap_or(false) })
            })
            .collect()
    }

    /// Re-enumerate devices. Returns the keys of devices that were not present before.
    pub fn rescan(&mut self) -> Vec<String> {
        let before: HashSet<String> = self.devices.iter().map(|d| d.key()).collect();
        self.devices.clear(); // close stale handles before reopening
        let _ = self.api.refresh_devices();
        let db = Db::load(Some(&config_dir().join("devices.json")))
            .or_else(|_| Db::builtin())
            .unwrap_or_default();
        self.devices = discover(&self.api, &db);
        self.unsupported = secbird_core::device::unanswered(&self.api, &self.devices)
            .into_iter()
            .map(|pid| match db.get(pid) {
                Some(spec) if spec.lighting == secbird_core::Lighting::Kraken => {
                    Unsupported { pid, name: spec.name.clone(), reason: "Headset protocol not supported yet" }
                }
                Some(spec) => Unsupported { pid, name: spec.name.clone(), reason: "Not answering. Is Synapse running?" },
                None => Unsupported { pid, name: format!("Razer device {pid:04X}"), reason: "Unknown model. Run secbird report" },
            })
            .collect();
        self.synapse = secbird_core::debloat::synapse_running();
        self.devices.iter().map(|d| d.key()).filter(|k| !before.contains(k)).collect()
    }

    /// Read profiles fresh from disk, so changes made with the CLI are picked up.
    fn store() -> Result<Store> {
        Store::load(&config_dir().join("profiles.json"))
    }

    /// Profile names for a device and which one is active.
    pub fn profiles(dev: &Device) -> (Vec<String>, String) {
        match Self::store() {
            Ok(mut store) => {
                let p = store.for_device(dev);
                (p.profiles.keys().cloned().collect(), p.active.clone())
            }
            Err(_) => (Vec::new(), String::new()),
        }
    }

    /// Apply the active profile of each listed device (by [`Device::key`]).
    pub fn apply(&self, keys: &[String]) -> Vec<ApplyReport> {
        let mut store = Self::store();
        self.devices
            .iter()
            .filter(|d| keys.contains(&d.key()))
            .map(|dev| match &mut store {
                Ok(store) => {
                    let p = store.for_device(dev);
                    let failures = p
                        .active()
                        .apply(dev)
                        .into_iter()
                        .filter_map(|a| a.result.err().map(|e| format!("{}: {e}", a.setting)))
                        .collect();
                    ApplyReport { device: dev.name(), profile: p.active.clone(), failures }
                }
                Err(e) => ApplyReport {
                    device: dev.name(),
                    profile: String::new(),
                    failures: vec![format!("profiles.json: {e}")],
                },
            })
            .collect()
    }

    pub fn all_keys(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.key()).collect()
    }

    /// Make `name` the active profile for a device and apply it.
    pub fn switch_profile(&self, key: &str, name: &str) -> Result<Vec<ApplyReport>> {
        let dev = self
            .devices
            .iter()
            .find(|d| d.key() == key)
            .ok_or_else(|| secbird_core::Error::NotFound(format!("device {key}")))?;
        let mut store = Self::store()?;
        store.for_device(dev).switch(name)?;
        store.save()?;
        Ok(self.apply(&[key.to_string()]))
    }

    /// Menu/tooltip label: the name, plus a serial tail when several units share it.
    pub fn label(&self, dev: &Device) -> String {
        let twins = self.devices.iter().filter(|d| d.pid == dev.pid).count();
        match dev.serial.len() {
            n if twins > 1 && n > 0 => format!("{} ({})", dev.name(), &dev.serial[n.saturating_sub(4)..]),
            _ => dev.name(),
        }
    }
}
