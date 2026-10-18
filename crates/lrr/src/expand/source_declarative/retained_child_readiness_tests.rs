//! LRR structural authority controls; no guest execution or stock acceptance.
use super::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

/// The retained tree of a body-scoped preparation of `cold`, a sibling of
/// `main`: every call of `main` (nested functions included) is still pending
/// in that tree, the state a readiness proof must refuse. Preparing `main`
/// itself now expands the calls of its nested functions (see
/// `preparing_a_body_expands_the_calls_of_its_nested_functions`).
fn selected_main(text: &str, edition: u16) -> (PreparedSourceSyntax, ParsedFile) {
    let (provider, index) = provider(text, edition);
    let definition = provider
        .module_source()
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "cold")
        .unwrap()
        .def_key
        .fingerprint();
    let prepared = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .finish_for_body(definition)
        .unwrap();
    assert!(prepared.permits_body(definition));
    assert_eq!(
        prepared.pending_calls().len(),
        1,
        "cold residual call must remain explicit"
    );
    let parsed =
        ParsedFile::from_syntax_snapshot(prepared.syntax().clone(), provider.profile().clone())
            .unwrap();
    (prepared, parsed)
}
fn actual_body(parsed: &ParsedFile, name: &str) -> SourceRange {
    let tree = parsed.parsed.tree();
    let mut functions = tree
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| function.name().is_some_and(|actual| actual.text() == name));
    let function = functions.next().expect("actual nested function absent");
    assert!(functions.next().is_none(), "ambiguous test function");
    let body = function.body().expect("actual nested body absent");
    let range = body.syntax().text_range();
    SourceRange::checked(range.start().into(), range.end().into()).unwrap()
}

#[test]
fn actual_child_owned_direct_and_closure_residual_calls_cannot_mint_readiness() {
    for edition in [2015, 2018, 2021, 2024] {
        for closure in [false, true] {
            let text = format!(
                "macro_rules! pending{{()=>{{()}}}}fn cold(){{}}fn main(){{fn f(){{{}}}f();}}",
                if closure {
                    "let c=||{pending!()};c();"
                } else {
                    "pending!();"
                }
            );
            let (prepared, parsed) = selected_main(&text, edition);
            let body = actual_body(&parsed, "f");
            let before_identity = prepared.identity();
            let before_pending = prepared.pending_calls().to_vec();
            let error = prepared
                .require_closed_item_body(&parsed, body)
                .unwrap_err();
            assert_eq!(
                error.code, "LRR-E-EXPANSION-INPUT",
                "edition={edition} closure={closure}: {error:?}"
            );
            assert_eq!(
                error.message,
                "local body still has pending expansion calls"
            );
            assert_eq!(prepared.identity(), before_identity);
            assert_eq!(prepared.pending_calls(), before_pending);
            assert_eq!(prepared.outputs().len(), 0);
            assert_eq!(
                parsed
                    .parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .filter_map(ast::ClosureExpr::cast)
                    .count(),
                usize::from(closure)
            );
        }
    }
}

#[test]
fn actual_nested_function_residual_stays_pending_without_blocking_its_closed_parent() {
    const TEXT: &str =
        "macro_rules! pending{()=>{()}}fn cold(){}fn main(){fn f(){fn g(){pending!();}0_i32;}f();}";
    for edition in [2015, 2018, 2021, 2024] {
        let (prepared, parsed) = selected_main(TEXT, edition);
        let f = actual_body(&parsed, "f");
        let g = actual_body(&parsed, "g");
        assert!(f.contains(g) && f != g);
        let before_identity = prepared.identity();
        let before_pending = prepared.pending_calls().to_vec();
        let certificate = prepared.require_closed_item_body(&parsed, f).unwrap();
        assert_eq!(
            prepared.require_closed_item_body(&parsed, f).unwrap(),
            certificate
        );
        let error = prepared.require_closed_item_body(&parsed, g).unwrap_err();
        assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
        assert_eq!(
            error.message,
            "local body still has pending expansion calls"
        );
        // Equal displayed source parsed without its retained attachment is not
        // authority over this prepared snapshot.
        let blob = SourceBlob::new(prepared.syntax().syntax().text.as_bytes().to_vec()).unwrap();
        let plain = ParsedFile::parse(&blob, parsed.profile.clone()).unwrap();
        let error = prepared.require_closed_item_body(&plain, f).unwrap_err();
        assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
        assert_eq!(error.message, "local readiness retained tree/body differs");
        assert_eq!(prepared.identity(), before_identity);
        assert_eq!(prepared.pending_calls(), before_pending);
        assert!(prepared.outputs().is_empty());
    }
}

/// A block-local function shares its enclosing body's retained tree, so
/// preparing the enclosing module-level body demands the calls of every
/// nested function (closures, doubly nested functions included) and refuses to
/// mint its scope while any remains; a sibling body demands none of them.
#[test]
fn preparing_a_body_expands_the_calls_of_its_nested_functions() {
    for edition in [2015, 2018, 2021, 2024] {
        for nested in [
            "fn f(){pending!();}f();",
            "fn f(){let c=||{pending!()};c();}f();",
            "fn f(){fn g(){pending!();}g();}f();",
        ] {
            let text =
                format!("macro_rules! pending{{()=>{{()}}}}fn cold(){{}}fn main(){{{nested}}}");
            let (provider, index) = provider(&text, edition);
            let definition = |name: &str| {
                provider
                    .module_source()
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == name)
                    .unwrap()
                    .def_key
                    .fingerprint()
            };
            let assembly = SourceSyntaxAssembly::new(&provider, &index, 1).unwrap();
            assert!(
                assembly
                    .next_body_invocation(definition("main"))
                    .unwrap()
                    .is_some(),
                "edition={edition} {nested}: the nested call must be demanded"
            );
            assert!(
                assembly
                    .next_body_invocation(definition("cold"))
                    .unwrap()
                    .is_none(),
                "edition={edition} {nested}: a sibling body owns no nested call"
            );
            let error = assembly.finish_for_body(definition("main")).err().unwrap();
            assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
            assert_eq!(
                error.message,
                "selected assembly body still has pending expansion calls"
            );
        }
    }
}
