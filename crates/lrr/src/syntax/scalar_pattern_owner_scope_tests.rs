//! Source lookup controls only; no capture, namespace, Copy or execution authority.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    source::snapshot::{CfgSet, SourceBlob},
};

fn entered(
    text: &str,
    edition: u16,
    owner: &str,
    nested: Option<&str>,
) -> (ParsedFile, BodyHandle, BodyIndex) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let cfg = CfgSet::guest_default();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"scalar binding owner source control"),
            vec![],
            &cfg,
        )
        .unwrap();
    let item = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == owner)
        .unwrap();
    let owner_body = item.body.as_ref().unwrap();
    let body = if let Some(nested) = nested {
        // Obtain the actual nested item's original source handle, rather than
        // rebasing its block or manufacturing a named function/source identity.
        let facts = parsed
            .entered_macro_facts(owner_body, &cfg, &item.scope)
            .unwrap();
        facts
            .items
            .iter()
            .find(|row| row.item.def_key.declared_name == nested)
            .unwrap()
            .item
            .body
            .clone()
            .unwrap()
    } else {
        owner_body.clone()
    };
    let index = parsed.index_body(&body, Default::default()).unwrap();
    (parsed, body, index)
}

fn resolved(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    path: &ast::PathExpr,
) -> Option<NodeId> {
    local_binding(parsed, index, body, path).unwrap()
}

#[test]
fn selected_nested_function_parameter_is_not_outer_arm_binding() {
    const SOURCE: &str =
        "fn chosen(c:u8)->u8{match c{v if true=>{fn nested(v:u8)->u8{v}nested(2u8)},_=>0u8}}";
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(SOURCE, edition, "chosen", Some("nested"));
        let root = parsed.entered_body_root(&body).unwrap();
        let path = root.descendants().find_map(ast::PathExpr::cast).unwrap();
        assert_eq!(path.syntax().text().to_string(), "v");
        assert_eq!(resolved(&parsed, &body, &index, &path), None);
        assert_eq!(
            parameter_at(
                &parsed,
                path.syntax(),
                &MacroName::parse("v", parsed.edition).unwrap(),
            )
            .unwrap(),
            Some(0)
        );
        let node = id_for(&index, &body, path.syntax()).unwrap();
        let recipe = parsed
            .initial_thunk(&body, &index, &index.pc(node, Phase::Entry).unwrap())
            .unwrap();
        assert!(matches!(
            recipe.recipe().operations.as_ref(),
            [Op::ReadParameter { ordinal: 0, destination }]
                if Some(*destination) == index.nodes()[node.0 as usize].value
        ));
    }
}

#[test]
fn same_body_guard_and_result_use_exact_arm_binding() {
    const SOURCE: &str = "fn chosen(v:u8)->u8{let v=9u8;match v{v if v==1u8=>v,_=>0u8}}";
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(SOURCE, edition, "chosen", None);
        let root = parsed.entered_body_root(&body).unwrap();
        let matched = root.descendants().find_map(ast::MatchExpr::cast).unwrap();
        let arm = matched.match_arm_list().unwrap().arms().next().unwrap();
        let pattern = arm.pat().unwrap();
        let binding = id_for(&index, &body, pattern.syntax()).unwrap();
        let ast::Expr::PathExpr(scrutinee) = matched.expr().unwrap() else {
            panic!("source control scrutinee")
        };
        let outer = resolved(&parsed, &body, &index, &scrutinee).unwrap();
        assert_ne!(outer, binding);
        let guard = arm.guard().unwrap().condition().unwrap();
        let guard_path = guard
            .syntax()
            .descendants()
            .find_map(ast::PathExpr::cast)
            .unwrap();
        assert_eq!(resolved(&parsed, &body, &index, &guard_path), Some(binding));
        let ast::Expr::PathExpr(result) = arm.expr().unwrap() else {
            panic!("source control arm result")
        };
        assert_eq!(resolved(&parsed, &body, &index, &result), Some(binding));
    }
}

#[test]
fn inner_let_shadow_keeps_initializer_at_outer_arm() {
    const SOURCE: &str = "fn chosen(c:u8)->u8{match c{v if true=>{{let v=v;let _=v;}v},_=>0u8}}";
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(SOURCE, edition, "chosen", None);
        let root = parsed.entered_body_root(&body).unwrap();
        let matched = root.descendants().find_map(ast::MatchExpr::cast).unwrap();
        let arm = matched.match_arm_list().unwrap().arms().next().unwrap();
        let arm_pattern = arm.pat().unwrap();
        let arm_binding = id_for(&index, &body, arm_pattern.syntax()).unwrap();
        let result = arm.expr().unwrap();
        let statement = result
            .syntax()
            .descendants()
            .find_map(ast::LetStmt::cast)
            .unwrap();
        let shadow_pattern = statement.pat().unwrap();
        let shadow = id_for(&index, &body, shadow_pattern.syntax()).unwrap();
        assert_ne!(arm_binding, shadow);
        let paths: Vec<_> = result
            .syntax()
            .descendants()
            .filter_map(ast::PathExpr::cast)
            .filter(|path| path.syntax().text() == "v")
            .collect();
        assert_eq!(paths.len(), 3);
        assert_eq!(
            resolved(&parsed, &body, &index, &paths[0]),
            Some(arm_binding)
        );
        assert_eq!(resolved(&parsed, &body, &index, &paths[1]), Some(shadow));
        assert_eq!(
            resolved(&parsed, &body, &index, &paths[2]),
            Some(arm_binding)
        );
    }
}

#[test]
fn original_false_guard_and_next_binding_have_distinct_declarations() {
    const ORIGINAL: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/match/guards.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(ORIGINAL, edition, "main", None);
        let matched = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .find_map(ast::MatchExpr::cast)
            .unwrap();
        let arms: Vec<_> = matched.match_arm_list().unwrap().arms().take(2).collect();
        assert_eq!(arms.len(), 2);
        let mut ids = Vec::new();
        for arm in arms {
            let pattern = arm.pat().unwrap();
            assert!(matches!(pattern, ast::Pat::IdentPat(_)));
            let binding = id_for(&index, &body, pattern.syntax()).unwrap();
            let guard = arm.guard().unwrap().condition().unwrap();
            let path = guard
                .syntax()
                .descendants()
                .find_map(ast::PathExpr::cast)
                .unwrap();
            assert_eq!(resolved(&parsed, &body, &index, &path), Some(binding));
            ids.push(binding);
        }
        assert_ne!(ids[0], ids[1]);
    }
}
