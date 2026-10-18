//! Apply explicit controls and persist only successfully sent settings.
use super::{CameraBackend, CameraSettings, Setting};
use crate::{profile::Applied, Error, Profile, Result, Store};

pub fn set(
    backend: &dyn CameraBackend,
    id: &str,
    store: &mut Store,
    profile: &str,
    setting: Setting,
) -> Result<()> {
    if !store.for_camera(id).profiles.contains_key(profile) {
        return Err(Error::NotFound(format!("profile '{profile}'")));
    }
    backend.set(id, setting)?;
    let settings = store
        .for_camera(id)
        .profiles
        .get_mut(profile)
        .expect("profile checked above")
        .camera
        .get_or_insert_with(CameraSettings::default);
    match setting {
        Setting::Hdr(value) => settings.hdr = Some(value),
        Setting::HdrMode(value) => settings.hdr_mode = Some(value),
        Setting::FieldOfView(value) => settings.fov = Some(value),
        Setting::AutofocusMode(value) => settings.af_mode = Some(value),
    }
    store.save().map_err(|error| {
        Error::System(format!(
            "camera changed but profile could not be saved: {error}"
        ))
    })
}

pub fn apply(backend: &dyn CameraBackend, id: &str, profile: &Profile) -> Vec<Applied> {
    let Some(settings) = &profile.camera else {
        return Vec::new();
    };
    [
        settings.hdr_mode.map(Setting::HdrMode),
        settings.hdr.map(Setting::Hdr),
        settings.fov.map(Setting::FieldOfView),
        settings.af_mode.map(Setting::AutofocusMode),
    ]
    .into_iter()
    .flatten()
    .map(|setting| Applied {
        setting: format!("camera {setting:?}"),
        result: backend.set(id, setting),
    })
    .collect()
}
