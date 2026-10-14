//! Syntax-only cardinality/provenance acceptance from unchanged stock cfg operands.
//! This does not execute the full stock main or remove disabled structural nodes.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    ir::pc::Phase,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::BodyIndex,
        constructors::{ConstructorKind, ConstructorPlan},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
const STOCK: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cfg/cfg_stmt_expr.rs");
fn prepare(source: &str, edition: u16, cfg: &CfgSet) -> (ParsedFile, BodyHandle, BodyIndex) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed.outline(Digest([147; 32]), vec![], cfg).unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    (parsed, body, index)
}
fn plan(
    source: &str,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    needle: &str,
    cfg: &CfgSet,
) -> ConstructorPlan {
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            source.get(
                (body.range.start + node.range.start) as usize
                    ..(body.range.start + node.range.end) as usize,
            ) == Some(needle)
        })
        .unwrap_or_else(|| panic!("original constructor boundary absent: {needle}"));
    let plan = parsed
        .constructor_plan(body, index, node.id, cfg)
        .unwrap()
        .unwrap();
    assert_eq!(plan.node(), node.id);
    assert_eq!(plan.origin(), &index.span(node.id).unwrap());
    assert_eq!(plan.profile().edition, parsed.key().profile.edition);
    assert_eq!(plan.cfg(), cfg.fingerprint());
    plan
}
fn fields(plan: &ConstructorPlan) -> &[lrr::syntax::constructors::ConstructorOperand] {
    match plan.kind() {
        ConstructorKind::Tuple(fields) | ConstructorKind::Array(fields) => fields,
        _ => panic!("expected tuple or element-list array"),
    }
}
#[test]
fn unchanged_cfg_stmt_expr_omits_only_disabled_constructor_operands() {
    for edition in [2015, 2018, 2021, 2024] {
        let cfg = CfgSet::guest_default();
        let (parsed, body, index) = prepare(STOCK, edition, &cfg);
        let before_pcs = index
            .nodes()
            .iter()
            .map(|node| index.pc(node.id, Phase::Entry).unwrap())
            .collect::<Vec<_>>();
        for (needle, expected, disabled) in [
            ("(#[cfg(false)] 341,)", 0, "341"),
            ("(1, #[cfg(false)] 3, 4)", 2, "3"),
            ("[#[cfg(false)] 126]", 0, "126"),
            ("[#[cfg(false)] 1, 2, 6]", 2, "1"),
        ] {
            let plan = plan(STOCK, &parsed, &body, &index, needle, &cfg);
            assert_eq!(
                fields(&plan).len(),
                expected,
                "original {needle}, edition {edition}"
            );
            let range = plan.origin();
            let disabled_node = index
                .nodes()
                .iter()
                .find(|node| {
                    node.range.start >= range.relative_start
                        && node.range.end <= range.relative_end
                        && STOCK
                            .get(
                                (body.range.start + node.range.start) as usize
                                    ..(body.range.start + node.range.end) as usize,
                            )
                            .is_some_and(|text| text == format!("#[cfg(false)] {disabled}"))
                })
                .expect("disabled original operand must retain a structural identity");
            assert!(
                fields(&plan)
                    .iter()
                    .all(|field| field.node != disabled_node.id)
            );
            assert_eq!(
                index.pc(disabled_node.id, Phase::Entry).unwrap().body,
                body.key()
            );
            for field in fields(&plan) {
                assert_eq!(index.pc(field.node, Phase::Entry).unwrap().body, body.key());
                assert_eq!(
                    index.nodes()[field.node.0 as usize].value,
                    Some(field.value)
                );
            }
        }
        let after_pcs = index
            .nodes()
            .iter()
            .map(|node| index.pc(node.id, Phase::Entry).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            before_pcs, after_pcs,
            "capture must preserve every original PC"
        );
        let mut foreign = body.clone();
        foreign.body_hash = Digest([99; 32]);
        let selected = plan(STOCK, &parsed, &body, &index, "(#[cfg(false)] 341,)", &cfg);
        assert!(
            parsed
                .constructor_plan(&foreign, &index, selected.node(), &cfg)
                .is_err(),
            "foreign body identity must not attach a plan to original PCs"
        );
    }
}
#[test]
fn captured_cfg_and_enabled_non_cfg_attributes_keep_written_operand_frontiers() {
    // Disclosed harness controls outside the unchanged upstream directory.
    let source = "fn main() { let a = (#[cfg_attr(flag, cfg(false))] 1, #[allow(dead_code)] 2, #[cfg(not(FALSE))] 3); }";
    let needle = "(#[cfg_attr(flag, cfg(false))] 1, #[allow(dead_code)] 2, #[cfg(not(FALSE))] 3)";
    let cfg = CfgSet::guest_default();
    let (parsed, body, index) = prepare(source, 2024, &cfg);
    let unconfigured = plan(source, &parsed, &body, &index, needle, &cfg);
    assert_eq!(fields(&unconfigured).len(), 3);
    let mut configured = cfg.clone();
    configured.atoms.insert("flag".into());
    configured.atoms.insert("FALSE".into());
    let active = plan(source, &parsed, &body, &index, needle, &configured);
    assert_eq!(fields(&active).len(), 1);
    let remaining = &index.nodes()[fields(&active)[0].node.0 as usize];
    assert_eq!(
        &source[(body.range.start + remaining.range.start) as usize
            ..(body.range.start + remaining.range.end) as usize],
        "#[allow(dead_code)] 2"
    );
    assert_ne!(active.cfg(), unconfigured.cfg());
}
