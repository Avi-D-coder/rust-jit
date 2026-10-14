//! Snapshot attachments are kept outside reusable semantic recipes.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::{
        manifest::BodyKey,
        snapshot::{LineColumn, Snapshot, SourcePath},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Owned diagnostic offsets mapped to the original expansion token span key.
/// Adapter types remain confined to the expansion boundary.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TokenOrigin {
    pub start: u32,
    pub end: u32,
    pub span: super::token_wire::Key,
}

/// Retains the exact parse and immutable source-map revision needed by a user.
pub struct SourceLease {
    parsed: std::sync::Arc<super::parse::ParsedFile>,
    snapshot: std::sync::Arc<Snapshot>,
    attachments: std::sync::Arc<SpanAttachments>,
}
impl SourceLease {
    pub(crate) fn new(
        parsed: std::sync::Arc<super::parse::ParsedFile>,
        snapshot: std::sync::Arc<Snapshot>,
        attachments: std::sync::Arc<SpanAttachments>,
        file: &SourcePath,
    ) -> Result<Self, Diagnostic> {
        if attachments.snapshot != snapshot.id()
            || snapshot
                .files()
                .get(file)
                .is_none_or(|blob| blob.content() != parsed.content)
        {
            return Err(mapping_error("source lease revision/content mismatch"));
        }
        Ok(Self {
            parsed,
            snapshot,
            attachments,
        })
    }
    pub fn parsed(&self) -> &super::parse::ParsedFile {
        &self.parsed
    }
    pub fn body(&self, span: &BodySpan) -> Result<AttachedSpan, Diagnostic> {
        self.attachments.body(&self.snapshot, span)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: u32,
    pub end: u32,
}
impl SourceRange {
    pub fn checked(start: u32, end: u32) -> Result<Self, Diagnostic> {
        if start > end {
            Err(mapping_error("reversed source range"))
        } else {
            Ok(Self { start, end })
        }
    }
    pub fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodySpan {
    pub body_key: BodyKey,
    pub relative_start: u32,
    pub relative_end: u32,
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HeaderSpan {
    pub def_key: Digest,
    pub relative_start: u32,
    pub relative_end: u32,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AttachedSpan {
    pub file: SourcePath,
    pub range: SourceRange,
    pub start: LineColumn,
    pub end: LineColumn,
}
#[derive(Debug, Clone)]
struct Base {
    file: SourcePath,
    range: SourceRange,
}
#[derive(Debug, Clone)]
pub struct SpanAttachments {
    snapshot: Digest,
    bodies: BTreeMap<Digest, Base>,
    headers: BTreeMap<Digest, Base>,
}
fn mapping_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SOURCE-MAP", message.into())
}
impl SpanAttachments {
    pub fn new(snapshot: &Snapshot) -> Self {
        Self {
            snapshot: snapshot.id(),
            bodies: BTreeMap::new(),
            headers: BTreeMap::new(),
        }
    }
    pub fn insert_body(
        &mut self,
        key: &BodyKey,
        file: SourcePath,
        range: SourceRange,
    ) -> Result<(), Diagnostic> {
        let base = Base { file, range };
        if let Some(old) = self.bodies.get(&key.fingerprint())
            && (old.file != base.file || old.range != base.range)
        {
            return Err(mapping_error(
                "one body identity has multiple current locations",
            ));
        }
        self.bodies.insert(key.fingerprint(), base);
        Ok(())
    }
    pub fn insert_header(
        &mut self,
        key: Digest,
        file: SourcePath,
        range: SourceRange,
    ) -> Result<(), Diagnostic> {
        let base = Base { file, range };
        if let Some(old) = self.headers.get(&key)
            && (old.file != base.file || old.range != base.range)
        {
            return Err(mapping_error(
                "one definition has multiple current locations",
            ));
        }
        self.headers.insert(key, base);
        Ok(())
    }
    fn attach(
        snapshot: &Snapshot,
        base: &Base,
        start: u32,
        end: u32,
    ) -> Result<AttachedSpan, Diagnostic> {
        let relative = SourceRange::checked(start, end)?;
        if relative.end > base.range.len() {
            return Err(mapping_error("relative origin exceeds current item extent"));
        }
        let start = base
            .range
            .start
            .checked_add(start)
            .ok_or_else(|| mapping_error("origin overflow"))?;
        let end = base
            .range
            .start
            .checked_add(end)
            .ok_or_else(|| mapping_error("origin overflow"))?;
        Ok(AttachedSpan {
            file: base.file.clone(),
            range: SourceRange { start, end },
            start: snapshot
                .source_map()
                .locate(snapshot.files(), &base.file, start)?,
            end: snapshot
                .source_map()
                .locate(snapshot.files(), &base.file, end)?,
        })
    }
    pub fn body(&self, snapshot: &Snapshot, span: &BodySpan) -> Result<AttachedSpan, Diagnostic> {
        if self.snapshot != snapshot.id() {
            return Err(mapping_error("attachment belongs to another snapshot"));
        }
        Self::attach(
            snapshot,
            self.bodies
                .get(&span.body_key.fingerprint())
                .ok_or_else(|| mapping_error("body origin has no current attachment"))?,
            span.relative_start,
            span.relative_end,
        )
    }
    pub fn header(
        &self,
        snapshot: &Snapshot,
        span: &HeaderSpan,
    ) -> Result<AttachedSpan, Diagnostic> {
        if self.snapshot != snapshot.id() {
            return Err(mapping_error("attachment belongs to another snapshot"));
        }
        Self::attach(
            snapshot,
            self.headers
                .get(&span.def_key)
                .ok_or_else(|| mapping_error("header origin has no current attachment"))?,
            span.relative_start,
            span.relative_end,
        )
    }
    pub fn estimated_bytes(&self) -> usize {
        self.bodies
            .values()
            .chain(self.headers.values())
            .map(|base| std::mem::size_of::<Base>() + 32 + base.file.as_str().len() + 32)
            .sum()
    }
}

/// A token diagnostic retains the exact immutable source revision and its unit identity.
/// Origin/hygiene are carried by the owned span, independently of display line positions.
pub struct TokenSpanLease {
    snapshot: std::sync::Arc<Snapshot>,
    unit: Digest,
}
impl TokenSpanLease {
    pub fn new(snapshot: std::sync::Arc<Snapshot>, unit: Digest) -> Self {
        Self { snapshot, unit }
    }
    pub fn attach(&self, span: &super::token_wire::SpanRecord) -> Result<AttachedSpan, Diagnostic> {
        self.attach_source_range(&span.source, SourceRange::checked(span.start, span.end)?)
    }
    pub fn attach_source_range(
        &self,
        source: &super::token_wire::SourceIdentity,
        range: SourceRange,
    ) -> Result<AttachedSpan, Diagnostic> {
        let path = SourcePath::try_from(source.path.clone())?;
        let blob = self
            .snapshot
            .files()
            .get(&path)
            .ok_or_else(|| mapping_error("token source missing from retained revision"))?;
        if Digest(source.unit.0) != self.unit || Digest(source.content.0) != blob.content() {
            return Err(mapping_error(
                "token unit/content differs from retained revision",
            ));
        }
        let range = SourceRange::checked(range.start, range.end)?;
        let map = self.snapshot.source_map();
        Ok(AttachedSpan {
            file: path.clone(),
            range,
            start: map.locate(self.snapshot.files(), &path, range.start)?,
            end: map.locate(self.snapshot.files(), &path, range.end)?,
        })
    }
    pub fn source_text<'a>(
        &'a self,
        span: &super::token_wire::SpanRecord,
    ) -> Result<&'a str, Diagnostic> {
        let attached = self.attach(span)?;
        let text = self.snapshot.files()[&attached.file].text()?;
        text.get(attached.range.start as usize..attached.range.end as usize)
            .ok_or_else(|| mapping_error("token span does not select a UTF8 range"))
    }
}
