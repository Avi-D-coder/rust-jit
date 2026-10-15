//! Actual length components plus source-isolation controls, not whole-fixture passes.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{dependency::InputKey, key::OutlineKey, memo::Database},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        constant_source_admission::{ConstantRequest, ConstantSourceAdmission},
        constants::{ConstLimits, evaluate},
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::SourcePath,
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const CHECK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-block-non-item-statement.rs"
);
const RUN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-block-non-item-statement-3.rs"
);
fn source(text: &str, edition: u16) -> ConstantSource {
    ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        Digest([218; 32]),
        CfgSet::guest_default(),
    )
    .unwrap()
}
fn alias_length(source: &ConstantSource, name: &str) -> SourceRange {
    let alias = source
        .0
        .parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::TypeAlias::cast)
        .find(|a| a.name().is_some_and(|n| n.text() == name))
        .unwrap();
    let ast::Type::ArrayType(array) = alias.ty().unwrap() else {
        panic!("original alias is not an array")
    };
    let expression = array
        .const_arg()
        .unwrap()
        .syntax()
        .children()
        .find_map(ast::Expr::cast)
        .unwrap();
    range(expression.syntax().text_range())
}
fn binding(source: &ConstantSource) -> StatementRef {
    StatementRef(ast::Stmt::LetStmt(
        source
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::LetStmt::cast)
            .unwrap(),
    ))
}
#[test]
fn unchanged_array_alias_lengths_default_only_unconstrained_locals_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = source(CHECK, edition);
        for (name, expected) in [("Array", 5), ("Array2", 3)] {
            let plan = source.array_length(alias_length(&source, name)).unwrap();
            let result = evaluate(&plan, ConstLimits::default()).unwrap();
            assert_eq!(
                result.value().scalar().unwrap(),
                crate::runtime::value::ValueRepr::unsigned(
                    crate::runtime::shape::TypeId(13),
                    expected,
                )
                .unwrap()
            );
            assert_eq!(result.source(), Digest::of_bytes(CHECK.as_bytes()));
            assert_eq!(plan.profile().edition, edition);
        }
    }
}
// Genuine current preparation mints the root allow-attribute evidence. No test
// constructor issues an inert-attribute, primitive, body or completion proof.
fn prepared_source(text: &str, edition: u16) -> (Database, ConstantSource) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("original-array-length.rs").unwrap();
    let unit = Digest([219; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("original source missing")
    };
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap();
    let definition = item.def_key.fingerprint();
    let scope = item.scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    for namespace in [Namespace::Value, Namespace::Type] {
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"LRR actual array-length original preparation"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"array original declarative"),
            Digest::of_bytes(b"array original compiler"),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                    .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                    panic!("{} {}", diagnostic.code, diagnostic.message)
                }
                _ => panic!("unestablished original array-length authority"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    let at = crate::syntax::pure_constants::fixture_length_sites(text, "main", edition)[0];
    let admission = ConstantSourceAdmission::capture_current(
        &db,
        &view,
        definition,
        ConstantRequest::ArrayLength(at),
        32 * 1024 * 1024,
    )
    .unwrap();
    admission.revalidate(&db, &view).unwrap();
    assert!(
        admission
            .dependencies()
            .contains_key(&InputKey::DefinitionBody(definition))
    );
    let source = admission.plan().source().clone();
    (db, source)
}
#[test]
fn unchanged_run_pass_array_component_requires_real_root_attribute_preparation() {
    for edition in [2015, 2018, 2021, 2024] {
        let unprepared = source(RUN, edition);
        assert_eq!(
            unprepared
                .array_length(alias_length(&unprepared, "Array"))
                .unwrap_err()
                .code,
            "LRR-E-CONST-SOURCE"
        );
        let (_db, prepared) = prepared_source(RUN, edition);
        let plan = prepared
            .array_length(alias_length(&prepared, "Array"))
            .unwrap();
        let result = evaluate(&plan, ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            crate::runtime::value::ValueRepr::unsigned(crate::runtime::shape::TypeId(13), 5)
                .unwrap()
        );
        assert_eq!(result.source(), Digest::of_bytes(RUN.as_bytes()));
        assert_eq!(plan.profile().edition, edition);
        // No claim that main's Array alias, storage or runtime contract executed.
    }
}
#[test]
fn repeat_length_locals_do_not_require_executing_the_ordinary_function() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "fn main(){let a=[0;{let mut n=1;while n<5{n+=1;}n}];missing(a);}";
        let source = source(text, edition);
        let at = crate::syntax::pure_constants::fixture_length_sites(text, "main", edition)[0];
        let plan = source.array_length(at).unwrap();
        let result = evaluate(&plan, ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            crate::runtime::value::ValueRepr::unsigned(crate::runtime::shape::TypeId(13), 5)
                .unwrap()
        );
        assert_eq!(result.source(), Digest::of_bytes(text.as_bytes()));
    }
}
#[test]
fn length_context_replays_parameter_outer_local_generic_and_closure_refusals() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, code, reason) in [
            (
                "fn main(n:usize){let _=[0;{let x=n;x}];}",
                "LRR-E-CONST-LENGTH-SOURCE",
                "length expression requires additional source local/control context",
            ),
            (
                "fn main(){let n=5;let _=[0;{let x=n;x}];}",
                "LRR-E-CONST-SOURCE",
                "anonymous array length requires captured preceding local bindings",
            ),
            (
                "fn main<T>(){let _=[0;{let x=5;x}];}",
                "LRR-E-CONST-SOURCE",
                "anonymous array length requires captured function binding context",
            ),
            (
                "fn main(){let _=||[0;{let x=5;x}];}",
                "LRR-E-CONST-SOURCE",
                "anonymous array length requires nested source namespace preparation",
            ),
        ] {
            let source = source(text, edition);
            let at = crate::syntax::pure_constants::fixture_length_sites(text, "main", edition)[0];
            let error = source.array_length(at).unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.message, reason);
        }
    }
}
#[test]
fn local_context_refuses_a_foreign_ast_and_charges_exact_length_admission() {
    let source = source("type Array=[u32;{let x=5;x}];fn main(){}", 2024);
    let own = binding(&source);
    let context = source
        .numeric_local_context(&own, 100_000, 32 * 1024 * 1024)
        .unwrap();
    assert_eq!(context.primitive(), PrimitiveType::Usize);
    assert_eq!(
        source
            .numeric_local_context(&own, context.work() - 1, 32 * 1024 * 1024)
            .unwrap_err()
            .code,
        "LRR-E-CONST-LIMIT"
    );
    assert_eq!(
        source
            .numeric_local_context(&own, 100_000, 128)
            .unwrap_err()
            .code,
        "LRR-E-CONST-LIMIT"
    );
    let foreign = ConstantSource::parse(
        &SourceBlob::new(b"type Array=[u32;{let x=7;x}];fn main(){}".to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([218; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    assert_eq!(
        source
            .numeric_local_context(&binding(&foreign), 100_000, 32 * 1024 * 1024)
            .unwrap_err()
            .code,
        "LRR-E-CONST-SOURCE"
    );
}

#[test]
fn cold_unresolved_exits_do_not_grant_targets_or_precheck_the_reached_scalar() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "const A:u8={let x=7;if false{break 'absent;}x};";
        let source = source(text, edition);
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == "A")
            .unwrap()
            .range();
        let result = evaluate(&source.constant(at).unwrap(), ConstLimits::default()).unwrap();
        assert_eq!(
            result.value().scalar().unwrap(),
            crate::runtime::value::ValueRepr::unsigned(crate::runtime::shape::TypeId(9), 7)
                .unwrap()
        );
        let entered = "const A:u8={let x=7;if true{break 'absent;}x};";
        let source = ConstantSource::parse(
            &SourceBlob::new(entered.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            Digest([218; 32]),
            CfgSet::guest_default(),
        )
        .unwrap();
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == "A")
            .unwrap()
            .range();
        let error = evaluate(&source.constant(at).unwrap(), ConstLimits::default()).unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-SOURCE");
        assert_eq!(
            error.diagnostic().message,
            "constant exit has no lexical loop target in its source body"
        );
        let start = entered.find("break 'absent").unwrap();
        assert_eq!(
            error.range(),
            SourceRange {
                start: start as u32,
                end: (start + "break 'absent".len()) as u32
            }
        );
        assert!(error.panic().is_none());
    }
}
