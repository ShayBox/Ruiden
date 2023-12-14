use std::array::TryFromSliceError;

use crate::{Word, WordPair, Words};

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
    pub v_mul: u16,
    pub v_set: f32,
}

impl From<WordPair> for HighLowPair {
    fn from(words: WordPair) -> Self {
        Self(words[0].wrapping_shl(16) | words[1])
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
            ID::RD6006 | ID::RD6012 | ID::RD6018 | ID::RD6024 => 100,
            ID::RD6006P | ID::RD6012P => 1000,
            ID::Unknown => 0,
        };

        Ok(Self {
            id,
            sn: SerialNumber::from(WordPair::try_from(&words[1..=2])?).0,
            fw: words[3],
            int_c: HighLowPair::from(WordPair::try_from(&words[4..=5])?).0,
            int_f: HighLowPair::from(WordPair::try_from(&words[6..=7])?).0,
            v_mul,
            v_set: f32::from(words[8] / v_mul),
        })
    }
}
