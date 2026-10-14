//! Source-owned lexical plan controls; namespace/admission runtime tests are separate.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn plan(
    text: &str,
    edition: u16,
    call: &str,
    target: &str,
) -> Result<LexicalFunctionPlan, Diagnostic> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"LRR lexical plan exact source"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let item = outline
        .items
        .iter()
        .find(|row| row.def_key.declared_name == "main")
        .unwrap();
    let body = item.body.as_ref().unwrap();
    let index = parsed.index_body(body, Default::default()).unwrap();
    let at = u32::try_from(text.find(call).unwrap()).unwrap() - body.range.start;
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == at && node.range.end == at + u32::try_from(call.len()).unwrap()
        })
        .unwrap();
    let facts = parsed
        .entered_macro_facts(body, index.cfg(), &item.scope)
        .unwrap();
    let selected = facts
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == target)
        .unwrap()
        .item
        .def_key
        .fingerprint();
    parsed.lexical_numeric_function_plan(
        body,
        &index,
        &index.pc(node.id, Phase::Entry).unwrap(),
        &item.header,
        &DeclarationContext {
            scope: item.scope.clone(),
            range: item.declaration_span,
        },
        selected,
        target,
        11,
        19,
    )
}
#[test]
fn all_nine_unchanged_original_lexical_headers_and_body_handles_are_exact_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (name, ty) in [
            ("id_i8", PrimitiveType::I8),
            ("id_i16", PrimitiveType::I16),
            ("id_i32", PrimitiveType::I32),
            ("id_i64", PrimitiveType::I64),
            ("id_uint", PrimitiveType::Usize),
            ("id_u8", PrimitiveType::U8),
            ("id_u16", PrimitiveType::U16),
            ("id_u32", PrimitiveType::U32),
            ("id_u64", PrimitiveType::U64),
        ] {
            let call = format!("{name}(j)");
            let p = plan(ORIGINAL, edition, &call, name).unwrap();
            assert_eq!(p.signature().parameters.len(), 1);
            assert!(p.signature().parameters[0].symbolic.is_none());
            assert!(p.signature().result_symbolic.is_none());
            assert_eq!(p.primitive_parameters(), &[ty]);
            assert_eq!(p.primitive_result(), ty);
            assert_eq!(p.signature().definition, p.item().def_key.fingerprint());
            assert_eq!(p.source().profile.edition, edition);
            assert_eq!(p.item().header.profile.edition, edition);
            let child = p.item().body.as_ref().unwrap();
            assert_eq!(child.def_key, p.item().def_key.fingerprint());
            assert_eq!(
                Digest::of_bytes(
                    &ORIGINAL.as_bytes()[child.range.start as usize..child.range.end as usize]
                ),
                child.body_hash
            );
            assert_eq!(
                &ORIGINAL[p.path_range().start as usize..p.path_range().end as usize],
                name
            );
            assert_eq!(p.site().body, p.caller().key());
            assert!(p.metadata_bytes() <= 32 * 1024 * 1024);
            assert_eq!(p.source_revision(), 11);
            assert_eq!(p.query_revision(), 19);
            assert_ne!(p.identity(), Digest::of_bytes(b"not a lexical plan"));
        }
    }
}
#[test]
fn generic_nominal_deferred_async_and_parameter_attribute_headers_are_not_erased() {
    for text in [
        "fn main(){fn id<T>(n:T)->T{n}id(1);}",
        "fn main(){struct I;fn id(n:I)->I{n}id(1);}",
        "fn main(){async fn id(n:i8)->i8{n}id(1);}",
        "fn main(){fn id(#[cfg(any())]n:i8)->i8{n}id(1);}",
        "fn main(){fn id(n:i8){n;}id(1);}",
        "fn main<i8>(){fn id(n:i8)->i8{n}id(1);}",
    ] {
        assert_eq!(
            plan(text, 2024, "id(1)", "id").unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
    }
}
#[test]
fn original_local_call_recipe_keeps_argument_thunks_cold_and_contains_no_unsupported_skip() {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"LRR lexical recipe source"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let main = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let index = parsed.index_body(main, Default::default()).unwrap();
    let start = u32::try_from(ORIGINAL.find("id_i8(j)").unwrap()).unwrap() - main.range.start;
    let node = index
        .nodes()
        .iter()
        .find(|node| node.range.start == start && node.range.end == start + 8)
        .unwrap();
    let recipe = parsed
        .initial_thunk(main, &index, &index.pc(node.id, Phase::Entry).unwrap())
        .unwrap();
    assert!(
        matches!(recipe.recipe().operations.as_ref(),[crate::ir::op::Op::CallPrepare {name,..}]if name=="id_i8")
    );
    assert_eq!(
        recipe.recipe().exit,
        crate::ir::recipe::RecipeExit::Next(index.pc(node.operands[1], Phase::Entry).unwrap())
    );
    // Pure preparation of this entry only; no argument/callee index or execution.
}
