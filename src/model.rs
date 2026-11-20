//! Device model detection and scale factors.
//!
//! The device ID register (register 0) encodes the model, e.g. `60181` is an
//! RD6018. Measured/set values are integers that must be divided by a model
//! dependent multiplier to get volts/amps/watts. The RD6030 factors mirror the
//! RD6024 and are unverified.

use std::fmt::{self, Display, Formatter};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Model {
    /// Device ID did not match any known model
    #[default]
    Unknown,
    /// Ruiden RK6006(-BT), a battery variant without a real-time clock
    RK6006,
    /// Ruiden RD6006 (60V/6A)
    RD6006,
    /// Ruiden RD6006P (60V/6A, higher precision)
    RD6006P,
    /// Ruiden RD6012 (60V/12A)
    RD6012,
    /// Ruiden RD6012P (60V/12A, higher precision, switchable 6A/12A range)
    RD6012P,
    /// Ruiden RD6018 (60V/18A), including the `WiFi` `RD6018W` variant, which
    /// reports the same device ID
    RD6018,
    /// Ruiden RD6024 (60V/24A)
    RD6024,
    /// Ruiden RD6030 (60V/30A)
    RD6030,
}

impl Model {
    /// Maps a raw device ID (register 0) to a [`Model`].
    #[must_use]
    pub const fn from_id(id: u16) -> Self {
        match id {
            60_060..=60_064 => Self::RD6006,
            60_065 => Self::RD6006P,
            60_066 => Self::RK6006,
            60_120..=60_124 => Self::RD6012,
            60_125..=60_129 => Self::RD6012P,
            60_180..=60_189 => Self::RD6018,
            60_240..=60_249 => Self::RD6024,
            60_300..=60_309 => Self::RD6030,
            _ => Self::Unknown,
        }
    }

    /// Voltage scale factor: `volts = raw / v_mul()`.
    #[must_use]
    pub const fn v_mul(self) -> f32 {
        match self {
            Self::RD6006P | Self::RD6012P => 1_000.0,
            Self::Unknown
            | Self::RK6006
            | Self::RD6006
            | Self::RD6012
            | Self::RD6018
            | Self::RD6024
            | Self::RD6030 => 100.0,
        }
    }

    /// Current scale factor: `amps = raw / i_mul(i_range)`.
    ///
    /// `i_range` is the value of [`crate::Register::I_RANGE`] and only affects
    /// the RD6012P, whose resolution depends on the selected range.
    #[must_use]
    pub const fn i_mul(self, i_range: u16) -> f32 {
        match self {
            Self::RD6006P => 10_000.0,
            Self::RD6012P => {
                if i_range == 0 {
                    10_000.0
                } else {
                    1_000.0
                }
            }
            Self::RK6006 | Self::RD6006 => 1_000.0,
            Self::Unknown | Self::RD6012 | Self::RD6018 | Self::RD6024 | Self::RD6030 => 100.0,
        }
    }

    /// Power scale factor: `watts = raw / p_mul()`.
    #[must_use]
    pub const fn p_mul(self) -> f32 {
        match self {
            Self::RD6006P | Self::RD6012P => 1_000.0,
            Self::Unknown
            | Self::RK6006
            | Self::RD6006
            | Self::RD6012
            | Self::RD6018
            | Self::RD6024
            | Self::RD6030 => 100.0,
        }
    }

    /// Input voltage scale factor: `volts = raw / v_in_mul()`.
    ///
    /// All known models report input voltage with the same 0.01V resolution.
    #[must_use]
    pub const fn v_in_mul(self) -> f32 {
        match self {
            Self::Unknown
            | Self::RK6006
            | Self::RD6006
            | Self::RD6006P
            | Self::RD6012
            | Self::RD6012P
            | Self::RD6018
            | Self::RD6024
            | Self::RD6030 => 100.0,
        }
    }

    /// Whether the model has a real-time clock (registers 48-53).
    #[must_use]
    pub const fn has_rtc(self) -> bool {
        !matches!(self, Self::Unknown | Self::RK6006)
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unknown => "UNKNOWN",
            Self::RK6006 => "RK6006",
            Self::RD6006 => "RD6006",
            Self::RD6006P => "RD6006P",
            Self::RD6012 => "RD6012",
            Self::RD6012P => "RD6012P",
            Self::RD6018 => "RD6018",
            Self::RD6024 => "RD6024",
            Self::RD6030 => "RD6030",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_ranges() {
        assert_eq!(Model::from_id(60_060), Model::RD6006);
        assert_eq!(Model::from_id(60_062), Model::RD6006);
        assert_eq!(Model::from_id(60_064), Model::RD6006);
        assert_eq!(Model::from_id(60_065), Model::RD6006P);
        assert_eq!(Model::from_id(60_066), Model::RK6006);
        assert_eq!(Model::from_id(60_120), Model::RD6012);
        assert_eq!(Model::from_id(60_124), Model::RD6012);
        assert_eq!(Model::from_id(60_125), Model::RD6012P);
        assert_eq!(Model::from_id(60_129), Model::RD6012P);
        assert_eq!(Model::from_id(60_181), Model::RD6018);
        assert_eq!(Model::from_id(60_241), Model::RD6024);
        assert_eq!(Model::from_id(60_301), Model::RD6030);
        assert_eq!(Model::from_id(0), Model::Unknown);
        assert_eq!(Model::from_id(1), Model::Unknown);
    }

    #[test]
    fn scale_factors() {
        assert_eq!(Model::RD6006.v_mul(), 100.0);
        assert_eq!(Model::RD6006.i_mul(0), 1_000.0);
        assert_eq!(Model::RD6006P.v_mul(), 1_000.0);
        assert_eq!(Model::RD6006P.i_mul(0), 10_000.0);
        // RD6012P resolution depends on the current range register
        assert_eq!(Model::RD6012P.i_mul(0), 10_000.0);
        assert_eq!(Model::RD6012P.i_mul(1), 1_000.0);
        assert_eq!(Model::RD6018.p_mul(), 100.0);
        assert_eq!(Model::RD6012P.p_mul(), 1_000.0);
        assert!(!Model::RK6006.has_rtc());
        assert!(Model::RD6018.has_rtc());
    }
}
