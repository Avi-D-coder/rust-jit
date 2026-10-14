//! Runtime macro semantic kinds. Source selection is identity-based in the frontend.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RuntimeMacroKind {
    Panic,
    Assert,
    AssertEq,
    AssertNe,
    Println,
    Vec,
}
impl RuntimeMacroKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Assert => "assert",
            Self::AssertEq => "assert_eq",
            Self::AssertNe => "assert_ne",
            Self::Println => "println",
            Self::Vec => "vec",
        }
    }
    pub(crate) fn tag(self) -> u32 {
        match self {
            Self::Panic => 0,
            Self::Assert => 1,
            Self::AssertEq => 2,
            Self::AssertNe => 3,
            Self::Println => 4,
            Self::Vec => 5,
        }
    }
}
