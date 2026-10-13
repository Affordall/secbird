//! The byte pipe a [`Device`](crate::Device) talks through: real USB HID, or a simulator.

use crate::error::Result;
use std::time::Duration;

pub trait Transport: Send {
    /// Write one feature report (byte 0 is the HID report id).
    fn send_feature_report(&self, data: &[u8]) -> Result<()>;
    /// Read one feature report into `buf`; returns the number of bytes read.
    fn get_feature_report(&self, buf: &mut [u8]) -> Result<usize>;
    /// Wait between write and each read. Synapse uses 5 ms; simulators need none.
    fn read_delay(&self) -> Duration {
        Duration::from_millis(5)
    }
}

impl Transport for hidapi::HidDevice {
    fn send_feature_report(&self, data: &[u8]) -> Result<()> {
        Ok(hidapi::HidDevice::send_feature_report(self, data)?)
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(hidapi::HidDevice::get_feature_report(self, buf)?)
    }
}
