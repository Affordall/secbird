//! Explicit experimental GetFormat/SetFormat operations on an unconnected pin.
use super::{
    capability_buffer_size, describe, read_capability, system, CameraFormat, IAMStreamConfig,
    IBaseFilter, Interface, MediaType, PINDIR_OUTPUT,
};
use crate::{
    camera::formats::{verify_change, FormatChange, FormatRequest},
    Error, Result,
};

fn config(
    filter: &IBaseFilter,
    wanted: u32,
) -> Result<(windows::Win32::Media::DirectShow::IPin, IAMStreamConfig)> {
    if wanted >= 64 {
        return Err(Error::InvalidArg(
            "camera pin must be between 0 and 63".into(),
        ));
    }
    // SAFETY: caller runs on the camera's COM worker; no interfaces leave it.
    unsafe {
        let pins = filter
            .EnumPins()
            .map_err(|error| system("enumerate format-control pins", error))?;
        for index in 0..=wanted {
            let mut next = [None];
            pins.Next(&mut next, None)
                .ok()
                .map_err(|error| system("read format-control pin", error))?;
            let Some(pin) = next[0].take() else {
                break;
            };
            if index != wanted {
                continue;
            }
            if pin
                .QueryDirection()
                .map_err(|error| system("read pin direction", error))?
                != PINDIR_OUTPUT
            {
                return Err(Error::Unsupported(
                    "selected camera pin is not an output pin".into(),
                ));
            }
            let config = pin
                .cast::<IAMStreamConfig>()
                .map_err(|error| system("query native format interface", error))?;
            return Ok((pin, config));
        }
    }
    Err(Error::NotFound(format!("camera output pin {wanted}")))
}

fn current(config: &IAMStreamConfig, pin: u32) -> Result<(MediaType, CameraFormat)> {
    // SAFETY: GetFormat transfers the media type allocation to this guard.
    let allocation = MediaType(
        unsafe { config.GetFormat() }
            .map_err(|error| system("read opened-handle format", error))?,
    );
    if allocation.0.is_null() {
        return Err(Error::System(
            "camera returned a null opened-handle format".into(),
        ));
    }
    // SAFETY: successful GetFormat promises the owned format allocation, which the
    // guard keeps alive through checked parsing; no pointer escapes the worker.
    let format = unsafe { describe(&*allocation.0, pin) }?
        .ok_or_else(|| Error::Unsupported("selected camera pin has a non-video format".into()))?;
    Ok((allocation, format))
}

pub(in crate::camera) fn read(filter: &IBaseFilter, pin: u32) -> Result<CameraFormat> {
    let (_, config) = config(filter, pin)?;
    current(&config, pin).map(|(_allocation, format)| format)
}

pub(in crate::camera) fn set(
    filter: &IBaseFilter,
    request: &FormatRequest,
) -> Result<FormatChange> {
    request.validate()?;
    let (pin, config) = config(filter, request.pin)?;
    let (original, previous) = current(&config, request.pin)?;
    let mut count = 0;
    let mut bytes = 0;
    // SAFETY: output integers have correct size and remain live during the call.
    unsafe { config.GetNumberOfCapabilities(&mut count, &mut bytes) }
        .map_err(|error| system("read native format capabilities", error))?;
    let buffer_size = capability_buffer_size(count, bytes)?;
    for index in 0..count {
        let Some((candidate, advertised)) =
            read_capability(filter, &pin, &config, request.pin, index, buffer_size)?
        else {
            continue;
        };
        let Some(interval) = request.supported_interval(&advertised)? else {
            continue;
        };
        // SAFETY: read_capability validated a VideoInfo/VideoInfo2 block and owns
        // its writable allocation. Both ABIs store AvgTimePerFrame at byte 40.
        // Dimensions/subtype are left exactly as advertised by the driver.
        unsafe {
            std::ptr::copy_nonoverlapping(
                (interval as i64).to_le_bytes().as_ptr(),
                (*candidate.0).pbFormat.add(40),
                8,
            );
        }
        return verify_change(
            request,
            previous.clone(),
            || {
                unsafe { config.SetFormat(candidate.0) }
                    .map_err(|error| system("SetFormat experimental request", error))
            },
            || current(&config, request.pin).map(|(_allocation, format)| format),
            || {
                // SAFETY: original allocation remains live for the whole operation.
                unsafe { config.SetFormat(original.0) }
                    .map_err(|error| system("restore previous handle format", error))?;
                let (_, restored) = current(&config, request.pin)?;
                if restored != previous {
                    return Err(Error::System(
                        "previous handle format rollback readback differs".into(),
                    ));
                }
                Ok(())
            },
        );
    }
    Err(Error::Unsupported("requested pin/resolution/pixel format/FPS combination is not advertised by this driver; no format changed".into()))
}
