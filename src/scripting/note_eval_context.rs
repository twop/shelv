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

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CodeBlockKind {
    Source,
    Output(Option<SourceHash>),
}

pub struct BlockEvalResult {
    pub body: String,
    pub output_lang: String,
}

pub trait NoteEvalContext {
    type State;
    fn begin(&mut self) -> Self::State;
    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind>;
    fn eval_block(
        &mut self,
        body: &str,
        hash: SourceHash,
        state: &mut Self::State,
    ) -> BlockEvalResult;
    fn should_force_eval(&self) -> bool;
}
