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
