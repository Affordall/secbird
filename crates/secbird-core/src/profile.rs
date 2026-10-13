//! Named settings profiles, kept per device and re-applied on demand.
//!
//! Every setting the user changes is recorded in the device's active profile, so the
//! store always mirrors what was last sent to the hardware.

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::Rgb;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = "Default";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<(u16, u16)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polling: Option<u32>,
    /// Lighting zone name (or numeric id) -> state.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lighting: BTreeMap<String, Zone>,
    /// Per-key colors, row-major over the device's matrix. Empty when unused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Rgb>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfiles {
    pub active: String,
    pub profiles: BTreeMap<String, Profile>,
}

impl Default for DeviceProfiles {
    fn default() -> Self {
        Self {
            active: DEFAULT_PROFILE.into(),
            profiles: BTreeMap::from([(DEFAULT_PROFILE.into(), Profile::default())]),
        }
    }
}

impl DeviceProfiles {
    pub fn active(&self) -> &Profile {
        &self.profiles[&self.active]
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        self.profiles.entry(self.active.clone()).or_default()
    }

    /// Create `name` as a copy of the active profile.
    pub fn create(&mut self, name: &str) -> Result<()> {
        if self.profiles.contains_key(name) {
            return Err(Error::InvalidArg(format!("profile '{name}' already exists")));
        }
        let copy = self.active().clone();
        self.profiles.insert(name.into(), copy);
        Ok(())
    }

    pub fn switch(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(Error::NotFound(format!("profile '{name}'")));
        }
        self.active = name.into();
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<()> {
        if name == self.active {
            return Err(Error::InvalidArg("cannot delete the active profile; switch first".into()));
        }
        self.profiles
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| Error::NotFound(format!("profile '{name}'")))
    }
}

/// All profiles on disk, keyed by device (`"0099:SERIAL"`, or `"0099"` without a serial).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceProfiles>,
    #[serde(skip)]
    path: PathBuf,
}

impl Store {
    pub fn load(path: &Path) -> Result<Self> {
        let mut store: Store = match std::fs::read_to_string(path) {
            // Notepad and PowerShell 5 may save with a UTF-8 BOM.
            Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map_err(|e| Error::Db(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
            Err(e) => return Err(Error::Db(format!("{}: {e}", path.display()))),
        };
        store.path = path.to_path_buf();
        Ok(store)
    }

    /// Write atomically: a crash mid-save never leaves a half-written file.
    pub fn save(&self) -> Result<()> {
        let io = |e: std::io::Error| Error::Db(format!("{}: {e}", self.path.display()));
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(|e| Error::Db(e.to_string()))?;
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)
    }

    /// Add every profile from `other` (an export). A name that already exists for that
    /// device is imported as "name (imported)", "name (imported 2)", ...; active
    /// profiles are left alone. Returns how many profiles were added.
    pub fn import(&mut self, other: Store) -> usize {
        let mut added = 0;
        for (key, incoming) in other.devices {
            let target = self.devices.entry(key).or_insert_with(|| DeviceProfiles {
                active: incoming.active.clone(),
                profiles: BTreeMap::new(),
            });
            for (name, profile) in incoming.profiles {
                let mut unique = name.clone();
                let mut n = 1;
                while target.profiles.contains_key(&unique) {
                    unique = if n == 1 { format!("{name} (imported)") } else { format!("{name} (imported {n})") };
                    n += 1;
                }
                target.profiles.insert(unique, profile);
                added += 1;
            }
            if !target.profiles.contains_key(&target.active) {
                target.active = target.profiles.keys().next().cloned().unwrap_or_default();
            }
        }
        added
    }

    /// Profiles of one physical device (see [`Device::key`](crate::Device::key)).
    /// Profiles saved before devices were told apart by serial live under the bare
    /// product id; the first device of that model to ask takes them over.
    pub fn profiles_for(&mut self, key: &str, pid: u16) -> &mut DeviceProfiles {
        let legacy = format!("{pid:04X}");
        if key != legacy && !self.devices.contains_key(key) {
            if let Some(old) = self.devices.remove(&legacy) {
                self.devices.insert(key.to_string(), old);
            }
        }
        self.devices.entry(key.to_string()).or_default()
    }

    pub fn for_device(&mut self, dev: &crate::Device) -> &mut DeviceProfiles {
        self.profiles_for(&dev.key(), dev.pid)
    }

    pub fn device(&mut self, pid: u16) -> &mut DeviceProfiles {
        self.devices.entry(format!("{pid:04X}")).or_default()
    }
}

/// Outcome of applying one setting from a profile.
#[derive(Debug)]
pub struct Applied {
    pub setting: String,
    pub result: Result<()>,
}

impl Profile {
    /// Send every recorded setting to the device. Keeps going past individual failures.
    pub fn apply(&self, dev: &Device) -> Vec<Applied> {
        let mut out = Vec::new();
        let mut push = |setting: String, result: Result<()>| out.push(Applied { setting, result });

        if let Some(hz) = self.polling {
            push(format!("polling {hz} Hz"), dev.set_polling_rate(hz));
        }
        if let Some((x, y)) = self.dpi {
            push(format!("dpi {x}x{y}"), dev.set_dpi(x, y).map(|_| ()));
        }
        for (name, zone) in &self.lighting {
            let led = match dev.led_id(name) {
                Ok(led) => led,
                Err(e) => {
                    push(format!("{name} lighting"), Err(e));
                    continue;
                }
            };
            if let Some(pct) = zone.brightness {
                push(format!("{name} brightness {pct}%"), dev.set_brightness(led, pct));
            }
            if zone.off {
                push(format!("{name} off"), dev.lights_off(led));
            } else if let Some(color) = zone.color {
                push(format!("{name} color {color}"), dev.set_static_color(led, color));
            }
        }
        if !self.keys.is_empty() {
            push(format!("per-key colors ({} keys)", self.keys.len()), dev.show_frame(&self.keys));
        }
        out
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_copies_active_and_switch_changes_it() {
        let mut p = DeviceProfiles::default();
        p.active_mut().dpi = Some((800, 800));
        p.create("Gaming").unwrap();
        p.switch("Gaming").unwrap();
        assert_eq!(p.active().dpi, Some((800, 800)));
        p.active_mut().dpi = Some((1600, 1600));
        assert_eq!(p.profiles[DEFAULT_PROFILE].dpi, Some((800, 800)));
    }

    #[test]
    fn cannot_delete_active_profile() {
        let mut p = DeviceProfiles::default();
        assert!(p.delete(DEFAULT_PROFILE).is_err());
    }

    #[test]
    fn store_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("secbird-test-{}", std::process::id()));
        let path = dir.join("profiles.json");
        let mut store = Store::load(&path).unwrap();
        let dev = store.device(0x0099);
        dev.active_mut().polling = Some(500);
        dev.active_mut().lighting.insert(
            "logo".into(),
            Zone { color: Some(Rgb(255, 0, 0)), off: false, brightness: Some(40) },
        );
        store.save().unwrap();

        let mut again = Store::load(&path).unwrap();
        let p = again.device(0x0099).active().clone();
        assert_eq!(p.polling, Some(500));
        assert_eq!(p.lighting["logo"].color, Some(Rgb(255, 0, 0)));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn legacy_profiles_move_to_the_serial_key() {
        let mut store = Store::default();
        store.device(0x0099).active_mut().dpi = Some((1600, 1600));
        let p = store.profiles_for("0099:PM24AB", 0x0099);
        assert_eq!(p.active().dpi, Some((1600, 1600)));
        assert!(!store.devices.contains_key("0099"));
        // A second unit of the same model starts fresh.
        assert_eq!(store.profiles_for("0099:XYZ999", 0x0099).active().dpi, None);
        assert_eq!(store.profiles_for("0099:PM24AB", 0x0099).active().dpi, Some((1600, 1600)));
    }

    #[test]
    fn import_keeps_existing_profiles() {
        let mut mine = Store::default();
        mine.device(0x0099).active_mut().dpi = Some((800, 800));
        let mut theirs = Store::default();
        theirs.device(0x0099).active_mut().dpi = Some((3200, 3200));
        theirs.device(0x0099).create("Gaming").unwrap();
        theirs.device(0x0084).active_mut().polling = Some(500);

        assert_eq!(mine.import(theirs), 3);
        let d = mine.device(0x0099);
        assert_eq!(d.active, "Default");
        assert_eq!(d.active().dpi, Some((800, 800)));
        assert_eq!(d.profiles["Default (imported)"].dpi, Some((3200, 3200)));
        assert!(d.profiles.contains_key("Gaming"));
        assert_eq!(mine.device(0x0084).active().polling, Some(500));
    }
}
