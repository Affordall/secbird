//! Built-in lighting effects the device runs by itself (`0F:02`), as Synapse sends them.
//!
//! Packet: `[store, led, effect, flags, rate, colour count, r, g, b, ...]`. Flags and rate
//! per effect come from Synapse's effect-settings converter (`main.js`, module 76665).

use crate::commands as cmd;
use crate::db::Lighting;
use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::Rgb;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Fades in and out: one or two colours, or random colours when empty.
    Breathing(Vec<Rgb>),
    /// Cycles through all colours.
    Spectrum,
    /// Colours flow across the device; `speed` 1 (slow) to 5 (fast).
    Wave { reverse: bool, speed: u8 },
    /// Lights up when used, then fades; `duration` 1 (short) to 3 (long).
    Reactive { color: Rgb, duration: u8 },
    /// Twinkling: one or two colours, or random when empty; `duration` 1 to 3.
    Starlight { colors: Vec<Rgb>, duration: u8 },
}

// Effect ids (Synapse's `RP` enum).
const BREATHING: u8 = 2;
const SPECTRUM: u8 = 3;
const WAVE: u8 = 4;
const REACTIVE: u8 = 5;
const STARLIGHT: u8 = 7;

/// Wave speed 1-5 to Synapse's rate byte (lower is faster).
const WAVE_RATES: [u8; 5] = [75, 55, 40, 25, 15];

impl Effect {
    /// `(effect id, flags, rate, colours)`.
    fn params(&self) -> (u8, u8, u8, Vec<Rgb>) {
        match self {
            Effect::Breathing(colors) => (BREATHING, colors.len() as u8, 0, colors.clone()),
            Effect::Spectrum => (SPECTRUM, 0, 0, Vec::new()),
            Effect::Wave { reverse, speed } => {
                (WAVE, if *reverse { 2 } else { 1 }, WAVE_RATES[(*speed).clamp(1, 5) as usize - 1], Vec::new())
            }
            Effect::Reactive { color, duration } => (REACTIVE, 0, *duration, vec![*color]),
            Effect::Starlight { colors, duration } => (STARLIGHT, colors.len() as u8, *duration, colors.clone()),
        }
    }

    /// The effect's name, as in its text form.
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::Breathing(_) => "breathing",
            Effect::Spectrum => "spectrum",
            Effect::Wave { .. } => "wave",
            Effect::Reactive { .. } => "reactive",
            Effect::Starlight { .. } => "starlight",
        }
    }

    fn check(&self) -> Result<()> {
        let bad = |what: &str| Err(Error::InvalidArg(what.into()));
        match self {
            Effect::Breathing(c) | Effect::Starlight { colors: c, .. } if c.len() > 2 => bad("at most two colours"),
            Effect::Wave { speed, .. } if !(1..=5).contains(speed) => bad("wave speed is 1-5"),
            Effect::Reactive { duration, .. } | Effect::Starlight { duration, .. } if !(1..=3).contains(duration) => {
                bad("duration is 1 (short) to 3 (long)")
            }
            _ => Ok(()),
        }
    }
}

impl Device {
    /// Run a built-in effect on one lighting zone.
    pub fn set_effect(&self, led: u8, effect: &Effect) -> Result<()> {
        effect.check()?;
        if self.spec.as_ref().is_some_and(|s| s.lighting != Lighting::Extended) {
            return Err(Error::Unsupported(format!("lighting effects on {} (older lighting commands)", self.name())));
        }
        let (id, flags, rate, colors) = effect.params();
        let mut args = vec![self.saved_slot(), led, id, flags, rate, colors.len() as u8];
        args.extend(colors.iter().flat_map(|c| [c.0, c.1, c.2]));
        match self.transact(cmd::MATRIX_EFFECT, args.len() as u8, &args) {
            // Some firmware runs only a few effects itself (the Basilisk V3: spectrum and wave);
            // Synapse animates the rest from the PC.
            Err(Error::Device { status: crate::proto::Status::NotSupported, .. }) => Err(Error::Unsupported(format!(
                "{} can't run {} by itself",
                self.name(),
                effect.kind()
            ))),
            other => other.map(|_| ()),
        }
    }
}

fn colors_text(colors: &[Rgb]) -> String {
    if colors.is_empty() {
        "random".into()
    } else {
        colors.iter().map(Rgb::to_string).collect::<Vec<_>>().join(",")
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Breathing(c) => write!(f, "breathing:{}", colors_text(c)),
            Effect::Spectrum => write!(f, "spectrum"),
            Effect::Wave { reverse, speed } => write!(f, "wave:{}@{speed}", if *reverse { "left" } else { "right" }),
            Effect::Reactive { color, duration } => write!(f, "reactive:{color}@{duration}"),
            Effect::Starlight { colors, duration } => write!(f, "starlight:{}@{duration}", colors_text(colors)),
        }
    }
}

impl FromStr for Effect {
    type Err = Error;

    /// `spectrum`, `breathing:FF0000,0000FF`, `breathing:random`, `wave:right@3`,
    /// `reactive:00FF00@2`, `starlight:random@2`. Speeds and durations are optional.
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim().to_ascii_lowercase();
        let bad = || {
            Error::InvalidArg(format!(
                "unknown effect '{s}' (spectrum, breathing:RRGGBB[,RRGGBB]|random, wave:left|right[@1-5], \
                 reactive:RRGGBB[@1-3], starlight:RRGGBB[,RRGGBB]|random[@1-3])"
            ))
        };
        let (kind, rest) = s.split_once(':').unwrap_or((&s, ""));
        let (arg, level) = match rest.split_once('@') {
            Some((a, n)) => (a, Some(n.parse::<u8>().map_err(|_| bad())?)),
            None => (rest, None),
        };
        let colors = |a: &str| -> Result<Vec<Rgb>> {
            match a {
                "random" | "" => Ok(Vec::new()),
                list => list.split(',').map(|c| c.parse()).collect(),
            }
        };
        let effect = match kind {
            "spectrum" => Effect::Spectrum,
            "breathing" => Effect::Breathing(colors(arg)?),
            "wave" => Effect::Wave {
                reverse: match arg {
                    "left" => true,
                    "right" | "" => false,
                    _ => return Err(bad()),
                },
                speed: level.unwrap_or(3),
            },
            "reactive" => Effect::Reactive { color: arg.parse()?, duration: level.unwrap_or(2) },
            "starlight" => Effect::Starlight { colors: colors(arg)?, duration: level.unwrap_or(2) },
            _ => return Err(bad()),
        };
        effect.check()?;
        Ok(effect)
    }
}

// Profiles store effects as their text, which stays readable in profiles.json.
impl Serialize for Effect {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Effect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_round_trip_through_text() {
        for text in [
            "spectrum",
            "breathing:FF0000",
            "breathing:FF0000,0000FF",
            "breathing:random",
            "wave:right@3",
            "wave:left@5",
            "reactive:00FF00@1",
            "starlight:random@3",
            "starlight:FFFFFF,FF00FF@2",
        ] {
            let e: Effect = text.parse().unwrap();
            assert_eq!(e.to_string(), text);
        }
        assert_eq!("wave".parse::<Effect>().unwrap(), Effect::Wave { reverse: false, speed: 3 });
        for bad in ["sparkle", "wave:up", "wave:left@9", "reactive:00FF00@4", "breathing:FF0000,00FF00,0000FF", "reactive"] {
            assert!(bad.parse::<Effect>().is_err(), "{bad}");
        }
    }

    #[test]
    fn params_match_synapse() {
        let red = Rgb(0xFF, 0, 0);
        assert_eq!(Effect::Breathing(vec![red]).params(), (2, 1, 0, vec![red]));
        assert_eq!(Effect::Breathing(vec![]).params(), (2, 0, 0, vec![]));
        assert_eq!(Effect::Spectrum.params(), (3, 0, 0, vec![]));
        assert_eq!(Effect::Wave { reverse: false, speed: 1 }.params(), (4, 1, 75, vec![]));
        assert_eq!(Effect::Wave { reverse: true, speed: 5 }.params(), (4, 2, 15, vec![]));
        assert_eq!(Effect::Reactive { color: red, duration: 2 }.params(), (5, 0, 2, vec![red]));
        assert_eq!(Effect::Starlight { colors: vec![], duration: 3 }.params(), (7, 0, 3, vec![]));
    }
}
