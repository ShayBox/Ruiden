//! Modbus RTU holding-register map for Ruiden RD-series power supplies.
//!
//! Registers 21-31, 42-47, 54, 63-65, 73-79 and 120-255 are unused/unknown.
//! The calibration registers (55-62) are only persisted after writing a commit
//! value to [`Register::CAL_COMMIT`]; changing them can make the unit unsafe.

/// A holding register address.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum Register {
    /* Identification */
    /// Device ID, e.g. `60181` for an RD6018 (see [`crate::Model::from_id`])
    ID         = 0,
    /// Serial number, high word
    SN_H       = 1,
    /// Serial number, low word
    SN_L       = 2,
    /// Firmware version, e.g. `136` = v1.36
    FW         = 3,

    /* Measurement & state */
    /// Internal temperature (°C) sign flag, `0` = positive, `1` = negative
    INT_C_S    = 4,
    /// Internal temperature (°C) magnitude
    INT_C      = 5,
    /// Internal temperature (°F) sign flag, `0` = positive, `1` = negative
    INT_F_S    = 6,
    /// Internal temperature (°F) magnitude
    INT_F      = 7,
    /// Voltage setpoint
    V_SET      = 8,
    /// Current setpoint
    I_SET      = 9,
    /// Voltage measured at the output
    V_OUT      = 10,
    /// Current measured at the output
    I_OUT      = 11,
    /// Charge (Ah) shown on the display, divide by 1000
    AH         = 12,
    /// Power measured at the output
    P_OUT      = 13,
    /// Voltage at the input
    V_IN       = 14,
    /// Keypad lock, `0` = unlocked, `1` = locked
    KEYPAD     = 15,
    /// Protection status, `0` = none, `1` = over-voltage, `2` = over-current
    OVP_OCP    = 16,
    /// Regulation mode, `0` = constant voltage, `1` = constant current
    CV_CC      = 17,
    /// Output enable, `0` = off, `1` = on
    OUTPUT     = 18,
    /// Active preset/memory slot (writing it recalls that preset)
    PRESET     = 19,
    /// Current range, only meaningful on the RD6012P: `0` = 6A, `1` = 12A
    I_RANGE    = 20,

    /* Battery mode */
    /// Battery mode active, `0` = off, `1` = on
    BAT_MODE   = 32,
    /// Battery voltage
    V_BAT      = 33,
    /// External temperature probe (°C) sign flag, `0` = positive, `1` = negative
    EXT_C_S    = 34,
    /// External temperature probe (°C) magnitude, unconnected probes read `-128`
    EXT_C      = 35,
    /// External temperature probe (°F) sign flag, `0` = positive, `1` = negative
    EXT_F_S    = 36,
    /// External temperature probe (°F) magnitude
    EXT_F      = 37,
    /// Charge counter (Ah), high word, divide the combined value by 1000
    AH_H       = 38,
    /// Charge counter (Ah), low word
    AH_L       = 39,
    /// Energy counter (Wh), high word, divide the combined value by 1000
    WH_H       = 40,
    /// Energy counter (Wh), low word
    WH_L       = 41,

    /* Real-time clock (absent on the RK6006) */
    /// Year, e.g. `2022`
    YEAR       = 48,
    /// Month, 1-12
    MONTH      = 49,
    /// Day of month, 1-31
    DAY        = 50,
    /// Hour, 0-23
    HOUR       = 51,
    /// Minute, 0-59
    MINUTE     = 52,
    /// Second, 0-59
    SECOND     = 53,

    /* Calibration - DO NOT CHANGE unless you know what you are doing! */
    /// Commit calibration registers (55-62) to NVRAM, write `0xA1` (161) to save
    CAL_COMMIT = 54,
    /// Output voltage, zero offset
    V_OUT_ZERO = 55,
    /// Output voltage, scale
    V_OUT_SCALE = 56,
    /// Back-side voltage, zero offset
    V_BACK_ZERO = 57,
    /// Back-side voltage, scale
    V_BACK_SCALE = 58,
    /// Output current, zero offset
    I_OUT_ZERO = 59,
    /// Output current, scale
    I_OUT_SCALE = 60,
    /// Back-side current, zero offset
    I_BACK_ZERO = 61,
    /// Back-side current, scale
    I_BACK_SCALE = 62,

    /* Settings/options */
    /// Option: accept set values while the output is on
    OPT_TAKE_OK = 66,
    /// Option: recall set values when the output turns off
    OPT_TAKE_OUT = 67,
    /// Option: restore the output state on power-on
    OPT_BOOT_POW = 68,
    /// Option: buzzer enabled
    OPT_BUZZ   = 69,
    /// Option: boot logo enabled
    OPT_LOGO   = 70,
    /// Option: display language
    OPT_LANG   = 71,
    /// Option: backlight
    OPT_LIGHT  = 72,

    /* Presets/memory slots M0-M9: voltage, current, OVP, OCP per slot */
    M0_V       = 80,
    M0_I       = 81,
    M0_OVP     = 82,
    M0_OCP     = 83,
    M1_V       = 84,
    M1_I       = 85,
    M1_OVP     = 86,
    M1_OCP     = 87,
    M2_V       = 88,
    M2_I       = 89,
    M2_OVP     = 90,
    M2_OCP     = 91,
    M3_V       = 92,
    M3_I       = 93,
    M3_OVP     = 94,
    M3_OCP     = 95,
    M4_V       = 96,
    M4_I       = 97,
    M4_OVP     = 98,
    M4_OCP     = 99,
    M5_V       = 100,
    M5_I       = 101,
    M5_OVP     = 102,
    M5_OCP     = 103,
    M6_V       = 104,
    M6_I       = 105,
    M6_OVP     = 106,
    M6_OCP     = 107,
    M7_V       = 108,
    M7_I       = 109,
    M7_OVP     = 110,
    M7_OCP     = 111,
    M8_V       = 112,
    M8_I       = 113,
    M8_OVP     = 114,
    M8_OCP     = 115,
    M9_V       = 116,
    M9_I       = 117,
    M9_OVP     = 118,
    M9_OCP     = 119,

    /* System */
    /// System command register. Write [`CMD_REBOOT`] to reboot into the
    /// bootloader (flash) mode; the unit usually does not answer this write.
    SYS_CMD    = 256,
}

impl Register {
    /// The register's Modbus address.
    #[must_use]
    pub const fn address(self) -> u16 {
        self as u16
    }

    /// The register's name, e.g. `"V_SET"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ID => "ID",
            Self::SN_H => "SN_H",
            Self::SN_L => "SN_L",
            Self::FW => "FW",
            Self::INT_C_S => "INT_C_S",
            Self::INT_C => "INT_C",
            Self::INT_F_S => "INT_F_S",
            Self::INT_F => "INT_F",
            Self::V_SET => "V_SET",
            Self::I_SET => "I_SET",
            Self::V_OUT => "V_OUT",
            Self::I_OUT => "I_OUT",
            Self::AH => "AH",
            Self::P_OUT => "P_OUT",
            Self::V_IN => "V_IN",
            Self::KEYPAD => "KEYPAD",
            Self::OVP_OCP => "OVP_OCP",
            Self::CV_CC => "CV_CC",
            Self::OUTPUT => "OUTPUT",
            Self::PRESET => "PRESET",
            Self::I_RANGE => "I_RANGE",
            Self::BAT_MODE => "BAT_MODE",
            Self::V_BAT => "V_BAT",
            Self::EXT_C_S => "EXT_C_S",
            Self::EXT_C => "EXT_C",
            Self::EXT_F_S => "EXT_F_S",
            Self::EXT_F => "EXT_F",
            Self::AH_H => "AH_H",
            Self::AH_L => "AH_L",
            Self::WH_H => "WH_H",
            Self::WH_L => "WH_L",
            Self::YEAR => "YEAR",
            Self::MONTH => "MONTH",
            Self::DAY => "DAY",
            Self::HOUR => "HOUR",
            Self::MINUTE => "MINUTE",
            Self::SECOND => "SECOND",
            Self::CAL_COMMIT => "CAL_COMMIT",
            Self::V_OUT_ZERO => "V_OUT_ZERO",
            Self::V_OUT_SCALE => "V_OUT_SCALE",
            Self::V_BACK_ZERO => "V_BACK_ZERO",
            Self::V_BACK_SCALE => "V_BACK_SCALE",
            Self::I_OUT_ZERO => "I_OUT_ZERO",
            Self::I_OUT_SCALE => "I_OUT_SCALE",
            Self::I_BACK_ZERO => "I_BACK_ZERO",
            Self::I_BACK_SCALE => "I_BACK_SCALE",
            Self::OPT_TAKE_OK => "OPT_TAKE_OK",
            Self::OPT_TAKE_OUT => "OPT_TAKE_OUT",
            Self::OPT_BOOT_POW => "OPT_BOOT_POW",
            Self::OPT_BUZZ => "OPT_BUZZ",
            Self::OPT_LOGO => "OPT_LOGO",
            Self::OPT_LANG => "OPT_LANG",
            Self::OPT_LIGHT => "OPT_LIGHT",
            Self::M0_V => "M0_V",
            Self::M0_I => "M0_I",
            Self::M0_OVP => "M0_OVP",
            Self::M0_OCP => "M0_OCP",
            Self::M1_V => "M1_V",
            Self::M1_I => "M1_I",
            Self::M1_OVP => "M1_OVP",
            Self::M1_OCP => "M1_OCP",
            Self::M2_V => "M2_V",
            Self::M2_I => "M2_I",
            Self::M2_OVP => "M2_OVP",
            Self::M2_OCP => "M2_OCP",
            Self::M3_V => "M3_V",
            Self::M3_I => "M3_I",
            Self::M3_OVP => "M3_OVP",
            Self::M3_OCP => "M3_OCP",
            Self::M4_V => "M4_V",
            Self::M4_I => "M4_I",
            Self::M4_OVP => "M4_OVP",
            Self::M4_OCP => "M4_OCP",
            Self::M5_V => "M5_V",
            Self::M5_I => "M5_I",
            Self::M5_OVP => "M5_OVP",
            Self::M5_OCP => "M5_OCP",
            Self::M6_V => "M6_V",
            Self::M6_I => "M6_I",
            Self::M6_OVP => "M6_OVP",
            Self::M6_OCP => "M6_OCP",
            Self::M7_V => "M7_V",
            Self::M7_I => "M7_I",
            Self::M7_OVP => "M7_OVP",
            Self::M7_OCP => "M7_OCP",
            Self::M8_V => "M8_V",
            Self::M8_I => "M8_I",
            Self::M8_OVP => "M8_OVP",
            Self::M8_OCP => "M8_OCP",
            Self::M9_V => "M9_V",
            Self::M9_I => "M9_I",
            Self::M9_OVP => "M9_OVP",
            Self::M9_OCP => "M9_OCP",
            Self::SYS_CMD => "SYS_CMD",
        }
    }
}

impl TryFrom<u16> for Register {
    type Error = u16;

    /// Converts a Modbus address into a known [`Register`].
    ///
    /// Returns the address back as the error when it is not a known register.
    fn try_from(address: u16) -> Result<Self, Self::Error> {
        Ok(match address {
            0 => Self::ID,
            1 => Self::SN_H,
            2 => Self::SN_L,
            3 => Self::FW,
            4 => Self::INT_C_S,
            5 => Self::INT_C,
            6 => Self::INT_F_S,
            7 => Self::INT_F,
            8 => Self::V_SET,
            9 => Self::I_SET,
            10 => Self::V_OUT,
            11 => Self::I_OUT,
            12 => Self::AH,
            13 => Self::P_OUT,
            14 => Self::V_IN,
            15 => Self::KEYPAD,
            16 => Self::OVP_OCP,
            17 => Self::CV_CC,
            18 => Self::OUTPUT,
            19 => Self::PRESET,
            20 => Self::I_RANGE,
            32 => Self::BAT_MODE,
            33 => Self::V_BAT,
            34 => Self::EXT_C_S,
            35 => Self::EXT_C,
            36 => Self::EXT_F_S,
            37 => Self::EXT_F,
            38 => Self::AH_H,
            39 => Self::AH_L,
            40 => Self::WH_H,
            41 => Self::WH_L,
            48 => Self::YEAR,
            49 => Self::MONTH,
            50 => Self::DAY,
            51 => Self::HOUR,
            52 => Self::MINUTE,
            53 => Self::SECOND,
            54 => Self::CAL_COMMIT,
            55 => Self::V_OUT_ZERO,
            56 => Self::V_OUT_SCALE,
            57 => Self::V_BACK_ZERO,
            58 => Self::V_BACK_SCALE,
            59 => Self::I_OUT_ZERO,
            60 => Self::I_OUT_SCALE,
            61 => Self::I_BACK_ZERO,
            62 => Self::I_BACK_SCALE,
            66 => Self::OPT_TAKE_OK,
            67 => Self::OPT_TAKE_OUT,
            68 => Self::OPT_BOOT_POW,
            69 => Self::OPT_BUZZ,
            70 => Self::OPT_LOGO,
            71 => Self::OPT_LANG,
            72 => Self::OPT_LIGHT,
            80 => Self::M0_V,
            81 => Self::M0_I,
            82 => Self::M0_OVP,
            83 => Self::M0_OCP,
            84 => Self::M1_V,
            85 => Self::M1_I,
            86 => Self::M1_OVP,
            87 => Self::M1_OCP,
            88 => Self::M2_V,
            89 => Self::M2_I,
            90 => Self::M2_OVP,
            91 => Self::M2_OCP,
            92 => Self::M3_V,
            93 => Self::M3_I,
            94 => Self::M3_OVP,
            95 => Self::M3_OCP,
            96 => Self::M4_V,
            97 => Self::M4_I,
            98 => Self::M4_OVP,
            99 => Self::M4_OCP,
            100 => Self::M5_V,
            101 => Self::M5_I,
            102 => Self::M5_OVP,
            103 => Self::M5_OCP,
            104 => Self::M6_V,
            105 => Self::M6_I,
            106 => Self::M6_OVP,
            107 => Self::M6_OCP,
            108 => Self::M7_V,
            109 => Self::M7_I,
            110 => Self::M7_OVP,
            111 => Self::M7_OCP,
            112 => Self::M8_V,
            113 => Self::M8_I,
            114 => Self::M8_OVP,
            115 => Self::M8_OCP,
            116 => Self::M9_V,
            117 => Self::M9_I,
            118 => Self::M9_OVP,
            119 => Self::M9_OCP,
            256 => Self::SYS_CMD,
            _ => return Err(address),
        })
    }
}

/// Value for [`Register::CAL_COMMIT`]: writes the calibration registers to NVRAM.
pub const CAL_COMMIT_NVRAM: u16 = 0xA1;

/// Value for [`Register::SYS_CMD`]: reboots the unit into bootloader (flash) mode.
pub const CMD_REBOOT: u16 = 0x1601;
