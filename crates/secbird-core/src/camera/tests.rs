use super::*;

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
