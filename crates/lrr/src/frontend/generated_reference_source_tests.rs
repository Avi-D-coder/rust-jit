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

#[test]
fn generated_body_context_preserves_complete_facts_without_inventing_outer_macro_rib() {
    use crate::{
        db::dependency::{InputKey, InputValue},
        frontend::{source_body_context, source_macros::PublishedMacroIndex},
        ir::pc::{NodeId, Phase},
    };
    use std::sync::Arc;
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("actual body context: {error:?}"),
            QueryPoll::NeedInput(input) => panic!("actual body context suspended: {input:?}"),
        }
    }
    for edition in [2015, 2018, 2021, 2024] {
        for suffix in ["emit!();", "macro_rules! wrap{()=>{emit!();}}wrap!();"] {
            let source = format!(
                "struct Point{{x:isize}}macro_rules! emit{{()=>{{fn answer(p:&Point){{type Local=i32;use self::Point as Alias;let mut value=1;value;}}}}}}{suffix}fn main(){{}}"
            );
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(&source, edition, true, false);
            let body = db
                .input(&InputKey::DefinitionBody(signature.definition))
                .unwrap();
            let InputValue::DefinitionBody(body) = body.as_ref() else {
                panic!()
            };
            let index = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
            let QueryValue::BodyStructure(index) = index.value.as_ref() else {
                panic!()
            };
            let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
            let admitted = Arc::new(ready(source_body_context::capture(
                &mut db,
                &[&owner],
                index,
                &pc,
            )));
            let prepared = ready(Arc::clone(&admitted).prepare(&mut db));
            assert!(Arc::ptr_eq(prepared.source().index(), admitted.index()));
            assert!(
                prepared
                    .facts()
                    .items
                    .iter()
                    .any(|item| item.item.def_key.declared_name == "Local")
            );
            assert_eq!(prepared.facts().imports.len(), 1);
            let binding = prepared
                .facts()
                .bindings
                .iter()
                .find(|binding| binding.name == "value")
                .unwrap();
            assert!(
                admitted
                    .identifier_context(binding.name_range)
                    .unwrap()
                    .is_some()
            );
            let (root, facts) = crate::syntax::floating_fixture_hygiene::root(
                owner.selection().outline.crate_key,
                edition,
            );
            let original_index =
                PublishedMacroIndex::from_view(owner.share(), root, facts).unwrap();
            // Complete generated body facts are available. The original module
            // index still cannot invent the generated insertion macro boundary.
            assert!(
                original_index
                    .enter_prepared_body(Arc::clone(&prepared))
                    .is_err()
            );
            ready(prepared.require_current(&mut db));
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                prepared.require_current(&mut db),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
}

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
            use crate::frontend::source_types::NamedTypePreparation;
            let source = match crate::frontend::source_types::prepare_named(
                &mut db, &graph, &views, &signature, "Chosen",
            ) {
                QueryPoll::Ready(Ok(NamedTypePreparation::Ready(source))) => source,
                QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(parameter))) => {
                    panic!("edition {edition}, {text:?}: unexpected parameter {parameter:?}")
                }
                QueryPoll::Ready(Ok(NamedTypePreparation::Alias(alias))) => {
                    panic!("edition {edition}, {text:?}: alias outcome {alias:?}")
                }
                QueryPoll::Ready(Ok(NamedTypePreparation::Name(name))) => {
                    panic!("edition {edition}, {text:?}: name outcome {name:?}")
                }
                QueryPoll::Ready(Err(error)) => {
                    panic!("edition {edition}, {text:?}: preparation error {error:?}")
                }
                QueryPoll::NeedInput(request) => {
                    panic!("edition {edition}, {text:?}: input request {request:?}")
                }
            };
            let TyTerm::Nominal(point, args) = source.source() else {
                panic!("nominal source identity erased")
            };
            assert!(args.is_empty());
            let mut store = crate::runtime::cell::GuestStore::new(1, 1000, 1_000_000).unwrap();
            let descriptor = source.materialize(&mut store).unwrap();
            let crate::runtime::shape::TypeKey::Nominal {
                identity,
                definition,
                ..
            } = store.type_descriptor(descriptor).unwrap().key.as_ref()
            else {
                panic!("source-certified nominal descriptor lost its declaration")
            };
            assert_eq!(identity, point);
            let nominal_source = db.input(&InputKey::DefinitionSource(*point)).unwrap();
            let InputValue::DefinitionSource(parse) = nominal_source.as_ref() else {
                panic!("selected nominal source absent")
            };
            if let Some(generated) = db.input(&InputKey::GeneratedSyntax(parse.content)) {
                let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
                    panic!()
                };
                assert_eq!(unit.definitions.get(point), Some(definition));
                assert_ne!(definition.fingerprint(), *identity);
            } else {
                assert_eq!(definition.fingerprint(), *identity);
            }
            source.require_descriptor(&store, descriptor).unwrap();
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

#[test]
fn separate_generated_nominals_keep_actual_source_identity_with_their_declared_keys() {
    use crate::frontend::source_types::{NamedTypePreparation, prepare_named};
    use crate::runtime::{cell::GuestStore, shape::TypeKey};
    for edition in [2015, 2018, 2021, 2024] {
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut prepared = Vec::new();
        for text in [
            "macro_rules! emit{()=>{struct Point{x:isize}fn answer(p:&Point){}}}emit!();fn main(){}",
            "macro_rules! other{()=>{struct Point{x:isize}fn answer(p:&Point){}}}other!();fn main(){}",
            "macro_rules! emit{()=>{struct Point{x:isize}fn answer(p:&Point){}}}macro_rules! wrap{()=>{emit!();}}wrap!();fn main(){}",
        ] {
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(text, edition, true, false);
            db.trace.detailed = true;
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            let views = [&owner];
            let QueryPoll::Ready(Ok(NamedTypePreparation::Ready(source))) =
                prepare_named(&mut db, &graph, &views, &signature, "Point")
            else {
                panic!("actual generated nominal source preparation failed")
            };
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
                panic!("actual generated reference function selection failed")
            };
            let reference = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &views, 0)
                .unwrap()
                .unwrap();
            assert_eq!(reference.pointee_source(), source.source());
            let target = source.materialize(&mut store).unwrap();
            reference.materialize_hint(&mut store).unwrap();
            source.require_descriptor(&store, target).unwrap();
            let TypeKey::Nominal {
                identity,
                definition,
                schema,
                ..
            } = store.type_descriptor(target).unwrap().key.as_ref().clone()
            else {
                panic!("nominal descriptor absent")
            };
            let wrong_key = TypeKey::Nominal {
                identity: definition.fingerprint(),
                definition: definition.clone(),
                schema,
                args: Box::new([]),
                const_args: Box::new([]),
            };
            assert_ne!(
                store.type_descriptor(target).unwrap().key.fingerprint(),
                wrong_key.fingerprint()
            );
            let wrong_identity = store.intern_type(wrong_key).unwrap();
            assert!(source.require_descriptor(&store, wrong_identity).is_err());
            assert_ne!(identity, definition.fingerprint());
            assert!(
                !db.trace.events.iter().any(|event| matches!(
                    event.key,
                    QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                ))
            );
            prepared.push((source, target, definition.fingerprint()));
        }
        for left in 0..prepared.len() {
            for right in left + 1..prepared.len() {
                assert_eq!(prepared[left].2, prepared[right].2);
                assert_ne!(prepared[left].0.source(), prepared[right].0.source());
                assert_ne!(prepared[left].1, prepared[right].1);
                assert!(
                    prepared[left]
                        .0
                        .require_descriptor(&store, prepared[right].1)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn current_named_field_schema_uses_actual_original_and_generated_type_owners() {
    use crate::frontend::{
        source_types::{NamedTypePreparation, prepare_named},
        types::{NominalFieldBinding, SchemaNames},
    };
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "struct Point{x:isize,cold:fn()}macro_rules! emit{()=>{fn answer(p:&Point){}}}emit!();fn main(){}",
            "macro_rules! emit{()=>{struct Point{x:isize,cold:fn()}fn answer(p:&Point){}}}emit!();fn main(){}",
            "macro_rules! emit{()=>{struct Point{x:isize,cold:fn()}fn answer(p:&Point){}}}macro_rules! wrap{()=>{emit!();}}wrap!();fn main(){}",
        ] {
            let (mut db, signature, _, _, owner) =
                super::tests::prepared_config_retained(text, edition, true, false);
            db.trace.detailed = true;
            let graph = NameNamespaces::from_outlines(&[owner.outline()]).unwrap();
            let views = [&owner];
            let QueryPoll::Ready(Ok(NamedTypePreparation::Ready(source))) =
                prepare_named(&mut db, &graph, &views, &signature, "Point")
            else {
                panic!("actual current field type owner failed")
            };
            let TyTerm::Nominal(identity, _) = source.source() else {
                panic!("nominal identity absent")
            };
            let captured = db
                .fetch(&SyntaxProvider, QueryKey::TypeSchema(*identity))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                panic!()
            };
            let names = SchemaNames::from_current_database(&mut db, &graph, &views, schema)
                .into_ready()
                .unwrap()
                .unwrap();
            let NominalFieldBinding::Ready(member) = names
                .bind_named_field_current(&mut db, &views, None, "x", &signature.scope)
                .into_ready()
                .unwrap()
                .unwrap()
            else {
                panic!("current selected field did not use its owning Type receipt")
            };
            assert_eq!(member.identity(), *identity);
            assert_eq!(member.primitive(), Some(PrimitiveType::Isize));
            assert_eq!(member.field(), 0);
            let mut foreign = signature.scope.clone();
            foreign.crate_key = Digest([210; 32]);
            assert!(matches!(
                names.bind_named_field_current(&mut db, &views, None, "x", &foreign),
                QueryPoll::Ready(Err(_))
            ));
            let mut wrong_header = schema.as_ref().clone();
            wrong_header.header.tokens.clear();
            assert!(matches!(
                SchemaNames::from_current_database(&mut db, &graph, &views, &wrong_header),
                QueryPoll::Ready(Err(_))
            ));
            let mut wrong_owner = schema.as_ref().clone();
            wrong_owner.scope.crate_key = foreign.crate_key;
            assert!(matches!(
                SchemaNames::from_current_database(&mut db, &graph, &views, &wrong_owner),
                QueryPoll::Ready(Err(_))
            ));
            assert!(
                !db.trace.events.iter().any(|event| matches!(
                    event.key,
                    QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
                ))
            );
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                names.bind_named_field_current(&mut db, &views, None, "x", &signature.scope),
                QueryPoll::Ready(Err(_))
                    | QueryPoll::Ready(Ok(NominalFieldBinding::Deferred { .. }))
            ));
        }
    }
}
