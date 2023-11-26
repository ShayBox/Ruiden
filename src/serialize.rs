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

impl From<Words> for Initialization {
    fn from(words: Words) -> Self {
        Self {
            id: words[0],
            sn: Into::<SerialNumber>::into((words[1], words[2])).0,
            fw: words[3],
        }
    }
}

impl From<Words> for Information {
    fn from(words: Words) -> Self {
        Self {
            int_c: Into::<HighLowPair>::into((words[0], words[1])).0,
            int_f: Into::<HighLowPair>::into((words[2], words[3])).0,
        }
    }
}

impl From<WordPair> for HighLowPair {
    fn from((high, low): WordPair) -> Self {
        Self(high.wrapping_shl(16) | low)
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
