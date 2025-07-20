use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SourceHash(u16);

impl SourceHash {
    pub fn parse(hex: &str) -> Option<Self> {
        u16::from_str_radix(hex, 16).ok().map(SourceHash)
    }

    pub fn from(code: &str) -> Self {
        let mut s = DefaultHasher::new();
        code.hash(&mut s);
        SourceHash(s.finish() as u16)
    }

    pub fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct BlockId(pub u32);

impl BlockId {
    pub fn to_string(&self) -> String {
        format!("{}", self.0)
    }
}

pub struct BlockEvalResult {
    pub body: String,
    pub output_lang: String,
}
