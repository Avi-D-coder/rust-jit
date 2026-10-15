//! Retained syntax through ordinary outlines/indexes/thunks. No source
//! publication, namespace authority or guest execution is fabricated here.
use super::*;
use crate::{
    expand::declarative_test_fixture as fixture,
    ir::pc::Phase,
    source::snapshot::CfgSet,
    syntax::{
        body_index::{BodyIndexLimits, ExpressionKind, NodeRole},
        outline::{BodyHandle, ScopeKey},
    },
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    syntax_attachment::{FragmentSlot, ListSlot, SyntaxAttachment},
    token_wire::{self as wire, Edition as WireEdition, Key},
};

fn source(text: &str, edition: WireEdition) -> wire::TokenForest {
    wire::parse_source(
        text,
        wire::SourceIdentity {
            unit: Key([91; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "retained.rs".into(),
        },
        wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: wire::Transparency::Opaque,
            defining_unit: Key([91; 32]),
            edition,
        },
        Key([92; 32]),
    )
    .unwrap()
}
fn captured(text: &str, rules: &str, edition: WireEdition) -> Arc<SyntaxSnapshot> {
    let mut syntax =
        SyntaxAttachment::parse(&source(text, WireEdition::E2024), DeclarationGrammar::Items)
            .unwrap();
    let call = syntax.materialize().unwrap().calls.remove(0);
    let definition = DeclarativeMacro::new(fixture::definition(rules, edition)).unwrap();
    let (result, _) = definition
        .expand_hygienic_with_limits(
            &call.input,
            call.site,
            DeclarativeSite {
                expansion: Key([93; 32]),
                defining_unit: Key([1; 32]),
                definition_edition: edition,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    syntax
        .attach(
            call.start,
            call.end,
            &result.unwrap().tokens,
            FragmentSlot::Expression,
        )
        .unwrap();
    Arc::new(syntax.snapshot().unwrap())
}
fn parsed(snapshot: Arc<SyntaxSnapshot>) -> ParsedFile {
    ParsedFile::from_syntax_snapshot(snapshot, ProfileIdentity::current()).unwrap()
}
fn body(parsed: &ParsedFile, name: &str) -> BodyHandle {
    parsed
        .outline(Digest([91; 32]), vec![], &CfgSet::guest_default())
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .unwrap()
}

#[test]
fn expanded_expression_uses_normal_body_nodes_and_verified_thunks() {
    let original = "fn main(){let value=selected!();value+3_i32;}";
    let retained = parsed(captured(original, "()=>{1_i32+2_i32}", WireEdition::E2015));
    let plain = ParsedFile::parse(
        &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let handle = body(&retained, "main");
    assert_eq!(handle.def_key, body(&plain, "main").def_key);
    let index = retained
        .index_body(&handle, BodyIndexLimits::default())
        .unwrap();
    assert!(
        !index
            .nodes()
            .iter()
            .any(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
    );
    assert!(
        index
            .nodes()
            .iter()
            .all(|node| node.runtime_scalar_slots.is_empty() && node.runtime_scalar_phases == 0)
    );
    let mut ordinary_expressions = 0;
    for node in index
        .nodes()
        .iter()
        .filter(|node| matches!(node.role, NodeRole::Expression(_)))
    {
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let recipe = retained.initial_thunk(&handle, &index, &pc).unwrap();
        assert_eq!(recipe.recipe().key, pc);
        assert_eq!(recipe.recipe().origin.span.body_key, handle.key());
        ordinary_expressions += 1;
    }
    assert!(ordinary_expressions >= 7);
}

#[test]
fn identical_display_text_cannot_hide_a_different_body_tree() {
    let snapshot = captured(
        "fn main(){let value=selected!()*3_i32;}",
        "()=>{1_i32+2_i32}",
        WireEdition::E2024,
    );
    let attached = parsed(snapshot.clone());
    let mut reparsed = parsed(snapshot.clone());
    // The pinned token-tree sink does not insert lexical whitespace between
    // identifier leaves. Construct the alternative grouping directly, keeping
    // exactly the same leaf tokens, origins, profile and displayed bytes.
    let root = attached.parsed.syntax_node();
    let binding = root.descendants().find_map(ast::LetStmt::cast).unwrap();
    let outer = ast::BinExpr::cast(binding.initializer().unwrap().syntax().clone()).unwrap();
    let inner = ast::BinExpr::cast(outer.lhs().unwrap().syntax().clone()).unwrap();
    let inner_rhs = inner.rhs().unwrap();
    let left_index = outer
        .syntax()
        .children_with_tokens()
        .position(|n| n.as_node() == Some(inner.syntax()))
        .unwrap();
    let right_index = inner
        .syntax()
        .children_with_tokens()
        .position(|n| n.as_node() == Some(inner_rhs.syntax()))
        .unwrap();
    let product = outer
        .syntax()
        .green()
        .replace_child(left_index, inner_rhs.syntax().green().to_owned().into());
    let sum = inner
        .syntax()
        .green()
        .replace_child(right_index, product.into());
    let changed = ra_ap_syntax::SyntaxNode::new_root(outer.syntax().replace_with(sum));
    let mut builder = ra_ap_syntax::SyntaxTreeBuilder::default();
    for event in changed.preorder_with_tokens() {
        match event {
            ra_ap_syntax::WalkEvent::Enter(ra_ap_syntax::NodeOrToken::Node(node)) => {
                builder.start_node(node.kind())
            }
            ra_ap_syntax::WalkEvent::Enter(ra_ap_syntax::NodeOrToken::Token(token)) => {
                builder.token(token.kind(), token.text())
            }
            ra_ap_syntax::WalkEvent::Leave(ra_ap_syntax::NodeOrToken::Node(_)) => {
                builder.finish_node()
            }
            ra_ap_syntax::WalkEvent::Leave(ra_ap_syntax::NodeOrToken::Token(_)) => {}
        }
    }
    reparsed.parsed = builder.finish().cast::<ast::SourceFile>().unwrap();
    let actual = body(&attached, "main");
    let flattened = body(&reparsed, "main");
    assert_eq!(actual.def_key, flattened.def_key);
    assert_eq!(actual.range, flattened.range);
    let a = attached.entered_body_root(&actual).unwrap();
    let b = reparsed.entered_body_root(&flattened).unwrap();
    assert_eq!(a.text().to_string(), b.text().to_string());
    assert_ne!(actual.body_hash, flattened.body_hash);
    assert!(
        reparsed
            .index_body(&actual, BodyIndexLimits::default())
            .is_err()
    );
}

#[test]
fn unrelated_cold_body_edits_preserve_retained_body_identity_across_offset_changes() {
    let left = parsed(captured(
        "fn cold(){} fn live(){selected!();}",
        "()=>{1_i32+2_i32}",
        WireEdition::E2015,
    ));
    let right = parsed(captured(
        "fn cold(){unentered_missing_name_with_longer_text();} fn live(){selected!();}",
        "()=>{1_i32+2_i32}",
        WireEdition::E2015,
    ));
    assert_ne!(left.key(), right.key());
    let a = body(&left, "live");
    let b = body(&right, "live");
    assert_ne!(a.range, b.range);
    assert_eq!(a.key(), b.key());
    assert_eq!(
        left.index_body(&a, BodyIndexLimits::default())
            .unwrap()
            .fingerprint(),
        right
            .index_body(&b, BodyIndexLimits::default())
            .unwrap()
            .fingerprint()
    );
}

#[test]
fn token_owning_editions_participate_in_retained_body_identity() {
    let original = "fn main(){selected!();}";
    let older = parsed(captured(original, "()=>{1_i32+2_i32}", WireEdition::E2015));
    let newer = parsed(captured(original, "()=>{1_i32+2_i32}", WireEdition::E2024));
    let a = body(&older, "main");
    let b = body(&newer, "main");
    assert_eq!(a.def_key, b.def_key);
    assert_eq!(
        older.entered_body_root(&a).unwrap().text().to_string(),
        newer.entered_body_root(&b).unwrap().text().to_string()
    );
    assert_ne!(a.body_hash, b.body_hash);
}

#[test]
fn original_utf8_body_identity_keeps_its_exact_byte_contract() {
    let text = "fn main(){let value=1_i32+2_i32;}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let handle = body(&parsed, "main");
    assert_eq!(
        handle.body_hash,
        Digest::of_bytes(&text.as_bytes()[handle.range.start as usize..handle.range.end as usize])
    );
    parsed
        .index_body(&handle, BodyIndexLimits::default())
        .unwrap();
}

#[test]
fn fused_tokens_can_cross_origin_intervals_without_resolving_their_names() {
    let syntax = SyntaxAttachment::parse(
        &source("fn main(){missing::unentered_name();}", WireEdition::E2024),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let retained = parsed(Arc::new(syntax.snapshot().unwrap()));
    let handle = body(&retained, "main");
    retained
        .index_body(&handle, BodyIndexLimits::default())
        .unwrap();
}

#[test]
fn upstream_nested_statement_items_enter_the_normal_index_and_lexical_inventory() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    for edition in [
        WireEdition::E2015,
        WireEdition::E2018,
        WireEdition::E2021,
        WireEdition::E2024,
    ] {
        let mut syntax =
            SyntaxAttachment::parse(&source(original, edition), DeclarationGrammar::Items).unwrap();
        for definition_index in [1, 0] {
            let capture = syntax.materialize().unwrap();
            let call = &capture.calls[0];
            let definition =
                DeclarativeMacro::new(capture.definitions[definition_index].body.clone()).unwrap();
            let output = definition.expand(&call.input, call.site).unwrap().unwrap();
            syntax
                .attach_list(call.start, call.end, &output.tokens, ListSlot::Statements)
                .unwrap();
        }
        let mut profile = ProfileIdentity::current();
        profile.edition = match edition {
            WireEdition::E2015 => 2015,
            WireEdition::E2018 => 2018,
            WireEdition::E2021 => 2021,
            WireEdition::E2024 => 2024,
        };
        let parsed =
            ParsedFile::from_syntax_snapshot(Arc::new(syntax.snapshot().unwrap()), profile)
                .unwrap();
        let handle = body(&parsed, "main");
        let index = parsed
            .index_body(&handle, BodyIndexLimits::default())
            .unwrap();
        assert!(
            !index
                .nodes()
                .iter()
                .any(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
        );
        let facts = parsed
            .entered_macro_facts(
                &handle,
                &CfgSet::guest_default(),
                &ScopeKey {
                    crate_key: Digest([91; 32]),
                    module_path: vec![],
                    owner: None,
                },
            )
            .unwrap();
        assert!(facts.invocations.is_empty());
        assert_eq!(
            facts
                .items
                .iter()
                .map(|item| item.item.def_key.declared_name.as_str())
                .collect::<Vec<_>>(),
            ["Bar", "Baz", "Xyzzy"]
        );
    }
}

#[test]
fn non_item_snapshots_cannot_be_replayed_as_source_units() {
    let syntax = SyntaxAttachment::parse(
        &source("1_i32+2_i32", WireEdition::E2024),
        DeclarationGrammar::Expression,
    )
    .unwrap();
    assert!(
        ParsedFile::from_syntax_snapshot(
            Arc::new(syntax.snapshot().unwrap()),
            ProfileIdentity::current()
        )
        .is_err()
    );
}
