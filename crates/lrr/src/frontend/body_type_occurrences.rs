//! Immutable entered Type occurrence data. Namespace selection is a later query.
use super::{AdmittedBodyContext, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::dependency::InputValue,
    expand::scopes::MacroName,
    ir::pc::NodeId,
    source::manifest::BodyKey,
    syntax::{ast_adapter::range, body_index::NodeRole, spans::SourceRange, token_wire::Key},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};

/// Expansion order is distinct from a token's diagnostic origin or raw context.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BodyTypeOrigin {
    Original,
    Generated(Key),
    NeedGeneratedBoundary,
    NeedRetainedOccurrence,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BodyTypeShape {
    Named,
    NeedGenericArguments,
    NeedQualifiedType,
}
/// Exact token data, never a completed local/generic/module lookup grant.
pub(crate) struct BodyTypeIdentifier {
    name: String,
    range: SourceRange,
    raw_context: Option<Key>,
    edition: u16,
}
impl BodyTypeIdentifier {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn raw_context(&self) -> Option<Key> {
        self.raw_context
    }
    pub(crate) fn edition(&self) -> u16 {
        self.edition
    }
}
pub(crate) enum BodyTypeSegment {
    Name(BodyTypeIdentifier),
    Crate(BodyTypeIdentifier),
    SelfModule(BodyTypeIdentifier),
    Super(BodyTypeIdentifier),
    NeedQualifiedType,
}
/// A query must consume a cached genuine local/import prefix before asking the
/// current module name service. These are gaps, never positive type authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BodyTypeAuthorityDemand {
    CachedLocalPrefix,
    GeneratedBoundary,
    RetainedExpansionOccurrence,
    TypeRecipe,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BodyTypeUse {
    RecordConstructor,
    TypeSyntax,
}
pub(crate) struct BodyTypeOccurrence {
    owner: Digest,
    body: BodyKey,
    node: NodeId,
    path: SourceRange,
    segments: Vec<BodyTypeSegment>,
    absolute: bool,
    origin: BodyTypeOrigin,
    shape: BodyTypeShape,
    usage: BodyTypeUse,
}
impl BodyTypeOccurrence {
    pub(crate) fn usage(&self) -> BodyTypeUse {
        self.usage
    }
    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn path_range(&self) -> SourceRange {
        self.path
    }
    pub(crate) fn segments(&self) -> &[BodyTypeSegment] {
        &self.segments
    }
    pub(crate) fn absolute(&self) -> bool {
        self.absolute
    }
    pub(crate) fn origin(&self) -> BodyTypeOrigin {
        self.origin
    }
    pub(crate) fn shape(&self) -> BodyTypeShape {
        self.shape
    }
    pub(crate) fn next_authority_demand(&self) -> BodyTypeAuthorityDemand {
        match self.origin {
            BodyTypeOrigin::NeedGeneratedBoundary => BodyTypeAuthorityDemand::GeneratedBoundary,
            BodyTypeOrigin::NeedRetainedOccurrence => {
                BodyTypeAuthorityDemand::RetainedExpansionOccurrence
            }
            _ if self.shape != BodyTypeShape::Named => BodyTypeAuthorityDemand::TypeRecipe,
            _ => BodyTypeAuthorityDemand::CachedLocalPrefix,
        }
    }
}
pub(crate) struct PreparedBodyTypeOccurrences {
    owner: Digest,
    rows: Vec<BodyTypeOccurrence>,
    extra_bytes: usize,
    fingerprint: Digest,
}
fn resource() -> Diagnostic {
    crate::runtime::arena::resource()
}
fn add(bytes: usize, extra: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(extra)
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(resource)
}
impl PreparedBodyTypeOccurrences {
    pub(super) fn prepare(source: &AdmittedBodyContext) -> Result<Self, Diagnostic> {
        let origin = match source.source_receipt.as_deref() {
            None => BodyTypeOrigin::Original,
            Some(InputValue::GeneratedSyntax(unit)) if source.generated_boundary().is_some() => {
                BodyTypeOrigin::Generated(Key(unit.items.record().key().fingerprint().0))
            }
            Some(InputValue::GeneratedSyntax(_)) => BodyTypeOrigin::NeedGeneratedBoundary,
            Some(InputValue::RetainedSyntax(unit)) if unit.prepared().outputs().is_empty() => {
                BodyTypeOrigin::Original
            }
            Some(InputValue::RetainedSyntax(_)) => BodyTypeOrigin::NeedRetainedOccurrence,
            _ => return Err(fault("body Type occurrence source receipt differs")),
        };
        let mut rows = Vec::new();
        for node in source.index().nodes() {
            if !matches!(node.role, NodeRole::Expression(_) | NodeRole::Syntax) {
                continue;
            }
            let syntax = if node.role == NodeRole::Syntax {
                // Syntax nodes have no executable role. Pick the canonical
                // indexed location, then demand the exact PathType AST kind.
                if source
                    .index()
                    .nodes_at_range(node.range)
                    .find(|candidate| candidate.role == NodeRole::Syntax)
                    .is_none_or(|candidate| candidate.id != node.id)
                {
                    continue;
                }
                let span = SourceRange::checked(
                    source
                        .body()
                        .range
                        .start
                        .checked_add(node.range.start)
                        .ok_or_else(resource)?,
                    source
                        .body()
                        .range
                        .start
                        .checked_add(node.range.end)
                        .ok_or_else(resource)?,
                )?;
                if !source.body().range.contains(span) {
                    return Err(fault("body Type syntax escapes source owner"));
                }
                source
                    .parsed()
                    .syntax_at(span, |syntax| ast::PathType::can_cast(syntax.kind()))?
            } else {
                source
                    .parsed()
                    .entered_node_syntax(source.body(), source.index(), node.id)?
            };
            let Some(syntax) = syntax else {
                continue;
            };
            let (path, usage) = if let Some(record) = ast::RecordExpr::cast(syntax.clone()) {
                (record.path(), BodyTypeUse::RecordConstructor)
            } else if let Some(ty) = ast::PathType::cast(syntax) {
                (ty.path(), BodyTypeUse::TypeSyntax)
            } else {
                continue;
            };
            let path = path.ok_or_else(|| fault("body Type occurrence path absent"))?;
            let path_range = range(path.syntax().text_range());
            if !source.body().range.contains(path_range) {
                return Err(fault("record Type occurrence escapes body"));
            }
            let mut current = path;
            let mut segments = Vec::new();
            let mut shape = BodyTypeShape::Named;
            let absolute;
            loop {
                if segments.len() >= 256 {
                    return Err(resource());
                }
                let segment = current
                    .segment()
                    .ok_or_else(|| fault("body Type path segment absent"))?;
                if segment.generic_arg_list().is_some() && shape == BodyTypeShape::Named {
                    shape = BodyTypeShape::NeedGenericArguments;
                }
                let owned = match segment.kind() {
                    Some(ast::PathSegmentKind::Name(reference)) => {
                        let actual = range(reference.syntax().text_range());
                        let edition = source.parsed().token_edition(actual)?;
                        MacroName::parse(reference.text(), edition)?;
                        BodyTypeSegment::Name(BodyTypeIdentifier {
                            name: reference.text().to_string(),
                            range: actual,
                            raw_context: source.identifier_context(actual)?,
                            edition,
                        })
                    }
                    Some(
                        kind @ (ast::PathSegmentKind::CrateKw
                        | ast::PathSegmentKind::SelfKw
                        | ast::PathSegmentKind::SuperKw),
                    ) => {
                        let actual = range(segment.syntax().text_range());
                        let token = BodyTypeIdentifier {
                            name: segment.syntax().text().to_string(),
                            range: actual,
                            raw_context: source.identifier_context(actual)?,
                            edition: source.parsed().token_edition(actual)?,
                        };
                        match kind {
                            ast::PathSegmentKind::CrateKw => BodyTypeSegment::Crate(token),
                            ast::PathSegmentKind::SelfKw => BodyTypeSegment::SelfModule(token),
                            ast::PathSegmentKind::SuperKw => BodyTypeSegment::Super(token),
                            _ => unreachable!("matched source keyword"),
                        }
                    }
                    _ => {
                        shape = BodyTypeShape::NeedQualifiedType;
                        BodyTypeSegment::NeedQualifiedType
                    }
                };
                segments.try_reserve(1).map_err(|_| resource())?;
                segments.push(owned);
                match current.qualifier() {
                    Some(parent) => current = parent,
                    None => {
                        absolute = segment.coloncolon_token().is_some();
                        break;
                    }
                }
            }
            segments.reverse();
            rows.try_reserve(1).map_err(|_| resource())?;
            rows.push(BodyTypeOccurrence {
                owner: source.identity(),
                body: source.body().key(),
                node: node.id,
                path: path_range,
                segments,
                absolute,
                origin,
                shape,
                usage,
            });
        }
        // BodyIndex NodeIds are ordered, and each record has one actual expression role.
        if rows.windows(2).any(|pair| pair[0].node.0 >= pair[1].node.0) {
            return Err(fault("body Type occurrence order is ambiguous"));
        }
        let mut extra_bytes = rows
            .capacity()
            .checked_mul(std::mem::size_of::<BodyTypeOccurrence>())
            .ok_or_else(resource)?;
        for row in &rows {
            extra_bytes = add(
                extra_bytes,
                row.segments
                    .capacity()
                    .checked_mul(std::mem::size_of::<BodyTypeSegment>())
                    .ok_or_else(resource)?,
            )?;
            for segment in &row.segments {
                match segment {
                    BodyTypeSegment::Name(name)
                    | BodyTypeSegment::Crate(name)
                    | BodyTypeSegment::SelfModule(name)
                    | BodyTypeSegment::Super(name) => {
                        extra_bytes = add(extra_bytes, name.name.capacity())?
                    }
                    BodyTypeSegment::NeedQualifiedType => {}
                }
            }
        }
        let mut prepared = Self {
            owner: source.identity(),
            rows,
            extra_bytes,
            fingerprint: Digest([0; 32]),
        };
        prepared.fingerprint = StableEncode::fingerprint(&prepared);
        Ok(prepared)
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(super) fn select(
        &self,
        source: &AdmittedBodyContext,
        node: NodeId,
    ) -> Result<Option<&BodyTypeOccurrence>, Diagnostic> {
        if self.owner != source.identity() || source.index().nodes().get(node.0 as usize).is_none()
        {
            return Err(fault("body Type occurrence owning source or node differs"));
        }
        let selected = self
            .rows
            .binary_search_by_key(&node.0, |row| row.node.0)
            .ok()
            .map(|index| &self.rows[index]);
        if selected
            .is_some_and(|row| row.owner != source.identity() || row.body != source.body().key())
        {
            return Err(fault("body Type occurrence retained source differs"));
        }
        Ok(selected)
    }
    pub(super) fn extra_metadata_bytes(&self) -> usize {
        self.extra_bytes
    }
}

impl StableEncode for PreparedBodyTypeOccurrences {
    const DOMAIN: &'static str = "lrr/v2/prepared-body-type-occurrences";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        let identifier = |e: &mut crate::core::digest::StableEncoder, name: &BodyTypeIdentifier| {
            e.string(name.name());
            e.u32(name.range().start);
            e.u32(name.range().end);
            e.option(&name.raw_context(), |e, key| e.digest(Digest(key.0)));
            e.u16(name.edition());
        };
        e.digest(self.owner);
        e.sequence(&self.rows, |e, row| {
            e.digest(row.owner);
            e.record(row.body());
            e.u32(row.node().0);
            e.u32(row.path_range().start);
            e.u32(row.path_range().end);
            e.bool(row.absolute());
            e.u8(match row.usage() {
                BodyTypeUse::RecordConstructor => 0,
                BodyTypeUse::TypeSyntax => 1,
            });
            match row.origin() {
                BodyTypeOrigin::Original => e.u8(0),
                BodyTypeOrigin::Generated(key) => {
                    e.u8(1);
                    e.digest(Digest(key.0));
                }
                BodyTypeOrigin::NeedGeneratedBoundary => e.u8(2),
                BodyTypeOrigin::NeedRetainedOccurrence => e.u8(3),
            }
            e.u8(match row.shape() {
                BodyTypeShape::Named => 0,
                BodyTypeShape::NeedGenericArguments => 1,
                BodyTypeShape::NeedQualifiedType => 2,
            });
            e.sequence(row.segments(), |e, segment| match segment {
                BodyTypeSegment::Name(name) => {
                    e.u8(0);
                    identifier(e, name);
                }
                BodyTypeSegment::Crate(name) => {
                    e.u8(1);
                    identifier(e, name);
                }
                BodyTypeSegment::SelfModule(name) => {
                    e.u8(2);
                    identifier(e, name);
                }
                BodyTypeSegment::Super(name) => {
                    e.u8(3);
                    identifier(e, name);
                }
                BodyTypeSegment::NeedQualifiedType => e.u8(4),
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::{
            key::{OutlineKey, QueryKey},
            memo::Database,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        frontend::source_body_context::capture,
        ir::pc::Phase,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    use std::sync::Arc;
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("body Type source: {error:?}"),
            QueryPoll::NeedInput(input) => panic!("body Type source suspended: {input:?}"),
        }
    }
    #[test]
    fn unchanged_const_record_occurrences_are_owned_data_before_type_lookup() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let cfg = CfgSet::guest_default();
            let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
            let path = SourcePath::new("issue-19244.rs").unwrap();
            let unit = Digest([237; 32]);
            let selection = SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: unit,
                    module_path: vec![],
                    cfg: cfg.fingerprint(),
                },
            };
            let inputs = PreparedSourceInputs::refresh(
                None,
                vec![SourceUnitInput {
                    path,
                    crate_key: unit,
                    module_path: vec![],
                    profile,
                    cfg,
                    blob,
                }],
                Default::default(),
            )
            .unwrap();
            let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
                panic!()
            };
            let mut db = Database::default();
            db.trace.detailed = true;
            inputs.publish(&mut db).unwrap();
            let mut owners = Vec::new();
            for (declaration, expected) in [("STRUCT", "MyStruct"), ("NESTED_S", "Nested")] {
                let item = view
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == declaration)
                    .unwrap();
                let body = item.body.as_ref().unwrap();
                let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
                let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
                    panic!()
                };
                let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
                let admitted = Arc::new(ready(capture(&mut db, &[&view], index, &pc)));
                let prepared = ready(Arc::clone(&admitted).prepare(&mut db));
                let occurrence = ready(prepared.body_type_occurrence(&mut db, NodeId(0))).unwrap();
                assert_eq!(occurrence.body(), &body.key());
                assert_eq!(occurrence.node(), NodeId(0));
                assert!(!occurrence.absolute());
                assert_eq!(occurrence.shape(), BodyTypeShape::Named);
                assert_eq!(occurrence.origin(), BodyTypeOrigin::Original);
                assert_eq!(
                    occurrence.next_authority_demand(),
                    BodyTypeAuthorityDemand::CachedLocalPrefix
                );
                assert!(body.range.contains(occurrence.path_range()));
                let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
                    panic!()
                };
                assert_eq!(name.name(), expected);
                assert_eq!(name.edition(), edition);
                assert_eq!(name.raw_context(), None);
                assert_eq!(
                    &ORIGINAL[name.range().start as usize..name.range().end as usize],
                    expected
                );
                assert!(Arc::ptr_eq(admitted.index(), prepared.source().index()));
                let (root, hygiene) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
                let parent = crate::frontend::source_macros::PublishedMacroIndex::from_view(
                    view.share(),
                    root,
                    hygiene,
                )
                .unwrap();
                let local = ready(
                    crate::frontend::source_locals::PreparedSourceLocalPlan::prepare(
                        &mut db,
                        &parent,
                        Arc::clone(&prepared),
                    ),
                );
                let prefix = ready(local.type_prefix(&mut db, NodeId(0)));
                assert!(matches!(prefix.selection(),crate::frontend::source_locals::owned_plan::LocalTypePrefixSelection::ModuleTail));
                assert_eq!(prefix.node(), Some(NodeId(0)));
                let actual = prefix.use_name().unwrap();
                assert_eq!(actual.name(), expected);
                assert_eq!(actual.edition(), edition);
                assert_eq!(actual.range(), name.range());
                assert_eq!(prefix.fingerprint(), StableEncode::fingerprint(&prefix));
                assert!(local.module_scope().is_some());
                assert!(Arc::ptr_eq(prefix.retained_plan(), &local));
                assert!(prefix.exclusive_metadata_bytes().unwrap() > 0);
                assert!(matches!(
                    local.type_prefix(&mut db, NodeId(u32::MAX)),
                    QueryPoll::Ready(Err(_))
                ));
                owners.push((admitted, prepared));
            }
            assert!(
                owners[0]
                    .1
                    .body_type_occurrences()
                    .select(&owners[1].0, NodeId(0))
                    .is_err()
            );
            assert!(
                owners[0]
                    .1
                    .body_type_occurrences()
                    .select(&owners[0].0, NodeId(u32::MAX))
                    .is_err()
            );
            assert_eq!(
                owners[0].1.body_type_occurrences().fingerprint(),
                StableEncode::fingerprint(owners[0].1.body_type_occurrences())
            );
            let (root, hygiene) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
            let parent = crate::frontend::source_macros::PublishedMacroIndex::from_view(
                view.share(),
                root,
                hygiene,
            )
            .unwrap();
            let local = ready(
                crate::frontend::source_locals::PreparedSourceLocalPlan::prepare(
                    &mut db,
                    &parent,
                    Arc::clone(&owners[0].1),
                ),
            );
            let mut replacement = view.prepared_inputs().clone();
            let mut header = owners[0].0.header().clone();
            header.profile.edition = if edition == 2024 { 2021 } else { 2024 };
            replacement.insert(
                crate::db::dependency::InputKey::DefinitionHeader(owners[0].0.body().def_key),
                InputValue::DefinitionHeader(header),
            );
            db.publish_inputs(replacement).unwrap();
            assert!(matches!(
                owners[0].1.body_type_occurrence(&mut db, NodeId(0)),
                QueryPoll::Ready(Err(_))
            ));
            assert!(matches!(
                local.type_prefix(&mut db, NodeId(0)),
                QueryPoll::Ready(Err(_))
            ));
            assert!(
                !db.trace.events.iter().any(|event| matches!(
                    event.key,
                    QueryKey::Thunk(_)
                        | QueryKey::Constant(_)
                        | QueryKey::FunctionSignature(_)
                        | QueryKey::TypeSchema(_)
                )),
                "exact occurrence extraction must not lower or resolve cold source"
            );
        }
        // This is source authority data, not a const/main/runtime fixture pass.
    }
    #[test]
    fn unchanged_generic_body_type_uses_actual_declaration_ordinal_and_token() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-outlives-projection-hrtype.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let unit = Digest([239; 32]);
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let cfg = CfgSet::guest_default();
            let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
            let path = SourcePath::new("regions-outlives-projection-hrtype.rs").unwrap();
            let selection = SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: unit,
                    module_path: vec![],
                    cfg: cfg.fingerprint(),
                },
            };
            let inputs = PreparedSourceInputs::refresh(
                None,
                vec![SourceUnitInput {
                    path,
                    crate_key: unit,
                    module_path: vec![],
                    profile,
                    cfg,
                    blob,
                }],
                Default::default(),
            )
            .unwrap();
            let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
                panic!()
            };
            let mut db = Database::default();
            db.trace.detailed = true;
            inputs.publish(&mut db).unwrap();
            let item = view
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "foo")
                .unwrap();
            let body = item.body.as_ref().unwrap();
            let value = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
            let QueryValue::BodyStructure(index) = value.value.as_ref() else {
                panic!()
            };
            let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
            let admitted = Arc::new(ready(capture(&mut db, &[&view], index, &pc)));
            let prepared = ready(admitted.prepare(&mut db));
            let (root, hygiene) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
            let parent = crate::frontend::source_macros::PublishedMacroIndex::from_view(
                view.share(),
                root,
                hygiene,
            )
            .unwrap();
            let plan = ready(
                crate::frontend::source_locals::PreparedSourceLocalPlan::prepare(
                    &mut db,
                    &parent,
                    Arc::clone(&prepared),
                ),
            );
            let mut selected = 0;
            for node in index.nodes() {
                let Some(occurrence) = ready(prepared.body_type_occurrence(&mut db, node.id))
                else {
                    continue;
                };
                let [BodyTypeSegment::Name(identifier)] = occurrence.segments() else {
                    continue;
                };
                if identifier.name() != "T" {
                    continue;
                }
                assert_eq!(occurrence.usage(), BodyTypeUse::TypeSyntax);
                let answer = ready(plan.type_prefix(&mut db, node.id));
                let crate::frontend::source_locals::owned_plan::LocalTypePrefixSelection::Parameter(
                    parameter,
                ) = answer.selection()
                else {
                    panic!("actual generic occurrence did not select its declaration")
                };
                assert_eq!(parameter.definition, body.def_key);
                assert_eq!(parameter.ordinal, 2);
                selected += 1;
            }
            assert!(
                selected > 0,
                "unchanged body must retain its actual generic Type syntax"
            );
            assert!(!db.trace.events.iter().any(|event| matches!(
                event.key,
                QueryKey::Thunk(_) | QueryKey::Constant(_) | QueryKey::TypeSchema(_)
            )));
        }
        // Source authority only; the check-pass original and main are unexecuted.
    }
}
