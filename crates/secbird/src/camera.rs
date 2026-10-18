use secbird_core::{
    camera::{actions, parse_setting, CameraBackend, CameraInfo, Setting},
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

pub(crate) fn run(
    backend: &dyn CameraBackend,
    camera: &CameraInfo,
    command: &str,
    args: &[String],
    store: &mut Store,
) -> Result<()> {
    match command {
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
