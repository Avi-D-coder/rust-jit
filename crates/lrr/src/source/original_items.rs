//! Actual original item targets from the ordinary outline/lexical collector.
//! A target is source metadata, never body entry or expansion completion.
use super::changes::{PreparedSourceView, SourceSelection};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::{
        declaration_scope_plan::DeclarationScopePlan, outline::ItemOutline, spans::SourceRange,
    },
};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProducerAttachment {
    Item(SourceRange),
    Meta(SourceRange),
}
struct OriginalAnnotationProducers {
    attachments: BTreeMap<Digest, ProducerAttachment>,
    groups: BTreeMap<Digest, Arc<OriginalAnnotationGroup>>,
    producer_groups: BTreeMap<Digest, Digest>,
}

/// One genuine ordinary item producer and its distinct scan annotations.
/// Pure immutable syntax ownership; no producer completion or source authority.
pub(crate) struct OriginalAnnotationGroup {
    source: SourceSelection,
    scan: Digest,
    item: Digest,
    header: Digest,
    call: crate::syntax::outline::ItemInvocation,
    annotations: Arc<[Digest]>,
}
impl OriginalAnnotationGroup {
    /// Match the actual canonical row, without interpreting or removing its annotation.
    pub(crate) fn require_item(
        &self,
        parsed: &crate::syntax::parse::ParsedFile,
        item: &crate::syntax::outline::ItemOutline,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        if parsed.key() != self.source.outline.parse
            || item.def_key.fingerprint() != self.item
            || item.header.fingerprint() != self.header
            || item.scope != self.call.scope
            || item.declaration_span != self.call.range
            || producer != self.call.item_input
        {
            return Err(fault("original annotation completion declaration differs"));
        }
        Ok(())
    }
    pub(crate) fn call(&self) -> &crate::syntax::outline::ItemInvocation {
        &self.call
    }
    pub(crate) fn annotations(&self) -> &[Digest] {
        &self.annotations
    }
    pub(crate) fn item(&self) -> Digest {
        self.item
    }
    pub(crate) fn scan_identity(&self) -> Digest {
        self.scan
    }
    pub(crate) fn require_source(&self, source: &PreparedSourceView) -> Result<(), Diagnostic> {
        if source.selection() != &self.source {
            return Err(fault("original item group source differs"));
        }
        // The private group was captured from this exact immutable source.
        // Replay the retained attachment receipt, not the entire declaration
        // scan or a body traversal on every annotation phase.
        let actual = source
            .original_annotation_group(self.call.item_input)?
            .ok_or_else(|| fault("original item group canonical producer absent"))?;
        if actual.fingerprint() != self.fingerprint() {
            return Err(fault("original item group target changed"));
        }
        Ok(())
    }
}
impl StableEncode for OriginalAnnotationGroup {
    const DOMAIN: &'static str = "lrr/v1/original-annotation-group";
    fn encode_fields(&self, key: &mut StableEncoder) {
        key.digest(self.source.location_identity());
        key.record(&self.source.outline.parse);
        key.digest(self.source.outline.cfg);
        key.digest(self.scan);
        key.digest(self.item);
        key.digest(self.header);
        key.record(&self.call.scope);
        key.u32(self.call.range.start);
        key.u32(self.call.range.end);
        key.digest(self.call.item_input);
        key.digest(self.call.frontier);
        key.option(&self.call.attribute_input, |key, input| key.digest(*input));
        key.sequence(&self.annotations, |key, annotation| key.digest(*annotation));
    }
}

enum Location<'a> {
    Outline(&'a ItemOutline),
    Local {
        plan: Arc<DeclarationScopePlan>,
        range: SourceRange,
    },
}
pub(crate) struct OriginalItemTarget<'a> {
    source: &'a SourceSelection,
    location: Location<'a>,
}
impl OriginalItemTarget<'_> {
    pub(crate) fn local_plan(&self) -> Option<&Arc<DeclarationScopePlan>> {
        match &self.location {
            Location::Local { plan, .. } => Some(plan),
            Location::Outline(_) => None,
        }
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        self.source
    }
    pub(crate) fn annotation_invocation(&self) -> Option<crate::syntax::outline::ItemInvocation> {
        match &self.location {
            Location::Outline(item) => crate::syntax::outline::potential_attribute_producer(item),
            Location::Local { plan, range } => {
                let row = plan.item_at(*range)?;
                let producer = row.attribute_input?;
                Some(crate::syntax::outline::ItemInvocation {
                    scope: row.item.scope.clone(),
                    range: row.item.declaration_span,
                    item_input: producer,
                    frontier: row.item.header.fingerprint(),
                    attribute_input: Some(producer),
                })
            }
        }
    }
    pub(crate) fn item(&self) -> &ItemOutline {
        match &self.location {
            Location::Outline(item) => item,
            Location::Local { plan, range } => {
                &plan
                    .item_at(*range)
                    .expect("private original target retains its canonical row")
                    .item
            }
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ORIGINAL-ANNOTATION", message.into())
}
impl PreparedSourceView {
    /// Real module worklist identities and cold DeclarationScan identities
    /// resolve through the same canonical item service. This only attaches
    /// syntax; it does not settle a producer or grant expansion authority.
    pub(crate) fn original_annotation_producer(
        &self,
        producer: Digest,
    ) -> Result<Option<OriginalItemTarget<'_>>, Diagnostic> {
        let index = self.original_annotation_index()?;
        match index.attachments.get(&producer) {
            Some(ProducerAttachment::Item(range)) => self.original_item_at(*range),
            Some(ProducerAttachment::Meta(range)) => self.original_annotation_target(*range),
            None => Ok(None),
        }
    }
    pub(crate) fn original_annotation_group(
        &self,
        producer: Digest,
    ) -> Result<Option<Arc<OriginalAnnotationGroup>>, Diagnostic> {
        let index = self.original_annotation_index()?;
        Ok(index
            .producer_groups
            .get(&producer)
            .and_then(|group| index.groups.get(group))
            .cloned())
    }
    fn original_annotation_index(&self) -> Result<Arc<OriginalAnnotationProducers>, Diagnostic> {
        self.require_original_hygiene()?;
        let mut key = StableEncoder::new("lrr/v2/original-annotation-producer-index");
        key.digest(self.selection().location_identity());
        key.record(&self.selection().outline.parse);
        key.digest(self.selection().outline.cfg);
        let key = key.finish();
        let index = if let Some(retained) = self.publication_prepared(key) {
            retained
                .downcast::<OriginalAnnotationProducers>()
                .map_err(|_| fault("original producer cache variant differs"))?
        } else {
            let mut attachments = BTreeMap::new();
            for item in &self.outline().items {
                if let Some(call) = crate::syntax::outline::potential_attribute_producer(item) {
                    if attachments
                        .insert(
                            call.item_input,
                            ProducerAttachment::Item(item.declaration_span),
                        )
                        .is_some()
                    {
                        return Err(fault("original item producer is duplicated"));
                    }
                }
            }
            let scan = self.declaration_inventory(Default::default())?;
            let mut groups = BTreeMap::<
                Digest,
                (
                    crate::syntax::outline::ItemInvocation,
                    Digest,
                    Digest,
                    Vec<Digest>,
                ),
            >::new();
            let mut producer_groups = BTreeMap::new();
            for row in scan.producers() {
                if row.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute {
                    continue;
                }
                if attachments
                    .insert(row.identity, ProducerAttachment::Meta(row.range))
                    .is_some()
                {
                    return Err(fault("original annotation producer is duplicated"));
                }
            }
            for row in scan.producers().iter().filter(|row| {
                row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            }) {
                let Some(range) = self
                    .retained_declaration_parser()?
                    .attribute_item_range_at(row.range)?
                else {
                    continue;
                };
                let Some(target) = self.original_item_at(range)? else {
                    continue;
                };
                // Field/parameter/expression annotations are not whole-item
                // producer groups. Their existing owning grammar handles them.
                if !target
                    .item()
                    .header
                    .attributes
                    .iter()
                    .any(|attribute| attribute.source_meta == Some(row.range))
                {
                    continue;
                }
                let Some(call) = target.annotation_invocation() else {
                    continue;
                };
                let group_id = call.item_input;
                let attachment = ProducerAttachment::Item(target.item().declaration_span);
                if attachments
                    .get(&group_id)
                    .is_some_and(|old| old != &attachment)
                {
                    return Err(fault(
                        "original group identity has conflicting source attachments",
                    ));
                }
                attachments.insert(group_id, attachment);
                let item = target.item().def_key.fingerprint();
                let header = target.item().header.fingerprint();
                let group = groups
                    .entry(group_id)
                    .or_insert_with(|| (call, item, header, Vec::new()));
                if group.1 != item || group.2 != header {
                    return Err(fault("original group canonical item/header is ambiguous"));
                }
                group.3.push(row.identity);
                if producer_groups.insert(row.identity, group_id).is_some() {
                    return Err(fault("original annotation has multiple item groups"));
                }
            }
            let groups = groups
                .into_iter()
                .map(|(identity, (call, item, header, annotations))| {
                    producer_groups.insert(identity, identity);
                    (
                        identity,
                        Arc::new(OriginalAnnotationGroup {
                            source: self.selection().clone(),
                            scan: scan.fingerprint(),
                            item,
                            header,
                            call,
                            annotations: annotations.into(),
                        }),
                    )
                })
                .collect();
            let index = Arc::new(OriginalAnnotationProducers {
                attachments,
                groups,
                producer_groups,
            });
            self.retain_publication_prepared(key, index.clone());
            index
        };
        Ok(index)
    }
    pub(crate) fn original_annotation_target(
        &self,
        meta: SourceRange,
    ) -> Result<Option<OriginalItemTarget<'_>>, Diagnostic> {
        self.require_original_hygiene()?;
        let Some(range) = self
            .retained_declaration_parser()?
            .attribute_item_range_at(meta)?
        else {
            return Ok(None);
        };
        let Some(target) = self.original_item_at(range)? else {
            return Ok(None);
        };
        if !target
            .item()
            .header
            .attributes
            .iter()
            .any(|attribute| attribute.source_meta == Some(meta))
        {
            return Err(fault(
                "original annotation Meta does not attach to the selected item",
            ));
        }
        Ok(Some(target))
    }
    pub(crate) fn original_item_at(
        &self,
        range: SourceRange,
    ) -> Result<Option<OriginalItemTarget<'_>>, Diagnostic> {
        self.require_original_hygiene()?;
        if let Some(item) = self
            .outline()
            .declaration_rows()
            .find(|item| item.declaration_span == range)
        {
            return Ok(Some(OriginalItemTarget {
                source: self.selection(),
                location: Location::Outline(item),
            }));
        }
        // Only an actual retained executable owner supplies a lexical plan.
        // This reads declarations; it never prepares the owner's bindings or
        // executable expressions, including cold branches.
        let Some(owner) = self
            .outline()
            .declaration_rows()
            .filter(|item| {
                item.body
                    .as_ref()
                    .is_some_and(|body| body.range.contains(range))
            })
            .min_by_key(|item| {
                item.body.as_ref().expect("filtered body").range.end
                    - item.body.as_ref().expect("filtered body").range.start
            })
        else {
            return Ok(None);
        };
        let parsed = self.retained_declaration_parser()?;
        let cfg = self.captured_macro_cfg()?;
        let mut body = owner.body.as_ref().expect("selected owner body").clone();
        let mut scope = owner.scope.clone();
        loop {
            if body.entry_blocked {
                return Ok(None);
            }
            let plan = parsed.declaration_scope_plan(&body, cfg, &scope)?;
            if plan.item_at(range).is_some() {
                return Ok(Some(OriginalItemTarget {
                    source: self.selection(),
                    location: Location::Local { plan, range },
                }));
            }
            // A nested function/member has its own genuine BodyHandle and
            // canonical scope. Never reuse the outer function's identity.
            let Some(next) = plan
                .declarations()
                .declarations()
                .filter(|row| {
                    row.item.body.as_ref().is_some_and(|next| {
                        next.range != body.range
                            && body.range.contains(next.range)
                            && next.range.contains(range)
                    })
                })
                .min_by_key(|row| {
                    let next = row.item.body.as_ref().expect("filtered nested body");
                    next.range.end - next.range.start
                })
            else {
                return Ok(None);
            };
            let next_body = next.item.body.as_ref().expect("selected nested body");
            if next_body.range == body.range {
                return Err(fault("original item owner did not advance"));
            }
            body = next_body.clone();
            scope = next.item.scope.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::key::OutlineKey,
        source::{
            changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
            manifest::ItemKind,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::{declaration_scan::DeclarationProducerKind, parse::ParseKey},
    };
    const LOCAL: &[u8] = include_bytes!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs"
    );
    const MODULE: &[u8] = include_bytes!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/methods/method-self-arg-trait.rs"
    );
    const MULTIPLE: &[u8] = include_bytes!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    );

    fn prepared(bytes: &[u8], edition: u16) -> (PreparedSourceInputs, SourceSelection) {
        let blob = SourceBlob::new(bytes.to_vec()).unwrap();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let cfg = CfgSet::guest_default();
        let path = SourcePath::new("upstream.rs").unwrap();
        let crate_key = Digest([197; 32]);
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        (inputs, selection)
    }

    #[test]
    fn unchanged_local_derive_target_reuses_the_entered_canonical_declarations() {
        // This is a source identity/retention control, not a runtime pass or
        // evidence that the still-held derive producer has completed.
        for edition in [2015, 2018, 2021, 2024] {
            let (inputs, selection) = prepared(LOCAL, edition);
            let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap()
            else {
                panic!("unchanged source missing")
            };
            let scan = source.declaration_inventory(Default::default()).unwrap();
            let annotation = scan
                .producers()
                .iter()
                .find(|row| {
                    row.kind == DeclarationProducerKind::Attribute
                        && row.unqualified_attribute.as_deref() == Some("derive")
                })
                .unwrap();
            let target = source
                .original_annotation_target(annotation.range)
                .unwrap()
                .unwrap();
            assert_eq!(target.source(), source.selection());
            let item = target.item();
            let from_producer = source
                .original_annotation_producer(annotation.identity)
                .unwrap()
                .unwrap();
            assert_eq!(from_producer.item(), item);
            let group = source
                .original_annotation_group(annotation.identity)
                .unwrap()
                .unwrap();
            assert_eq!(
                group.call().item_input,
                target.annotation_invocation().unwrap().item_input
            );
            assert_eq!(group.annotations(), &[annotation.identity]);
            assert_ne!(group.call().item_input, annotation.identity);
            let same_group = source
                .original_annotation_group(group.call().item_input)
                .unwrap()
                .unwrap();
            assert!(Arc::ptr_eq(&group, &same_group));
            group.require_source(&source).unwrap();
            assert!(
                source
                    .original_annotation_producer(Digest([0; 32]))
                    .unwrap()
                    .is_none()
            );
            assert!(
                source
                    .original_annotation_target(item.declaration_span)
                    .is_err()
            );
            assert_eq!(item.def_key.item_kind, ItemKind::Enum);
            assert_eq!(item.def_key.declared_name, "x");
            assert_eq!(item.header.profile.edition, edition);
            assert!(item.scope.owner.is_some());
            assert!(
                item.header
                    .attributes
                    .iter()
                    .any(|attribute| attribute.source_meta == Some(annotation.range))
            );
            let Location::Local { plan, .. } = &target.location else {
                panic!("body-local enum must not become a module outline row")
            };
            let repeated = source
                .original_annotation_target(annotation.range)
                .unwrap()
                .unwrap();
            let Location::Local { plan: again, .. } = &repeated.location else {
                panic!()
            };
            assert!(Arc::ptr_eq(plan, again));
            let entered = source.entered_macro_facts(&plan.body().key()).unwrap();
            assert!(Arc::ptr_eq(
                plan.declaration_facts(),
                entered.declaration_facts()
            ));
            assert_eq!(
                entered
                    .items
                    .iter()
                    .find(|row| row.item.declaration_span == item.declaration_span)
                    .unwrap()
                    .item
                    .def_key
                    .fingerprint(),
                item.def_key.fingerprint()
            );
            // Declaring an attributed enum does not give it a completed
            // expansion or permit a body entry hidden behind that attribute.
            assert!(
                plan.item_at(item.declaration_span)
                    .unwrap()
                    .attribute_input
                    .is_some()
            );
            let (changed, changed_selection) = prepared(MODULE, edition);
            let SourceSelectionOutcome::Selected(changed_source) =
                changed.select(&changed_selection).unwrap()
            else {
                panic!()
            };
            assert!(group.require_source(&changed_source).is_err());
            assert!(matches!(
                changed.select(&selection).unwrap(),
                SourceSelectionOutcome::Changed { .. }
            ));
        }
    }

    #[test]
    fn unchanged_multiple_annotations_share_one_original_item_group() {
        // Source ownership only: executing the retained procedural auxiliaries
        // and the full stock fixture remains a separate runtime contract.
        for edition in [2015, 2018, 2021, 2024] {
            let (inputs, selection) = prepared(MULTIPLE, edition);
            let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap()
            else {
                panic!("unchanged source missing")
            };
            let scan = source.declaration_inventory(Default::default()).unwrap();
            let rows: Vec<_> = scan
                .producers()
                .iter()
                .filter(|row| {
                    row.kind == DeclarationProducerKind::Attribute
                        && row.unqualified_attribute.as_deref() == Some("derive")
                })
                .collect();
            assert_eq!(rows.len(), 4);
            let group = source
                .original_annotation_group(rows[0].identity)
                .unwrap()
                .unwrap();
            assert_eq!(
                group.annotations(),
                rows.iter().map(|row| row.identity).collect::<Vec<_>>()
            );
            let mut end = 0;
            for row in rows {
                assert!(row.range.start >= end);
                end = row.range.end;
                let same = source
                    .original_annotation_group(row.identity)
                    .unwrap()
                    .unwrap();
                assert!(Arc::ptr_eq(&group, &same));
                assert_ne!(row.identity, group.call().item_input);
            }
            assert!(Arc::ptr_eq(
                &group,
                &source
                    .original_annotation_group(group.call().item_input)
                    .unwrap()
                    .unwrap()
            ));
            group.require_source(&source).unwrap();
        }
    }

    #[test]
    fn unchanged_module_derive_target_keeps_the_real_outline_row() {
        for edition in [2015, 2018, 2021, 2024] {
            let (inputs, selection) = prepared(MODULE, edition);
            let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap()
            else {
                panic!("unchanged source missing")
            };
            let item = source
                .outline()
                .items
                .iter()
                .find(|item| {
                    item.def_key.declared_name == "Foo"
                        && item.def_key.item_kind == ItemKind::Struct
                })
                .unwrap();
            let meta = item
                .header
                .attributes
                .iter()
                .find(|attribute| attribute.name == "derive")
                .unwrap()
                .source_meta
                .unwrap();
            let target = source.original_annotation_target(meta).unwrap().unwrap();
            assert_eq!(target.source(), source.selection());
            assert!(std::ptr::eq(target.item(), item));
            let call = crate::syntax::outline::potential_attribute_producer(item).unwrap();
            let from_producer = source
                .original_annotation_producer(call.item_input)
                .unwrap()
                .unwrap();
            assert!(std::ptr::eq(from_producer.item(), item));
            let group = source
                .original_annotation_group(call.item_input)
                .unwrap()
                .unwrap();
            assert_eq!(group.call().item_input, call.item_input);
            assert_eq!(group.call().scope, call.scope);
            assert_eq!(group.item(), item.def_key.fingerprint());
            group.require_source(&source).unwrap();
            assert!(matches!(target.location, Location::Outline(_)));
        }
    }
}
