//! Exact upstream lifetime spellings with disclosed header/Unit adaptations.
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
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, type_aliases::TypeAliasRecipe},
    traits::{
        subst::validate_closed,
        term::{Binder, BoundKind, ParamKey, RegionTerm, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasName},
};

const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/lexer/ident_normalization.rs");

fn alias(text: &str) -> Result<Box<TypeAliasRecipe>, Diagnostic> {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    // Retain the original case's owning edition in every source query.
    assert!(ORIGINAL.contains("//@edition:2021"));
    let profile = ProfileIdentity {
        edition: 2021,
        ..ProfileIdentity::current()
    };
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
    let provider = SourceUnitInput {
        path,
        crate_key,
        module_path: vec![],
        profile,
        cfg: cfg.clone(),
        blob,
    };
    // The original edition is a declaration dependency, never a live entry.
    PreparedSourceInputs::refresh(None, vec![provider.clone()], Default::default()).unwrap();
    let consumer = Digest::of_bytes(b"LRR Unicode header declaration consumer");
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![
            provider,
            SourceUnitInput {
                path: SourcePath::new("src/consumer.rs").unwrap(),
                crate_key: consumer,
                module_path: vec![],
                profile: ProfileIdentity::current(),
                cfg,
                blob: SourceBlob::new(Vec::new()).unwrap(),
            },
        ],
        Default::default(),
        vec![SourceDependency {
            consumer,
            alias: "unicode_headers".into(),
            provider: crate_key,
        }],
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source missing")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let definition = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "LifetimeHeader")
        .unwrap()
        .def_key
        .fingerprint();
    let result = match db.fetch(&SyntaxProvider, QueryKey::TypeAlias(definition)) {
        QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
            QueryValue::TypeAlias(recipe) => {
                assert_eq!(recipe.header.profile.edition, 2021);
                Ok(recipe.clone())
            }
            _ => panic!("wrong query record"),
        },
        QueryPoll::Ready(Err(error)) => Err(error),
        QueryPoll::NeedInput(input) => panic!("unexpected input: {input:?}"),
    };
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    result
}

struct OriginalHeader {
    generics: String,
    references: Vec<String>,
    lifetimes: Vec<String>,
    declared: String,
}

fn original_header(name: &str, unit_target: bool) -> OriginalHeader {
    let parsed = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2021);
    assert!(parsed.errors().is_empty());
    let original = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().unwrap().text() == name)
        .unwrap();
    let generics = original.generic_param_list().unwrap();
    let ast::GenericParam::LifetimeParam(declared) = generics.generic_params().next().unwrap()
    else {
        panic!("expected original lifetime parameter")
    };
    let declared = declared.lifetime().unwrap().syntax().text().to_string();
    let mut references = Vec::new();
    let mut lifetimes = Vec::new();
    for parameter in original.param_list().unwrap().params() {
        let ast::Type::RefType(reference) = parameter.ty().unwrap() else {
            panic!("expected original reference")
        };
        assert_eq!(reference.ty().unwrap().syntax().text().to_string(), "str");
        let lifetime = reference.lifetime().unwrap().syntax().text().to_string();
        // Only the unresolved str target is adapted. Keep the lifetime token,
        // parameter order and generic declaration exactly as in the original.
        references.push(if unit_target {
            format!("&{lifetime} ()")
        } else {
            reference.syntax().text().to_string()
        });
        lifetimes.push(lifetime);
    }
    OriginalHeader {
        generics: generics.syntax().text().to_string(),
        references,
        lifetimes,
        declared,
    }
}

fn function(term: &TyTerm) -> &Binder<lrr::traits::term::FunctionSignature> {
    let TyTerm::Function(function) = term else {
        panic!("function pointer missing")
    };
    function
}

fn region(term: &TyTerm) -> &RegionTerm {
    let TyTerm::Ref(region, _, _) = term else {
        panic!("reference missing")
    };
    region
}

#[test]
fn original_unicode_lifetime_headers_preserve_unresolved_primitive_refusal() {
    for name in ["lifetime_nfc", "raw_lifetime_nfc"] {
        let header = original_header(name, false);
        let text = format!(
            "type LifetimeHeader{} = fn({});",
            header.generics,
            header.references.join(",")
        );
        assert_eq!(alias(&text).unwrap_err().code, "LRR-E-UNSUPPORTED");
    }
}

#[test]
fn original_unicode_lifetime_unit_adapter_preserves_rigid_parameter_identity() {
    for name in ["lifetime_nfc", "raw_lifetime_nfc"] {
        let header = original_header(name, true);
        let text = format!(
            "type LifetimeHeader{} = fn({});",
            header.generics,
            header.references.join(",")
        );
        let recipe = alias(&text).unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let pointer = function(term);
        assert!(pointer.variables.is_empty());
        assert_eq!(recipe.generics[0].name, header.declared);
        let expected = RegionTerm::Param(ParamKey {
            definition: recipe.definition,
            ordinal: recipe.generics[0].ordinal,
        });
        assert_eq!(*region(&pointer.value.parameters[0]), expected);
        assert_eq!(*region(&pointer.value.parameters[1]), expected);
        validate_closed(term).unwrap();
    }
}

#[test]
fn original_unicode_lifetime_binder_adapter_normalizes_raw_and_nfc_spellings() {
    let mut previous = None;
    for name in ["lifetime_nfc", "raw_lifetime_nfc"] {
        let header = original_header(name, true);
        for bound in std::iter::once(&header.declared).chain(&header.lifetimes) {
            // Supplemental binder-position adaptation: use an exact original
            // lifetime token as the explicit binder, with Unit targets.
            let text = format!(
                "type LifetimeHeader = for<{bound}> fn({});",
                header.references.join(",")
            );
            let recipe = alias(&text).unwrap();
            let term = recipe.symbolic.as_ref().unwrap();
            let pointer = function(term);
            assert_eq!(pointer.variables, vec![BoundKind::Region]);
            for parameter in &pointer.value.parameters {
                assert_eq!(*region(parameter), RegionTerm::Bound { depth: 0, index: 0 });
            }
            if let Some(previous) = previous {
                assert_eq!(term.fingerprint(), previous);
            }
            previous = Some(term.fingerprint());
            validate_closed(term).unwrap();
        }
    }
}

#[test]
fn original_unicode_lifetime_nested_binder_adapter_retains_empty_scope_depth() {
    for name in ["lifetime_nfc", "raw_lifetime_nfc"] {
        let header = original_header(name, true);
        let text = format!(
            "type LifetimeHeader = for{} fn(fn({}));",
            header.generics,
            header.references.join(",")
        );
        let recipe = alias(&text).unwrap();
        let term = recipe.symbolic.as_ref().unwrap();
        let outer = function(term);
        let inner = function(&outer.value.parameters[0]);
        assert_eq!(outer.variables, vec![BoundKind::Region]);
        assert!(inner.variables.is_empty());
        for parameter in &inner.value.parameters {
            assert_eq!(*region(parameter), RegionTerm::Bound { depth: 1, index: 0 });
        }
        validate_closed(term).unwrap();
    }
}

#[test]
fn unicode_lifetime_binder_corruption_rejects_duplicate_canonical_identity() {
    for name in ["lifetime_nfc", "raw_lifetime_nfc"] {
        let header = original_header(name, true);
        // Supplemental duplicate-binder corruption using the two distinct
        // written lifetime tokens already present in each unchanged original.
        assert_ne!(header.lifetimes[0], header.lifetimes[1]);
        let text = format!(
            "type LifetimeHeader = for<{}> fn({});",
            header.lifetimes.join(","),
            header.references.join(",")
        );
        assert_eq!(alias(&text).unwrap_err().code, "LRR-E-UNSUPPORTED");
    }
}
