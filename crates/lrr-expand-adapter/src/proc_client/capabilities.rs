//! Admission contract for the measured Rust1.99/API6 span interface.
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcSpanOperation {
    CallSite,
    MixedSite,
    SourceText,
    ResolvedAt,
    LocatedAt,
    Start,
    End,
    Line,
    Column,
    File,
    LocalFile,
    DefSite,
    Source,
    Parent,
    Join,
    ByteRange,
}
impl ProcSpanOperation {
    pub const fn admitted(self) -> bool {
        matches!(
            self,
            Self::CallSite
                | Self::MixedSite
                | Self::SourceText
                | Self::ResolvedAt
                | Self::LocatedAt
                | Self::Start
                | Self::End
                | Self::Line
                | Self::Column
                | Self::File
                | Self::LocalFile
        )
    }
    pub fn stable_set() -> BTreeSet<Self> {
        [
            Self::CallSite,
            Self::MixedSite,
            Self::SourceText,
            Self::ResolvedAt,
            Self::LocatedAt,
            Self::Start,
            Self::End,
            Self::Line,
            Self::Column,
            Self::File,
            Self::LocalFile,
        ]
        .into_iter()
        .collect()
    }
}
