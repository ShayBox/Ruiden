use crate::{WordPair, Words};

#[derive(Debug, Default)]
pub struct Initialization {
    pub id: u16,
    pub sn: String,
    pub fw: u16,
}

#[derive(Debug, Default)]
pub struct Information {}

struct SerialNumber(String);

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
    fn from(_words: Words) -> Self {
        Self {}
    }
}

impl From<WordPair> for SerialNumber {
    fn from((high, low): WordPair) -> Self {
        Self(format!("{:08}", high.wrapping_shl(16) | low))
    }
}
