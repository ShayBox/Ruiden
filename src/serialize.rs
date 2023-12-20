use std::array::TryFromSliceError;

use crate::{register::Register, Word, WordPair, Words};

macro_rules! hlp {
    ($e:expr) => {
        HighLowPair::from($e).0
    };
}

macro_rules! r {
    ($i:ident) => {
        Register::$i as usize
    };
}

macro_rules! sn {
    ($e:expr) => {
        SerialNumber::from($e).0
    };
}

macro_rules! wp {
    ($e:expr) => {
        WordPair::try_from($e)?
    };
}

#[derive(Debug, Default)]
pub enum ID {
    #[default]
    Unknown,
    RD6006,
    RD6006P,
    RD6012,
    RD6012P,
    RD6018,
    RD6024,
}

#[derive(Debug, Default)]
pub struct HighLowPair(pub u16);

#[derive(Debug, Default)]
pub struct SerialNumber(pub String);

#[derive(Debug, Default)]
pub struct Information {
    pub id:    ID,
    pub sn:    String,
    pub fw:    u16,
    pub int_c: u16,
    pub int_f: u16,
    pub v_mul: f32,
    pub v_set: f32,
    pub i_mul: f32,
    pub i_set: f32,
    pub v_out: f32,
    pub i_out: f32,
}

impl From<WordPair> for HighLowPair {
    fn from([high, low]: WordPair) -> Self {
        Self(high.wrapping_shl(16) | low)
    }
}

impl From<Word> for SerialNumber {
    fn from(word: Word) -> Self {
        Self(format!("{word:08}"))
    }
}

impl From<WordPair> for SerialNumber {
    fn from(word_pair: WordPair) -> Self {
        let high_low_pair = HighLowPair::from(word_pair);
        Self::from(high_low_pair.0)
    }
}

impl TryFrom<Words> for Information {
    type Error = TryFromSliceError;

    fn try_from(words: Words) -> Result<Self, Self::Error> {
        let id = match words.first() {
            Some(60181) => ID::RD6018,
            _ => ID::Unknown,
        };

        let v_mul = match id {
            ID::RD6006 | ID::RD6012 | ID::RD6018 | ID::RD6024 => 100.0,
            ID::RD6006P | ID::RD6012P => 1000.0,
            ID::Unknown => 0.0,
        };

        let i_mul = match id {
            ID::RD6012 | ID::RD6012P | ID::RD6018 | ID::RD6024 => 100.0,
            ID::RD6006 => 1000.0,
            ID::RD6006P => 10000.0,
            ID::Unknown => 0.0,
        };

        Ok(Self {
            id,
            sn: sn!(wp!(&words[r!(SN_H)..=r!(SN_L)])),
            fw: words[r!(FW)],
            int_c: hlp!(wp!(&words[r!(INT_C_S)..=r!(INT_C)])),
            int_f: hlp!(wp!(&words[r!(INT_F_S)..=r!(INT_F)])),
            v_mul,
            v_set: f32::from(words[r!(V_SET)]) / v_mul,
            i_mul,
            i_set: f32::from(words[r!(I_SET)]) / i_mul,
            v_out: f32::from(words[r!(V_OUT)]) / v_mul,
            i_out: f32::from(words[r!(I_OUT)]) / i_mul,
        })
    }
}
