//! Source authority controls; these do not execute a guest or a stock fixture.
use super::*;
use crate::{
    db::{poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::{
        callables::{CallResolver, CallSelection},
        names::{NameDefinition, NameLookup},
    },
    traits::term::RegionTerm,
};

fn syntax(
    db: &mut Database,
    signature: &FunctionSignature,
    parse: &ParseKey,
    ordinal: usize,
) -> ClosedReferenceSyntax {
    let QueryPoll::Ready(Ok(outcome)) =
        db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone()))
    else {
        panic!("actual admitted parser query did not complete")
    };
    let QueryValue::ParseFile(parsed) = outcome.value.as_ref() else {
        panic!("actual parser query variant differs")
    };
    let InputValue::DeclarationContext(context) = db
        .published_input(&InputKey::DeclarationContext(signature.definition))
        .unwrap()
        .0
    else {
        panic!()
    };
    let InputValue::DefinitionBody(body) = db
        .published_input(&InputKey::DefinitionBody(signature.definition))
        .unwrap()
        .0
    else {
        panic!()
    };
    parsed
        .closed_reference_parameter(signature, context, Some(body), ordinal)
        .unwrap()
        .unwrap()
}

#[test]
fn generated_reference_uses_its_parameter_name_without_a_primitive_result_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for source in [
            "macro_rules! emit{()=>{fn answer<'a>(x:&'a i32,y:&mut bool){}}}emit!();fn main(){}",
            "macro_rules! emit{()=>{fn answer<'a>(x:&'a i32,y:&mut bool){}}}macro_rules! wrap{()=>{emit!();}}wrap!();fn main(){}",
        ] {
            let (mut db, signature, parse, _, owner) =
                super::tests::prepared_config_retained(source, edition, true, false);
            assert!(signature.result.is_empty());
            db.trace.detailed = true;
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            let views = [&owner];
            let resolver = CallResolver::new(&graph, &views).unwrap();
            // Select the actual published generated identity, not an invented
            // function or a same-name original declaration.
            let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
                &mut db,
                NameLookup::Defined(NameDefinition {
                    namespace: Namespace::Value,
                    identity: signature.definition,
                    defining_unit: signature.scope.crate_key,
                    definition_edition: edition,
                }),
            ) else {
                panic!("actual generated source function was not selected")
            };
            for (ordinal, primitive, mutable) in [
                (0, PrimitiveType::I32, false),
                (1, PrimitiveType::Bool, true),
            ] {
                let proof =
                    ClosedReferenceParameter::capture(&mut db, &callee, &graph, &views, ordinal)
                        .unwrap()
                        .unwrap();
                assert_eq!(proof.pointee(), primitive);
                assert_eq!(proof.mutable(), mutable);
                proof.require_current(&mut db, &callee).unwrap();
                let TyTerm::Ref(region, _, _) = proof.source_template() else {
                    panic!("ordinary reference source template was erased")
                };
                assert!(match ordinal {
                    0 =>
                        matches!(region, RegionTerm::Param(key) if key.definition == signature.definition),
                    _ => matches!(region, RegionTerm::Bound { depth: 0, index: 1 }),
                });
            }
            assert_eq!(syntax(&mut db, &signature, &parse, 0).target(), "i32");
            assert!(
                !db.trace.events.iter().any(|event| matches!(
                    event.key,
                    QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                ))
            );
        }
    }
}

#[test]
fn generated_reference_shadow_and_epoch_keep_their_type_authority_refusals() {
    for edition in [2015, 2018, 2021, 2024] {
        for prefix in ["", "type i32=bool;"] {
            let source = format!(
                "{prefix}macro_rules! emit{{()=>{{fn answer(x:&i32){{}}}}}}emit!();fn main(){{}}"
            );
            let (mut db, signature, parse, _, _) =
                super::tests::prepared_config_retained(&source, edition, true, false);
            let parameter = syntax(&mut db, &signature, &parse, 0);
            if prefix.is_empty() {
                require_reference(&db, &signature, &parameter).unwrap();
                let mut foreign = signature.clone();
                foreign.definition = Digest([193; 32]);
                assert!(require_reference(&db, &foreign, &parameter).is_err());
                db.begin_expansion_epoch(2).unwrap();
            }
            assert_eq!(
                require_reference(&db, &signature, &parameter)
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
        }
    }
}

#[test]
fn generated_reference_static_region_remains_an_explicit_source_prerequisite() {
    let (mut db, signature, _, _, owner) = super::tests::prepared_config_retained(
        "macro_rules! emit{()=>{fn answer(x:&'static i32){}}}emit!();fn main(){}",
        2024,
        true,
        false,
    );
    let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
    let views = [&owner];
    let resolver = CallResolver::new(&graph, &views).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
        &mut db,
        NameLookup::Defined(NameDefinition {
            namespace: Namespace::Value,
            identity: signature.definition,
            defining_unit: signature.scope.crate_key,
            definition_edition: 2024,
        }),
    ) else {
        panic!()
    };
    let error = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &views, 0).unwrap_err();
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(error.message.contains("actual static-origin producer"));
}

#[test]
fn generated_reference_foreign_template_checks_the_actual_definer_type_scope() {
    use crate::source::dependencies::SourceDependency;
    const APP: Digest = Digest([201; 32]);
    const LIB: Digest = Digest([202; 32]);
    for edition in [2015, 2018, 2021, 2024] {
        for shadow in [false, true] {
            let library = format!(
                "{}#[macro_export]macro_rules! seed{{()=>{{pub fn made(x:&i32){{}}}}}}",
                if shadow { "type i32=bool;" } else { "" },
            );
            let (mut db, signature, parse, _sources) = super::cross_source_tests::prepared_world(
                vec![
                    ("app/main.rs", APP, "renamed::seed!();fn main(){}", edition),
                    ("dependency/lib.rs", LIB, &library, edition),
                ],
                vec![SourceDependency {
                    consumer: APP,
                    alias: "renamed".into(),
                    provider: LIB,
                }],
                true,
            );
            let parameter = syntax(&mut db, &signature, &parse, 0);
            let result = require_reference(&db, &signature, &parameter);
            if shadow {
                assert_eq!(result.unwrap_err().code, "LRR-E-CALL-CONTEXT");
            } else {
                result.unwrap();
            }
        }
    }
}
