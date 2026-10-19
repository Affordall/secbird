//! Native advertised capture formats, not the current stream or saved defaults.
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CameraFormat {
    /// Zero-based filter-pin index; only output pins are listed.
    pub pin: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    /// Interval in the enumerated media type, not a live measured frame rate.
    pub frame_interval_100ns: Option<u64>,
    /// Advertised interval bounds; intermediate values are not necessarily supported.
    pub min_frame_interval_100ns: Option<u64>,
    pub max_frame_interval_100ns: Option<u64>,
    /// Explicitly enumerated intervals, when the driver supplies them.
    pub frame_intervals_100ns: Vec<u64>,
}

/// An explicit experimental format request, never part of a saved profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatRequest {
    pub pin: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub frame_interval_100ns: u64,
}

#[derive(Debug, Clone)]
pub struct FormatChange {
    pub previous: CameraFormat,
    pub current: CameraFormat,
}

fn intervals_match(left: u64, right: u64) -> bool {
    left > 0
        && right > 0
        && (left == right
            || (left.abs_diff(right) <= 1
                && (10_000_000.0 / left as f64 - 10_000_000.0 / right as f64).abs() <= 0.001))
}

impl FormatRequest {
    pub fn new(
        pin: u32,
        width: u32,
        height: u32,
        fps: &str,
        pixel_format: &str,
    ) -> crate::Result<Self> {
        let invalid = || {
            crate::Error::InvalidArg("format needs a valid pin, positive dimensions, positive finite FPS, and a FourCC or subtype GUID".into())
        };
        let fps = match fps.split_once('/') {
            Some((numerator, denominator)) => {
                let numerator: f64 = numerator.parse().map_err(|_| invalid())?;
                let denominator: f64 = denominator.parse().map_err(|_| invalid())?;
                if !numerator.is_finite()
                    || !denominator.is_finite()
                    || numerator <= 0.0
                    || denominator <= 0.0
                {
                    return Err(invalid());
                }
                numerator / denominator
            }
            None => fps.parse::<f64>().map_err(|_| invalid())?,
        };
        let interval = (10_000_000.0 / fps).round();
        if !fps.is_finite()
            || fps <= 0.0
            || fps > 10_000_000.0
            || !interval.is_finite()
            || interval < 1.0
            || interval >= i64::MAX as f64
        {
            return Err(invalid());
        }
        let request = Self {
            pin,
            width,
            height,
            pixel_format: pixel_format.to_ascii_uppercase(),
            frame_interval_100ns: interval as u64,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> crate::Result<()> {
        let bytes = self.pixel_format.as_bytes();
        let fourcc = bytes.len() == 4 && bytes.iter().all(u8::is_ascii_graphic);
        let guid = bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    *byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            });
        if self.pin >= 64
            || self.width == 0
            || self.height == 0
            || self.width > i32::MAX as u32
            || self.height > i32::MAX as u32
            || self.frame_interval_100ns == 0
            || self.frame_interval_100ns > i64::MAX as u64
            || (!fourcc && !guid)
        {
            return Err(crate::Error::InvalidArg(
                "invalid native camera format request".into(),
            ));
        }
        Ok(())
    }

    fn same_layout(&self, format: &CameraFormat) -> bool {
        self.pin == format.pin
            && self.width == format.width
            && self.height == format.height
            && self.pixel_format.eq_ignore_ascii_case(&format.pixel_format)
    }

    pub fn matches(&self, format: &CameraFormat) -> bool {
        self.same_layout(format)
            && format
                .frame_interval_100ns
                .is_some_and(|interval| intervals_match(self.frame_interval_100ns, interval))
    }

    /// Choose an explicitly listed interval, or request an in-range interval if the
    /// driver supplies no discrete list. SetFormat may still reject such a request.
    pub fn supported_interval(&self, format: &CameraFormat) -> crate::Result<Option<u64>> {
        self.validate()?;
        if !self.same_layout(format) {
            return Ok(None);
        }
        if !format.frame_intervals_100ns.is_empty() {
            return Ok(format
                .frame_intervals_100ns
                .iter()
                .copied()
                .find(|interval| intervals_match(self.frame_interval_100ns, *interval)));
        }
        if let Some(interval) = format
            .frame_interval_100ns
            .filter(|interval| intervals_match(self.frame_interval_100ns, *interval))
        {
            return Ok(Some(interval));
        }
        match (
            format.min_frame_interval_100ns,
            format.max_frame_interval_100ns,
        ) {
            (Some(min), Some(max))
                if min > 0
                    && min <= self.frame_interval_100ns
                    && self.frame_interval_100ns <= max =>
            {
                Ok(Some(self.frame_interval_100ns))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(any(windows, test))]
pub(super) fn verify_change(
    request: &FormatRequest,
    previous: CameraFormat,
    apply: impl FnOnce() -> crate::Result<()>,
    read: impl FnOnce() -> crate::Result<CameraFormat>,
    restore: impl FnOnce() -> crate::Result<()>,
) -> crate::Result<FormatChange> {
    let result = apply().and_then(|()| read()).and_then(|current| {
        if request.matches(&current) {
            Ok(FormatChange { previous, current })
        } else {
            Err(crate::Error::System(
                "native format readback does not match the request".into(),
            ))
        }
    });
    match result {
        Ok(change) => Ok(change),
        Err(error) => match restore() {
            Ok(()) => Err(crate::Error::System(format!(
                "{error}; previous handle format restored"
            ))),
            Err(rollback) => Err(crate::Error::System(format!(
                "{error}; rollback also failed: {rollback}"
            ))),
        },
    }
}

impl fmt::Display for CameraFormat {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "pin {}  {}x{}  {}",
            self.pin, self.width, self.height, self.pixel_format
        )?;
        if let Some(interval) = self.frame_interval_100ns.filter(|value| *value > 0) {
            write!(out, "  listed FPS {:.3}", 10_000_000.0 / interval as f64)?;
        } else {
            write!(out, "  listed FPS unknown")?;
        }
        if let (Some(min), Some(max)) =
            (self.min_frame_interval_100ns, self.max_frame_interval_100ns)
        {
            if min > 0 && max >= min {
                write!(
                    out,
                    "  advertised range {:.3}..{:.3} FPS",
                    10_000_000.0 / max as f64,
                    10_000_000.0 / min as f64
                )?;
            }
        }
        if !self.frame_intervals_100ns.is_empty() {
            let rates = self
                .frame_intervals_100ns
                .iter()
                .filter(|interval| **interval > 0)
                .map(|interval| format!("{:.3}", 10_000_000.0 / *interval as f64))
                .collect::<Vec<_>>();
            write!(out, "  discrete FPS [{}]", rates.join(", "))?;
        }
        Ok(())
    }
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy)]
pub(super) enum Header {
    VideoInfo,
    VideoInfo2,
}

#[cfg(any(windows, test))]
pub(super) struct Dimensions {
    pub width: u32,
    pub height: u32,
    pub frame_interval: Option<u64>,
}

#[cfg(any(windows, test))]
pub(super) fn capability_buffer_size(count: i32, bytes: i32) -> crate::Result<usize> {
    if !(0..=4096).contains(&count) || !(0..=65_536).contains(&bytes) || (count > 0 && bytes == 0) {
        return Err(crate::Error::System(format!(
            "camera driver returned invalid capability buffer: count={count}, bytes={bytes}"
        )));
    }
    Ok(bytes as usize)
}

#[cfg(any(windows, test))]
pub(super) fn check_video_capabilities(
    is_video: bool,
    count: i32,
    bytes: i32,
    expected: usize,
) -> crate::Result<bool> {
    capability_buffer_size(count, bytes)?;
    if !is_video {
        return Ok(false);
    }
    check_capabilities(count, bytes, expected)?;
    Ok(true)
}

#[cfg(any(windows, test))]
pub(super) fn check_capabilities(count: i32, bytes: i32, expected: usize) -> crate::Result<()> {
    if !(0..=4096).contains(&count) || usize::try_from(bytes).ok() != Some(expected) {
        return Err(crate::Error::System(format!("camera driver returned invalid video capabilities: count={count}, bytes={bytes}, expected={expected}")));
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub(super) fn parse_header(kind: Header, bytes: &[u8]) -> crate::Result<Dimensions> {
    let (bitmap, minimum) = match kind {
        Header::VideoInfo => (48, 88),
        Header::VideoInfo2 => (72, 112),
    };
    let invalid = || {
        crate::Error::System(
            "camera driver returned a truncated or invalid video format header".into(),
        )
    };
    if bytes.len() < minimum {
        return Err(invalid());
    }
    let bitmap_size = u32::from_le_bytes(
        bytes[bitmap..bitmap + 4]
            .try_into()
            .expect("checked header length"),
    ) as usize;
    if bitmap_size < 40 || bitmap_size > bytes.len() - bitmap {
        return Err(invalid());
    }
    let width = i32::from_le_bytes(
        bytes[bitmap + 4..bitmap + 8]
            .try_into()
            .expect("checked header length"),
    );
    let height = i32::from_le_bytes(
        bytes[bitmap + 8..bitmap + 12]
            .try_into()
            .expect("checked header length"),
    );
    let height = height
        .checked_abs()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)?;
    if width <= 0 {
        return Err(invalid());
    }
    let interval = i64::from_le_bytes(bytes[40..48].try_into().expect("checked header length"));
    Ok(Dimensions {
        width: width as u32,
        height: height as u32,
        frame_interval: u64::try_from(interval).ok().filter(|value| *value > 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode() -> CameraFormat {
        CameraFormat {
            pin: 0,
            width: 1920,
            height: 1080,
            pixel_format: "MJPG".into(),
            frame_interval_100ns: Some(166_667),
            min_frame_interval_100ns: Some(166_667),
            max_frame_interval_100ns: Some(2_000_000),
            frame_intervals_100ns: vec![166_667, 333_333, 666_667],
        }
    }

    #[test]
    fn camera_native_format_requests_use_advertised_discrete_intervals() {
        let request = FormatRequest::new(0, 1920, 1080, "30", "mjpg").unwrap();
        assert_eq!(request.supported_interval(&mode()).unwrap(), Some(333_333));
        assert_eq!(
            FormatRequest::new(0, 1920, 1080, "60000/1001", "MJPG")
                .unwrap()
                .frame_interval_100ns,
            166_833
        );
        assert!(FormatRequest::new(0, 1920, 1080, "25", "MJPG")
            .unwrap()
            .supported_interval(&mode())
            .unwrap()
            .is_none());
        assert!(FormatRequest::new(0, 1280, 720, "30", "MJPG")
            .unwrap()
            .supported_interval(&mode())
            .unwrap()
            .is_none());
        assert!(FormatRequest::new(1, 1920, 1080, "30", "MJPG")
            .unwrap()
            .supported_interval(&mode())
            .unwrap()
            .is_none());
        assert!(FormatRequest::new(0, 1920, 1080, "30", "YUY2")
            .unwrap()
            .supported_interval(&mode())
            .unwrap()
            .is_none());
    }

    #[test]
    fn camera_native_format_requests_do_not_invent_rates_when_bounds_are_unknown() {
        let request = FormatRequest::new(0, 1920, 1080, "30", "MJPG").unwrap();
        let mut format = mode();
        format.frame_intervals_100ns.clear();
        assert_eq!(request.supported_interval(&format).unwrap(), Some(333_333));
        format.min_frame_interval_100ns = None;
        format.max_frame_interval_100ns = None;
        assert_eq!(request.supported_interval(&format).unwrap(), None);
        assert_eq!(
            FormatRequest::new(0, 1920, 1080, "60", "MJPG")
                .unwrap()
                .supported_interval(&format)
                .unwrap(),
            Some(166_667)
        );
    }

    #[test]
    fn camera_native_format_rejects_invalid_requests_and_mismatched_readback() {
        for fps in [
            "0",
            "-1",
            "NaN",
            "inf",
            "1/0",
            "0/1",
            "1/NaN",
            "10000001",
            "0.0000000000000000001",
        ] {
            assert!(
                FormatRequest::new(0, 1920, 1080, fps, "MJPG").is_err(),
                "{fps}"
            );
        }
        for (pin, width, height, pixel) in [
            (64, 1920, 1080, "MJPG"),
            (0, 0, 1080, "MJPG"),
            (0, u32::MAX, 1080, "MJPG"),
            (0, 1920, 0, "MJPG"),
            (0, 1920, 1080, ""),
        ] {
            assert!(FormatRequest::new(pin, width, height, "30", pixel).is_err());
        }
        let request = FormatRequest::new(0, 1920, 1080, "60", "MJPG").unwrap();
        assert!(request.matches(&mode()));
        let mut readback = mode();
        readback.frame_interval_100ns = Some(333_333);
        assert!(!request.matches(&readback));
    }

    #[test]
    fn camera_native_format_mismatch_restores_previous_handle_and_reports_failure() {
        use std::cell::RefCell;
        let events = RefCell::new(Vec::new());
        let request = FormatRequest::new(0, 1920, 1080, "30", "MJPG").unwrap();
        let error = verify_change(
            &request,
            mode(),
            || {
                events.borrow_mut().push("set");
                Ok(())
            },
            || {
                events.borrow_mut().push("read");
                Ok(mode())
            },
            || {
                events.borrow_mut().push("restore");
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(*events.borrow(), vec!["set", "read", "restore"]);
        assert!(error.to_string().contains("readback does not match"));
        assert!(error.to_string().contains("restored"));
    }

    #[test]
    fn camera_native_format_apply_and_rollback_failures_are_both_visible() {
        let request = FormatRequest::new(0, 1920, 1080, "30", "MJPG").unwrap();
        let error = verify_change(
            &request,
            mode(),
            || Err(crate::Error::System("SetFormat rejected".into())),
            || panic!("must not read after rejected write"),
            || Err(crate::Error::System("restore disconnected".into())),
        )
        .unwrap_err();
        assert!(error.to_string().contains("SetFormat rejected"));
        assert!(error.to_string().contains("restore disconnected"));
    }

    #[test]
    fn camera_native_format_read_failure_attempts_rollback() {
        use std::cell::Cell;
        let restored = Cell::new(false);
        let request = FormatRequest::new(0, 1920, 1080, "30", "MJPG").unwrap();
        let error = verify_change(
            &request,
            mode(),
            || Ok(()),
            || Err(crate::Error::System("GetFormat failed".into())),
            || {
                restored.set(true);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(restored.get());
        assert!(error.to_string().contains("GetFormat failed"));
    }

    #[test]
    fn camera_native_format_success_returns_before_and_after_without_restoring() {
        let request = FormatRequest::new(0, 1920, 1080, "30", "MJPG").unwrap();
        let mut current = mode();
        current.frame_interval_100ns = Some(333_333);
        let result = verify_change(
            &request,
            mode(),
            || Ok(()),
            || Ok(current),
            || panic!("must not restore successful change"),
        )
        .unwrap();
        assert_eq!(result.previous.frame_interval_100ns, Some(166_667));
        assert_eq!(result.current.frame_interval_100ns, Some(333_333));
    }

    fn video_info() -> [u8; 88] {
        let mut bytes = [0; 88];
        bytes[40..48].copy_from_slice(&166_667i64.to_le_bytes());
        bytes[48..52].copy_from_slice(&40u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&1920i32.to_le_bytes());
        bytes[56..60].copy_from_slice(&1080i32.to_le_bytes());
        bytes
    }

    #[test]
    fn camera_formats_parse_native_dimensions_and_frame_interval() {
        let format = parse_header(Header::VideoInfo, &video_info()).unwrap();
        assert_eq!(
            (format.width, format.height, format.frame_interval),
            (1920, 1080, Some(166_667))
        );
        let mut bytes = [0; 112];
        bytes[40..48].copy_from_slice(&333_667i64.to_le_bytes());
        bytes[72..76].copy_from_slice(&40u32.to_le_bytes());
        bytes[76..80].copy_from_slice(&1280i32.to_le_bytes());
        bytes[80..84].copy_from_slice(&(-720i32).to_le_bytes());
        let format = parse_header(Header::VideoInfo2, &bytes).unwrap();
        assert_eq!(
            (format.width, format.height, format.frame_interval),
            (1280, 720, Some(333_667))
        );
    }

    #[test]
    fn camera_formats_reject_truncated_or_invalid_dimensions() {
        assert!(parse_header(Header::VideoInfo, &[0; 87]).is_err());
        assert!(parse_header(Header::VideoInfo2, &[0; 111]).is_err());
        for (offset, value) in [(48, 0i32), (48, 200), (52, 0), (52, -1920), (56, i32::MIN)] {
            let mut bytes = video_info();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(parse_header(Header::VideoInfo, &bytes).is_err());
        }
        let mut bytes = video_info();
        bytes[40..48].copy_from_slice(&0i64.to_le_bytes());
        assert_eq!(
            parse_header(Header::VideoInfo, &bytes)
                .unwrap()
                .frame_interval,
            None
        );
    }

    #[test]
    fn camera_formats_bound_driver_capability_counts_and_buffer_sizes() {
        assert!(check_capabilities(1, 128, 128).is_ok());
        assert!(check_capabilities(0, 128, 128).is_ok());
        for (count, bytes) in [(-1, 128), (4097, 128), (1, -1), (1, 127), (1, 129)] {
            assert!(check_capabilities(count, bytes, 128).is_err());
        }
    }

    #[test]
    fn camera_formats_skip_audio_layouts_without_losing_video_formats() {
        let layouts = [(false, 52), (true, 128), (false, 64)];
        let mut video_layouts = Vec::new();
        for (is_video, bytes) in layouts {
            assert_eq!(capability_buffer_size(1, bytes).unwrap(), bytes as usize);
            if check_video_capabilities(is_video, 1, bytes, 128).unwrap() {
                video_layouts.push(bytes);
            }
        }
        assert_eq!(video_layouts, vec![128]);
        assert!(check_video_capabilities(true, 1, 52, 128).is_err());
    }

    #[test]
    fn camera_formats_bound_buffers_before_reading_any_media_type() {
        for (count, bytes) in [(1, 0), (1, -1), (1, 65_537), (-1, 128), (4097, 128)] {
            assert!(capability_buffer_size(count, bytes).is_err());
        }
        assert_eq!(capability_buffer_size(0, 0).unwrap(), 0);
    }

    #[test]
    fn camera_formats_label_advertised_ranges_without_inventing_discrete_rates() {
        let format = CameraFormat {
            pin: 0,
            width: 1920,
            height: 1080,
            pixel_format: "MJPG".into(),
            frame_interval_100ns: Some(333_667),
            min_frame_interval_100ns: Some(166_667),
            max_frame_interval_100ns: Some(666_667),
            frame_intervals_100ns: vec![],
        };
        let text = format.to_string();
        assert!(text.contains("1920x1080") && text.contains("MJPG"));
        assert!(text.contains("29.970") && text.contains("15.000") && text.contains("60.000"));
        assert!(text.contains("range") && !text.contains("discrete"));
    }
}
