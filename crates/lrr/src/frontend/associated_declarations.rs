//! Original associated declaration facts joined to their actual captured Outline.
//! This is not member selection, catalog completeness, currentness or call entry.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::dependency::{InputKey, InputValue},
    expand::identity::GrammarSlot,
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::{
        declaration_scan::{
            AssociatedDeclarationHeader, AssociatedDeclarationKind, AssociatedDeclarationOwner,
            DeclarationProducer, DeclarationProducerKind, DeclarationScan, ScannedImpl,
            ScannedTrait,
        },
        outline::ItemOutline,
        spans::SourceRange,
    },
};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

#[path = "declaration_impl_owners.rs"]
mod local_impl_owners;
use local_impl_owners::{LocalDeclarationLocation, LocalImplOwners};

#[derive(Clone, Copy)]
enum CanonicalLocation {
    Outline(u32),
    Local(LocalDeclarationLocation),
}
fn canonical_item<'a>(
    source: &'a PreparedSourceView,
    local: &'a LocalImplOwners,
    location: CanonicalLocation,
) -> &'a ItemOutline {
    match location {
        CanonicalLocation::Outline(index) => &source.outline().items[index as usize],
        CanonicalLocation::Local(location) => local.item(location),
    }
}

pub(crate) enum AssociatedDeclarationsPreparation<'a> {
    Original(PreparedOriginalAssociatedDeclarations<'a>),
    /// A generated grammar/root is not proof of its enclosing trait/impl.
    NeedGeneratedParent,
}
#[derive(Clone, Copy)]
enum OwnerKind {
    Trait,
    Impl,
}
#[derive(Clone, Copy)]
struct OwnerLocation {
    identity: Digest,
    kind: OwnerKind,
    scan: u32,
    canonical: Option<CanonicalLocation>,
    member_owner: Option<Digest>,
}
#[derive(Clone, Copy)]
enum SlotTarget {
    Declaration(CanonicalLocation),
    Producer(u32),
    NeedCanonicalOwner,
}
#[derive(Clone, Copy)]
struct SlotLocation {
    identity: Digest,
    owner: u32,
    associated: u32,
    target: SlotTarget,
}
/// The borrowed captured source is the existing immutable source root. New rows
/// retain one actual scan Arc; no source tree, header or body buffer is cloned.
/// Consumers must obtain their own current dependency guard before publication/use.
pub(crate) struct PreparedOriginalAssociatedDeclarations<'a> {
    source: &'a PreparedSourceView,
    scan: Arc<DeclarationScan>,
    local: LocalImplOwners,
    owners: Vec<OwnerLocation>,
    slots: Vec<SlotLocation>,
    producer_slots: Vec<(Digest, u32)>,
    fingerprint: Digest,
}
pub(crate) enum AssociatedOwnerSource<'a> {
    Trait(&'a ScannedTrait),
    Impl(&'a ScannedImpl),
}
pub(crate) struct AssociatedOwnerHeader<'a> {
    pub(crate) source: AssociatedOwnerSource<'a>,
    /// None is an unfinished canonical attachment (for example a cold local
    /// impl absent from the module Outline), never absence of an impl/member.
    pub(crate) canonical: Option<&'a ItemOutline>,
    /// Actual Outline child-scope identity, prepared once for this owner.
    pub(crate) member_owner: Option<Digest>,
}
/// Actual captured lexical declaration scope, retaining its canonical plan.
/// This grants metadata publication only, never executable body/name authority.
/// Fields are private: only a checked canonical local owner can supply it.
pub(crate) struct CapturedDeclarationScope<'a> {
    source: &'a PreparedSourceView,
    parent: &'a ItemOutline,
    plan: &'a Arc<crate::syntax::declaration_scope_plan::DeclarationScopePlan>,
}
/// Owned retained declaration metadata, never an entered executable context.
pub(crate) struct RetainedDeclarationScope {
    source: PreparedSourceView,
    parent: ItemOutline,
    plan: Arc<crate::syntax::declaration_scope_plan::DeclarationScopePlan>,
}
impl RetainedDeclarationScope {
    pub(crate) fn item_at(&self, range: SourceRange) -> Option<&crate::syntax::body_items::BodyItem> {
        self.plan.item_at(range)
    }
    pub(crate) fn captured(&self) -> CapturedDeclarationScope<'_> {
        CapturedDeclarationScope { source: &self.source, parent: &self.parent, plan: &self.plan }
    }
}
impl CapturedDeclarationScope<'_> {
    pub(crate) fn fingerprint(&self) -> Digest { self.plan.fingerprint() }
    pub(crate) fn item_at(&self, range: SourceRange) -> Option<&crate::syntax::body_items::BodyItem> {
        self.plan.item_at(range)
    }
    pub(crate) fn retain(&self) -> RetainedDeclarationScope {
        RetainedDeclarationScope { source: self.source.share(), parent: self.parent.clone(), plan: Arc::clone(self.plan) }
    }

    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    pub(crate) fn body(&self) -> &crate::syntax::outline::BodyHandle {
        self.plan.body()
    }
    pub(crate) fn scope(&self) -> &crate::syntax::outline::ScopeKey {
        self.plan.scope()
    }
    pub(crate) fn facts(&self) -> &crate::syntax::entered_macros::DeclarationFacts {
        self.plan.declarations()
    }
    pub(crate) fn parsed(&self) -> Result<&crate::syntax::parse::ParsedFile, Diagnostic> {
        Ok(self.source.retained_declaration_parser()?.as_ref())
    }
    /// Cold coordinator check of this exact captured parent. No namespace
    /// census, body preparation, producer execution or substitute parser.
    pub(crate) fn require_current(&self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        publication(self.source, self.parent)?;
        let parent = self.parent.def_key.fingerprint();
        let current = db
            .prepared_source(self.source.selection())?
            .ok_or_else(|| fault("cold declaration source publication absent"))?;
        if !self.source.same_publication(&current)
            || self.parent.body.as_ref() != Some(self.plan.body())
            || self.parent.scope != *self.plan.scope()
            || self.parsed()?.key() != *self.plan.parse()
            || !matches!(db.input(&InputKey::DefinitionSource(parent)).as_deref(),
                Some(InputValue::DefinitionSource(parse)) if parse == self.plan.parse())
            || !matches!(db.input(&InputKey::DefinitionHeader(parent)).as_deref(),
                Some(InputValue::DefinitionHeader(header)) if header == &self.parent.header)
            || !matches!(db.input(&InputKey::DefinitionBody(parent)).as_deref(),
                Some(InputValue::DefinitionBody(body)) if body == self.plan.body())
            || !matches!(db.input(&InputKey::DeclarationContext(parent)).as_deref(),
                Some(InputValue::DeclarationContext(context))
                    if context.scope == self.parent.scope && context.range == self.parent.declaration_span)
            || !matches!(db.input(&InputKey::CrateCfg(self.parent.scope.crate_key)).as_deref(),
                Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint() == self.plan.cfg())
        {
            return Err(fault("cold declaration owning publication changed"));
        }
        Ok(())
    }
}
pub(crate) enum OriginalAssociatedTarget<'a> {
    Declaration(&'a ItemOutline),
    Producer(&'a DeclarationProducer),
    NeedCanonicalOwner,
}
pub(crate) struct OriginalAssociatedSlot<'a> {
    pub(crate) source: &'a AssociatedDeclarationHeader,
    pub(crate) owner: AssociatedOwnerHeader<'a>,
    pub(crate) target: OriginalAssociatedTarget<'a>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ASSOCIATED-DECLARATION", message.into())
}
fn resource() -> Diagnostic {
    crate::runtime::arena::resource()
}
fn key(range: SourceRange) -> (u32, u32) {
    (range.start, range.end)
}
fn reserve<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|_| resource())?;
    Ok(rows)
}
fn publication(source: &PreparedSourceView, item: &ItemOutline) -> Result<(), Diagnostic> {
    let id = item.def_key.fingerprint();
    let inputs = source.prepared_inputs();
    if !matches!(inputs.get(&InputKey::DefinitionSource(id)),Some(InputValue::DefinitionSource(parse)) if parse == &source.selection().outline.parse)
        || !matches!(inputs.get(&InputKey::DefinitionHeader(id)),Some(InputValue::DefinitionHeader(header)) if header == &item.header)
        || !matches!(inputs.get(&InputKey::DeclarationContext(id)),Some(InputValue::DeclarationContext(context)) if context.scope == item.scope && context.range == item.declaration_span)
        || item.header.profile != source.selection().outline.parse.profile
        || item.def_key.crate_key != source.selection().outline.crate_key
        || item.def_key.module_path != item.scope.module_path
        || item.scope.crate_key != item.def_key.crate_key
    {
        return Err(fault("captured canonical declaration inputs differ"));
    }
    match (&item.body, inputs.get(&InputKey::DefinitionBody(id))) {
        (Some(body), Some(InputValue::DefinitionBody(actual)))
            if body == actual && body.def_key == id => {}
        (None, None) => {}
        _ => return Err(fault("captured canonical body attachment differs")),
    }
    Ok(())
}
fn at_span(rows: &[((u32, u32), u32)], range: SourceRange) -> Result<Option<u32>, Diagnostic> {
    let selected = key(range);
    let start = rows.partition_point(|(range, _)| *range < selected);
    let Some((actual, index)) = rows.get(start) else {
        return Ok(None);
    };
    if *actual != selected {
        return Ok(None);
    }
    if rows
        .get(start + 1)
        .is_some_and(|(range, _)| *range == selected)
    {
        return Err(fault("canonical declaration span has multiple owners"));
    }
    Ok(Some(*index))
}
impl<'a> PreparedOriginalAssociatedDeclarations<'a> {
    pub(crate) fn prepare(
        source: &'a PreparedSourceView,
    ) -> Result<AssociatedDeclarationsPreparation<'a>, Diagnostic> {
        if source
            .retained_declaration_parser()?
            .generated_origin
            .is_some()
        {
            return Ok(AssociatedDeclarationsPreparation::NeedGeneratedParent);
        }
        let scan = source.declaration_inventory(Default::default())?;
        Ok(AssociatedDeclarationsPreparation::Original(
            Self::from_scan(source, scan)?,
        ))
    }
    fn from_scan(
        source: &'a PreparedSourceView,
        scan: Arc<DeclarationScan>,
    ) -> Result<Self, Diagnostic> {
        source.require_declaration_syntax()?;
        let selection = source.selection();
        let outline = source.outline();
        if scan.generated_origin().is_some()
            || source
                .retained_declaration_parser()?
                .generated_origin
                .is_some()
            || scan.parse() != &selection.outline.parse
            || scan.path() != &selection.path
            || scan.cfg() != selection.outline.cfg
            || scan.scope() != &outline.root_scope
            || outline.cfg != scan.cfg()
            || outline.profile != scan.parse().profile
            || outline.content != scan.parse().content
            || !scan.associated_fragments().is_empty()
        {
            return Err(fault("original captured scan/source owner differs"));
        }
        let local = LocalImplOwners::prepare(source, &scan)?;
        let count = scan
            .traits()
            .len()
            .checked_add(scan.impls().len())
            .ok_or_else(resource)?;
        let slot_count = scan
            .traits()
            .iter()
            .map(|r| r.associated.len())
            .chain(scan.impls().iter().map(|r| r.associated.len()))
            .try_fold(0usize, |n, m| n.checked_add(m).ok_or_else(resource))?;
        let mut spans = reserve::<((u32, u32), u32)>(outline.items.len())?;
        for (i, item) in outline.items.iter().enumerate() {
            spans.push((
                key(item.declaration_span),
                u32::try_from(i).map_err(|_| resource())?,
            ));
        }
        spans.sort_unstable_by_key(|row| row.0);
        let mut producers = reserve::<(Digest, u32)>(scan.producers().len())?;
        for (i, row) in scan.producers().iter().enumerate() {
            producers.push((row.identity, u32::try_from(i).map_err(|_| resource())?));
        }
        producers.sort_unstable_by_key(|row| row.0);
        if producers.windows(2).any(|r| r[0].0 == r[1].0) {
            return Err(fault("associated producer identity is duplicated"));
        }
        let mut owners = reserve::<OwnerLocation>(count)?;
        for (kind, index, identity, range, header, scope) in
            scan.traits()
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        OwnerKind::Trait,
                        i,
                        r.identity,
                        r.range,
                        &r.header,
                        &r.scope,
                    )
                })
                .chain(
                    scan.impls().iter().enumerate().map(|(i, r)| {
                        (OwnerKind::Impl, i, r.identity, r.range, &r.header, &r.scope)
                    }),
                )
        {
            let actual = at_span(&spans, range)?;
            let mut canonical = actual.map(CanonicalLocation::Outline);
            let mut member_owner = None;
            if let Some(index) = actual {
                let item = &outline.items[index as usize];
                let expected = match kind {
                    OwnerKind::Trait => ItemKind::Trait,
                    OwnerKind::Impl => ItemKind::Impl,
                };
                if item.def_key.item_kind != expected
                    || item.def_key.namespace != Namespace::Type
                    || item.scope != *scope
                    || item.header.tokens != *header
                {
                    return Err(fault("original associated owner canonical header differs"));
                }
                publication(source, item)?;
                member_owner = Some(match kind {
                    OwnerKind::Trait => item.def_key.fingerprint(),
                    OwnerKind::Impl => item
                        .def_key
                        .enclosing_impl
                        .ok_or_else(|| fault("original associated impl identity is absent"))?,
                });
            }
            if canonical.is_none()
                && matches!(kind, OwnerKind::Impl)
                && let Some(location) = local.owner(range)
            {
                member_owner = local.item(location).def_key.enclosing_impl;
                canonical = Some(CanonicalLocation::Local(location));
            }
            owners.push(OwnerLocation {
                identity,
                kind,
                scan: u32::try_from(index).map_err(|_| resource())?,
                canonical,
                member_owner,
            });
        }
        owners.sort_unstable_by_key(|r| r.identity);
        if owners.windows(2).any(|r| r[0].identity == r[1].identity) {
            return Err(fault("associated owner source identity is duplicated"));
        }
        let mut slots = reserve::<SlotLocation>(slot_count)?;
        for (ordinal, owner) in owners.iter().enumerate() {
            let associated = match owner.kind {
                OwnerKind::Trait => &scan.traits()[owner.scan as usize].associated,
                OwnerKind::Impl => &scan.impls()[owner.scan as usize].associated,
            };
            for (member, row) in associated.iter().enumerate() {
                let expected = match owner.kind {
                    OwnerKind::Trait => AssociatedDeclarationOwner::Trait(owner.identity),
                    OwnerKind::Impl => AssociatedDeclarationOwner::Impl(owner.identity),
                };
                if row.owner != expected {
                    return Err(fault("associated source entry enclosing owner differs"));
                }
                let target = if let Some(canonical) = owner.canonical {
                    let parent = canonical_item(source, &local, canonical);
                    let child_owner = owner
                        .member_owner
                        .ok_or_else(|| fault("canonical associated owner child scope absent"))?;
                    if row.kind == AssociatedDeclarationKind::MacroProducer {
                        let producer = row
                            .producer
                            .ok_or_else(|| fault("associated macro producer attachment absent"))?;
                        let index = producers
                            .binary_search_by_key(&producer, |r| r.0)
                            .ok()
                            .map(|i| producers[i].1)
                            .ok_or_else(|| fault("associated macro producer source absent"))?;
                        let actual = &scan.producers()[index as usize];
                        let grammar = match owner.kind {
                            OwnerKind::Trait => GrammarSlot::TraitItems,
                            OwnerKind::Impl => GrammarSlot::ImplItems,
                        };
                        if actual.kind != DeclarationProducerKind::Macro
                            || actual.range != row.range
                            || actual.lexical_scope != row.lexical_scope
                            || actual.grammar != grammar
                        {
                            return Err(fault(
                                "associated macro producer source attachment differs",
                            ));
                        }
                        SlotTarget::Producer(index)
                    } else {
                        let location = match canonical {
                            CanonicalLocation::Outline(_) => Some(CanonicalLocation::Outline(
                                at_span(&spans, row.range)?.ok_or_else(|| {
                                    fault("original associated declaration canonical row absent")
                                })?,
                            )),
                            CanonicalLocation::Local(owner) => {
                                local.member(owner, row.range).map(CanonicalLocation::Local)
                            }
                        };
                        if let Some(location) = location {
                            let item = canonical_item(source, &local, location);
                            let (kind, namespace) = match row.kind {
                                AssociatedDeclarationKind::Function => {
                                    (ItemKind::Function, Namespace::Value)
                                }
                                AssociatedDeclarationKind::Const => {
                                    (ItemKind::Constant, Namespace::Value)
                                }
                                AssociatedDeclarationKind::TypeAlias => {
                                    (ItemKind::TypeAlias, Namespace::Type)
                                }
                                AssociatedDeclarationKind::MacroProducer => unreachable!(),
                            };
                            let name_matches = match &row.name {
                                Some(name) => {
                                    name.edition == item.header.profile.edition
                                        && row.range.contains(name.range)
                                        && name
                                            .spelling
                                            .strip_prefix("r#")
                                            .unwrap_or(&name.spelling)
                                            .nfc()
                                            .eq(item.def_key.declared_name.chars())
                                }
                                // An actual unnamed constant is a declaration row,
                                // not an invented member callable by an empty name.
                                None => {
                                    row.kind == AssociatedDeclarationKind::Const
                                        && item.def_key.declared_name.is_empty()
                                }
                            };
                            if item.def_key.item_kind != kind
                                || item.def_key.namespace != namespace
                                || item.scope.owner != Some(child_owner)
                                || item.def_key.enclosing_impl != Some(child_owner)
                                || item.scope.crate_key != parent.scope.crate_key
                                || item.scope.module_path != parent.scope.module_path
                                || item.header.context != parent.header.tokens
                                || item.header.tokens != row.header
                                || row.producer.is_some()
                                || item.body.as_ref().map(|b| b.range) != row.body
                                || !name_matches
                            {
                                return Err(fault(
                                    "original associated declaration canonical attachment differs",
                                ));
                            }
                            if matches!(location, CanonicalLocation::Outline(_)) {
                                publication(source, item)?;
                            }
                            SlotTarget::Declaration(location)
                        } else {
                            // An opaque or not-yet-canonical local member is
                            // pending, never an absent associated declaration.
                            SlotTarget::NeedCanonicalOwner
                        }
                    }
                } else {
                    SlotTarget::NeedCanonicalOwner
                };
                slots.push(SlotLocation {
                    identity: row.identity,
                    owner: u32::try_from(ordinal).map_err(|_| resource())?,
                    associated: u32::try_from(member).map_err(|_| resource())?,
                    target,
                });
            }
        }
        slots.sort_unstable_by_key(|r| r.identity);
        if slots.windows(2).any(|r| r[0].identity == r[1].identity) {
            return Err(fault("associated source entry is duplicated"));
        }
        let producer_count = slots
            .iter()
            .filter(|row| matches!(row.target, SlotTarget::Producer(_)))
            .count();
        let mut producer_slots = reserve::<(Digest, u32)>(producer_count)?;
        for (i, row) in slots.iter().enumerate() {
            if let SlotTarget::Producer(producer) = row.target {
                producer_slots.push((
                    scan.producers()[producer as usize].identity,
                    u32::try_from(i).map_err(|_| resource())?,
                ));
            }
        }
        producer_slots.sort_unstable_by_key(|row| row.0);
        if producer_slots.windows(2).any(|rows| rows[0].0 == rows[1].0) {
            return Err(fault("associated producer slot owner is duplicated"));
        }
        let mut value = Self {
            source,
            scan,
            local,
            owners,
            slots,
            producer_slots,
            fingerprint: Digest([0; 32]),
        };
        let mut encoded = StableEncoder::new("lrr/v2/original-associated-declaration-source-join");
        encoded.string(selection.path.as_str());
        encoded.record(&selection.outline);
        encoded.digest(value.scan.fingerprint());
        encoded.sequence(&value.owners, |e, row| {
            e.digest(row.identity);
            e.u8(match row.kind {
                OwnerKind::Trait => 0,
                OwnerKind::Impl => 1,
            });
            e.u32(row.scan);
            e.option(&row.member_owner, |e, id| e.digest(*id));
            e.option(&row.canonical, |e, location| {
                e.u8(match location {
                    CanonicalLocation::Outline(_) => 0,
                    CanonicalLocation::Local(_) => 1,
                });
                let item = canonical_item(source, &value.local, *location);
                e.record(&item.def_key);
                e.record(&item.header);
            });
        });
        encoded.sequence(&value.slots, |e, row| {
            e.digest(row.identity);
            e.u32(row.owner);
            e.u32(row.associated);
            match row.target {
                SlotTarget::Declaration(location) => {
                    e.u8(0);
                    e.u8(match location {
                        CanonicalLocation::Outline(_) => 0,
                        CanonicalLocation::Local(_) => 1,
                    });
                    let item = canonical_item(source, &value.local, location);
                    e.record(&item.def_key);
                    e.option(&item.body, |e, b| e.record(b));
                }
                SlotTarget::Producer(i) => {
                    e.u8(1);
                    e.u32(i);
                    e.digest(value.scan.producers()[i as usize].identity);
                }
                SlotTarget::NeedCanonicalOwner => e.u8(2),
            }
        });
        value.fingerprint = encoded.finish();
        Ok(value)
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    pub(crate) fn owner_count(&self) -> usize {
        self.owners.len()
    }
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }
    fn owner_at(&self, row: &OwnerLocation) -> AssociatedOwnerHeader<'_> {
        AssociatedOwnerHeader {
            source: match row.kind {
                OwnerKind::Trait => {
                    AssociatedOwnerSource::Trait(&self.scan.traits()[row.scan as usize])
                }
                OwnerKind::Impl => {
                    AssociatedOwnerSource::Impl(&self.scan.impls()[row.scan as usize])
                }
            },
            canonical: row
                .canonical
                .map(|location| canonical_item(self.source, &self.local, location)),
            member_owner: row.member_owner,
        }
    }
    pub(crate) fn owner(&self, identity: Digest) -> Option<AssociatedOwnerHeader<'_>> {
        let row = &self.owners[self
            .owners
            .binary_search_by_key(&identity, |r| r.identity)
            .ok()?];
        Some(self.owner_at(row))
    }
    /// Share the retained real scope of this selected cold local impl.
    /// A missing canonical/generated parent remains pending; no plan is rebuilt.
    pub(crate) fn declaration_scope(
        &self,
        identity: Digest,
    ) -> Option<CapturedDeclarationScope<'_>> {
        let row = &self.owners[self
            .owners
            .binary_search_by_key(&identity, |row| row.identity)
            .ok()?];
        let CanonicalLocation::Local(location) = row.canonical? else {
            return None;
        };
        let (parent, plan) = self.local.scope(location);
        Some(CapturedDeclarationScope {
            source: self.source,
            parent: &self.source.outline().items[parent as usize],
            plan,
        })
    }
    /// Actual retained scopes already captured by this declaration inventory.
    /// Sharing them never prepares executable bindings or a BodyIndex.
    pub(crate) fn declaration_scopes(&self) -> impl Iterator<Item = CapturedDeclarationScope<'_>> {
        self.local.scopes().map(|(parent, plan)| CapturedDeclarationScope {
            source: self.source,
            parent: &self.source.outline().items[parent as usize],
            plan,
        })
    }
    pub(crate) fn associated(&self, identity: Digest) -> Option<OriginalAssociatedSlot<'_>> {
        let row = &self.slots[self
            .slots
            .binary_search_by_key(&identity, |r| r.identity)
            .ok()?];
        let owner = &self.owners[row.owner as usize];
        let source = match owner.kind {
            OwnerKind::Trait => {
                &self.scan.traits()[owner.scan as usize].associated[row.associated as usize]
            }
            OwnerKind::Impl => {
                &self.scan.impls()[owner.scan as usize].associated[row.associated as usize]
            }
        };
        Some(OriginalAssociatedSlot {
            source,
            owner: self.owner_at(owner),
            target: match row.target {
                SlotTarget::Declaration(location) => OriginalAssociatedTarget::Declaration(
                    canonical_item(self.source, &self.local, location),
                ),
                SlotTarget::Producer(i) => {
                    OriginalAssociatedTarget::Producer(&self.scan.producers()[i as usize])
                }
                SlotTarget::NeedCanonicalOwner => OriginalAssociatedTarget::NeedCanonicalOwner,
            },
        })
    }
    pub(crate) fn associated_producer(
        &self,
        identity: Digest,
    ) -> Option<OriginalAssociatedSlot<'_>> {
        let index = self
            .producer_slots
            .binary_search_by_key(&identity, |row| row.0)
            .ok()?;
        let slot = &self.slots[self.producer_slots[index].1 as usize];
        self.associated(slot.identity)
    }
}
#[cfg(test)]
#[path = "associated_declaration_tests.rs"]
mod tests;

#[path = "generated_associated_declarations.rs"]
pub(crate) mod generated;
