//! Header-only elision controls. Pointer wrappers and corruptions are disclosed
//! adaptations of exact upstream nodes, never whole stock fixture acceptance.
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
    syntax::{
        parse::ParseKey,
        signatures::{FunctionSignature, GenericKind},
        type_aliases::TypeAliasRecipe,
    },
    traits::{
        subst::{Substitution, validate_closed},
        term::{Binder, BoundKind, GenericArg, ParamKey, RegionTerm, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasName, HasTypeBounds},
};

const POINTERS: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/traits/trait-upcasting/impossible-method-modulo-binders-2.rs"
);
const BORROW: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope-within-loop-ok.rs"
);
const GENERIC_POINTER: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/inline-fn-with-trait-method-as-value.rs"
);
const UNDERSCORE: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/nll/local-outlives-static-via-hrtb.rs"
);
const CALLABLES: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/higher-ranked/anonymous-higher-ranked-lifetime.rs"
);
const NESTED: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/higher-ranked/nested-binder-cmp-fn-sig-print.rs"
);
const PRIMITIVE: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/lifetimes/issue-19707.rs");
const ARRAYS: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/coercion/coerce-unsize-subtype.rs"
);

fn source(text: &str) -> (Database, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
    let crate_key = Digest::of_bytes(text.as_bytes());
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

fn alias(text: &str, name: &str) -> Result<Box<TypeAliasRecipe>, Diagnostic> {
    let (mut db, view) = source(text);
    let result = match db.fetch(
        &SyntaxProvider,
        QueryKey::TypeAlias(definition(&view, name)),
    ) {
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

fn signature(text: &str, name: &str) -> Box<FunctionSignature> {
    let (mut db, view) = source(text);
    let QueryPoll::Ready(Ok(value)) = db.fetch(
        &SyntaxProvider,
        QueryKey::FunctionSignature(definition(&view, name)),
    ) else {
        panic!("signature missing")
    };
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("wrong signature record")
    };
    cold(&db);
    signature.clone()
}

fn original_function(original: &str, name: &str) -> ast::Fn {
    let parsed = ast::SourceFile::parse(original, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().unwrap().text() == name)
        .unwrap()
}

fn wrap_function(function: &ast::Fn, pointer: &str) -> String {
    let generics = function
        .generic_param_list()
        .map_or_else(String::new, |generics| generics.syntax().text().to_string());
    let predicates = function
        .where_clause()
        .map_or_else(String::new, |clause| clause.syntax().text().to_string());
    format!("type ElisionHeader{generics} {predicates} = {pointer};")
}

/// Preserve exact generic declarations, parameter/result types and where
/// metadata. The function body and argument patterns are absent in the wrapper.
fn function_adapter(original: &str, name: &str) -> String {
    let function = original_function(original, name);
    let parameters = function.param_list().unwrap();
    assert!(parameters.self_param().is_none());
    let types: Vec<_> = parameters
        .params()
        .map(|parameter| parameter.ty().unwrap().syntax().text().to_string())
        .collect();
    let result = function
        .ret_type()
        .map_or_else(String::new, |result| result.syntax().text().to_string());
    wrap_function(&function, &format!("fn({}) {result}", types.join(",")))
}

/// The stock Fn-bound diagnostics expose the binder and parameter nodes. This
/// test adapter replaces only the callable trait spelling with `fn`, preserving
/// every lifetime/type node and the original owning generic/where context.
fn callable_adapter(name: &str) -> String {
    let function = original_function(CALLABLES, name);
    let bound = function
        .where_clause()
        .unwrap()
        .predicates()
        .next()
        .unwrap()
        .type_bound_list()
        .unwrap()
        .bounds()
        .next()
        .unwrap();
    let binder = bound
        .for_binder()
        .map_or_else(String::new, |binder| binder.syntax().text().to_string());
    let ast::Type::PathType(path) = bound.ty().unwrap() else {
        panic!("callable path missing")
    };
    let segment = path.path().unwrap().segment().unwrap();
    assert_eq!(segment.name_ref().unwrap().text(), "Fn");
    let arguments = segment
        .parenthesized_arg_list()
        .unwrap()
        .syntax()
        .text()
        .to_string();
    let result = segment
        .ret_type()
        .map_or_else(String::new, |result| result.syntax().text().to_string());
    wrap_function(&function, &format!("{binder} fn{arguments} {result}"))
}

fn original_alias(original: &str, name: &str) -> String {
    let parsed = ast::SourceFile::parse(original, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::TypeAlias::cast)
        .find(|alias| alias.name().unwrap().text() == name)
        .unwrap()
        .syntax()
        .text()
        .to_string()
}

fn function(term: &TyTerm) -> &Binder<lrr::traits::term::FunctionSignature> {
    let TyTerm::Function(function) = term else {
        panic!("function pointer missing")
    };
    function
}

fn reference(term: &TyTerm) -> (&RegionTerm, &TyTerm) {
    let TyTerm::Ref(region, _, target) = term else {
        panic!("reference missing")
    };
    (region, target)
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

#[test]
fn original_pointer_alias_elision_creates_a_late_bound_region_distinct_from_static() {
    let static_recipe = alias(POINTERS, "A").unwrap();
    let anonymous = alias(POINTERS, "B").unwrap();
    let static_term = static_recipe.symbolic.as_ref().unwrap();
    let anonymous_term = anonymous.symbolic.as_ref().unwrap();
    assert!(function(static_term).variables.is_empty());
    assert_eq!(function(anonymous_term).variables, vec![BoundKind::Region]);
    assert_eq!(
        *reference(&function(static_term).value.parameters[0]).0,
        RegionTerm::Static
    );
    assert_eq!(
        *reference(&function(anonymous_term).value.parameters[0]).0,
        RegionTerm::Bound { depth: 0, index: 0 }
    );
    assert_ne!(static_term.fingerprint(), anonymous_term.fingerprint());
    validate_closed(anonymous_term).unwrap();
}

#[test]
fn original_generic_pointer_and_borrow_adapter_keep_rigid_type_and_unique_output() {
    let stock = signature(GENERIC_POINTER, "take_method");
    let pointer = stock.parameters[0].symbolic.as_ref().unwrap();
    let expected = TyTerm::Param(ParamKey {
        definition: stock.definition,
        ordinal: 0,
    });
    assert_eq!(function(pointer).variables, vec![BoundKind::Region]);
    assert_eq!(
        reference(&function(pointer).value.parameters[0]),
        (&RegionTerm::Bound { depth: 0, index: 0 }, &expected)
    );
    // Function-item signature elaboration is still out of scope here.
    assert!(stock.parameters[1].symbolic.is_none());
    let stock_borrow = signature(BORROW, "borrow");
    assert!(stock_borrow.parameters[0].symbolic.is_none());
    assert!(stock_borrow.result_symbolic.is_none());
    let text = function_adapter(BORROW, "borrow");
    let recipe = alias(&text, "ElisionHeader").unwrap();
    let term = recipe.symbolic.as_ref().unwrap();
    let pointer = function(term);
    assert_eq!(pointer.variables, vec![BoundKind::Region]);
    assert_eq!(pointer.value.parameters[0], *pointer.value.result);
    assert_eq!(
        *reference(&pointer.value.parameters[0]).1,
        TyTerm::Param(parameter(&recipe, "T", GenericKind::Type))
    );
    validate_closed(term).unwrap();
}

#[test]
fn original_underscore_reference_adapter_uses_a_fresh_late_bound_input() {
    let text = function_adapter(UNDERSCORE, "assert_static_via_hrtb_with_assoc_type");
    let recipe = alias(&text, "ElisionHeader").unwrap();
    let term = recipe.symbolic.as_ref().unwrap();
    let pointer = function(term);
    assert_eq!(pointer.variables, vec![BoundKind::Region]);
    assert_eq!(
        reference(&pointer.value.parameters[0]),
        (
            &RegionTerm::Bound { depth: 0, index: 0 },
            &TyTerm::Param(parameter(&recipe, "T", GenericKind::Type))
        )
    );
    validate_closed(term).unwrap();
}

#[test]
fn original_callable_header_adapter_keeps_early_explicit_and_implicit_regions_separate() {
    for (name, variables, indices) in [
        ("f1", 2, [0, 1]),
        ("f2", 2, [0, 1]),
        ("f4", 2, [1, 0]),
        ("f5", 1, [0, 0]),
    ] {
        let recipe = alias(&callable_adapter(name), "ElisionHeader").unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let pointer = function(term);
        assert_eq!(pointer.variables, vec![BoundKind::Region; variables]);
        for (argument, index) in pointer.value.parameters.iter().zip(indices) {
            assert_eq!(
                *reference(argument).0,
                RegionTerm::Bound { depth: 0, index }
            );
        }
        validate_closed(term).unwrap();
    }
    let recipe = alias(&callable_adapter("f3"), "ElisionHeader").unwrap();
    let pointer = function(recipe.symbolic.as_ref().unwrap());
    assert_eq!(pointer.variables, vec![BoundKind::Region]);
    assert_eq!(
        *reference(&pointer.value.parameters[0]).0,
        RegionTerm::Param(parameter(&recipe, "'a", GenericKind::Lifetime))
    );
    assert_eq!(
        *reference(&pointer.value.parameters[1]).0,
        RegionTerm::Bound { depth: 0, index: 0 }
    );
}

#[test]
fn original_nested_callable_headers_allocate_regions_in_their_own_binders() {
    for name in ["g2", "g4"] {
        let recipe = alias(&callable_adapter(name), "ElisionHeader").unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let outer = function(term);
        let inner = function(&outer.value.parameters[1]);
        assert_eq!(outer.variables, vec![BoundKind::Region]);
        assert_eq!(inner.variables, vec![BoundKind::Region]);
        assert_eq!(
            *reference(&outer.value.parameters[0]).0,
            RegionTerm::Bound { depth: 0, index: 0 }
        );
        assert_eq!(
            *reference(&inner.value.parameters[0]).0,
            RegionTerm::Bound { depth: 0, index: 0 }
        );
        validate_closed(term).unwrap();
    }
    let recipe = alias(NESTED, "F2").unwrap();
    let outer = function(recipe.symbolic.as_ref().unwrap());
    let inner = function(&outer.value.parameters[0]);
    assert!(inner.variables.is_empty());
    assert_eq!(
        *reference(&inner.value.parameters[0]).0,
        RegionTerm::Bound { depth: 1, index: 0 }
    );
}

#[test]
fn elision_output_corruption_refuses_multiple_parameters_even_with_the_same_region() {
    for name in ["f1", "f2", "f3", "f4", "f5"] {
        let text = callable_adapter(name);
        // LRR corruption: append an anonymous reference result to the stock
        // two-input header. A repeated explicit lifetime in f5 does not make
        // the two parameter positions eligible for output elision.
        for result in ["&()", "&'_ ()"] {
            let corrupted = format!("{} -> {result};", text.strip_suffix(';').unwrap());
            assert_eq!(
                alias(&corrupted, "ElisionHeader").unwrap_err().code,
                "LRR-E-UNSUPPORTED"
            );
        }
    }
}

#[test]
fn elision_output_corruption_cannot_import_a_nested_function_private_region() {
    for name in ["F1", "F2"] {
        let text = original_alias(NESTED, name);
        for result in ["&()", "&'_ ()"] {
            let corrupted = format!("{} -> {result};", text.strip_suffix(';').unwrap());
            assert_eq!(
                alias(&corrupted, name).unwrap_err().code,
                "LRR-E-UNSUPPORTED"
            );
        }
    }
    // g2 has one outer reference plus a nested pointer. The injected result
    // must reuse only the outer reference, never the inner function's region.
    let text = callable_adapter("g2");
    let injected = format!("{} -> &();", text.strip_suffix(';').unwrap());
    let recipe = alias(&injected, "ElisionHeader").unwrap();
    let pointer = function(recipe.symbolic.as_ref().unwrap());
    assert_eq!(pointer.variables, vec![BoundKind::Region]);
    assert_eq!(pointer.value.parameters[0], *pointer.value.result);
}

#[test]
fn implicit_function_binder_substitution_quarantines_an_inserted_free_region() {
    let recipe = alias(&function_adapter(BORROW, "borrow"), "ElisionHeader").unwrap();
    let term = recipe.symbolic.as_ref().unwrap();
    let fingerprint = term.fingerprint();
    let mut substitution = Substitution::default();
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
    let corrupted = substitution.apply(term).unwrap();
    let argument = &function(&corrupted).value.parameters[0];
    assert_eq!(
        *reference(argument).0,
        RegionTerm::Bound { depth: 0, index: 0 }
    );
    assert_eq!(
        *reference(reference(argument).1).0,
        RegionTerm::Bound { depth: 1, index: 0 }
    );
    assert_eq!(
        validate_closed(&corrupted).unwrap_err().code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(term.fingerprint(), fingerprint);
    validate_closed(term).unwrap();
}

#[test]
fn original_array_header_elision_keeps_named_elements_and_fresh_reference_inputs() {
    let recipe = alias(&function_adapter(ARRAYS, "long_and_short"), "ElisionHeader").unwrap();
    let term = recipe.symbolic.as_ref().unwrap();
    let pointer = function(term);
    assert_eq!(pointer.variables, vec![BoundKind::Region; 2]);
    for (index, argument) in pointer.value.parameters.iter().enumerate() {
        assert_eq!(
            *reference(argument).0,
            RegionTerm::Bound {
                depth: 0,
                index: u32::try_from(index).unwrap()
            }
        );
        let TyTerm::Array(element, _) = reference(argument).1 else {
            panic!("array missing")
        };
        let expected = if index == 0 {
            RegionTerm::Static
        } else {
            RegionTerm::Param(parameter(&recipe, "'a", GenericKind::Lifetime))
        };
        assert_eq!(*reference(element).0, expected);
    }
    validate_closed(term).unwrap();
}

#[test]
fn original_primitive_pointer_stays_incomplete_despite_available_elision_rules() {
    // This exact stock header has unresolved primitive paths in this API. Its
    // E0106 compiletest behavior remains unexecuted; no primitive identity or
    // successful original diagnostic comparison is fabricated by the adapter.
    assert_eq!(
        alias(PRIMITIVE, "Foo").unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
}

#[test]
fn elision_result_injection_reuses_static_explicit_and_rigid_region_identity() {
    // LRR injected output: preserve the exact stock static input and require
    // both anonymous result spellings to retain that one applicable region.
    let stock = original_alias(POINTERS, "A");
    for result in ["&()", "&'_ ()"] {
        let text = format!("{} -> {result};", stock.strip_suffix(';').unwrap());
        let recipe = alias(&text, "A").unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let pointer = function(term);
        assert!(pointer.variables.is_empty());
        assert_eq!(pointer.value.parameters[0], *pointer.value.result);
        assert_eq!(*reference(&pointer.value.result).0, RegionTerm::Static);
        validate_closed(term).unwrap();
    }
    for name in ["f2", "f3"] {
        // LRR parameter/result injection: select the first exact named input
        // from each stock callable, preserving its explicit or rigid context.
        let original = original_function(CALLABLES, name);
        let wrapper = callable_adapter(name);
        let parsed = ast::SourceFile::parse(&wrapper, ra_ap_syntax::Edition::Edition2024);
        let pointer = parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::FnPtrType::cast)
            .unwrap();
        let first = pointer.param_list().unwrap().params().next().unwrap();
        let binder = pointer
            .syntax()
            .parent()
            .and_then(ast::ForType::cast)
            .and_then(|ty| ty.for_binder())
            .map_or_else(String::new, |binder| binder.syntax().text().to_string());
        let text = wrap_function(
            &original,
            &format!("{binder} fn({}) -> &'_ ()", first.ty().unwrap().syntax()),
        );
        let recipe = alias(&text, "ElisionHeader").unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let pointer = function(term);
        assert_eq!(pointer.value.parameters[0], *pointer.value.result);
        if name == "f2" {
            assert_eq!(pointer.variables, vec![BoundKind::Region]);
            assert_eq!(
                *reference(&pointer.value.result).0,
                RegionTerm::Bound { depth: 0, index: 0 }
            );
        } else {
            assert!(pointer.variables.is_empty());
            assert_eq!(
                *reference(&pointer.value.result).0,
                RegionTerm::Param(parameter(&recipe, "'a", GenericKind::Lifetime))
            );
        }
        validate_closed(term).unwrap();
    }
}

#[test]
fn elision_parameter_grouping_injection_deduplicates_only_identical_regions() {
    for name in ["f1", "f5"] {
        let wrapper = callable_adapter(name);
        let parsed = ast::SourceFile::parse(&wrapper, ra_ap_syntax::Edition::Edition2024);
        let pointer = parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::FnPtrType::cast)
            .unwrap();
        // LRR structural mutation: put the two exact stock reference nodes
        // into one tuple parameter, then inject an omitted result lifetime.
        let types: Vec<_> = pointer
            .param_list()
            .unwrap()
            .params()
            .map(|parameter| parameter.ty().unwrap().syntax().text().to_string())
            .collect();
        let binder = pointer
            .syntax()
            .parent()
            .and_then(ast::ForType::cast)
            .and_then(|ty| ty.for_binder())
            .map_or_else(String::new, |binder| binder.syntax().text().to_string());
        let text = wrap_function(
            &original_function(CALLABLES, name),
            &format!("{binder} fn(({})) -> &()", types.join(",")),
        );
        if name == "f1" {
            assert_eq!(
                alias(&text, "ElisionHeader").unwrap_err().code,
                "LRR-E-UNSUPPORTED"
            );
        } else {
            let recipe = alias(&text, "ElisionHeader").unwrap();
            let term = recipe.symbolic.as_ref().unwrap();
            let pointer = function(term);
            assert_eq!(pointer.variables, vec![BoundKind::Region]);
            let TyTerm::Tuple(fields) = &pointer.value.parameters[0] else {
                panic!("tuple missing")
            };
            assert_eq!(fields[0], fields[1]);
            assert_eq!(fields[0], *pointer.value.result);
            validate_closed(term).unwrap();
        }
    }
}

#[test]
fn explicit_output_injection_retains_its_region_despite_ambiguous_inputs() {
    for (name, result) in [("f1", "&'static ()"), ("f3", "&'a ()"), ("f5", "&'r ()")] {
        let wrapper = callable_adapter(name);
        // LRR output injection: ambiguous input candidates have no authority
        // to replace an explicit static, rigid, or locally bound output region.
        let text = format!("{} -> {result};", wrapper.strip_suffix(';').unwrap());
        let recipe = alias(&text, "ElisionHeader").unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let pointer = function(term);
        let expected = match name {
            "f1" => RegionTerm::Static,
            "f3" => RegionTerm::Param(parameter(&recipe, "'a", GenericKind::Lifetime)),
            "f5" => RegionTerm::Bound { depth: 0, index: 0 },
            _ => unreachable!(),
        };
        assert_eq!(*reference(&pointer.value.result).0, expected);
        validate_closed(term).unwrap();
    }
}
