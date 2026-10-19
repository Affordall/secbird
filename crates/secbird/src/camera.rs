use secbird_core::{
    camera::{
        actions,
        formats::{CameraFormat, FormatRequest},
        parse_setting, CameraBackend, CameraInfo, Setting,
    },
    Error, Result, Store,
};

pub(crate) fn select<'a>(
    cameras: &'a [CameraInfo],
    selector: Option<&str>,
) -> Result<&'a CameraInfo> {
    let matches: Vec<_> = cameras
        .iter()
        .filter(|camera| {
            selector.is_none_or(|id| {
                id.eq_ignore_ascii_case(&camera.id)
                    || id.eq_ignore_ascii_case(&format!("{:04X}", camera.pid))
            })
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(Error::NotFound(
            "no matching camera; use secbird list".into(),
        )),
        _ => Err(Error::InvalidArg(
            "several cameras match; use -d with the full camera identity from secbird list".into(),
        )),
    }
}

fn parse_command(args: &[String]) -> Result<Setting> {
    let [control, value] = args else {
        return Err(Error::InvalidArg(
            "usage: camera <hdr|hdr-mode|fov|af-mode> <value>".into(),
        ));
    };
    parse_setting(control, value)
}

enum NativeFormatCommand {
    Read(u32),
    Set(FormatRequest),
}

fn parse_native_format(args: &[String]) -> Result<NativeFormatCommand> {
    let usage =
        || Error::InvalidArg("usage: camera format [WIDTHxHEIGHT FPS FOURCC] [--pin N]".into());
    let (args, pin) = if args.len() >= 2 && args[args.len() - 2] == "--pin" {
        let pin = args[args.len() - 1].parse::<u32>().map_err(|_| usage())?;
        (&args[..args.len() - 2], pin)
    } else {
        (args, 0)
    };
    if pin >= 64 {
        return Err(usage());
    }
    match args {
        [] => Ok(NativeFormatCommand::Read(pin)),
        [dimensions, fps, pixel] => {
            let (width, height) = dimensions.split_once(['x', 'X']).ok_or_else(usage)?;
            Ok(NativeFormatCommand::Set(FormatRequest::new(
                pin,
                width.parse().map_err(|_| usage())?,
                height.parse().map_err(|_| usage())?,
                fps,
                pixel,
            )?))
        }
        _ => Err(usage()),
    }
}

fn handle_format(format: &CameraFormat) -> String {
    let fps = format
        .frame_interval_100ns
        .filter(|interval| *interval > 0)
        .map(|interval| {
            format!(
                "{:.3} (exact FPS 10000000/{interval})",
                10_000_000.0 / interval as f64
            )
        })
        .unwrap_or_else(|| "unknown".into());
    format!(
        "pin {}  {}x{}  {}  handle FPS {fps}",
        format.pin, format.width, format.height, format.pixel_format
    )
}

pub(crate) fn run(
    backend: &dyn CameraBackend,
    camera: &CameraInfo,
    command: &str,
    args: &[String],
    store: &mut Store,
) -> Result<()> {
    match command {
        "camera" if args.first().is_some_and(|arg| arg == "format") => {
            match parse_native_format(&args[1..])? {
                NativeFormatCommand::Read(pin) => {
                    let format = backend.format(&camera.id, pin)?;
                    println!("Opened-handle format (not another app's stream or a proven global default):\n  {}", handle_format(&format));
                }
                NativeFormatCommand::Set(request) => {
                    println!("Experimental native-handle change; no persistent or cross-application effect is promised.");
                    let change = backend.set_format(&camera.id, &request)?;
                    if !request.matches(&change.current) {
                        return Err(Error::System(
                            "native format readback does not match the request".into(),
                        ));
                    }
                    println!(
                        "Before: {}\nAfter:  {}",
                        handle_format(&change.previous),
                        handle_format(&change.current)
                    );
                    println!("SetFormat and same-handle readback succeeded. After this process exits, run camera format in a NEW invocation to test retention.");
                    println!("An unconnected pin may store this only in memory. Losing it on reopen does not rule out other device-specific persistence mechanisms.");
                }
            }
            println!("No profiles saved; no stream started. Close other camera apps and the tray for a clean persistence test.");
            Ok(())
        }
        "camera" if args.first().is_some_and(|arg| arg == "formats") => {
            if args.len() != 1 {
                return Err(Error::InvalidArg("usage: camera formats".into()));
            }
            let formats = backend.formats(&camera.id)?;
            println!(
                "Native driver formats for {} (advertised, not current stream or saved defaults):",
                camera.name
            );
            if formats.is_empty() {
                println!("  No native video output formats were reported.");
            }
            for format in formats {
                println!("  {format}");
            }
            println!("Ranges do not imply that every intermediate FPS is supported. No settings changed; no stream started.");
            Ok(())
        }
        "info" | "camera" if args.is_empty() => {
            println!("{} [{}]", camera.name, camera.id);
            println!("Saved settings (current hardware values unknown):");
            println!(
                "{}",
                serde_json::to_string_pretty(&store.for_camera(&camera.id).active().camera)
                    .map_err(|e| Error::Db(e.to_string()))?
            );
            if let Some(error) = &camera.control_error {
                println!("Controls: {error}");
            }
            Ok(())
        }
        "camera" => {
            let setting = parse_command(args)?;
            let profile = store.for_camera(&camera.id).active.clone();
            actions::set(backend, &camera.id, store, &profile, setting)?;
            println!("Sent {setting:?} and saved to '{profile}' (not hardware readback)");
            Ok(())
        }
        "apply" if args.is_empty() => crate::report_apply(actions::apply(
            backend,
            &camera.id,
            store.for_camera(&camera.id).active(),
        )),
        "profile" => profile_cmd(backend, camera, args, store),
        _ => Err(Error::Unsupported(format!(
            "'{command}' is not a camera command; use camera, info, profile, or apply"
        ))),
    }
}

fn profile_cmd(
    backend: &dyn CameraBackend,
    camera: &CameraInfo,
    args: &[String],
    store: &mut Store,
) -> Result<()> {
    let profiles = store.for_camera(&camera.id);
    let action = args.first().map(String::as_str).unwrap_or("list");
    let name = args.get(1).map(String::as_str);
    if action != "apps" && args.len() > 2 {
        return Err(Error::InvalidArg("too many profile arguments".into()));
    }
    match action {
        "list" => {
            for name in profiles.profiles.keys() {
                println!(
                    "{} {name}",
                    if *name == profiles.active { "*" } else { " " }
                );
            }
            Ok(())
        }
        "show" => {
            let name = name.unwrap_or(&profiles.active);
            let profile = profiles
                .profiles
                .get(name)
                .ok_or_else(|| Error::NotFound(format!("profile '{name}'")))?;
            println!(
                "Saved settings (not readback):\n{}",
                serde_json::to_string_pretty(profile).map_err(|e| Error::Db(e.to_string()))?
            );
            Ok(())
        }
        "new" | "delete" | "use" | "apps" => {
            let name =
                name.ok_or_else(|| Error::InvalidArg("profile command requires a name".into()))?;
            match action {
                "new" => profiles.create(name)?,
                "delete" => profiles.delete(name)?,
                "use" => profiles.switch(name)?,
                "apps" => profiles.set_apps(name, &args[2..])?,
                _ => unreachable!(),
            }
            let profile = profiles.active().clone();
            store.save()?;
            if action == "use" {
                crate::report_apply(actions::apply(backend, &camera.id, &profile))?;
            }
            Ok(())
        }
        _ => Err(Error::InvalidArg(format!(
            "unknown profile action '{action}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::camera::{CameraInfo, FieldOfView, Setting};

    #[test]
    fn camera_native_format_readback_keeps_exact_rate_for_manual_restore() {
        let value = CameraFormat {
            pin: 0,
            width: 1920,
            height: 1080,
            pixel_format: "MJPG".into(),
            frame_interval_100ns: Some(1_334_667),
            min_frame_interval_100ns: None,
            max_frame_interval_100ns: None,
            frame_intervals_100ns: vec![],
        };
        let text = handle_format(&value);
        assert!(text.contains("10000000/1334667"));
        assert_eq!(
            FormatRequest::new(0, 1920, 1080, "10000000/1334667", "MJPG")
                .unwrap()
                .frame_interval_100ns,
            1_334_667
        );
    }

    #[test]
    fn camera_native_format_command_parses_read_set_and_pin_without_profile_writes() {
        use secbird_core::camera::formats::{CameraFormat, FormatChange, FormatRequest};
        use std::sync::Mutex;
        struct Native {
            calls: Mutex<Vec<String>>,
        }
        fn mode() -> CameraFormat {
            CameraFormat {
                pin: 0,
                width: 1280,
                height: 720,
                pixel_format: "MJPG".into(),
                frame_interval_100ns: Some(333_333),
                min_frame_interval_100ns: None,
                max_frame_interval_100ns: None,
                frame_intervals_100ns: vec![],
            }
        }
        impl CameraBackend for Native {
            fn discover(&self) -> Result<Vec<CameraInfo>> {
                panic!("unexpected rediscovery")
            }
            fn set(&self, _: &str, _: Setting) -> Result<()> {
                panic!("unexpected vendor-control write")
            }
            fn format(&self, id: &str, pin: u32) -> Result<CameraFormat> {
                self.calls.lock().unwrap().push(format!("get {id} {pin}"));
                let mut value = mode();
                value.pin = pin;
                Ok(value)
            }
            fn set_format(&self, id: &str, request: &FormatRequest) -> Result<FormatChange> {
                self.calls.lock().unwrap().push(format!(
                    "set {id} {} {}x{} {}",
                    request.pin, request.width, request.height, request.pixel_format
                ));
                assert_eq!(request.frame_interval_100ns, 333_333);
                let mut value = mode();
                value.pin = request.pin;
                Ok(FormatChange {
                    previous: value.clone(),
                    current: value,
                })
            }
        }
        let backend = Native {
            calls: Mutex::new(vec![]),
        };
        let camera = CameraInfo {
            id: "camera:test".into(),
            pid: 0x0e05,
            name: "Kiyo Pro".into(),
            control_error: None,
        };
        let mut store = Store::default();
        for args in [
            vec!["format"],
            vec!["format", "--pin", "1"],
            vec!["format", "1280x720", "30", "MJPG"],
            vec!["format", "1280x720", "30", "MJPG", "--pin", "1"],
        ] {
            run(
                &backend,
                &camera,
                "camera",
                &args.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                &mut store,
            )
            .unwrap();
        }
        assert_eq!(
            *backend.calls.lock().unwrap(),
            vec![
                "get camera:test 0",
                "get camera:test 1",
                "set camera:test 0 1280x720 MJPG",
                "set camera:test 1 1280x720 MJPG"
            ]
        );
        assert!(store.devices.is_empty());
        for args in [
            vec!["format", "1280x720"],
            vec!["format", "1280x720", "NaN", "MJPG"],
            vec!["format", "1280x720", "30", "MJPG", "extra"],
            vec!["format", "--pin", "64"],
        ] {
            assert!(run(
                &backend,
                &camera,
                "camera",
                &args.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                &mut store
            )
            .is_err());
        }
        assert_eq!(backend.calls.lock().unwrap().len(), 4);
    }

    #[test]
    fn camera_formats_command_never_writes_controls_or_profiles() {
        struct FormatsOnly;
        impl CameraBackend for FormatsOnly {
            fn discover(&self) -> Result<Vec<CameraInfo>> {
                panic!("command must not rediscover")
            }
            fn set(&self, _: &str, _: Setting) -> Result<()> {
                panic!("formats must not write")
            }
            fn formats(
                &self,
                id: &str,
            ) -> Result<Vec<secbird_core::camera::formats::CameraFormat>> {
                assert_eq!(id, "camera:test");
                Ok(Vec::new())
            }
        }
        let camera = CameraInfo {
            id: "camera:test".into(),
            pid: 0x0e05,
            name: "Kiyo Pro".into(),
            control_error: Some("vendor controls unavailable".into()),
        };
        let mut store = Store::default();
        run(
            &FormatsOnly,
            &camera,
            "camera",
            &["formats".into()],
            &mut store,
        )
        .unwrap();
        assert!(store.devices.is_empty());
        assert!(run(
            &FormatsOnly,
            &camera,
            "camera",
            &["formats".into(), "extra".into()],
            &mut store
        )
        .is_err());
    }

    #[test]
    fn camera_select_never_chooses_first_of_twins() {
        let cameras = vec![
            CameraInfo {
                id: "camera:one".into(),
                pid: 0x0e05,
                name: "Kiyo Pro".into(),
                control_error: None,
            },
            CameraInfo {
                id: "camera:two".into(),
                pid: 0x0e05,
                name: "Kiyo Pro".into(),
                control_error: None,
            },
        ];
        assert!(select(&cameras, None).is_err());
        assert!(select(&cameras, Some("0E05")).is_err());
        assert!(select(&cameras, Some("0099")).is_err());
        assert_eq!(
            select(&cameras, Some("CAMERA:TWO")).unwrap().id,
            "camera:two"
        );
    }

    #[test]
    fn camera_parser_rejects_missing_extra_and_invalid_values() {
        assert_eq!(
            parse_command(&["fov".into(), "narrow".into()]).unwrap(),
            Setting::FieldOfView(FieldOfView::Narrow)
        );
        for args in [
            vec![],
            vec!["hdr".into()],
            vec!["hdr".into(), "yes".into()],
            vec!["hdr".into(), "on".into(), "extra".into()],
        ] {
            assert!(parse_command(&args).is_err());
        }
    }
}
