//! Source authority controls; these do not execute a guest or a stock fixture.
use super::*;
use crate::{
    db::{poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::{
        callables::{CallResolver, CallSelection},
        names::{NameDefinition, NameLookup},
    },
    syntax::closed_reference_signatures::ClosedReferenceSyntax,
    traits::term::RegionTerm,
};

fn syntax(
    db: &mut Database,
    signature: &FunctionSignature,
    parse: &ParseKey,
    ordinal: usize,
) -> ClosedReferenceSyntax {
    let QueryPoll::Ready(Ok(outcome)) = db.fetch(
        &SyntaxProvider,
        QueryKey::ReferenceHeader(signature.definition),
    ) else {
        panic!("actual admitted reference header query did not complete")
    };
    let QueryValue::ReferenceHeader(plan) = outcome.value.as_ref() else {
        panic!("actual reference header query variant differs")
    };
    plan.require(signature, parse).unwrap();
    plan.parameter(ordinal).unwrap().as_ref().clone()
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
                assert_eq!(proof.pointee_source(), &TyTerm::Primitive(primitive));
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
            assert_eq!(db.trace.counts.get("ReferenceHeader:ProviderRun"), Some(&1));
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

#[test]
fn generated_named_reference_uses_the_same_positive_source_type_receipt_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "struct Point{x:isize}macro_rules! emit{()=>{fn answer(p:&Point)->&Point{p}}}emit!();fn main(){}",
            "struct Point{x:isize}macro_rules! emit{()=>{fn answer(p:&Point)->&Point{p}}}macro_rules! wrap{()=>{emit!();}}wrap!();fn main(){}",
        ] {
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(text, edition, true, false);
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            let views = [&owner];
            let resolver = CallResolver::new(&graph, &views).unwrap();
            let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
                &mut db,
                NameLookup::Defined(NameDefinition {
                    namespace: Namespace::Value,
                    identity: signature.definition,
                    defining_unit: signature.scope.crate_key,
                    definition_edition: edition,
                }),
            ) else {
                panic!()
            };
            let proof = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &views, 0)
                .unwrap()
                .unwrap();
            let point = owner
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "Point")
                .unwrap()
                .def_key
                .fingerprint();
            assert_eq!(proof.pointee_source(), &TyTerm::Nominal(point, vec![]));
            proof.require_current(&mut db, &callee).unwrap();
            let mut store = crate::runtime::cell::GuestStore::new(1, 1000, 1_000_000).unwrap();
            proof.materialize_hint(&mut store).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(proof.require_current(&mut db, &callee).is_err());
        }
    }
}

#[test]
fn current_generated_lookup_serves_ordinary_and_reference_parameter_hints() {
    use crate::frontend::{
        signature_names::{SignatureName, SignatureNames},
        source_types::{self, NamedTypePreparation},
    };
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "macro_rules! emit{()=>{fn answer(value:i32,borrow:&i32){}}}emit!();fn main(){}",
            "macro_rules! emit{()=>{fn answer(value:i32,borrow:&i32){}}}macro_rules! wrap{()=>{emit!();}}wrap!();fn main(){}",
        ] {
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(text, edition, true, false);
            db.trace.detailed = true;
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            // This is the actual historical catalogue/frontier, not a hand-
            // edited negative map. Completion lives in the real DB receipt.
            let historical = SignatureNames::from_database(&mut db, &graph, &signature)
                .unwrap()
                .unqualified("i32", None)
                .unwrap();
            assert!(!matches!(historical, SignatureName::Primitive(_)));
            let views = [&owner];
            let resolver = CallResolver::new(&graph, &views).unwrap();
            let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
                &mut db,
                NameLookup::Defined(NameDefinition {
                    namespace: Namespace::Value,
                    identity: signature.definition,
                    defining_unit: signature.scope.crate_key,
                    definition_edition: edition,
                }),
            ) else {
                panic!("actual generated function not selected")
            };
            let QueryPoll::Ready(Ok(NamedTypePreparation::Ready(ordinary))) =
                source_types::prepare_named(&mut db, &graph, &views, &signature, "i32")
            else {
                panic!("current ordinary Type lookup failed")
            };
            let reference = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &views, 1)
                .unwrap()
                .unwrap();
            assert_eq!(ordinary.source(), reference.pointee_source());
            let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) =
                prepare_argument_hints(&mut db, &callee, &graph, &views)
            else {
                panic!("shared ordinary/reference hints failed")
            };
            assert_eq!(hints[0].source, TyTerm::Primitive(PrimitiveType::I32));
            assert!(hints[0].closed_reference.is_none());
            assert!(hints[1].closed_reference.is_some());
            assert_eq!(
                db.trace.counts.get("FunctionSignature:ProviderRun"),
                Some(&1)
            );
            assert_eq!(db.trace.counts.get("ReferenceHeader:ProviderRun"), Some(&1));
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
fn current_generated_scalar_result_uses_common_lookup_and_memo_signature_proof() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, signature, parse, _, owner) = super::tests::prepared_config_retained(
            "macro_rules! emit{()=>{fn answer(value:i32)->i32{value}}}emit!();fn main(){}",
            edition,
            true,
            false,
        );
        db.trace.detailed = true;
        let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
        let views = [&owner];
        let resolver = CallResolver::new(&graph, &views).unwrap();
        let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
            &mut db,
            NameLookup::Defined(NameDefinition {
                namespace: Namespace::Value,
                identity: signature.definition,
                defining_unit: signature.scope.crate_key,
                definition_edition: edition,
            }),
        ) else {
            panic!("actual generated scalar callee not selected")
        };
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) = prepare_contracts(
            &mut db,
            &callee,
            &graph,
            &views,
            &[],
            None,
            &[ArgumentType {
                source: TyTerm::Primitive(PrimitiveType::I32),
                descriptor: primitive_runtime_id(PrimitiveType::I32).unwrap(),
            }],
        ) else {
            panic!("current generated scalar result contracts failed")
        };
        assert_eq!(
            packet.source_result,
            Some(TyTerm::Primitive(PrimitiveType::I32))
        );
        assert!(packet.normalized_deferred_return.is_some());
        crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &packet).unwrap();
        // The replacement for full parser replay must consume the actual
        // prepared signature, not just a matching header plus arbitrary fields.
        let mut altered = signature.clone();
        altered.parameters.clear();
        assert!(matches!(
            lookup_prepared_name(&mut db, &altered, &parse, "i32"),
            QueryPoll::Ready(Err(_))
        ));
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        db.begin_expansion_epoch(2).unwrap();
        assert!(
            crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &packet).is_err()
        );
    }
}

#[test]
fn nominal_and_alias_normalization_use_the_actual_completed_declaration_type_scope() {
    use crate::frontend::type_aliases::{AliasNormalizer, AliasTarget};
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "struct Point{x:isize}type Chosen=Point;macro_rules! emit{()=>{fn answer(p:&Chosen){}}}emit!();fn main(){}",
            "macro_rules! emit{()=>{struct Point{x:isize}type Chosen=Point;fn answer(p:&Chosen){}}}emit!();fn main(){}",
        ] {
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(text, edition, true, false);
            db.trace.detailed = true;
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            let views = [&owner];
            let normalizer = AliasNormalizer::new(&graph, &views).unwrap();
            let QueryPoll::Ready(Ok(crate::frontend::source_types::NamedTypePreparation::Ready(
                source,
            ))) = crate::frontend::source_types::prepare_named(
                &mut db, &graph, &views, &signature, "Chosen",
            )
            else {
                panic!("actual current alias-to-nominal source proof failed")
            };
            let TyTerm::Nominal(point, args) = source.source() else {
                panic!("nominal source identity erased")
            };
            assert!(args.is_empty());
            assert!(
                matches!(normalizer.normalize(&mut db,*point),QueryPoll::Ready(Ok(AliasTarget::Nominal(actual))) if actual==*point)
            );
            assert!(
                !db.trace.events.iter().any(|event| matches!(
                    event.key,
                    QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                ))
            );
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                crate::frontend::source_types::prepare_named(
                    &mut db, &graph, &views, &signature, "Chosen"
                ),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
}

#[test]
fn generated_alias_target_and_scalar_result_keep_current_receipt_authority() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, signature, _, _, owner) = super::tests::prepared_config_retained(
            "macro_rules! emit{()=>{type Chosen=isize;fn answer()->Chosen{3isize}}}emit!();fn main(){}",
            edition,
            true,
            false,
        );
        db.trace.detailed = true;
        let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
        let views = [&owner];
        let resolver = CallResolver::new(&graph, &views).unwrap();
        let QueryPoll::Ready(Ok(CallSelection::Source(callee))) = resolver.select(
            &mut db,
            NameLookup::Defined(NameDefinition {
                namespace: Namespace::Value,
                identity: signature.definition,
                defining_unit: signature.scope.crate_key,
                definition_edition: edition,
            }),
        ) else {
            panic!("actual generated alias result callee absent")
        };
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
            prepare_contracts(&mut db, &callee, &graph, &views, &[], None, &[])
        else {
            panic!("current generated alias target result did not normalize")
        };
        assert_eq!(
            packet.source_result,
            Some(TyTerm::Primitive(PrimitiveType::Isize))
        );
        crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &packet).unwrap();
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        db.begin_expansion_epoch(2).unwrap();
        assert!(
            crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &packet).is_err()
        );
    }
}

#[test]
fn generated_type_declaration_lookup_requires_the_actual_memo_header_and_scope() {
    let (mut db, signature, _, _, _) = super::tests::prepared_config_retained(
        "macro_rules! emit{()=>{type Chosen=isize;fn answer()->Chosen{3isize}}}emit!();fn main(){}",
        2024,
        true,
        false,
    );
    let locator = db
        .input(&InputKey::DefinitionSource(signature.definition))
        .unwrap();
    let InputValue::DefinitionSource(parse) = locator.as_ref() else {
        panic!()
    };
    let input = db.input(&InputKey::GeneratedSyntax(parse.content)).unwrap();
    let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
        panic!()
    };
    let identity = unit
        .definitions
        .iter()
        .find(|(_, definition)| {
            definition.item_kind == crate::source::manifest::ItemKind::TypeAlias
        })
        .map(|(identity, _)| *identity)
        .unwrap();
    let header = db.input(&InputKey::DefinitionHeader(identity)).unwrap();
    let context = db.input(&InputKey::DeclarationContext(identity)).unwrap();
    let (InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context)) =
        (header.as_ref(), context.as_ref())
    else {
        panic!()
    };
    let owner = crate::frontend::type_name_namespace::SourceTypeOwner {
        definition: identity,
        scope: &context.scope,
        header,
    };
    assert!(
        matches!(lookup_type_declaration(&mut db,owner,parse,"Chosen"),QueryPoll::Ready(Ok(NameLookup::Defined(selected))) if selected.identity==identity)
    );
    let mut changed = header.clone();
    changed.tokens.clear();
    assert!(matches!(
        lookup_type_declaration(
            &mut db,
            crate::frontend::type_name_namespace::SourceTypeOwner {
                header: &changed,
                ..owner
            },
            parse,
            "Chosen"
        ),
        QueryPoll::Ready(Err(_))
    ));
    let mut foreign = context.scope.clone();
    foreign.crate_key = Digest([249; 32]);
    assert!(matches!(
        lookup_type_declaration(
            &mut db,
            crate::frontend::type_name_namespace::SourceTypeOwner {
                scope: &foreign,
                ..owner
            },
            parse,
            "Chosen"
        ),
        QueryPoll::Ready(Err(_))
    ));
}
