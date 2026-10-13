//! Razer Protocol 2.5: every request and reply is one 90-byte HID feature report.
//!
//! | byte  | field                                           |
//! |-------|-------------------------------------------------|
//! | 0     | status (reply only)                             |
//! | 1     | transaction id                                  |
//! | 2..5  | remaining packets / protocol type (always 0)    |
//! | 5     | argument size                                   |
//! | 6     | command class                                   |
//! | 7     | command id (bit 7 set = GET variant)            |
//! | 8..88 | arguments (max 80 bytes)                        |
//! | 88    | checksum: XOR of bytes 2..88                    |
//! | 89    | reserved                                        |

pub const REPORT_LEN: usize = 90;
pub const MAX_ARGS: usize = 80;

/// A command is identified by its class and id. `id | 0x80` is the GET variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub class: u8,
    pub id: u8,
}

impl Command {
    pub const fn new(class: u8, id: u8) -> Self {
        Self { class, id }
    }

    pub const fn get(self) -> Self {
        Self { class: self.class, id: self.id | 0x80 }
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}:{:02X}", self.class, self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    New,
    Busy,
    Success,
    Failed,
    Timeout,
    NotSupported,
    Unknown(u8),
}

impl From<u8> for Status {
    fn from(b: u8) -> Self {
        match b {
            0 => Status::New,
            1 => Status::Busy,
            2 => Status::Success,
            3 => Status::Failed,
            4 => Status::Timeout,
            5 => Status::NotSupported,
            other => Status::Unknown(other),
        }
    }
}

#[derive(Clone)]
pub struct Report(pub [u8; REPORT_LEN]);

impl Report {
    /// Build a request. `size` is the argument length the device expects, which can be
    /// larger than `args` (the rest is zero-filled).
    pub fn request(txid: u8, cmd: Command, size: u8, args: &[u8]) -> Self {
        let mut b = [0u8; REPORT_LEN];
        b[1] = txid;
        b[5] = size;
        b[6] = cmd.class;
        b[7] = cmd.id;
        let n = args.len().min(MAX_ARGS);
        b[8..8 + n].copy_from_slice(&args[..n]);
        b[88] = checksum(&b);
        Report(b)
    }

    pub fn status(&self) -> Status {
        Status::from(self.0[0])
    }

    pub fn txid(&self) -> u8 {
        self.0[1]
    }

    pub fn command(&self) -> Command {
        Command::new(self.0[6], self.0[7])
    }

    pub fn args(&self) -> &[u8] {
        &self.0[8..8 + MAX_ARGS]
    }
}

impl std::fmt::Debug for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let used = 8 + (self.0[5] as usize).min(MAX_ARGS);
        write!(f, "Report[{:?} tx={:02X} cmd={}", self.status(), self.txid(), self.command())?;
        for b in &self.0[8..used] {
            write!(f, " {b:02X}")?;
        }
        write!(f, "]")
    }
}

pub fn checksum(b: &[u8; REPORT_LEN]) -> u8 {
    b[2..88].iter().fold(0, |acc, x| acc ^ x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_serial_packet_layout() {
        let r = Report::request(0x1F, Command::new(0x00, 0x02).get(), 0x16, &[]);
        assert_eq!(&r.0[..8], &[0x00, 0x1F, 0, 0, 0, 0x16, 0x00, 0x82]);
        // XOR of size, class and id: 0x16 ^ 0x00 ^ 0x82
        assert_eq!(r.0[88], 0x94);
    }

    #[test]
    fn args_are_copied_and_checksummed() {
        let r = Report::request(0x1F, Command::new(0x04, 0x05), 7, &[0x01, 0x03, 0xE8, 0x03, 0xE8]);
        assert_eq!(&r.0[8..13], &[0x01, 0x03, 0xE8, 0x03, 0xE8]);
        assert_eq!(r.0[88], checksum(&r.0));
    }
}
