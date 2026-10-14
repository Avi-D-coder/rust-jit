//! Exact generated type sites from pinned owned syntax, never printed-source reparsing.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        generated_items::GeneratedItems, identity::GeneratedItemKey, record::ExpansionRecord,
    },
    source::snapshot::CfgSet,
    syntax::{
        ast_adapter::{range, semantic_name},
        outline::ScopeKey,
        parse::ParsedFile,
        spans::SourceRange,
        token_wire::{SpanRecord, TokenTree},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
use std::sync::Arc;
const MAX_BYTES: usize = 32 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERATED-TYPE-SITE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated written type budget".into(),
    )
}
fn charge(bytes: &mut usize, n: usize, maximum: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= maximum)
        .ok_or_else(limit)?;
    Ok(())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GeneratedTypeRole {
    Declaration,
    ArrayElement,
    AliasTarget,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TypeKeyword {
    Crate,
    SelfModule,
    SuperModule,
    DollarCrate,
}
pub(crate) enum GeneratedTypeSegmentKind {
    Name(String),
    Keyword(TypeKeyword),
}
pub(crate) struct GeneratedTypeSegment {
    kind: GeneratedTypeSegmentKind,
    range: SourceRange,
    span: SpanRecord,
}
impl GeneratedTypeSegment {
    pub(crate) fn kind(&self) -> &GeneratedTypeSegmentKind {
        &self.kind
    }
    pub(crate) fn span(&self) -> &SpanRecord {
        &self.span
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
}
pub(crate) enum GeneratedTypeSyntax {
    Unit,
    Path(Vec<GeneratedTypeSegment>),
    GenericOrProjection,
    AbsolutePath,
    Unsupported,
}
pub(crate) struct WrittenGeneratedType {
    record: Arc<ExpansionRecord>,
    member: GeneratedItemKey,
    scope: ScopeKey,
    role: GeneratedTypeRole,
    declaration: SourceRange,
    declaration_fingerprint: Digest,
    header_fingerprint: Digest,
    range: SourceRange,
    shape: GeneratedTypeSyntax,
    bytes: usize,
}
fn actual_dollar_token(
    nodes: &[TokenTree],
    wanted: crate::syntax::token_wire::Key,
    depth: usize,
) -> Result<bool, Diagnostic> {
    if depth > 256 {
        return Err(limit());
    }
    for node in nodes {
        match node {
            TokenTree::Ident { text, raw, span }
                if *span == wanted && !*raw && text == "$crate" =>
            {
                return Ok(true);
            }
            TokenTree::Group { children, .. }
                if actual_dollar_token(children, wanted, depth + 1)? =>
            {
                return Ok(true);
            }
            _ => {}
        }
    }
    Ok(false)
}
impl WrittenGeneratedType {
    pub(crate) fn capture(
        items: &GeneratedItems,
        owner: &ScopeKey,
        cfg: &CfgSet,
        member: &GeneratedItemKey,
        role: GeneratedTypeRole,
        maximum: usize,
    ) -> Result<Self, Diagnostic> {
        if maximum > MAX_BYTES || maximum < std::mem::size_of::<Self>() {
            return Err(limit());
        }
        member.validate()?;
        let header = items
            .items()
            .iter()
            .find(|h| h.key == *member)
            .ok_or_else(|| fault("written type actual generated member absent"))?;
        let mut bytes = std::mem::size_of::<Self>();
        charge(&mut bytes, items.metadata_bytes()?, maximum)?;
        charge(&mut bytes, owner.heap_bytes(), maximum)?;
        charge(
            &mut bytes,
            member
                .structural_path
                .len()
                .checked_mul(4)
                .ok_or_else(limit)?,
            maximum,
        )?;
        let transient = items
            .text()
            .len()
            .checked_mul(128)
            .and_then(|n| n.checked_add(items.syntax_events().len().checked_mul(128)?))
            .and_then(|n| n.checked_add(items.items().len().checked_mul(4096)?))
            .ok_or_else(limit)?;
        charge(&mut bytes, transient, maximum)?;
        let parsed = ParsedFile::from_generated_items(items)?;
        let outline = parsed.outline(owner.crate_key, owner.module_path.clone(), cfg)?;
        if parsed
            .estimated_bytes()
            .checked_add(outline.estimated_bytes())
            .is_none_or(|n| n > transient)
        {
            return Err(limit());
        }
        let item = outline
            .items
            .iter()
            .find(|i| {
                i.declaration_span.start == header.start && i.declaration_span.end == header.end
            })
            .ok_or_else(|| fault("written type exact generated declaration absent"))?;
        let syntax = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find(|n| {
                range(n.text_range()) == item.declaration_span
                    && (ast::Const::can_cast(n.kind()) || ast::TypeAlias::can_cast(n.kind()))
            })
            .ok_or_else(|| fault("written type declaration syntax prerequisite"))?;
        let generic_owner = syntax.children().any(|n| {
            ast::GenericParamList::can_cast(n.kind()) || ast::WhereClause::can_cast(n.kind())
        });
        let ty = match role {
            GeneratedTypeRole::Declaration | GeneratedTypeRole::ArrayElement => {
                ast::Const::cast(syntax).and_then(|c| c.ty())
            }
            GeneratedTypeRole::AliasTarget => ast::TypeAlias::cast(syntax).and_then(|a| a.ty()),
        }
        .ok_or_else(|| fault("written type exact owning role absent"))?;
        let ty = if role == GeneratedTypeRole::ArrayElement {
            let ast::Type::ArrayType(array) = ty else {
                return Err(fault("written type is not an array element demand"));
            };
            array
                .ty()
                .ok_or_else(|| fault("written array element absent"))?
        } else {
            ty
        };
        let site = range(ty.syntax().text_range());
        if !item.declaration_span.contains(site) {
            return Err(fault("written type escapes actual declaration"));
        }
        let shape = if generic_owner {
            GeneratedTypeSyntax::GenericOrProjection
        } else {
            match ty {
                ast::Type::TupleType(tuple) if tuple.fields().next().is_none() => {
                    GeneratedTypeSyntax::Unit
                }
                ast::Type::PathType(path) => {
                    let mut cursor = path
                        .path()
                        .ok_or_else(|| fault("written generated path absent"))?;
                    let mut segments = Vec::new();
                    let mut unsupported = false;
                    let mut absolute = false;
                    loop {
                        if segments.len() == 256 {
                            return Err(limit());
                        }
                        let part = cursor
                            .segment()
                            .ok_or_else(|| fault("written path segment absent"))?;
                        if part.generic_arg_list().is_some()
                            || part.parenthesized_arg_list().is_some()
                            || part.ret_type().is_some()
                            || matches!(
                                part.kind(),
                                Some(ast::PathSegmentKind::Type { .. })
                                    | Some(ast::PathSegmentKind::SelfTypeKw)
                                    | None
                            )
                        {
                            unsupported = true;
                            break;
                        }
                        let segment_range = range(part.syntax().text_range());
                        charge(
                            &mut bytes,
                            std::mem::size_of::<GeneratedTypeSegment>()
                                .checked_add(
                                    64usize
                                        .checked_mul(std::mem::size_of::<&SpanRecord>())
                                        .ok_or_else(limit)?,
                                )
                                .ok_or_else(limit)?,
                            maximum,
                        )?;
                        let projected = items.project_source_spans(segment_range)?;
                        if projected.len() != 1 {
                            return Err(fault(
                                "generated path segment lacks one exact token origin",
                            ));
                        }
                        let span = projected[0];
                        charge(&mut bytes, span.source.path.capacity(), maximum)?;
                        let kind = match part.kind() {
                            Some(ast::PathSegmentKind::Name(name)) => {
                                let raw = name.syntax().text().to_string();
                                charge(&mut bytes, raw.capacity(), maximum)?;
                                if raw == "$crate" {
                                    if !actual_dollar_token(items.tokens().roots(), span.key(), 0)?
                                    {
                                        return Err(fault(
                                            "printed dollar-crate segment lacks actual retained special token",
                                        ));
                                    }
                                    GeneratedTypeSegmentKind::Keyword(TypeKeyword::DollarCrate)
                                } else {
                                    let normalized = semantic_name(&raw).ok_or_else(|| {
                                        fault("written type semantic identifier absent")
                                    })?;
                                    charge(&mut bytes, normalized.capacity(), maximum)?;
                                    GeneratedTypeSegmentKind::Name(normalized)
                                }
                            }
                            Some(ast::PathSegmentKind::CrateKw) => {
                                GeneratedTypeSegmentKind::Keyword(TypeKeyword::Crate)
                            }
                            Some(ast::PathSegmentKind::SelfKw) => {
                                GeneratedTypeSegmentKind::Keyword(TypeKeyword::SelfModule)
                            }
                            Some(ast::PathSegmentKind::SuperKw) => {
                                GeneratedTypeSegmentKind::Keyword(TypeKeyword::SuperModule)
                            }
                            _ => {
                                unsupported = true;
                                GeneratedTypeSegmentKind::Name(String::new())
                            }
                        };
                        segments.try_reserve_exact(1).map_err(|_| limit())?;
                        segments.push(GeneratedTypeSegment {
                            kind,
                            range: segment_range,
                            span: span.clone(),
                        });
                        if let Some(parent) = cursor.qualifier() {
                            cursor = parent;
                        } else {
                            absolute = cursor.colon_colon_token().is_some();
                            break;
                        }
                    }
                    segments.reverse();
                    if unsupported {
                        GeneratedTypeSyntax::GenericOrProjection
                    } else if absolute {
                        GeneratedTypeSyntax::AbsolutePath
                    } else {
                        GeneratedTypeSyntax::Path(segments)
                    }
                }
                _ => GeneratedTypeSyntax::Unsupported,
            }
        };
        charge(&mut bytes, item.scope.heap_bytes(), maximum)?;
        Ok(Self {
            record: items.record_input(),
            member: member.clone(),
            scope: item.scope.clone(),
            role,
            declaration: item.declaration_span,
            declaration_fingerprint: item.def_key.fingerprint(),
            header_fingerprint: item.header.fingerprint(),
            range: site,
            shape,
            bytes,
        })
    }
    pub(crate) fn shape(&self) -> &GeneratedTypeSyntax {
        &self.shape
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn member(&self) -> &GeneratedItemKey {
        &self.member
    }
    pub(crate) fn record(&self) -> &ExpansionRecord {
        &self.record
    }
    pub(crate) fn role(&self) -> GeneratedTypeRole {
        self.role
    }
    pub(crate) fn declaration(&self) -> SourceRange {
        self.declaration
    }
    pub(crate) fn declaration_fingerprint(&self) -> Digest {
        self.declaration_fingerprint
    }
    pub(crate) fn header_fingerprint(&self) -> Digest {
        self.header_fingerprint
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/generated-written-type");
        e.record(&self.record.descriptor());
        e.record(&self.member);
        e.record(&self.scope);
        e.u8(match self.role {
            GeneratedTypeRole::Declaration => 0,
            GeneratedTypeRole::ArrayElement => 1,
            GeneratedTypeRole::AliasTarget => 2,
        });
        e.u32(self.declaration.start);
        e.u32(self.declaration.end);
        e.digest(self.declaration_fingerprint);
        e.digest(self.header_fingerprint);
        e.u32(self.range.start);
        e.u32(self.range.end);
        match &self.shape {
            GeneratedTypeSyntax::Unit => e.u8(0),
            GeneratedTypeSyntax::GenericOrProjection => e.u8(2),
            GeneratedTypeSyntax::Unsupported => e.u8(3),
            GeneratedTypeSyntax::AbsolutePath => e.u8(4),
            GeneratedTypeSyntax::Path(parts) => {
                e.u8(1);
                e.sequence(parts, |e, p| {
                    match &p.kind {
                        GeneratedTypeSegmentKind::Name(n) => {
                            e.u8(0);
                            e.string(n)
                        }
                        GeneratedTypeSegmentKind::Keyword(k) => {
                            e.u8(1);
                            e.u8(match k {
                                TypeKeyword::Crate => 0,
                                TypeKeyword::SelfModule => 1,
                                TypeKeyword::SuperModule => 2,
                                TypeKeyword::DollarCrate => 3,
                            })
                        }
                    };
                    e.u32(p.range.start);
                    e.u32(p.range.end);
                    e.digest(Digest(p.span.key().0));
                });
            }
        }
        e.finish()
    }
}
