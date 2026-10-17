//! Item-local generated helpers from exact declaration receipts. Pure data only.
use super::{
    generated_derive_exports::GeneratedDeriveExports, procedural_exports::PreparedProcCatalog,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        derive_receipt::PreparedDeriveDeclaration, generated_derives::GeneratedDerivePlan,
        generated_items::GeneratedItems,
    },
    source::snapshot::CfgSet,
    syntax::{
        attributes::ItemAttributeInputs,
        imports::ImportSegment,
        parse::ParsedFile,
        spans::SourceRange,
        token_wire::{NameContextKey, TokenForest, TokenTree},
    },
};
use std::{collections::BTreeMap, sync::Arc};
/// Helpers introduced by completed original-source derives, for attribute
/// descendants only. The prefix binds actual receipts and completed selections.
pub(crate) struct OriginalDeriveHelpers {
    producer: Digest,
    fingerprint: Digest,
    providers: std::collections::BTreeSet<(String, NameContextKey)>,
    retained_bytes: usize,
}
impl OriginalDeriveHelpers {
    pub(crate) fn new(
        producer: Digest,
        prefix: Digest,
        providers: impl IntoIterator<Item = (String, NameContextKey)>,
    ) -> Result<Self, Diagnostic> {
        let mut names = std::collections::BTreeSet::new();
        let mut bytes = std::mem::size_of::<Self>();
        for (name, class) in providers {
            bytes = bytes
                .checked_add(name.capacity())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(limit)?;
            names.insert((name, class));
        }
        let mut e = StableEncoder::new("lrr/v2/original-inherited-derive-helpers");
        e.digest(producer);
        e.digest(prefix);
        e.u64(u64::try_from(names.len()).map_err(|_| limit())?);
        for (name, class) in &names {
            e.string(name);
            e.bytes(&class.0.0);
        }
        Ok(Self {
            producer,
            fingerprint: e.finish(),
            providers: names,
            retained_bytes: bytes,
        })
    }
    pub(crate) fn producer(&self) -> Digest {
        self.producer
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn inherited_match(&self, name: &str, class: NameContextKey) -> bool {
        self.providers.contains(&(name.to_owned(), class))
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-DISCOVERY-LIMIT",
        "generated helper metadata limit".into(),
    )
}
pub struct GeneratedHelperDeclarations<'a> {
    pub catalog: &'a PreparedProcCatalog,
    pub receipts: &'a BTreeMap<Digest, Arc<PreparedDeriveDeclaration>>,
}
pub enum GeneratedHelperPoll {
    Ready(Arc<GeneratedDeriveHelpers>),
    NeedInput(Digest),
}
pub struct GeneratedDeriveHelpers {
    plan: GeneratedDerivePlan,
    receipts: Vec<Digest>,
    definitions: Vec<Digest>,
    catalog: Digest,
    providers: BTreeMap<(String, NameContextKey), Vec<(usize, Digest)>>,
    outer: BTreeMap<usize, Vec<Digest>>,
    members: BTreeMap<(u32, u32, usize), Vec<Digest>>,
    legacy: Vec<usize>,
    unresolved_outer: Vec<usize>,
    unresolved_members: Vec<(SourceRange, usize)>,
    retained_bytes: usize,
}
impl GeneratedDeriveHelpers {
    pub(crate) fn resolve(
        plan: GeneratedDerivePlan,
        items: &GeneratedItems,
        index: usize,
        exports: &GeneratedDeriveExports,
        catalog: &PreparedProcCatalog,
        receipts: &BTreeMap<Digest, Arc<PreparedDeriveDeclaration>>,
    ) -> Result<GeneratedHelperPoll, Diagnostic> {
        let mut providers: BTreeMap<(String, NameContextKey), Vec<(usize, Digest)>> =
            BTreeMap::new();
        let hygiene = items.hygiene_facts();
        let mut receipt_ids = Vec::new();
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(exports.rows().len())
            .map_err(|_| limit())?;
        let mut bytes = plan
            .retained_bytes()
            .checked_add(std::mem::size_of::<Self>() + 32)
            .and_then(|n| {
                definitions
                    .capacity()
                    .checked_mul(std::mem::size_of::<Digest>())
                    .and_then(|size| n.checked_add(size))
            })
            .ok_or_else(limit)?;
        let mut row_index = 0usize;
        for annotation in plan.annotations() {
            for path in annotation.paths() {
                let row = exports
                    .rows()
                    .get(row_index)
                    .ok_or_else(|| fault("helper export occurrence absent"))?;
                if row.occurrence() != path.identity {
                    return Err(fault("helper plan/export occurrence differs"));
                }
                let Some(demand) = row.helper_demand() else {
                    return Ok(GeneratedHelperPoll::NeedInput(
                        exports
                            .row_demand(row_index)?
                            .ok_or_else(|| fault("helper export unresolved without demand"))?,
                    ));
                };
                let request = exports.declaration_request(row_index, catalog)?;
                let Some(receipt) = receipts.get(&demand) else {
                    return Ok(GeneratedHelperPoll::NeedInput(demand));
                };
                receipt.validate_request(&request)?;
                definitions.push(request.definition.identity);
                let span = path
                    .tokens()
                    .spans()
                    .iter()
                    .find(|s| s.key() == path.last_span)
                    .ok_or_else(|| fault("helper introduction context absent"))?;
                let class = hygiene.name_item(span.context)?;
                for name in receipt.helpers() {
                    bytes = bytes
                        .checked_add(name.len())
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(limit)?;
                    providers
                        .entry((name.clone(), class))
                        .or_default()
                        .push((annotation.index(), row.occurrence()));
                }
                let prior_capacity = receipt_ids.capacity();
                receipt_ids.try_reserve(1).map_err(|_| limit())?;
                bytes = receipt_ids
                    .capacity()
                    .checked_sub(prior_capacity)
                    .and_then(|n| n.checked_mul(std::mem::size_of::<Digest>()))
                    .and_then(|n| bytes.checked_add(n))
                    .ok_or_else(limit)?;
                receipt_ids.push(receipt.fingerprint());
                row_index += 1;
            }
        }
        if row_index != exports.rows().len() {
            return Err(fault("helper export inventory differs"));
        }
        let parsed = ParsedFile::from_generated_items(items)?;
        let mut result = Self {
            plan,
            receipts: receipt_ids,
            definitions,
            catalog: catalog.fingerprint(),
            providers: BTreeMap::new(),
            outer: BTreeMap::new(),
            members: BTreeMap::new(),
            legacy: Vec::new(),
            unresolved_outer: Vec::new(),
            unresolved_members: Vec::new(),
            retained_bytes: bytes,
        };
        let outer = result.plan.facts().item.clone();
        let members = result.plan.facts().members.clone();
        let mut classify = |facts: &ItemAttributeInputs, member: bool| -> Result<(), Diagnostic> {
            if !facts.enabled {
                return Ok(());
            }
            for (at, attribute) in facts.attributes.iter().enumerate() {
                if !member && result.plan.annotations().iter().any(|a| a.index() == at) {
                    continue;
                }
                if attribute.name == "cfg"
                    || crate::expand::builtin_attributes::retained_descriptor(&attribute.name)?
                        .is_some()
                {
                    continue;
                }
                let syntax = parsed.attribute_path_at(attribute.range)?;
                let matches = if !syntax.absolute()
                    && let [ImportSegment::Name(name)] = syntax.segments()
                {
                    let forest = crate::expand::generated_derives::annotation_path(
                        items,
                        index,
                        attribute.range,
                    )?;
                    let [TokenTree::Group { children, .. }] = forest.roots() else {
                        return Err(fault("helper path root absent"));
                    };
                    let [TokenTree::Ident { span, .. }] = children.as_slice() else {
                        return Err(fault("helper bare path token absent"));
                    };
                    let span = forest
                        .spans()
                        .iter()
                        .find(|s| s.key() == *span)
                        .ok_or_else(|| fault("helper path span absent"))?;
                    providers.get(&(name.clone(), hygiene.name_item(span.context)?))
                } else {
                    None
                };
                let mut active = Vec::new();
                if let Some(matches) = matches {
                    for &(introduced, occurrence) in matches {
                        if member || introduced < at {
                            active.try_reserve(1).map_err(|_| limit())?;
                            active.push(occurrence);
                        }
                    }
                }
                result.retained_bytes = result
                    .retained_bytes
                    .checked_add(256)
                    .and_then(|n| {
                        active
                            .capacity()
                            .checked_mul(std::mem::size_of::<Digest>())
                            .and_then(|size| n.checked_add(size))
                    })
                    .ok_or_else(limit)?;
                if !active.is_empty() {
                    if member {
                        result
                            .members
                            .insert((facts.item.start, facts.item.end, at), active);
                    } else {
                        result.outer.insert(at, active);
                    }
                } else if member {
                    result
                        .unresolved_members
                        .try_reserve(1)
                        .map_err(|_| limit())?;
                    result.unresolved_members.push((facts.item, at));
                } else {
                    result
                        .unresolved_outer
                        .try_reserve(1)
                        .map_err(|_| limit())?;
                    result.unresolved_outer.push(at);
                    if matches.is_some() {
                        result.legacy.try_reserve(1).map_err(|_| limit())?;
                        result.legacy.push(at);
                    }
                }
            }
            Ok(())
        };
        // The closure reads plan facts, so own only the bounded per-item facts while
        // constructing decisions; the retained plan remains unchanged.
        classify(&outer, false)?;
        for member in &members {
            classify(member, true)?;
        }
        result.providers = providers;
        Ok(GeneratedHelperPoll::Ready(Arc::new(result)))
    }
    pub(crate) fn inherited_match(
        &self,
        name: &str,
        class: NameContextKey,
        introduced: &std::collections::BTreeSet<Digest>,
    ) -> bool {
        self.providers
            .get(&(name.to_owned(), class))
            .is_some_and(|rows| rows.iter().any(|(_, id)| introduced.contains(id)))
    }
    pub(crate) fn invocation_path(
        &self,
        row: usize,
        definition: Digest,
    ) -> Result<&crate::expand::generated_derives::GeneratedDerivePath, Diagnostic> {
        let annotation = self.plan.annotation_index(row)?;
        if self.definitions.get(row) != Some(&definition)
            || self.unresolved_outer.iter().any(|at| *at <= annotation)
            || !self.unresolved_members.is_empty()
        {
            return Err(fault(
                "derive invocation requires exact definition and settled preceding/member attributes",
            ));
        }
        self.plan
            .annotations()
            .iter()
            .flat_map(|a| a.paths())
            .nth(row)
            .ok_or_else(|| fault("derive invocation occurrence absent"))
    }
    pub(crate) fn occurrences(&self) -> impl Iterator<Item = Digest> + '_ {
        self.plan
            .annotations()
            .iter()
            .flat_map(|a| a.paths())
            .map(|p| p.identity)
    }
    pub(crate) fn declaration_occurrences(&self) -> impl Iterator<Item = (usize, Digest)> + '_ {
        self.plan
            .annotations()
            .iter()
            .flat_map(|a| a.paths().iter().map(move |p| (a.index(), p.identity)))
    }
    pub(crate) fn annotation_index(&self, row: usize) -> Result<usize, Diagnostic> {
        self.plan.annotation_index(row)
    }
    pub(crate) fn completed_annotations(
        &self,
        appended: &std::collections::BTreeSet<Digest>,
    ) -> std::collections::BTreeSet<usize> {
        self.plan
            .annotations()
            .iter()
            .filter(|a| {
                !a.paths().is_empty() && a.paths().iter().all(|p| appended.contains(&p.identity))
            })
            .map(|a| a.index())
            .collect()
    }
    pub(crate) fn next_annotation(
        &self,
        appended: &std::collections::BTreeSet<Digest>,
    ) -> Option<usize> {
        self.plan
            .annotations()
            .iter()
            .find(|a| a.paths().iter().any(|p| !appended.contains(&p.identity)))
            .map(|a| a.index())
    }
    pub(crate) fn empty_annotation_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.plan
            .annotations()
            .iter()
            .filter(|a| a.paths().is_empty())
            .map(|a| a.index())
    }
    pub(crate) fn can_invoke(&self, row: usize) -> Result<bool, Diagnostic> {
        let at = self.plan.annotation_index(row)?;
        Ok(self.unresolved_outer.iter().all(|i| *i > at) && self.unresolved_members.is_empty())
    }
    pub(crate) fn materialize_occurrence_input(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        row: usize,
    ) -> Result<TokenForest, Diagnostic> {
        self.plan
            .materialize_occurrence_input(items, index, cfg, row)
    }
    pub(crate) fn catalog_identity(&self) -> Digest {
        self.catalog
    }
    pub fn item_identity(&self) -> Digest {
        self.plan.item_identity()
    }
    pub fn outer_helpers(&self) -> &BTreeMap<usize, Vec<Digest>> {
        &self.outer
    }
    pub fn member_helpers(&self) -> &BTreeMap<(u32, u32, usize), Vec<Digest>> {
        &self.members
    }
    pub fn legacy_outer(&self) -> &[usize] {
        &self.legacy
    }
    pub fn unresolved_outer(&self) -> &[usize] {
        &self.unresolved_outer
    }
    pub fn unresolved_members(&self) -> &[(SourceRange, usize)] {
        &self.unresolved_members
    }
    pub fn inert_count(&self) -> usize {
        self.outer
            .len()
            .checked_add(self.members.len())
            .expect("bounded helper decision count")
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Preserve helpers and all token identity. This grants no execution authority.
    pub fn materialize_input(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
    ) -> Result<TokenForest, Diagnostic> {
        self.plan.materialize_input(items, index, cfg)
    }
}
impl StableEncode for GeneratedDeriveHelpers {
    const DOMAIN: &'static str = "lrr/v2/generated-derive-helper-decisions";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.plan.binding_digest());
        e.digest(self.catalog);
        e.sequence(&self.definitions, |e, id| e.digest(*id));
        e.sequence(&self.receipts, |e, id| e.digest(*id));
        e.map(&self.providers, |e, (name, class), rows| {
            e.string(name);
            e.digest(Digest(class.0.0));
            e.sequence(rows, |e, (at, id)| {
                e.u64(u64::try_from(*at).expect("bounded helper annotation"));
                e.digest(*id);
            });
        });
        e.map(&self.outer, |e, at, owners| {
            e.u64(u64::try_from(*at).expect("bounded attribute index"));
            e.sequence(owners, |e, id| e.digest(*id));
        });
        e.map(&self.members, |e, (start, end, at), owners| {
            e.u32(*start);
            e.u32(*end);
            e.u64(u64::try_from(*at).expect("bounded member attribute index"));
            e.sequence(owners, |e, id| e.digest(*id));
        });
        e.sequence(&self.legacy, |e, at| {
            e.u64(u64::try_from(*at).expect("bounded legacy attribute index"))
        });
        e.sequence(&self.unresolved_outer, |e, at| {
            e.u64(u64::try_from(*at).expect("bounded attribute index"))
        });
        e.sequence(&self.unresolved_members, |e, (range, at)| {
            e.u32(range.start);
            e.u32(range.end);
            e.u64(u64::try_from(*at).expect("bounded member attribute index"));
        });
    }
}
