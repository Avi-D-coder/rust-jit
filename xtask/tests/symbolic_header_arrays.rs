//! Array header component controls derived from unchanged official declarations.
//! The test-only function-pointer wrappers are adaptations, not stock fixture passes.
use lrr::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::op::PrimitiveType,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, signatures::GenericKind, type_aliases::TypeAliasRecipe},
    traits::{
        subst::{Substitution, validate_closed},
        term::{BoundKind, ConstTerm, GenericArg, ParamKey, RegionTerm, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasName},
};

const RETURN_ARRAY: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/issues/issue-70180-1-stalled_on.rs"
);
const REGIONS: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/coercion/coerce-unsize-subtype.rs"
);
const MISSING_CONST: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/missing/missing-items/missing-const-parameter.rs"
);
const CONST_EXPRESSION: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/ensure_is_evaluatable.rs"
);
const PRIMITIVE_POINTER: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/abi/debug.rs");

fn source(text: &str) -> (Database, PreparedSourceView) {
    source_with_identity(text, Digest::of_bytes(text.as_bytes()))
}

// A source edit preserves the owning crate/definition identity. Content-derived
// identities used by isolated fixture helpers cannot prove alpha equivalence.
fn source_with_identity(text: &str, crate_key: Digest) -> (Database, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
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
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source missing")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, view)
}

fn definition(view: &PreparedSourceView, name: &str) -> Digest {
    view.outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint()
}

fn cold(db: &Database) {
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
}

/// Preserve the original types and ordered generic declarations. Only the
/// function item name/patterns/body are replaced by an anonymous pointer alias;
/// the original where clause remains owned metadata on the alias declaration.
/// The binder mode relocates the original lifetime declarations into `for`;
/// each extra anonymous pointer layer is an LRR capture-avoidance control.
/// This adapter uses the pinned parser solely in the test fixture harness.
fn pointer_adapter(original: &str, name: &str, bind_regions: bool, layers: usize) -> String {
    let parsed = ast::SourceFile::parse(original, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let function = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().unwrap().text() == name)
        .unwrap();
    let mut generics = Vec::new();
    let mut regions = Vec::new();
    for parameter in function.generic_param_list().unwrap().generic_params() {
        let written = parameter.syntax().text().to_string();
        if bind_regions && matches!(parameter, ast::GenericParam::LifetimeParam(_)) {
            regions.push(written);
        } else {
            generics.push(written);
        }
    }
    let parameters: Vec<_> = function
        .param_list()
        .unwrap()
        .params()
        .map(|parameter| parameter.ty().unwrap().syntax().text().to_string())
        .collect();
    let result = function
        .ret_type()
        .map_or_else(String::new, |result| result.syntax().text().to_string());
    let predicates = function
        .where_clause()
        .map_or_else(String::new, |clause| clause.syntax().text().to_string());
    let mut pointer = format!("fn({}) {result}", parameters.join(","));
    for _ in 0..layers {
        pointer = format!("fn({pointer})");
    }
    if !regions.is_empty() {
        pointer = format!("for<{}> {pointer}", regions.join(","));
    }
    let generics = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(","))
    };
    format!("type ArrayHeader{generics} {predicates} = {pointer};")
}

fn alias(text: &str, name: &str) -> Result<Box<TypeAliasRecipe>, Diagnostic> {
    alias_with_identity(text, name, Digest::of_bytes(text.as_bytes()))
}

fn alias_with_identity(
    text: &str,
    name: &str,
    crate_key: Digest,
) -> Result<Box<TypeAliasRecipe>, Diagnostic> {
    let (mut db, view) = source_with_identity(text, crate_key);
    let key = definition(&view, name);
    let result = match db.fetch(&SyntaxProvider, QueryKey::TypeAlias(key)) {
        QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
            QueryValue::TypeAlias(recipe) => Ok(recipe.clone()),
            _ => panic!("alias query returned another record"),
        },
        QueryPoll::Ready(Err(error)) => Err(error),
        QueryPoll::NeedInput(input) => panic!("unexpected input: {input:?}"),
    };
    cold(&db);
    result
}

fn parameter(recipe: &TypeAliasRecipe, name: &str, kind: GenericKind) -> ParamKey {
    ParamKey {
        definition: recipe.definition,
        ordinal: recipe
            .generics
            .iter()
            .find(|parameter| parameter.name == name && parameter.kind == kind)
            .unwrap()
            .ordinal,
    }
}

fn function(term: &TyTerm) -> &lrr::traits::term::Binder<lrr::traits::term::FunctionSignature> {
    let TyTerm::Function(function) = term else {
        panic!("function pointer missing")
    };
    function
}

#[test]
fn original_array_return_adapter_preserves_rigid_type_and_const_namespaces() {
    let (mut db, view) = source(RETURN_ARRAY);
    let original_definition = definition(&view, "default_array");
    let QueryPoll::Ready(Ok(value)) = db.fetch(
        &SyntaxProvider,
        QueryKey::FunctionSignature(original_definition),
    ) else {
        panic!("original generic array signature missing")
    };
    let QueryValue::FunctionSignature(original) = value.value.as_ref() else {
        panic!("original generic array signature result missing")
    };
    assert_eq!(
        original.result_symbolic,
        Some(TyTerm::Array(
            Box::new(TyTerm::Param(ParamKey {
                definition: original_definition,
                ordinal: 0,
            })),
            ConstTerm::Param(ParamKey {
                definition: original_definition,
                ordinal: 1,
            })
        ))
    );
    assert!(!original.predicates.is_empty());
    cold(&db);
    let text = pointer_adapter(RETURN_ARRAY, "default_array", false, 0);
    let recipe = alias(&text, "ArrayHeader").unwrap();
    assert_eq!(recipe.generics, original.generics);
    let ty = parameter(&recipe, "T", GenericKind::Type);
    let length = parameter(&recipe, "N", GenericKind::Const);
    assert_ne!(ty, length);
    let symbolic = recipe.symbolic.as_ref().unwrap();
    let pointer = function(symbolic);
    assert!(pointer.variables.is_empty());
    assert!(pointer.value.parameters.is_empty());
    assert_eq!(
        *pointer.value.result,
        TyTerm::Array(Box::new(TyTerm::Param(ty)), ConstTerm::Param(length))
    );
    validate_closed(symbolic).unwrap();
    // Wrong-namespace substitution entries are LRR admission controls. They
    // cannot replace the original type or length even at the same ParamKey.
    let mut substitution = Substitution::default();
    substitution
        .insert(ty, GenericArg::Const(ConstTerm::Value(4)))
        .unwrap();
    substitution
        .insert(
            length,
            GenericArg::Type(TyTerm::Primitive(PrimitiveType::Unit)),
        )
        .unwrap();
    assert_eq!(substitution.apply(symbolic).unwrap(), *symbolic);
    substitution
        .insert(length, GenericArg::Const(ConstTerm::Value(4)))
        .unwrap();
    let substituted = substitution.apply(symbolic).unwrap();
    assert_eq!(
        *function(&substituted).value.result,
        TyTerm::Array(Box::new(TyTerm::Param(ty)), ConstTerm::Value(4))
    );

    // Alpha-renaming the original T/N identifiers preserves symbolic identity
    // when source content changes inside the same owning definition. This is
    // a disclosed header adaptation, not execution of Default or Foo.
    let renamed = text.replace("T", "Element").replace("N", "COUNT");
    let owner = Digest::of_bytes(RETURN_ARRAY.as_bytes());
    let before = alias_with_identity(&text, "ArrayHeader", owner).unwrap();
    let after = alias_with_identity(&renamed, "ArrayHeader", owner).unwrap();
    assert_eq!(before.definition, after.definition);
    assert_ne!(before.generics, after.generics);
    assert_eq!(before.symbolic, after.symbolic);
    assert_eq!(
        before.symbolic.as_ref().unwrap().fingerprint(),
        after.symbolic.as_ref().unwrap().fingerprint()
    );
    assert_eq!(
        parameter(&before, "T", GenericKind::Type),
        parameter(&after, "Element", GenericKind::Type)
    );
    assert_eq!(
        parameter(&before, "N", GenericKind::Const),
        parameter(&after, "COUNT", GenericKind::Const)
    );
}

#[test]
fn original_array_reference_adapter_preserves_rigid_lifetimes_and_literal_length() {
    let text = pointer_adapter(REGIONS, "long_and_short_lub2", false, 0);
    let recipe = alias(&text, "ArrayHeader").unwrap();
    let a = RegionTerm::Param(parameter(&recipe, "'a", GenericKind::Lifetime));
    let b = RegionTerm::Param(parameter(&recipe, "'b", GenericKind::Lifetime));
    let ty = TyTerm::Param(parameter(&recipe, "T", GenericKind::Type));
    let symbolic = recipe.symbolic.as_ref().unwrap();
    assert_eq!(
        function(symbolic).value.parameters,
        vec![
            TyTerm::Ref(
                b.clone(),
                false,
                Box::new(TyTerm::Slice(Box::new(TyTerm::Ref(
                    RegionTerm::Static,
                    false,
                    Box::new(ty.clone())
                ))))
            ),
            TyTerm::Ref(
                b,
                false,
                Box::new(TyTerm::Array(
                    Box::new(TyTerm::Ref(a, false, Box::new(ty))),
                    ConstTerm::Value(1)
                ))
            )
        ]
    );
    validate_closed(symbolic).unwrap();
    // Compare lifetime/type spelling edits in one immutable definition world.
    let renamed = text
        .replace("'a", "'first")
        .replace("'b", "'second")
        .replace("T", "Element");
    let owner = Digest::of_bytes(REGIONS.as_bytes());
    let before = alias_with_identity(&text, "ArrayHeader", owner).unwrap();
    let after = alias_with_identity(&renamed, "ArrayHeader", owner).unwrap();
    assert_eq!(before.definition, after.definition);
    assert_ne!(before.generics, after.generics);
    assert_eq!(before.symbolic, after.symbolic);
    assert_eq!(
        before.symbolic.as_ref().unwrap().fingerprint(),
        after.symbolic.as_ref().unwrap().fingerprint()
    );
    let bound_text = pointer_adapter(REGIONS, "long_and_short_lub2", true, 0);
    let bound = alias_with_identity(&bound_text, "ArrayHeader", owner).unwrap();
    assert_eq!(before.definition, bound.definition);
    assert_ne!(
        before.symbolic.as_ref().unwrap().fingerprint(),
        bound.symbolic.as_ref().unwrap().fingerprint()
    );
    let TyTerm::Ref(free, _, _) = &function(before.symbolic.as_ref().unwrap()).value.parameters[1]
    else {
        panic!("original free lifetime lost")
    };
    let TyTerm::Ref(bound_region, _, _) =
        &function(bound.symbolic.as_ref().unwrap()).value.parameters[1]
    else {
        panic!("original bound lifetime lost")
    };
    assert!(matches!(free, RegionTerm::Param(_)));
    assert_eq!(*bound_region, RegionTerm::Bound { depth: 0, index: 1 });
}

#[test]
fn original_array_reference_binder_adapter_counts_empty_nested_function_scopes() {
    // A source-derived deep but admitted header must retain exact binder
    // identity on the normal host test stack, without widening the quota.
    for layers in [0, 1, 2, 120] {
        let text = pointer_adapter(REGIONS, "long_and_short_lub2", true, layers);
        let recipe = alias(&text, "ArrayHeader").unwrap();
        let ty = TyTerm::Param(parameter(&recipe, "T", GenericKind::Type));
        let symbolic = recipe.symbolic.as_ref().unwrap();
        validate_closed(symbolic).unwrap();
        // Existing original-derived deep wrapper: a legal term must survive
        // pure identity substitution and zero-distance shifting on the ordinary
        // host test stack, preserving empty scopes and exact region identities.
        let copied = Substitution::default().apply(symbolic).unwrap();
        assert_eq!(&copied, symbolic);
        validate_closed(&copied).unwrap();
        let shifted = lrr::traits::subst::shift(symbolic, 0, 0).unwrap();
        assert_eq!(&shifted, symbolic);
        validate_closed(&shifted).unwrap();

        let mut pointer = function(symbolic);
        assert_eq!(pointer.variables, vec![BoundKind::Region; 2]);
        for _ in 0..layers {
            assert_eq!(pointer.value.parameters.len(), 1);
            pointer = function(&pointer.value.parameters[0]);
            assert!(pointer.variables.is_empty());
        }
        let depth = u32::try_from(layers).unwrap();
        assert_eq!(
            pointer.value.parameters[1],
            TyTerm::Ref(
                RegionTerm::Bound { depth, index: 1 },
                false,
                Box::new(TyTerm::Array(
                    Box::new(TyTerm::Ref(
                        RegionTerm::Bound { depth, index: 0 },
                        false,
                        Box::new(ty)
                    )),
                    ConstTerm::Value(1)
                ))
            )
        );
    }
}

#[test]
fn array_header_substitution_cannot_capture_an_inserted_free_region() {
    let text = pointer_adapter(REGIONS, "long_and_short_lub2", true, 1);
    let recipe = alias(&text, "ArrayHeader").unwrap();
    let symbolic = recipe.symbolic.as_ref().unwrap();
    let before = symbolic.fingerprint();
    let mut substitution = Substitution::default();
    // Deliberately corrupt the rigid T replacement with a free bound region.
    // It must shift across both pointer binders and remain invalid at the root.
    substitution
        .insert(
            parameter(&recipe, "T", GenericKind::Type),
            GenericArg::Type(TyTerm::Ref(
                RegionTerm::Bound { depth: 0, index: 0 },
                false,
                Box::new(TyTerm::Primitive(PrimitiveType::Unit)),
            )),
        )
        .unwrap();
    let substituted = substitution.apply(symbolic).unwrap();
    let nested = function(&function(&substituted).value.parameters[0]);
    let TyTerm::Ref(_, _, array) = &nested.value.parameters[1] else {
        panic!("outer reference lost")
    };
    let TyTerm::Array(element, _) = array.as_ref() else {
        panic!("array lost")
    };
    let TyTerm::Ref(region, _, replacement) = element.as_ref() else {
        panic!("array reference lost")
    };
    assert_eq!(*region, RegionTerm::Bound { depth: 1, index: 0 });
    let TyTerm::Ref(inserted, _, _) = replacement.as_ref() else {
        panic!("replacement lost")
    };
    assert_eq!(*inserted, RegionTerm::Bound { depth: 2, index: 0 });
    assert_eq!(
        validate_closed(&substituted).unwrap_err().code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(symbolic.fingerprint(), before);
    validate_closed(symbolic).unwrap();
}

#[test]
fn original_missing_and_general_const_array_headers_remain_incomplete_and_cold() {
    for (original, name) in [(MISSING_CONST, "func2"), (CONST_EXPRESSION, "foo")] {
        let (mut db, view) = source(original);
        let QueryPoll::Ready(Ok(value)) = db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(definition(&view, name)),
        ) else {
            panic!("original header missing")
        };
        let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
            panic!("signature missing")
        };
        if name == "func2" {
            assert!(signature.parameters[0].symbolic.is_none());
        } else {
            assert!(signature.result_symbolic.is_none());
        }
        cold(&db);
        let text = pointer_adapter(original, name, false, 0);
        assert_eq!(
            alias(&text, "ArrayHeader").unwrap_err().code,
            "LRR-E-UNSUPPORTED"
        );
    }
}

#[test]
fn original_primitive_function_pointer_array_requires_source_name_resolution() {
    // This is the unchanged original alias. Its concrete primitive spellings
    // have no resolved source identity in this capture API, so no term is
    // fabricated from their text. Original ABI errors/revisions are unexecuted.
    assert_eq!(
        alias(PRIMITIVE_POINTER, "TestAbiNeLarger")
            .unwrap_err()
            .code,
        "LRR-E-UNSUPPORTED"
    );
}

#[test]
fn original_array_header_adapter_rejects_excessive_binder_nesting() {
    // LRR metadata-quota corruption of the source-derived wrapper, not an
    // upstream language case. Rejection must occur before any body request.
    let text = pointer_adapter(REGIONS, "long_and_short_lub2", true, 130);
    assert_eq!(
        alias(&text, "ArrayHeader").unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
}

#[test]
fn array_header_length_corruption_cannot_fabricate_a_literal_or_truncate_overflow() {
    let text = pointer_adapter(RETURN_ARRAY, "default_array", false, 0);
    let array = "[T; N]";
    let start = text.rfind(array).unwrap();
    for replacement in ["_4", "18446744073709551616", "4usize", "0x4", "{4}"] {
        // LRR boundary corruption of the adapted length only: an identifier
        // must not become a number by deleting underscores, overflowing input
        // must not truncate, and unimplemented forms must not be evaluated.
        let mut corrupted = text.clone();
        corrupted.replace_range(start..start + array.len(), &format!("[T; {replacement}]"));
        assert_eq!(
            alias(&corrupted, "ArrayHeader").unwrap_err().code,
            "LRR-E-UNSUPPORTED"
        );
    }
}
