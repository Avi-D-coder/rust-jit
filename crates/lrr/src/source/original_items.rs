//! Actual original item targets from the ordinary outline/lexical collector.
//! A target is source metadata, never body entry or expansion completion.
use super::changes::{PreparedSourceView, SourceSelection};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        declaration_scope_plan::DeclarationScopePlan, outline::ItemOutline, spans::SourceRange,
    },
};
use std::sync::Arc;

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
    pub(crate) fn source(&self) -> &SourceSelection {
        self.source
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
            let (changed, _) = prepared(MODULE, edition);
            assert!(matches!(
                changed.select(&selection).unwrap(),
                SourceSelectionOutcome::Changed { .. }
            ));
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
            assert!(matches!(target.location, Location::Outline(_)));
        }
    }
}
