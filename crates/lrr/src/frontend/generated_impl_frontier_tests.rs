//! Distinct declaration frontiers must not release unrelated source authority.
//! These controls inspect actual admitted macro headers, not guest execution.
use super::GeneratedHeaderPlan;
use crate::{
    core::digest::{Digest, StableEncode},
    expand::{
        generated_declarations::GeneratedDeclarations, generated_items::GeneratedItems,
        source_declarative::SourceDeclarativeDemand,
    },
    source::snapshot::CfgSet,
    syntax::{outline::ScopeKey, token_wire::Key},
};
use std::sync::Arc;

fn output(
    payload: &str,
    edition: u16,
) -> (GeneratedItems, GeneratedDeclarations, ScopeKey, Key, CfgSet) {
    let text = format!("macro_rules! emit{{()=>{{{payload}}}}}emit!();");
    let (provider, _, _) =
        crate::expand::source_declarative::assembly::tests::world(&text, edition);
    let source = provider.module_source();
    let call = source
        .outline()
        .item_invocations
        .iter()
        .find(|call| call.scope.owner.is_none())
        .expect("actual root item invocation")
        .clone();
    let cfg = source.captured_macro_cfg().unwrap().clone();
    let SourceDeclarativeDemand::Ready(request) = provider
        .demand(
            call.item_input,
            Digest::of_bytes(b"impl frontier authority compiler"),
            Digest::of_bytes(b"impl frontier authority engine"),
        )
        .unwrap()
    else {
        panic!("actual ordinary macro definition did not resolve")
    };
    let context = Key(request.key().parent_context.0);
    let items =
        GeneratedItems::admit(request.key(), Arc::new(request.expand().unwrap()), 1).unwrap();
    let declarations = items
        .declaration_facet(&request, source, &cfg, Default::default())
        .unwrap();
    (items, declarations, call.scope, context, cfg)
}

#[test]
fn actual_impl_member_scope_uses_only_its_distinct_completion_frontier() {
    for edition in [2015, 2018, 2021, 2024] {
        let (items, _, scope, context, cfg) =
            output("struct Kept; impl Kept { fn cold() {} }", edition);
        let plan = GeneratedHeaderPlan::prepare(&items, scope, context, &cfg, 1).unwrap();
        let frontier = plan.impl_frontier().expect("actual impl frontier");
        assert_ne!(frontier, items.record().key().invocation);
        assert!(plan.blocked().any(|blocked| *blocked == frontier));
        assert!(
            !plan
                .blocked()
                .any(|blocked| *blocked == items.record().key().invocation),
            "exact impl-owned member scope must not become an unrelated owner refusal"
        );
    }
}

#[test]
fn mixed_impl_and_unrelated_trait_owner_keep_independent_frontiers() {
    for edition in [2015, 2018, 2021, 2024] {
        let (items, _, scope, context, cfg) = output(
            "struct Kept; impl Kept { fn cold() {} } trait Held { fn unrelated(); }",
            edition,
        );
        let plan = GeneratedHeaderPlan::prepare(&items, scope, context, &cfg, 1).unwrap();
        let frontier = plan.impl_frontier().expect("actual impl frontier");
        let original = items.record().key().invocation;
        assert_ne!(frontier, original);
        assert!(plan.blocked().any(|blocked| *blocked == frontier));
        assert!(
            plan.blocked().any(|blocked| *blocked == original),
            "impl completion cannot certify the unrelated trait-owner scope"
        );
    }
}

#[test]
fn active_impl_projection_keeps_cfg_disabled_headers_out_of_required_completion() {
    for edition in [2015, 2018, 2021, 2024] {
        for include_active in [false, true] {
            let payload = if include_active {
                "struct Hidden; #[cfg(any())] impl Hidden {} struct Live; impl Live { fn cold() {} }"
            } else {
                "struct Hidden; #[cfg(any())] impl Hidden {}"
            };
            let (items, declarations, scope, context, cfg) = output(payload, edition);
            let raw_impls = items
                .items()
                .iter()
                .filter(|item| {
                    item.kind == crate::expand::generated_items::GeneratedHeaderKind::Impl
                })
                .count();
            assert_eq!(raw_impls, if include_active { 2 } else { 1 });
            let plan =
                GeneratedHeaderPlan::prepare(&items, scope.clone(), context, &cfg, 1).unwrap();
            let active = plan.active_impl_ranges(&items, &cfg, &scope, 1).unwrap();
            assert_eq!(active.len(), usize::from(include_active));
            let headers = declarations.item_impl_headers(&items, &cfg, &plan).unwrap();
            assert_eq!(headers.len(), usize::from(include_active));
            for (key, _) in headers {
                let item = items
                    .items()
                    .iter()
                    .find(|item| item.key.fingerprint() == key)
                    .unwrap();
                assert!(
                    active
                        .iter()
                        .any(|range| range.start == item.start && range.end == item.end)
                );
            }
            assert_eq!(plan.impl_frontier().is_some(), include_active);
            let error = plan
                .active_impl_ranges(&items, &cfg, &scope, 2)
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
            assert_eq!(error.message, "active impl projection owner differs");
        }
    }
}
