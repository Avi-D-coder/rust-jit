//! Owned failure facts, retained before a successful expansion record exists.
use super::{
    spans::SourceRange,
    token_wire::{SourceIdentity, SpanRecord},
};
use crate::core::digest::Digest;
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct SourceOrigin {
    pub source: SourceIdentity,
    pub range: SourceRange,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum MacroFailureKind {
    DefinitionSyntax,
    NoMatchingRule,
    LeftoverTokens,
    Binding,
    UnresolvedBinding,
    UnexpectedToken,
    Limit,
    UnresolvedGeneratedProducer,
}
#[derive(Debug, Clone, Serialize)]
pub struct DefinitionOrigin {
    pub identity: Digest,
    pub name: String,
    pub location: SourceOrigin,
}
#[derive(Debug, Clone, Serialize)]
pub struct ExpansionOrigin {
    pub expansion: Digest,
    pub invocation_identity: Option<Digest>,
    pub invocation: Option<SourceOrigin>,
}
#[derive(Debug, Clone, Serialize)]
pub struct MacroOrigin {
    pub invocation_identity: Digest,
    pub invocation: Option<SourceOrigin>,
    pub definition: Option<DefinitionOrigin>,
    pub failure_kind: Option<MacroFailureKind>,
    pub failure_span_key: Option<super::token_wire::Key>,
    pub failure_span: Option<SpanRecord>,
    pub parents: Vec<ExpansionOrigin>,
}
/// Locations are populated only by checking an immutable captured snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct RenderedMacroOrigin {
    pub snapshot: Digest,
    pub invocation: Option<OriginLocation>,
    pub definition: Option<OriginLocation>,
    pub failure: Option<OriginLocation>,
    pub parents: Vec<Option<OriginLocation>>,
    pub omitted_parent_count: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct OriginLocation {
    /// Exact captured source world and snapshot, disambiguating relative paths.
    pub unit: Digest,
    pub snapshot: Digest,
    pub file: crate::source::snapshot::SourcePath,
    pub range: SourceRange,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub excerpt: Option<SourceExcerpt>,
}

/// A bounded, escaped window of a validated captured source line.
#[derive(Debug, Clone, Serialize)]
pub struct SourceExcerpt {
    pub text: String,
    pub omitted_before: bool,
    pub omitted_after: bool,
}

/// Dispatcher attachment to an entered generated body. Distinct token origins
/// remain distinct; no range spanning unrelated source locations is invented.
#[derive(Debug, Clone, Serialize)]
pub struct GeneratedOperationOrigin {
    pub body: super::spans::BodySpan,
    pub expansion: Digest,
    pub invocation_identity: Digest,
    pub invocation: Option<SourceOrigin>,
    pub definition: Option<DefinitionOrigin>,
    pub tokens: Vec<SpanRecord>,
    pub parents: Vec<ExpansionOrigin>,
}
impl GeneratedOperationOrigin {
    pub fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>().checked_add(
            self.tokens
                .capacity()
                .checked_mul(std::mem::size_of::<SpanRecord>())?,
        )?;
        if let Some(at) = &self.invocation {
            bytes = bytes.checked_add(at.source.path.capacity())?;
        }
        if let Some(definition) = &self.definition {
            bytes = bytes
                .checked_add(definition.name.capacity())?
                .checked_add(definition.location.source.path.capacity())?;
        }
        for span in &self.tokens {
            bytes = bytes.checked_add(span.source.path.capacity())?;
        }
        bytes = bytes.checked_add(
            self.parents
                .capacity()
                .checked_mul(std::mem::size_of::<ExpansionOrigin>())?,
        )?;
        for parent in &self.parents {
            if let Some(at) = &parent.invocation {
                bytes = bytes.checked_add(at.source.path.capacity())?;
            }
        }
        Some(bytes)
    }
}
impl crate::core::digest::StableEncode for GeneratedOperationOrigin {
    const DOMAIN: &'static str = "lrr/v2/generated-operation-origin";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.record(&self.body.body_key);
        e.u32(self.body.relative_start);
        e.u32(self.body.relative_end);
        e.digest(self.expansion);
        e.digest(self.invocation_identity);
        e.option(&self.invocation, |e, at| {
            e.digest(Digest(at.source.unit.0));
            e.digest(Digest(at.source.content.0));
            e.string(&at.source.path);
            e.u32(at.range.start);
            e.u32(at.range.end);
        });
        e.option(&self.definition, |e, definition| {
            e.digest(definition.identity);
            e.string(&definition.name);
            let at = &definition.location;
            e.digest(Digest(at.source.unit.0));
            e.digest(Digest(at.source.content.0));
            e.string(&at.source.path);
            e.u32(at.range.start);
            e.u32(at.range.end);
        });
        // SpanRecord's canonical key explicitly encodes source, range,
        // context, edition and origin in the schema-checked token transport.
        e.sequence(&self.tokens, |e, span| e.digest(Digest(span.key().0)));
        e.sequence(&self.parents, |e, parent| {
            e.digest(parent.expansion);
            e.option(&parent.invocation_identity, |e, id| e.digest(*id));
            e.option(&parent.invocation, |e, at| {
                e.digest(Digest(at.source.unit.0));
                e.digest(Digest(at.source.content.0));
                e.string(&at.source.path);
                e.u32(at.range.start);
                e.u32(at.range.end);
            });
        });
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct RenderedGeneratedOrigin {
    pub snapshot: Digest,
    pub invocation: Option<OriginLocation>,
    pub definition: Option<OriginLocation>,
    pub tokens: Vec<Option<OriginLocation>>,
    pub omitted_tokens: usize,
    pub parents: Vec<Option<OriginLocation>>,
    pub omitted_parents: usize,
}

impl OriginLocation {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        self.file.heap_bytes().checked_add(
            self.excerpt
                .as_ref()
                .map_or(0, |excerpt| excerpt.text.capacity()),
        )
    }

    pub(crate) fn write_excerpt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(excerpt) = &self.excerpt {
            write!(
                f,
                "\n      | {}{}{}",
                if excerpt.omitted_before { "…" } else { "" },
                excerpt.text,
                if excerpt.omitted_after { "…" } else { "" },
            )?;
        }
        Ok(())
    }
}

impl crate::core::digest::StableEncode for MacroOrigin {
    const DOMAIN: &'static str = "lrr/v2/macro-failure-origin";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        use crate::core::digest::Digest;
        fn location(e: &mut crate::core::digest::StableEncoder, at: &SourceOrigin) {
            e.digest(Digest(at.source.unit.0));
            e.digest(Digest(at.source.content.0));
            e.string(&at.source.path);
            e.u32(at.range.start);
            e.u32(at.range.end);
        }
        e.digest(self.invocation_identity);
        e.sequence(&self.parents, |e, parent| {
            e.digest(parent.expansion);
            e.option(&parent.invocation_identity, |e, id| e.digest(*id));
            e.option(&parent.invocation, location);
        });
        e.option(&self.invocation, location);
        e.option(&self.definition, |e, d| {
            e.digest(d.identity);
            e.string(&d.name);
            location(e, &d.location);
        });
        e.option(&self.failure_kind, |e, k| {
            e.u32(match k {
                MacroFailureKind::DefinitionSyntax => 0,
                MacroFailureKind::NoMatchingRule => 1,
                MacroFailureKind::LeftoverTokens => 2,
                MacroFailureKind::Binding => 3,
                MacroFailureKind::UnresolvedBinding => 4,
                MacroFailureKind::UnexpectedToken => 5,
                MacroFailureKind::Limit => 6,
                MacroFailureKind::UnresolvedGeneratedProducer => 7,
            })
        });
        e.option(&self.failure_span_key, |e, key| e.digest(Digest(key.0)));
        e.option(&self.failure_span, |e, s| {
            location(
                e,
                &SourceOrigin {
                    source: s.source.clone(),
                    range: SourceRange {
                        start: s.start,
                        end: s.end,
                    },
                },
            );
            e.digest(Digest(s.context.0));
            e.u32(match s.edition {
                super::token_wire::Edition::E2015 => 2015,
                super::token_wire::Edition::E2018 => 2018,
                super::token_wire::Edition::E2021 => 2021,
                super::token_wire::Edition::E2024 => 2024,
            });
            e.digest(Digest(s.origin.0));
        });
    }
}

/// Entered dispatcher frames, innermost first. Empty locations mean no captured
/// source attachment is available; they are never replaced by guessed lines.
#[derive(Debug, Clone, Serialize)]
pub struct GuestFrameOrigin {
    pub pc: Digest,
    pub body: super::spans::BodySpan,
    pub locations: Vec<SourceOrigin>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GuestStackOrigin {
    pub frames: Vec<GuestFrameOrigin>,
    pub omitted_frames: usize,
}
impl GuestStackOrigin {
    pub fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>().checked_add(
            self.frames
                .capacity()
                .checked_mul(std::mem::size_of::<GuestFrameOrigin>())?,
        )?;
        for frame in &self.frames {
            bytes = bytes.checked_add(
                frame
                    .locations
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceOrigin>())?,
            )?;
            for at in &frame.locations {
                bytes = bytes.checked_add(at.source.path.capacity())?;
            }
        }
        Some(bytes)
    }
}
impl crate::core::digest::StableEncode for GuestStackOrigin {
    const DOMAIN: &'static str = "lrr/v2/guest-stack-origin";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.sequence(&self.frames, |e, frame| {
            e.digest(frame.pc);
            e.record(&frame.body.body_key);
            e.u32(frame.body.relative_start);
            e.u32(frame.body.relative_end);
            e.sequence(&frame.locations, |e, at| {
                e.digest(Digest(at.source.unit.0));
                e.digest(Digest(at.source.content.0));
                e.string(&at.source.path);
                e.u32(at.range.start);
                e.u32(at.range.end);
            });
        });
        e.u64(self.omitted_frames as u64);
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct RenderedGuestStack {
    pub snapshot: Digest,
    pub frames: Vec<Vec<Option<OriginLocation>>>,
    pub omitted_frames: usize,
    pub omitted_locations: usize,
}

impl SourceOrigin {
    pub(crate) fn copy_with_limit(
        &self,
        max_bytes: usize,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        let limit = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "source origin copy limit".into(),
            )
        };
        if self.source.path.len() > max_bytes {
            return Err(limit());
        }
        let mut path = String::new();
        path.try_reserve_exact(self.source.path.len())
            .map_err(|_| limit())?;
        path.push_str(&self.source.path);
        if path.capacity() > max_bytes {
            return Err(limit());
        }
        Ok(Self {
            source: SourceIdentity {
                unit: self.source.unit,
                content: self.source.content,
                path,
            },
            range: self.range,
        })
    }
}
impl MacroOrigin {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>();
        if let Some(at) = &self.invocation {
            bytes = bytes.checked_add(at.source.path.capacity())?;
        }
        if let Some(definition) = &self.definition {
            bytes = bytes
                .checked_add(definition.name.capacity())?
                .checked_add(definition.location.source.path.capacity())?;
        }
        bytes = bytes.checked_add(
            self.parents
                .capacity()
                .checked_mul(std::mem::size_of::<ExpansionOrigin>())?,
        )?;
        for parent in &self.parents {
            if let Some(at) = &parent.invocation {
                bytes = bytes.checked_add(at.source.path.capacity())?;
            }
        }
        if let Some(at) = &self.failure_span {
            bytes = bytes.checked_add(at.source.path.capacity())?;
        }
        Some(bytes)
    }
}
