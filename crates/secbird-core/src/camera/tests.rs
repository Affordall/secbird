use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct Fake {
    writes: Mutex<Vec<Setting>>,
    fail: bool,
}
impl CameraBackend for Fake {
    fn discover(&self) -> crate::Result<Vec<CameraInfo>> {
        Ok(Vec::new())
    }
    fn set(&self, _: &str, setting: Setting) -> crate::Result<()> {
        self.writes.lock().unwrap().push(setting);
        if self.fail {
            Err(crate::Error::System("disconnected".into()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn camera_successful_write_is_saved_and_save_failure_is_reported() {
    let directory = std::env::temp_dir().join(format!(
        "secbird-camera-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("profiles.json");
    let fake = Fake::default();
    let mut store = crate::Store::load(&path).unwrap();
    actions::set(
        &fake,
        "camera:test",
        &mut store,
        "Default",
        Setting::Hdr(true),
    )
    .unwrap();
    let mut loaded = crate::Store::load(&path).unwrap();
    assert_eq!(
        loaded
            .for_camera("camera:test")
            .active()
            .camera
            .as_ref()
            .unwrap()
            .hdr,
        Some(true)
    );
    let mut invalid_store = crate::Store::default();
    let error = actions::set(
        &fake,
        "camera:test",
        &mut invalid_store,
        "Default",
        Setting::Hdr(false),
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("camera changed but profile could not be saved"));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn camera_profiles_only_apply_explicit_camera_settings_and_continue_after_failure() {
    let fake = Fake {
        fail: true,
        ..Fake::default()
    };
    let mut profile: crate::Profile = serde_json::from_str(r#"{"dpi":[1600,1600]}"#).unwrap();
    assert!(profile.camera.is_none());
    assert!(actions::apply(&fake, "camera:test", &profile).is_empty());
    profile.camera = Some(CameraSettings {
        hdr: Some(true),
        fov: Some(FieldOfView::Narrow),
        ..Default::default()
    });
    let results = actions::apply(&fake, "camera:test", &profile);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.result.is_err()));
    assert_eq!(
        *fake.writes.lock().unwrap(),
        vec![
            Setting::Hdr(true),
            Setting::FieldOfView(FieldOfView::Narrow)
        ]
    );
}

#[test]
fn camera_failed_write_and_missing_profile_never_change_saved_values() {
    let fake = Fake {
        fail: true,
        ..Fake::default()
    };
    let mut store = crate::Store::default();
    store.for_camera("camera:test").active_mut().camera = Some(CameraSettings {
        hdr: Some(false),
        ..Default::default()
    });
    assert!(actions::set(
        &fake,
        "camera:test",
        &mut store,
        "Default",
        Setting::Hdr(true)
    )
    .is_err());
    assert_eq!(
        store
            .for_camera("camera:test")
            .active()
            .camera
            .as_ref()
            .unwrap()
            .hdr,
        Some(false)
    );
    assert!(actions::set(
        &fake,
        "camera:test",
        &mut store,
        "missing",
        Setting::Hdr(true)
    )
    .is_err());
    assert_eq!(fake.writes.lock().unwrap().len(), 1);
}

#[test]
fn camera_profile_clone_import_and_identity_are_preserved() {
    let mut store = crate::Store::default();
    store.device(0x0e05).active_mut().dpi = Some((100, 100));
    let profiles = store.for_camera("camera:one");
    profiles.active_mut().camera = Some(CameraSettings {
        hdr: Some(true),
        ..Default::default()
    });
    profiles.active_mut().apps = vec!["obs.exe".into()];
    profiles.create("copy").unwrap();
    assert!(profiles.profiles["copy"].apps.is_empty());
    assert_eq!(
        profiles.profiles["copy"].camera.as_ref().unwrap().hdr,
        Some(true)
    );
    assert!(store.for_camera("camera:two").active().camera.is_none());
    assert!(store.devices.contains_key("0E05"));
    let json = serde_json::to_string(&store).unwrap();
    let mut imported = crate::Store::default();
    imported.import(serde_json::from_str(&json).unwrap());
    assert_eq!(
        imported
            .for_camera("camera:one")
            .active()
            .camera
            .as_ref()
            .unwrap()
            .hdr,
        Some(true)
    );
}

#[test]
fn camera_usb_matching_is_exact_and_identity_keeps_units_apart() {
    let path = r"\\?\usb#vid_1532&pid_0e05&mi_00#one";
    assert!(is_kiyo_pro_path(path));
    assert!(is_kiyo_pro_path(&path.to_uppercase()));
    for path in [
        "",
        "Razer Kiyo Pro",
        "vid_1532&pid_0e050",
        "vid_15320&pid_0e05",
        "vid_9999&pid_0e05",
    ] {
        assert!(!is_kiyo_pro_path(path), "{path}");
    }
    assert_eq!(camera_id(path), camera_id(&path.to_uppercase()));
    assert_ne!(camera_id(path), camera_id(&path.replace("one", "two")));
}

#[cfg(not(windows))]
#[test]
fn camera_fallback_is_explicitly_windows_only() {
    let backend = backend().unwrap();
    assert!(backend.discover().unwrap().is_empty());
    assert!(matches!(
        backend.set("camera:test", Setting::Hdr(true)),
        Err(crate::Error::Unsupported(_))
    ));
}

#[test]
fn camera_settings_default_does_not_invent_values() {
    let settings = CameraSettings::default();
    assert_eq!(serde_json::to_string(&settings).unwrap(), "{}");
    for (control, value) in [
        ("hdr", "yes"),
        ("fov", "90"),
        ("af-mode", "auto"),
        ("other", "on"),
    ] {
        assert!(parse_setting(control, value).is_err());
    }
    assert_eq!(parse_setting("hdr", "off").unwrap(), Setting::Hdr(false));
}

#[test]
fn camera_database_has_no_peripheral_capabilities() {
    let db = crate::Db::builtin().unwrap();
    let spec = db.get(0x0e05).expect("Kiyo Pro entry");
    assert_eq!(spec.kind, crate::Kind::Webcam);
    assert!(spec.leds.is_empty() && spec.dpi.is_none() && spec.polling.is_none());
}
