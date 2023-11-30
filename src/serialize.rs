use std::array::TryFromSliceError;

use crate::{WordPair, Words};

#[derive(Debug, Default)]
pub struct Initialization {
    pub id: u16,
    pub sn: String,
    pub fw: u16,
}

#[derive(Debug, Default)]
pub struct Information {
    pub int_c: u16,
    pub int_f: u16,
}

pub struct HighLowPair(pub u16);

pub struct SerialNumber(pub String);

impl TryFrom<Words> for Initialization {
    type Error = TryFromSliceError;

    fn try_from(words: Words) -> Result<Self, Self::Error> {
        Ok(Self {
            id: words[0],
            sn: Into::<SerialNumber>::into(TryInto::<WordPair>::try_into(&words[0..=1])?).0,
            fw: words[3],
        })
    }
}

impl TryFrom<Words> for Information {
    type Error = TryFromSliceError;

    fn try_from(words: Words) -> Result<Self, Self::Error> {
        Ok(Self {
            int_c: Into::<HighLowPair>::into(TryInto::<WordPair>::try_into(&words[0..=1])?).0,
            int_f: Into::<HighLowPair>::into(TryInto::<WordPair>::try_into(&words[2..=3])?).0,
        })
    }
}

impl From<WordPair> for HighLowPair {
    fn from(words: WordPair) -> Self {
        Self(words[0].wrapping_shl(16) | words[1])
    }
}

impl From<u16> for SerialNumber {
    fn from(high_low_pair: u16) -> Self {
        Self(format!("{high_low_pair:08}"))
    }
}

impl From<WordPair> for SerialNumber {
    fn from(word_pair: WordPair) -> Self {
        let high_low_pair = Into::<HighLowPair>::into(word_pair);
        Into::<Self>::into(high_low_pair.0)
    }
}
