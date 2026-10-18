//! Kiyo Pro controls, separate from the HID peripheral protocol.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[cfg(any(windows, test))]
mod protocol;

pub const KIYO_PRO_PID: u16 = 0x0E05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrMode {
    Dark,
    Bright,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldOfView {
    Wide,
    Medium,
    Narrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutofocusMode {
    Responsive,
    Passive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Hdr(bool),
    HdrMode(HdrMode),
    FieldOfView(FieldOfView),
    AutofocusMode(AutofocusMode),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CameraSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr_mode: Option<HdrMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fov: Option<FieldOfView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub af_mode: Option<AutofocusMode>,
}

pub fn parse_setting(control: &str, value: &str) -> Result<Setting> {
    match (control, value) {
        ("hdr", "on") => Ok(Setting::Hdr(true)),
        ("hdr", "off") => Ok(Setting::Hdr(false)),
        ("hdr-mode", "dark") => Ok(Setting::HdrMode(HdrMode::Dark)),
        ("hdr-mode", "bright") => Ok(Setting::HdrMode(HdrMode::Bright)),
        ("fov", "wide") => Ok(Setting::FieldOfView(FieldOfView::Wide)),
        ("fov", "medium") => Ok(Setting::FieldOfView(FieldOfView::Medium)),
        ("fov", "narrow") => Ok(Setting::FieldOfView(FieldOfView::Narrow)),
        ("af-mode", "responsive") => Ok(Setting::AutofocusMode(AutofocusMode::Responsive)),
        ("af-mode", "passive") => Ok(Setting::AutofocusMode(AutofocusMode::Passive)),
        _ => Err(Error::InvalidArg(format!("camera {control} {value}: expected hdr on|off, hdr-mode dark|bright, fov wide|medium|narrow, or af-mode responsive|passive"))),
    }
}

#[cfg(test)]
mod tests;
