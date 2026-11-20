//! Register-block decoding: typed views over raw Modbus words.
//!
//! Values are decoded from contiguous blocks read by
//! [`crate::Ruiden::fetch_info`] (registers 0-20) and [`crate::Ruiden::fetch_status`]
//! (registers 32-41), following the layout of the official register map.

use std::fmt::{self, Display, Formatter};

use anyhow::{bail, Error as AnyhowError, Result};

use crate::{model::Model, register::Register, Word, WordPair};

/// Combines a `[high, low]` word pair into a 32-bit value.
///
/// This is a plain big-endian double word: e.g. `[0x0001, 0x0000]` becomes
/// `65_536`. Do **not** use for the sign/value temperature pairs, see
/// [`temperature`] instead.
#[must_use]
pub fn double_word([high, low]: WordPair) -> u32 {
    (u32::from(high) << 16) | u32::from(low)
}

/// Decodes a `[sign, value]` word pair into a signed temperature.
///
/// The sign register uses `0` for positive and `1` for negative; the value
/// register holds the unsigned magnitude in degrees.
#[must_use]
pub const fn temperature([sign, value]: WordPair) -> i16 {
    // Device temperature magnitudes always fit in an i16.
    let degrees = value.cast_signed();
    if sign == 0 {
        degrees
    } else {
        -degrees
    }
}

/// Formats a serial number the way the device's own display does.
#[must_use]
pub fn serial_number(high_low: u32) -> String {
    format!("{high_low:08}")
}

/// Converts a milli-unit counter (mAh/mWh words) to a float, e.g. `65_536 -> 65.536`.
///
/// Note: `f32` loses integer precision above 2^24 (~16,777 Ah in 1 mAh steps).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn per_mille(value: u32) -> f32 {
    value as f32 / 1_000.0
}

/// Regulation mode of the output ([`Register::CV_CC`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CvCc {
    /// Constant voltage
    #[default]
    Cv,
    /// Constant current
    Cc,
}

impl From<Word> for CvCc {
    fn from(word: Word) -> Self {
        match word {
            1 => Self::Cc,
            _ => Self::Cv,
        }
    }
}

impl Display for CvCc {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cv => "CV",
            Self::Cc => "CC",
        })
    }
}

/// Output protection that tripped ([`Register::OVP_OCP`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Protection {
    /// No protection has tripped
    #[default]
    None,
    /// Over-voltage protection
    Ovp,
    /// Over-current protection
    Ocp,
}

impl From<Word> for Protection {
    fn from(word: Word) -> Self {
        match word {
            1 => Self::Ovp,
            2 => Self::Ocp,
            _ => Self::None,
        }
    }
}

impl Display for Protection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "NONE",
            Self::Ovp => "OVP",
            Self::Ocp => "OCP",
        })
    }
}

/// Real-time clock ([`Register::YEAR`]..=[`Register::SECOND`], registers 48-53).
///
/// Not every model has one, see [`Model::has_rtc`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year:   Word,
    pub month:  Word,
    pub day:    Word,
    pub hour:   Word,
    pub minute: Word,
    pub second: Word,
}

impl DateTime {
    /// Decodes `[year, month, day, hour, minute, second]`.
    #[must_use]
    pub const fn from_words(words: [Word; 6]) -> Self {
        Self {
            year:   words[0],
            month:  words[1],
            day:    words[2],
            hour:   words[3],
            minute: words[4],
            second: words[5],
        }
    }

    /// Encodes as `[year, month, day, hour, minute, second]` for writing.
    #[must_use]
    pub const fn to_words(self) -> [Word; 6] {
        [
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
        ]
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Identification, identification and settings block: registers
/// [`Register::ID`] through [`Register::I_RANGE`] (0-20).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Info {
    /// Raw device ID (register 0)
    pub id:      Word,
    /// Device model decoded from `id`
    pub model:   Model,
    /// Zero-padded serial number
    pub sn:      String,
    /// Firmware version, `136` = v1.36
    pub fw:      Word,
    /// Internal temperature in °C
    pub int_c:   i16,
    /// Internal temperature in °F
    pub int_f:   i16,
    /// Voltage setpoint (V)
    pub v_set:   f32,
    /// Current setpoint (A)
    pub i_set:   f32,
    /// Output voltage (V)
    pub v_out:   f32,
    /// Output current (A)
    pub i_out:   f32,
    /// Charge shown on the display (Ah)
    pub ah:      f32,
    /// Output power (W)
    pub p_out:   f32,
    /// Input voltage (V)
    pub v_in:    f32,
    /// Keypad locked
    pub keypad:  bool,
    /// Tripped output protection
    pub ovp_ocp: Protection,
    /// Regulation mode
    pub cv_cc:   CvCc,
    /// Output enabled
    pub output:  bool,
    /// Active preset slot
    pub preset:  Word,
    /// Current range (RD6012P only: 0 = 6A, 1 = 12A)
    pub i_range: Word,
}

/// Battery/energy block: registers [`Register::BAT_MODE`] through
/// [`Register::WH_L`] (32-41).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Status {
    /// Battery mode active
    pub bat_mode: bool,
    /// Battery voltage (V)
    pub v_bat:    f32,
    /// External temperature probe in °C (`-128` usually means unconnected)
    pub ext_c:    i16,
    /// External temperature probe in °F
    pub ext_f:    i16,
    /// Charge counter (Ah)
    pub ah:       f32,
    /// Energy counter (Wh)
    pub wh:       f32,
}

/// One preset/memory slot (M0-M9): setpoints plus output protection limits.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Preset {
    /// Voltage setpoint (V)
    pub v:   f32,
    /// Current setpoint (A)
    pub i:   f32,
    /// Over-voltage limit (V)
    pub ovp: f32,
    /// Over-current limit (A)
    pub ocp: f32,
}

const fn word_at(words: &[Word], register: Register) -> Word {
    words[register.address() as usize]
}

const fn pair_at(words: &[Word], register: Register) -> WordPair {
    let at = register.address() as usize;
    [words[at], words[at + 1]]
}

impl TryFrom<&[Word]> for Info {
    type Error = AnyhowError;

    /// Decodes the register block `ID..=I_RANGE` (addresses 0-20).
    fn try_from(words: &[Word]) -> Result<Self> {
        let first = Register::ID.address() as usize;
        let last = Register::I_RANGE.address() as usize;
        if words.len() < last - first + 1 {
            bail!(
                "expected at least {} words for the info block, got {}",
                last - first + 1,
                words.len()
            );
        }

        let id = word_at(words, Register::ID);
        let model = Model::from_id(id);
        let i_range = word_at(words, Register::I_RANGE);

        let sn = serial_number(double_word(pair_at(words, Register::SN_H)));
        let fw = word_at(words, Register::FW);
        let int_c = temperature(pair_at(words, Register::INT_C_S));
        let int_f = temperature(pair_at(words, Register::INT_F_S));

        let v_mul = model.v_mul();
        let i_mul = model.i_mul(i_range);

        Ok(Self {
            id,
            model,
            sn,
            fw,
            int_c,
            int_f,
            v_set: f32::from(word_at(words, Register::V_SET)) / v_mul,
            i_set: f32::from(word_at(words, Register::I_SET)) / i_mul,
            v_out: f32::from(word_at(words, Register::V_OUT)) / v_mul,
            i_out: f32::from(word_at(words, Register::I_OUT)) / i_mul,
            ah: f32::from(word_at(words, Register::AH)) / 1_000.0,
            p_out: f32::from(word_at(words, Register::P_OUT)) / model.p_mul(),
            v_in: f32::from(word_at(words, Register::V_IN)) / model.v_in_mul(),
            keypad: word_at(words, Register::KEYPAD) != 0,
            ovp_ocp: Protection::from(word_at(words, Register::OVP_OCP)),
            cv_cc: CvCc::from(word_at(words, Register::CV_CC)),
            output: word_at(words, Register::OUTPUT) != 0,
            preset: word_at(words, Register::PRESET),
            i_range,
        })
    }
}

impl Status {
    /// Decodes the register block `BAT_MODE..=WH_L` (addresses 32-41).
    ///
    /// `model` supplies the scale factor for the battery voltage.
    ///
    /// # Errors
    /// Will return `Err` if the slice is shorter than the register block.
    pub fn from_words(words: &[Word], model: Model) -> Result<Self> {
        let len = Register::WH_L.address() - Register::BAT_MODE.address() + 1;
        if words.len() < usize::from(len) {
            bail!(
                "expected at least {} words for the status block, got {}",
                len,
                words.len()
            );
        }

        // The slice is indexed relative to the block start.
        let at = |register: Register| {
            words[usize::from(register.address() - Register::BAT_MODE.address())]
        };

        Ok(Self {
            bat_mode: at(Register::BAT_MODE) != 0,
            v_bat:    f32::from(at(Register::V_BAT)) / model.v_mul(),
            ext_c:    temperature([at(Register::EXT_C_S), at(Register::EXT_C)]),
            ext_f:    temperature([at(Register::EXT_F_S), at(Register::EXT_F)]),
            ah:       per_mille(double_word([at(Register::AH_H), at(Register::AH_L)])),
            wh:       per_mille(double_word([at(Register::WH_H), at(Register::WH_L)])),
        })
    }
}

impl Preset {
    /// Decodes a preset block of four words: `[v, i, ovp, ocp]`.
    #[must_use]
    pub fn from_words(words: [Word; 4], model: Model) -> Self {
        Self {
            v:   f32::from(words[0]) / model.v_mul(),
            i:   f32::from(words[1]) / model.i_mul(0),
            ovp: f32::from(words[2]) / model.v_mul(),
            ocp: f32::from(words[3]) / model.i_mul(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example RD6018 register block:
    /// `ID 60181, SN 00011608, FW 136, INT 29C/84F, V_IN 68.07, CV, EXT -89C/-128F`.
    const RD6018_INFO: [Word; 21] = [
        60_181, 0, 11_608, 136, // ID, SN_H, SN_L, FW
        0, 29, 0, 84, // INT_C_S, INT_C, INT_F_S, INT_F
        0, 0, // V_SET, I_SET
        0, 0, 0, 0, 6_807, // V_OUT, I_OUT, AH, P_OUT, V_IN
        0, 0, 0, 0, 0, 0, // KEYPAD, OVP_OCP, CV_CC, OUTPUT, PRESET, I_RANGE
    ];

    const RD6018_STATUS: [Word; 10] = [
        0, 0, // BAT_MODE, V_BAT
        1, 89, 1, 128, // EXT_C_S, EXT_C, EXT_F_S, EXT_F
        1, 0, 0, 500, // AH_H, AH_L, WH_H, WH_L
    ];

    #[test]
    fn decode_info() {
        let info = Info::try_from(&RD6018_INFO[..]).unwrap();
        assert_eq!(info.id, 60_181);
        assert_eq!(info.model, Model::RD6018);
        assert_eq!(info.sn, "00011608");
        assert_eq!(info.fw, 136);
        assert_eq!(info.int_c, 29);
        assert_eq!(info.int_f, 84);
        assert!((info.v_in - 68.07).abs() < 1e-3);
        assert_eq!(info.cv_cc, CvCc::Cv);
        assert_eq!(info.ovp_ocp, Protection::None);
        assert!(!info.output);
        assert_eq!(info.i_range, 0);
    }

    #[test]
    fn short_blocks_error() {
        assert!(Info::try_from(&RD6018_INFO[..20]).is_err());
        assert!(Status::from_words(&RD6018_STATUS[..9], Model::RD6018).is_err());
    }

    #[test]
    fn decode_status() {
        let status = Status::from_words(&RD6018_STATUS, Model::RD6018).unwrap();
        assert!(!status.bat_mode);
        // Sign-flag registers are flags, not high words: EXT_C_S=1 means -89,
        // not (1 << 16) | 89.
        assert_eq!(status.ext_c, -89);
        assert_eq!(status.ext_f, -128);
        // AH must combine both words: [1, 0] = 65536 mWh -> 65.536 Ah
        assert!((status.ah - 65.536).abs() < 1e-3);
        assert!((status.wh - 0.5).abs() < 1e-6);
    }

    #[test]
    fn serial_number_zero_pads() {
        assert_eq!(serial_number(double_word([0, 11_608])), "00011608");
        // Regression: shifting a u16 by 16 is a no-op, the high word must be
        // widened before the shift.
        assert_eq!(double_word([1, 0]), 65_536);
    }

    #[test]
    fn date_time_roundtrip() {
        let dt = DateTime::from_words([2022, 2, 13, 11, 26, 2]);
        assert_eq!(dt.to_string(), "2022-02-13 11:26:02");
        assert_eq!(dt, DateTime::from_words(dt.to_words()));
    }
}
