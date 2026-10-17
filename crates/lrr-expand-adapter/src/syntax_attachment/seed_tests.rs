//! Pinned syntax/hygiene transport only; no source authority or guest pass.
use super::*;
use crate::{declarative::DeclarativeMacro, token_wire::*};
use syntax::ast::HasName;

const EDITIONS: [Edition; 4] = [
    Edition::E2015,
    Edition::E2018,
    Edition::E2021,
    Edition::E2024,
];
fn input(text: &str, edition: Edition, unit: u8) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([unit; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: format!("seed-{unit}.rs"),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([unit; 32]),
            edition,
        },
        Key([unit + 1; 32]),
    )
    .unwrap()
}
fn call_named(tree: &SyntaxAttachment, name: &str) -> declaration_output::CallCapture {
    tree.materialize()
        .unwrap()
        .calls
        .into_iter()
        .find(|call| {
            declaration_output::materialize(&call.path, DeclarationGrammar::Expression)
                .unwrap()
                .text
                .trim()
                == name
        })
        .unwrap()
}
fn expand(tree: &SyntaxAttachment, call: &declaration_output::CallCapture) -> TokenForest {
    let captured = tree.materialize().unwrap();
    DeclarativeMacro::new(captured.definitions[0].body.clone())
        .unwrap()
        .expand(&call.input, call.site)
        .unwrap()
        .unwrap()
        .tokens
}

#[test]
fn actual_upstream_associated_headers_survive_shared_forks_all_editions() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SyntaxAttachmentSeed>();
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
    );
    for edition in EDITIONS {
        let tree =
            SyntaxAttachment::parse(&input(original, edition, 91), DeclarationGrammar::Items)
                .unwrap();
        let identity = tree.syntax_identity().unwrap();
        let seed = tree.into_seed().unwrap();
        assert_eq!(seed.identity(), identity);
        assert!(seed.metadata_bytes() >= std::mem::size_of::<SyntaxAttachmentSeed>());
        let untouched = seed.fork().unwrap();
        let mut changed = seed.fork().unwrap();
        assert!(Arc::ptr_eq(&untouched.origins, &changed.origins));
        assert!(Arc::ptr_eq(&untouched.store, &changed.store));
        assert_eq!(
            seed.text_len(),
            u32::from(untouched.root.text_range().len())
        );
        let before = untouched.snapshot().unwrap();
        for slot in [ListSlot::TraitItems, ListSlot::ImplItems, ListSlot::Items] {
            let call = call_named(&changed, "def_fn");
            let output = expand(&changed, &call);
            changed
                .attach_list(call.start, call.end, &output, slot)
                .unwrap();
        }
        assert!(!Arc::ptr_eq(&untouched.store, &changed.store));
        let functions = changed
            .root
            .descendants()
            .filter_map(ast::Fn::cast)
            .filter(|f| f.name().is_some_and(|n| n.text() == "bar"))
            .count();
        assert_eq!(functions, 3);
        let after = untouched.snapshot().unwrap();
        assert_eq!(before.identity(), after.identity());
        assert_eq!(
            before.origin_records().encode().unwrap(),
            after.origin_records().encode().unwrap()
        );
        assert_eq!(seed.fork().unwrap().syntax_identity().unwrap(), identity);
        let reissued = changed.into_seed().unwrap();
        assert_eq!(
            reissued.fork().unwrap().syntax_identity().unwrap(),
            reissued.identity()
        );
    }
}

#[test]
fn fork_preserves_attached_grouping_and_mixed_origin_records() {
    let mut tree = SyntaxAttachment::parse(
        &input("fn main(){let n=selected!()*3_i32;}", Edition::E2024, 41),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let call = call_named(&tree, "selected");
    tree.attach(
        call.start,
        call.end,
        &input("1_i32+2_i32", Edition::E2015, 51),
        FragmentSlot::Expression,
    )
    .unwrap();
    let before = tree.snapshot().unwrap();
    let seed = tree.into_seed().unwrap();
    let fork = seed.fork().unwrap();
    let after = fork.snapshot().unwrap();
    assert_eq!(before.identity(), after.identity());
    assert_eq!(
        before.origin_records().encode().unwrap(),
        after.origin_records().encode().unwrap()
    );
    let binding = fork
        .root
        .descendants()
        .find_map(ast::LetStmt::cast)
        .unwrap();
    let outer = ast::BinExpr::cast(binding.initializer().unwrap().syntax().clone()).unwrap();
    assert!(ast::BinExpr::cast(outer.lhs().unwrap().syntax().clone()).is_some());
    assert!(
        after
            .origin_records()
            .spans()
            .iter()
            .any(|span| span.edition == Edition::E2015)
    );
    assert!(
        after
            .origin_records()
            .spans()
            .iter()
            .any(|span| span.edition == Edition::E2024)
    );
    let flattened = SyntaxAttachment::parse(
        &input(&outer.syntax().text().to_string(), Edition::E2024, 61),
        DeclarationGrammar::Expression,
    )
    .unwrap();
    let reparsed = ast::BinExpr::cast(flattened.root.clone()).unwrap();
    assert!(ast::BinExpr::cast(reparsed.lhs().unwrap().syntax().clone()).is_none());
}

#[test]
fn failed_real_output_edit_preserves_seed_and_other_forks() {
    let tree = SyntaxAttachment::parse(
        &input("fn main(){selected!();}", Edition::E2024, 71),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let seed = tree.into_seed().unwrap();
    let mut fork = seed.fork().unwrap();
    let call = call_named(&fork, "selected");
    assert!(
        fork.attach(
            call.start,
            call.end,
            &input("let broken =;", Edition::E2024, 81),
            FragmentSlot::Expression
        )
        .is_err()
    );
    assert_eq!(fork.syntax_identity().unwrap(), seed.identity());
    assert!(Arc::ptr_eq(&fork.store, &seed.store));
    assert_eq!(
        seed.fork().unwrap().syntax_identity().unwrap(),
        seed.identity()
    );
}

#[test]
fn seed_limit_and_poison_are_typed_errors() {
    let mut tree = SyntaxAttachment::parse(
        &input("fn main(){}", Edition::E2024, 31),
        DeclarationGrammar::Items,
    )
    .unwrap();
    tree.retained_input_bytes = MAX_RETAINED_INPUT + 1;
    assert!(tree.into_seed().is_err());
    let tree = SyntaxAttachment::parse(
        &input("fn main(){}", Edition::E2024, 32),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let seed = tree.into_seed().unwrap();
    let store = Arc::clone(&seed.store);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = store.lock().unwrap();
        panic!("LRR poison authority control");
    }));
    assert!(seed.fork().is_err());
}
