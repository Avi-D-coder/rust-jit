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

const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();

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
    outline: Option<u32>,
    member_owner: Option<Digest>,
}
#[derive(Clone, Copy)]
enum SlotTarget {
    Declaration(u32),
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
    owners: Vec<OwnerLocation>,
    slots: Vec<SlotLocation>,
    producer_slots: Vec<(Digest, u32)>,
    fingerprint: Digest,
    metadata_bytes: usize,
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
fn checked_add(bytes: usize, added: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(added)
        .ok_or_else(resource)
}
fn reserve<T>(count: usize, bytes: &mut usize) -> Result<Vec<T>, Diagnostic> {
    let planned = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(resource)?;
    *bytes = checked_add(*bytes, planned)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|_| resource())?;
    *bytes = checked_add(
        *bytes,
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
            .ok_or_else(resource)?,
    )?;
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
            .try_fold(0usize, |n, m| {
                n.checked_add(m)
                    .ok_or_else(resource)
            })?;
        let mut budget = checked_add(std::mem::size_of::<Self>(), scan.retained_bytes())?;
        budget = checked_add(budget, ARC_HEADER)?;
        let mut spans = reserve::<((u32, u32), u32)>(outline.items.len(), &mut budget)?;
        for (i, item) in outline.items.iter().enumerate() {
            spans.push((
                key(item.declaration_span),
                u32::try_from(i).map_err(|_| resource())?,
            ));
        }
        spans.sort_unstable_by_key(|row| row.0);
        let mut producers = reserve::<(Digest, u32)>(scan.producers().len(), &mut budget)?;
        for (i, row) in scan.producers().iter().enumerate() {
            producers.push((row.identity, u32::try_from(i).map_err(|_| resource())?));
        }
        producers.sort_unstable_by_key(|row| row.0);
        if producers.windows(2).any(|r| r[0].0 == r[1].0) {
            return Err(fault("associated producer identity is duplicated"));
        }
        let mut owners = reserve::<OwnerLocation>(count, &mut budget)?;
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
                    OwnerKind::Impl => item.def_key.enclosing_impl.ok_or_else(|| {
                        fault("original associated impl identity is absent")
                    })?,
                });
            }
            owners.push(OwnerLocation {
                identity,
                kind,
                scan: u32::try_from(index).map_err(|_| resource())?,
                outline: actual,
                member_owner,
            });
        }
        owners.sort_unstable_by_key(|r| r.identity);
        if owners.windows(2).any(|r| r[0].identity == r[1].identity) {
            return Err(fault("associated owner source identity is duplicated"));
        }
        let mut slots = reserve::<SlotLocation>(slot_count, &mut budget)?;
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
                let target = if let Some(canonical) = owner.outline {
                    let parent = &outline.items[canonical as usize];
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
                        let index = at_span(&spans, row.range)?.ok_or_else(|| {
                            fault("original associated declaration canonical row absent")
                        })?;
                        let item = &outline.items[index as usize];
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
                        publication(source, item)?;
                        SlotTarget::Declaration(index)
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
        let mut producer_slots = reserve::<(Digest, u32)>(producer_count, &mut budget)?;
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
        let metadata_bytes = checked_add(
            checked_add(std::mem::size_of::<Self>(), ARC_HEADER)?,
            owners
                .capacity()
                .checked_mul(std::mem::size_of::<OwnerLocation>())
                .and_then(|n| {
                    slots
                        .capacity()
                        .checked_mul(std::mem::size_of::<SlotLocation>())
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| {
                    producer_slots
                        .capacity()
                        .checked_mul(std::mem::size_of::<(Digest, u32)>())
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(resource)?,
        )?;
        let mut value = Self {
            source,
            scan,
            owners,
            slots,
            producer_slots,
            fingerprint: Digest([0; 32]),
            metadata_bytes,
        };
        let mut encoded = StableEncoder::new("lrr/v1/original-associated-declaration-source-join");
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
            e.option(&row.outline, |e, i| {
                let item = &outline.items[*i as usize];
                e.u32(*i);
                e.record(&item.def_key);
                e.record(&item.header);
            });
        });
        encoded.sequence(&value.slots, |e, row| {
            e.digest(row.identity);
            e.u32(row.owner);
            e.u32(row.associated);
            match row.target {
                SlotTarget::Declaration(i) => {
                    e.u8(0);
                    e.u32(i);
                    let item = &outline.items[i as usize];
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
    /// Exclusive locator/handle storage. Retained scan payload is shared separately.
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
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
                .outline
                .map(|i| &self.source.outline().items[i as usize]),
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
                SlotTarget::Declaration(i) => {
                    OriginalAssociatedTarget::Declaration(&self.source.outline().items[i as usize])
                }
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
