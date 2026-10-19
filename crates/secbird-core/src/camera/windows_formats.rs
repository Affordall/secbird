//! DirectShow capability enumeration and explicitly requested handle-format tests.
//! No operation connects or runs a graph. SetFormat is confined to control::set.
use super::{
    camera_id,
    formats::{
        capability_buffer_size, check_video_capabilities, parse_header, CameraFormat, Header,
    },
    is_kiyo_pro_path,
    windows::{property, system, DEVICE_ENUM, VIDEO_CATEGORY},
};
use crate::{Error, Result};
use std::{
    mem::{size_of, ManuallyDrop},
    ptr,
};
use windows::{
    core::{w, Interface, GUID},
    Win32::{
        Foundation::SIZE,
        Media::{
            DirectShow::{
                IAMStreamConfig, IAMVideoControl, IBaseFilter, ICreateDevEnum, IPin, PINDIR_OUTPUT,
                VIDEO_STREAM_CONFIG_CAPS,
            },
            MediaFoundation::{
                FORMAT_VideoInfo, FORMAT_VideoInfo2, MEDIATYPE_Video, AM_MEDIA_TYPE,
            },
        },
        System::Com::{
            CoCreateInstance, CoTaskMemFree, StructuredStorage::IPropertyBag, CLSCTX_INPROC_SERVER,
        },
    },
};

pub(super) mod control;

pub(super) fn for_camera(id: &str) -> Result<Vec<CameraFormat>> {
    with_camera(id, enumerate)
}

pub(super) fn with_camera<T>(
    id: &str,
    operation: impl FnOnce(&IBaseFilter) -> Result<T>,
) -> Result<T> {
    // SAFETY: invoked exclusively by the already COM-initialized camera worker.
    unsafe {
        let devices: ICreateDevEnum = CoCreateInstance(&DEVICE_ENUM, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| system("enumerate camera formats", error))?;
        let mut monikers = None;
        devices
            .CreateClassEnumerator(&VIDEO_CATEGORY, &mut monikers, 0)
            .map_err(|error| system("enumerate video inputs", error))?;
        if let Some(monikers) = monikers {
            loop {
                let mut next = [None];
                monikers
                    .Next(&mut next, None)
                    .ok()
                    .map_err(|error| system("read video input", error))?;
                let Some(moniker) = next[0].take() else {
                    break;
                };
                let Ok(bag) = moniker.BindToStorage::<_, _, IPropertyBag>(None, None) else {
                    continue;
                };
                let Ok(path) = property(&bag, w!("DevicePath")) else {
                    continue;
                };
                if !is_kiyo_pro_path(&path) || camera_id(&path) != id {
                    continue;
                }
                let filter: IBaseFilter = moniker
                    .BindToObject(None, None)
                    .map_err(|error| system("open camera for formats", error))?;
                return operation(&filter);
            }
        }
    }
    Err(Error::NotFound(
        "selected camera disconnected before reading formats".into(),
    ))
}

struct MediaType(*mut AM_MEDIA_TYPE);
impl Drop for MediaType {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: GetStreamCaps transfers ownership of these COM-task allocations.
        unsafe {
            let media = &mut *self.0;
            if !media.pbFormat.is_null() {
                CoTaskMemFree(Some(media.pbFormat.cast()));
            }
            ManuallyDrop::drop(&mut media.pUnk);
            CoTaskMemFree(Some(self.0.cast()));
        }
    }
}

struct Rates(*mut i64);
impl Drop for Rates {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: GetFrameRateList transfers this COM-task allocation.
            unsafe {
                CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
}

fn pixel_format(subtype: GUID) -> String {
    if subtype.data2 == 0
        && subtype.data3 == 0x0010
        && subtype.data4 == [0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]
    {
        let fourcc = subtype.data1.to_le_bytes();
        if fourcc.iter().all(u8::is_ascii_graphic) {
            return String::from_utf8_lossy(&fourcc).into_owned();
        }
    }
    format!("{subtype:?}")
}

/// Caller guarantees the media type and its format allocation came from a
/// successful COM call and remain alive for the duration of this function.
unsafe fn describe(media: &AM_MEDIA_TYPE, pin: u32) -> Result<Option<CameraFormat>> {
    if media.majortype != MEDIATYPE_Video {
        return Ok(None);
    }
    let header = if media.formattype == FORMAT_VideoInfo {
        Header::VideoInfo
    } else if media.formattype == FORMAT_VideoInfo2 {
        Header::VideoInfo2
    } else {
        return Err(Error::Unsupported(format!(
            "camera returned an unsupported native video header {:?}",
            media.formattype
        )));
    };
    if media.pbFormat.is_null() || media.cbFormat > 1024 * 1024 {
        return Err(Error::System(
            "camera returned a missing or oversized media format".into(),
        ));
    }
    // SAFETY: pointer lifetime/allocation is the caller's contract; size is bounded,
    // and parse_header checks minimum lengths and dimensions without aligned casts.
    let dimensions = parse_header(header, unsafe {
        std::slice::from_raw_parts(media.pbFormat, media.cbFormat as usize)
    })?;
    Ok(Some(CameraFormat {
        pin,
        width: dimensions.width,
        height: dimensions.height,
        pixel_format: pixel_format(media.subtype),
        frame_interval_100ns: dimensions.frame_interval,
        min_frame_interval_100ns: None,
        max_frame_interval_100ns: None,
        frame_intervals_100ns: Vec::new(),
    }))
}

fn read_capability(
    filter: &IBaseFilter,
    pin: &IPin,
    config: &IAMStreamConfig,
    pin_index: u32,
    index: i32,
    buffer_size: usize,
) -> Result<Option<(MediaType, CameraFormat)>> {
    let mut buffer = vec![0u8; buffer_size];
    let mut raw = ptr::null_mut();
    // SAFETY: buffer has the exact size requested by the bounded capabilities query.
    let result = unsafe { config.GetStreamCaps(index, &mut raw, buffer.as_mut_ptr()) };
    let allocation = MediaType(raw);
    result.map_err(|error| system("read native camera format", error))?;
    if allocation.0.is_null() {
        return Err(Error::System("camera returned a null media type".into()));
    }
    // SAFETY: successful GetStreamCaps provides the media type and its allocation.
    let media = unsafe { &*allocation.0 };
    if !check_video_capabilities(
        media.majortype == MEDIATYPE_Video,
        1,
        buffer_size as i32,
        size_of::<VIDEO_STREAM_CONFIG_CAPS>(),
    )? {
        return Ok(None);
    }
    // SAFETY: allocation guard outlives parsing, and layout was validated above.
    let mut format =
        unsafe { describe(media, pin_index) }?.expect("video media type checked above");
    // SAFETY: the buffer has the checked ABI size, but may be unaligned.
    let caps = unsafe { ptr::read_unaligned(buffer.as_ptr().cast::<VIDEO_STREAM_CONFIG_CAPS>()) };
    if caps.MinFrameInterval > 0 && caps.MaxFrameInterval >= caps.MinFrameInterval {
        format.min_frame_interval_100ns = Some(caps.MinFrameInterval as u64);
        format.max_frame_interval_100ns = Some(caps.MaxFrameInterval as u64);
    }
    format.frame_intervals_100ns = frame_rates(filter, pin, index, format.width, format.height)?;
    Ok(Some((allocation, format)))
}

fn frame_rates(
    filter: &IBaseFilter,
    pin: &IPin,
    index: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u64>> {
    let Ok(control) = filter.cast::<IAMVideoControl>() else {
        return Ok(Vec::new());
    };
    let mut count = 0;
    let mut raw = ptr::null_mut();
    // SAFETY: driver allocates the returned list; guard owns it even on error.
    let result = unsafe {
        control.GetFrameRateList(
            pin,
            index,
            SIZE {
                cx: width as i32,
                cy: height as i32,
            },
            &mut count,
            &mut raw,
        )
    };
    let allocation = Rates(raw);
    if result.is_err() {
        return Ok(Vec::new());
    } // Optional driver interface.
    if !(0..=4096).contains(&count) || (count > 0 && allocation.0.is_null()) {
        return Err(Error::System(
            "camera driver returned an invalid frame-rate list".into(),
        ));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: successful COM call promises count entries; count and pointer checked.
    let intervals = unsafe { std::slice::from_raw_parts(allocation.0, count as usize) };
    if intervals.iter().any(|value| *value <= 0) {
        return Err(Error::System(
            "camera driver returned a non-positive frame interval".into(),
        ));
    }
    let mut intervals: Vec<_> = intervals.iter().map(|value| *value as u64).collect();
    intervals.sort_unstable();
    intervals.dedup();
    Ok(intervals)
}

pub(super) fn enumerate(filter: &IBaseFilter) -> Result<Vec<CameraFormat>> {
    let mut formats = Vec::new();
    // SAFETY: all interfaces belong to the current initialized worker. Every media
    // type is guarded and all format bytes are length-checked before parsing.
    unsafe {
        let pins = filter
            .EnumPins()
            .map_err(|error| system("enumerate camera pins", error))?;
        let mut pin_index = 0;
        loop {
            let mut next = [None];
            pins.Next(&mut next, None)
                .ok()
                .map_err(|error| system("read camera pin", error))?;
            let Some(pin) = next[0].take() else {
                break;
            };
            if pin_index >= 64 {
                return Err(Error::System("camera returned too many pins".into()));
            }
            let output_index = pin_index;
            pin_index += 1;
            if pin
                .QueryDirection()
                .map_err(|error| system("read pin direction", error))?
                != PINDIR_OUTPUT
            {
                continue;
            }
            let Ok(config) = pin.cast::<IAMStreamConfig>() else {
                continue;
            };
            let mut count = 0;
            let mut bytes = 0;
            config
                .GetNumberOfCapabilities(&mut count, &mut bytes)
                .map_err(|error| system("read camera capabilities", error))?;
            let buffer_size = capability_buffer_size(count, bytes)?;
            for index in 0..count {
                if let Some((_allocation, format)) =
                    read_capability(filter, &pin, &config, output_index, index, buffer_size)?
                {
                    formats.push(format);
                }
            }
        }
    }
    formats.sort_by(|a, b| {
        (
            a.pin,
            a.width,
            a.height,
            &a.pixel_format,
            a.frame_interval_100ns,
        )
            .cmp(&(
                b.pin,
                b.width,
                b.height,
                &b.pixel_format,
                b.frame_interval_100ns,
            ))
    });
    formats.dedup();
    Ok(formats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::MediaFoundation::{VIDEOINFOHEADER, VIDEOINFOHEADER2};

    #[test]
    fn camera_formats_header_offsets_match_windows_abi() {
        assert_eq!(std::mem::size_of::<VIDEOINFOHEADER>(), 88);
        assert_eq!(std::mem::offset_of!(VIDEOINFOHEADER, AvgTimePerFrame), 40);
        assert_eq!(std::mem::offset_of!(VIDEOINFOHEADER, bmiHeader), 48);
        assert_eq!(std::mem::size_of::<VIDEOINFOHEADER2>(), 112);
        assert_eq!(std::mem::offset_of!(VIDEOINFOHEADER2, AvgTimePerFrame), 40);
        assert_eq!(std::mem::offset_of!(VIDEOINFOHEADER2, bmiHeader), 72);
    }

    #[test]
    fn camera_formats_pixel_subtypes_use_fourcc_or_full_guid() {
        assert_eq!(
            pixel_format(GUID::from_u128(0x47504a4d_0000_0010_8000_00aa00389b71)),
            "MJPG"
        );
        assert_eq!(
            pixel_format(GUID::from_u128(0x32595559_0000_0010_8000_00aa00389b71)),
            "YUY2"
        );
        assert_eq!(
            pixel_format(GUID::from_u128(0x47504a4d_1111_0010_8000_00aa00389b71)),
            "47504A4D-1111-0010-8000-00AA00389B71"
        );
    }
}
