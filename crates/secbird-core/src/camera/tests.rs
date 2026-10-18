use super::*;

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
