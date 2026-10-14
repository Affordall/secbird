//! Macros stored on the device, as Synapse's on-board memory engine writes them.
//!
//! A macro is a list of steps (key down/up, mouse buttons, wheel, delays) kept in the
//! device's flash under a 16-bit id, with a 250-byte header holding its name. Buttons play
//! it through an assignment (`macro:<id>` in [`crate::buttons::Action`]), so it works
//! without any software running. Macros belong to the device, not to a profile.

use crate::buttons::{key_name, key_usage, MODIFIERS};
use crate::commands as cmd;
use crate::device::Device;
use crate::error::{Error, Result};
use std::fmt;
use std::str::FromStr;

/// One step of a macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A key goes down or up (USB HID usage; modifiers are 0xE0-0xE7).
    KeyDown(u8),
    KeyUp(u8),
    /// Mouse buttons held from now on: 1 left, 2 right, 4 middle, 8 back, 16 forward.
    Buttons(u8),
    /// One wheel notch, up (`true`) or down.
    Wheel(bool),
    /// Wait this many milliseconds.
    Delay(u32),
    /// Move the cursor to an absolute position.
    Cursor(u16, u16),
}

// Event opcodes (Synapse's `iR` enum).
const KEY_MAKE: u8 = 1;
const KEY_BREAK: u8 = 2;
const BUTTON: u8 = 8;
const SCROLL_WHEEL: u8 = 10;
const DELAY_1: u8 = 17;
const DELAY_2: u8 = 18;
const DELAY_3: u8 = 19;
const DELAY_4: u8 = 20;
const CURSOR: u8 = 21;

/// Steps as stored on the device.
pub fn encode(steps: &[Step]) -> Vec<u8> {
    let mut out = Vec::new();
    for &step in steps {
        match step {
            Step::KeyDown(usage) => out.extend([KEY_MAKE, usage]),
            Step::KeyUp(usage) => out.extend([KEY_BREAK, usage]),
            Step::Buttons(mask) => out.extend([BUTTON, mask]),
            Step::Wheel(up) => out.extend([SCROLL_WHEEL, if up { 1 } else { 0xFF }]),
            Step::Delay(ms) => {
                let b = ms.to_be_bytes();
                match ms {
                    0..=0xFF => out.extend([DELAY_1, b[3]]),
                    0x100..=0xFFFF => out.extend([DELAY_2, b[2], b[3]]),
                    0x1_0000..=0xFF_FFFF => out.extend([DELAY_3, b[1], b[2], b[3]]),
                    _ => out.extend([DELAY_4, b[0], b[1], b[2], b[3]]),
                }
            }
            Step::Cursor(x, y) => {
                let ([xh, xl], [yh, yl]) = (x.to_be_bytes(), y.to_be_bytes());
                out.extend([CURSOR, xh, xl, yh, yl]);
            }
        }
    }
    out
}

/// Steps from device bytes. Stops at the first event it doesn't know (media and power
/// keys, which Synapse doesn't decode either) and says so.
pub fn decode(bytes: &[u8]) -> Result<Vec<Step>> {
    let mut steps = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let need = |n: usize| {
            rest.get(..n).ok_or_else(|| Error::Unsupported(format!("macro data ends inside an event at byte {i}")))
        };
        let (step, len) = match rest[0] {
            KEY_MAKE => (Step::KeyDown(need(2)?[1]), 2),
            KEY_BREAK => (Step::KeyUp(need(2)?[1]), 2),
            BUTTON => (Step::Buttons(need(2)?[1]), 2),
            SCROLL_WHEEL => (Step::Wheel(need(2)?[1] == 1), 2),
            DELAY_1 => (Step::Delay(u32::from(need(2)?[1])), 2),
            DELAY_2 => {
                let b = need(3)?;
                (Step::Delay(u32::from_be_bytes([0, 0, b[1], b[2]])), 3)
            }
            DELAY_3 => {
                let b = need(4)?;
                (Step::Delay(u32::from_be_bytes([0, b[1], b[2], b[3]])), 4)
            }
            DELAY_4 => {
                let b = need(5)?;
                (Step::Delay(u32::from_be_bytes([b[1], b[2], b[3], b[4]])), 5)
            }
            CURSOR => {
                let b = need(5)?;
                (Step::Cursor(u16::from_be_bytes([b[1], b[2]]), u16::from_be_bytes([b[3], b[4]])), 5)
            }
            other => return Err(Error::Unsupported(format!("macro event 0x{other:02x} at byte {i}"))),
        };
        steps.push(step);
        i += len;
    }
    Ok(steps)
}

const BUTTON_BITS: &[(u8, &str)] = &[(1, "left"), (2, "right"), (4, "middle"), (8, "back"), (16, "forward")];

/// Modifier keys as HID usages, by the names button combos use.
fn modifier_usage(name: &str) -> Option<u8> {
    MODIFIERS.iter().position(|m| m.1 == name).map(|i| 0xE0 + i as u8)
}

fn usage_of(name: &str) -> Result<u8> {
    let name = name.trim();
    modifier_usage(name)
        .or_else(|| key_usage(name))
        .or_else(|| name.strip_prefix("0x").and_then(|h| u8::from_str_radix(h, 16).ok()))
        .ok_or_else(|| Error::InvalidArg(format!("unknown key '{name}'")))
}

fn usage_name(usage: u8) -> String {
    match usage {
        0xE0..=0xE7 => MODIFIERS[(usage - 0xE0) as usize].1.to_string(),
        _ => key_name(usage),
    }
}

fn button_bit(name: &str) -> Result<u8> {
    BUTTON_BITS
        .iter()
        .find(|b| b.1 == name)
        .map(|b| b.0)
        .ok_or_else(|| Error::InvalidArg(format!("unknown mouse button '{name}' (left, right, middle, back, forward)")))
}

/// A whole macro as text, one token per step, e.g. `tap:ctrl+c wait:50 click:left`.
///
/// Tokens: `down:<key>`, `up:<key>`, `tap:<key or combo>` (all down, then up in reverse),
/// `press:<button>`, `release:<button>`, `click:<button>`, `wheel:up|down`, `wait:<ms>`,
/// `cursor:<x>,<y>`. Keys use the names of `key:` actions (a, f5, space, ctrl, shift, ...).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script(pub Vec<Step>);

impl FromStr for Script {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut steps = Vec::new();
        let mut held = 0u8;
        for token in s.split_whitespace() {
            let token = token.to_ascii_lowercase();
            let bad = || Error::InvalidArg(format!("unknown macro step '{token}' (e.g. tap:a, wait:50, click:left)"));
            let (kind, arg) = token.split_once(':').ok_or_else(bad)?;
            match kind {
                "down" => steps.push(Step::KeyDown(usage_of(arg)?)),
                "up" => steps.push(Step::KeyUp(usage_of(arg)?)),
                "tap" => {
                    let keys = arg.split('+').map(usage_of).collect::<Result<Vec<_>>>()?;
                    steps.extend(keys.iter().map(|&k| Step::KeyDown(k)));
                    steps.extend(keys.iter().rev().map(|&k| Step::KeyUp(k)));
                }
                "press" => {
                    held |= button_bit(arg)?;
                    steps.push(Step::Buttons(held));
                }
                "release" => {
                    held &= !button_bit(arg)?;
                    steps.push(Step::Buttons(held));
                }
                "click" => {
                    let bit = button_bit(arg)?;
                    steps.push(Step::Buttons(held | bit));
                    steps.push(Step::Buttons(held & !bit));
                }
                "wheel" => match arg {
                    "up" => steps.push(Step::Wheel(true)),
                    "down" => steps.push(Step::Wheel(false)),
                    _ => return Err(bad()),
                },
                "wait" => {
                    let ms: u32 = arg.trim_end_matches("ms").parse().map_err(|_| bad())?;
                    steps.push(Step::Delay(ms));
                }
                "cursor" => {
                    let (x, y) = arg.split_once(',').ok_or_else(bad)?;
                    steps.push(Step::Cursor(x.parse().map_err(|_| bad())?, y.parse().map_err(|_| bad())?));
                }
                _ => return Err(bad()),
            }
        }
        if steps.is_empty() {
            return Err(Error::InvalidArg("a macro needs at least one step".into()));
        }
        Ok(Script(steps))
    }
}

impl fmt::Display for Script {
    /// The most compact text that parses back to the same steps: a key that goes down and
    /// straight back up is `tap:`, a button pressed and released is `click:`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.0;
        let mut out: Vec<String> = Vec::new();
        let mut held = 0u8;
        let mut i = 0;
        while i < s.len() {
            match s[i] {
                Step::KeyDown(k) if s.get(i + 1) == Some(&Step::KeyUp(k)) => {
                    out.push(format!("tap:{}", usage_name(k)));
                    i += 1;
                }
                Step::KeyDown(k) => out.push(format!("down:{}", usage_name(k))),
                Step::KeyUp(k) => out.push(format!("up:{}", usage_name(k))),
                Step::Buttons(mask) => {
                    let pressed = mask & !held;
                    let released = held & !mask;
                    let single = |bits: u8| BUTTON_BITS.iter().find(|b| b.0 == bits).map(|b| b.1);
                    match (single(pressed), released, s.get(i + 1)) {
                        (Some(name), 0, Some(&Step::Buttons(next))) if next == held => {
                            out.push(format!("click:{name}"));
                            i += 1;
                        }
                        _ => {
                            for b in BUTTON_BITS {
                                if pressed & b.0 != 0 {
                                    out.push(format!("press:{}", b.1));
                                }
                                if released & b.0 != 0 {
                                    out.push(format!("release:{}", b.1));
                                }
                            }
                            held = mask;
                        }
                    }
                }
                Step::Wheel(up) => out.push(format!("wheel:{}", if up { "up" } else { "down" })),
                Step::Delay(ms) => out.push(format!("wait:{ms}")),
                Step::Cursor(x, y) => out.push(format!("cursor:{x},{y}")),
            }
            i += 1;
        }
        f.write_str(&out.join(" "))
    }
}

/// The device's macro memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Storage {
    /// Highest macro id the device accepts.
    pub max_id: u16,
    pub max_bytes: u32,
    pub free_bytes: u32,
    /// Freed by deletions, reusable after the device compacts its memory.
    pub recyclable_bytes: u32,
}

/// A macro on the device, without its steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroInfo {
    pub id: u16,
    pub name: String,
    /// Size of its steps in bytes.
    pub size: u32,
}

/// Memory one macro takes: its steps plus the 256-byte header, in 256-byte blocks.
pub fn footprint(size: u32) -> u32 {
    (size + 256).div_ceil(256) * 256
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// 250-byte header: guid, step count (little-endian), UTF-8 name, then reserved space
/// (Synapse puts a timestamp and hash there; empty is fine).
fn header(name: &str, steps: usize) -> Vec<u8> {
    let mut h = vec![0u8; cmd::MACRO_HEADER_LEN];
    h[..16].copy_from_slice(&guid());
    h[16..20].copy_from_slice(&(steps as u32).to_le_bytes());
    let mut end = name.len().min(99);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    h[20..20 + end].copy_from_slice(&name.as_bytes()[..end]);
    h
}

/// A random-enough id for the header, so Synapse can tell macros apart.
fn guid() -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher};
    let mut out = [0u8; 16];
    for (i, half) in out.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        half.copy_from_slice(&h.finish().to_le_bytes());
    }
    out
}

impl Device {
    pub fn macro_storage(&self) -> Result<Storage> {
        let r = self.get(cmd::MACRO_STORAGE, cmd::MACRO_STORAGE_SIZE, &[])?;
        let a = r.args();
        Ok(Storage {
            max_id: u16::from_be_bytes([a[0], a[1]]),
            max_bytes: be32(&a[2..6]),
            free_bytes: be32(&a[6..10]),
            recyclable_bytes: be32(&a[10..14]),
        })
    }

    /// Ids of every macro on the device.
    pub fn macro_ids(&self) -> Result<Vec<u16>> {
        let r = self.get(cmd::MACRO_COUNT, cmd::MACRO_COUNT_SIZE, &[])?;
        let count = u16::from_be_bytes([r.args()[0], r.args()[1]]) as usize;
        let mut ids = Vec::with_capacity(count);
        while ids.len() < count {
            let n = (count - ids.len()).min(32);
            let [hi, lo] = (ids.len() as u16).to_be_bytes();
            let r = self.get(cmd::MACRO_IDS, (2 * n + 4) as u8, &[hi, lo])?;
            ids.extend(r.args()[4..4 + 2 * n].as_chunks::<2>().0.iter().map(|&p| u16::from_be_bytes(p)));
        }
        Ok(ids)
    }

    /// Size of a macro's steps in bytes.
    fn macro_size(&self, id: u16) -> Result<u32> {
        let r = self.get(cmd::MACRO_ALLOCATE, cmd::MACRO_ALLOCATE_SIZE, &id.to_be_bytes())?;
        Ok(be32(&r.args()[2..6]))
    }

    fn macro_header(&self, id: u16) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(cmd::MACRO_HEADER_LEN);
        while out.len() < cmd::MACRO_HEADER_LEN {
            let n = (cmd::MACRO_HEADER_LEN - out.len()).min(cmd::MACRO_NAME_CHUNK);
            let ([ih, il], [oh, ol]) = (id.to_be_bytes(), (out.len() as u16).to_be_bytes());
            let r = self.get(cmd::MACRO_NAME, (n + 6) as u8, &[ih, il, oh, ol])?;
            out.extend_from_slice(&r.args()[6..6 + n]);
        }
        Ok(out)
    }

    pub fn macro_info(&self, id: u16) -> Result<MacroInfo> {
        let h = self.macro_header(id)?;
        let name = &h[20..120];
        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        Ok(MacroInfo { id, name: String::from_utf8_lossy(&name[..end]).into_owned(), size: self.macro_size(id)? })
    }

    /// Every macro on the device with its name.
    pub fn macros(&self) -> Result<Vec<MacroInfo>> {
        self.macro_ids()?.into_iter().map(|id| self.macro_info(id)).collect()
    }

    /// The raw bytes of a macro's steps.
    fn macro_bytes(&self, id: u16) -> Result<Vec<u8>> {
        let size = self.macro_size(id)? as usize;
        let mut out = Vec::with_capacity(size);
        while out.len() < size {
            let n = (size - out.len()).min(cmd::MACRO_DATA_CHUNK);
            let [ih, il] = id.to_be_bytes();
            let [o0, o1, o2, o3] = (out.len() as u32).to_be_bytes();
            let r = self.get(cmd::MACRO_DATA, (n + 7) as u8, &[ih, il, o0, o1, o2, o3, n as u8])?;
            let got = (r.args()[6] as usize).min(n);
            if got == 0 {
                return Err(Error::Unsupported(format!("macro {id} stopped answering at byte {}", out.len())));
            }
            out.extend_from_slice(&r.args()[7..7 + got]);
        }
        Ok(out)
    }

    pub fn macro_steps(&self, id: u16) -> Result<Vec<Step>> {
        decode(&self.macro_bytes(id)?)
    }

    /// Store a new macro and return its id. Checks there's room first, writes the header
    /// and the steps, then reads the steps back.
    pub fn create_macro(&self, name: &str, steps: &[Step]) -> Result<u16> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::InvalidArg("a macro needs a name".into()));
        }
        let data = encode(steps);
        if data.is_empty() {
            return Err(Error::InvalidArg("a macro needs at least one step".into()));
        }
        let storage = self.macro_storage()?;
        let need = footprint(data.len() as u32);
        if need > storage.free_bytes {
            return Err(Error::InvalidArg(format!(
                "not enough macro memory: this macro needs {need} bytes, {} are free",
                storage.free_bytes
            )));
        }
        let taken = self.macro_ids()?;
        let id = (1..=storage.max_id.max(1))
            .find(|i| !taken.contains(i))
            .ok_or_else(|| Error::InvalidArg(format!("the device holds at most {} macros", storage.max_id)))?;

        let [ih, il] = id.to_be_bytes();
        let [s0, s1, s2, s3] = (data.len() as u32).to_be_bytes();
        self.transact_slow(cmd::MACRO_ALLOCATE, cmd::MACRO_ALLOCATE_SIZE, &[ih, il, s0, s1, s2, s3])?;
        let written = (|| -> Result<()> {
            let head = header(name, steps.len());
            let [th, tl] = (cmd::MACRO_HEADER_LEN as u16).to_be_bytes();
            for (i, chunk) in head.chunks(cmd::MACRO_NAME_CHUNK).enumerate() {
                let [oh, ol] = ((i * cmd::MACRO_NAME_CHUNK) as u16).to_be_bytes();
                let mut args = vec![ih, il, oh, ol, th, tl];
                args.extend_from_slice(chunk);
                self.transact_slow(cmd::MACRO_NAME, args.len() as u8, &args)?;
            }
            for (i, chunk) in data.chunks(cmd::MACRO_DATA_CHUNK).enumerate() {
                let [o0, o1, o2, o3] = ((i * cmd::MACRO_DATA_CHUNK) as u32).to_be_bytes();
                let mut args = vec![ih, il, o0, o1, o2, o3, chunk.len() as u8];
                args.extend_from_slice(chunk);
                self.transact_slow(cmd::MACRO_DATA, args.len() as u8, &args)?;
            }
            if self.macro_bytes(id)? != data {
                return Err(Error::Unsupported("the macro read back differently from what was written".into()));
            }
            Ok(())
        })();
        if let Err(e) = written {
            // Don't leave a half-written macro behind.
            let _ = self.delete_macro(id);
            return Err(e);
        }
        Ok(id)
    }

    pub fn delete_macro(&self, id: u16) -> Result<()> {
        self.transact_slow(cmd::MACRO_DELETE, cmd::MACRO_DELETE_SIZE, &id.to_be_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_round_trip_through_bytes() {
        let steps = vec![
            Step::KeyDown(0xE0),
            Step::KeyDown(0x06),
            Step::KeyUp(0x06),
            Step::KeyUp(0xE0),
            Step::Delay(50),
            Step::Delay(300),
            Step::Delay(70_000),
            Step::Delay(20_000_000),
            Step::Buttons(1),
            Step::Buttons(0),
            Step::Wheel(true),
            Step::Wheel(false),
            Step::Cursor(1920, 1080),
        ];
        let bytes = encode(&steps);
        assert_eq!(&bytes[..10], [1, 0xE0, 1, 6, 2, 6, 2, 0xE0, 17, 50]);
        assert_eq!(&bytes[10..13], [18, 0x01, 0x2C], "300 ms as two bytes");
        assert_eq!(&bytes[13..17], [19, 0x01, 0x11, 0x70], "70000 ms as three bytes");
        assert_eq!(decode(&bytes).unwrap(), steps);
        assert!(decode(&[1]).is_err(), "cut-off event");
        assert!(decode(&[5, 0xE9]).is_err(), "media events aren't decoded");
    }

    #[test]
    fn scripts_parse_and_print() {
        let s: Script = "tap:ctrl+c wait:50 click:left press:right wheel:down release:right down:shift up:shift"
            .parse()
            .unwrap();
        assert_eq!(
            s.0,
            [
                Step::KeyDown(0xE0),
                Step::KeyDown(0x06),
                Step::KeyUp(0x06),
                Step::KeyUp(0xE0),
                Step::Delay(50),
                Step::Buttons(1),
                Step::Buttons(0),
                Step::Buttons(2),
                Step::Wheel(false),
                Step::Buttons(0),
                Step::KeyDown(0xE1),
                Step::KeyUp(0xE1),
            ]
        );
        // Printing is compact but parses back to the same steps.
        let text = s.to_string();
        assert_eq!(text, "down:ctrl tap:c up:ctrl wait:50 click:left press:right wheel:down release:right tap:shift");
        assert_eq!(text.parse::<Script>().unwrap(), s);
        for bad in ["", "tap:nope", "click:thumb", "wait:soon", "hello"] {
            assert!(bad.parse::<Script>().is_err(), "{bad}");
        }
    }

    #[test]
    fn header_holds_name_and_count() {
        let h = header("Build ✓ order", 12);
        assert_eq!(h.len(), 250);
        assert_eq!(&h[16..20], [12, 0, 0, 0]);
        assert_eq!(&h[20..20 + "Build ✓ order".len()], "Build ✓ order".as_bytes());
        let long = header(&"é".repeat(80), 1);
        assert!(std::str::from_utf8(long[20..120].split(|&b| b == 0).next().unwrap()).is_ok(), "never cuts a character");
        assert_eq!(footprint(10), 512);
        assert_eq!(footprint(256), 512);
        assert_eq!(footprint(257), 768);
    }
}
