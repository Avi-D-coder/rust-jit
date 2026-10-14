//! Original-derived owned recipes; no full rustc fixture or runtime acceptance.
use lrr::{
    core::digest::{Digest, StableEncode, StableEncoder},
    ir::op::PrimitiveType,
    traits::{
        subst::{Substitution, instantiate_binder, validate_closed},
        term::{
            Binder, BoundKind, ConstTerm, DynSchemaKey, FunctionSignature, GenericArg, InferVarKey,
            OpaqueKey, ParamEnv, ParamKey, PlaceholderKey, RegionTerm, TraitRef, TyTerm,
        },
        unify::Unifier,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasName},
};
const CAPTURE: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/impl-trait/type_parameters_captured.rs"
);
const PRECISE: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/impl-trait/precise-capturing/higher-ranked.rs"
);
const DYN: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/dyn-compatibility/assoc_type_bounds2.rs"
);

fn function(text: &str, name: &str) -> ast::Fn {
    let parsed = ast::SourceFile::parse(text, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|f| f.name().unwrap().text() == name)
        .unwrap()
}
fn capture() -> (OpaqueKey, ParamKey) {
    let f = function(CAPTURE, "foo");
    assert!(matches!(
        f.ret_type().unwrap().ty().unwrap(),
        ast::Type::ImplTraitType(_)
    ));
    let generics: Vec<_> = f.generic_param_list().unwrap().generic_params().collect();
    assert_eq!(generics.len(), 1);
    assert!(
        matches!(&generics[0], ast::GenericParam::TypeParam(p) if p.name().unwrap().text() == "T")
    );
    // Source-derived recipe anchors only; world/DefKey admission is later work.
    let definition = Digest::of_bytes(f.syntax().text().to_string().as_bytes());
    let parameter = ParamKey {
        definition,
        ordinal: 0,
    };
    (
        OpaqueKey::new(definition, 0, vec![(BoundKind::Type, parameter)]).unwrap(),
        parameter,
    )
}
fn typed_key() -> (OpaqueKey, ParamKey) {
    let (_, p) = capture();
    // Additional LRR namespace safety recipe over the original defining owner.
    // The original itself has one type capture, no const or lifetime parameter.
    (
        OpaqueKey::new(
            p.definition,
            0,
            vec![
                (BoundKind::Type, p),
                (BoundKind::Region, p),
                (BoundKind::Const, p),
            ],
        )
        .unwrap(),
        p,
    )
}
fn args(p: ParamKey) -> Vec<GenericArg> {
    vec![
        GenericArg::Type(TyTerm::Param(p)),
        GenericArg::Region(RegionTerm::Param(p)),
        GenericArg::Const(ConstTerm::Param(p)),
    ]
}
fn unit() -> TyTerm {
    TyTerm::Primitive(PrimitiveType::Unit)
}

#[test]
fn original_opaque_return_identity_preserves_capture_set_and_position() {
    let (key, p) = capture();
    assert_eq!(key.definition(), p.definition);
    assert_eq!(key.return_position(), 0);
    assert_eq!(key.captures(), &[(BoundKind::Type, p)]);
    let source = TyTerm::Opaque(key.clone(), vec![GenericArg::Type(TyTerm::Param(p))]);
    for changed in [
        OpaqueKey::new(p.definition, 1, key.captures().to_vec()).unwrap(),
        OpaqueKey::new(p.definition, 0, vec![]).unwrap(),
        OpaqueKey::new(
            Digest::of_bytes(PRECISE.as_bytes()),
            0,
            key.captures().to_vec(),
        )
        .unwrap(),
        OpaqueKey::new(p.definition, 0, vec![(BoundKind::Region, p)]).unwrap(),
    ] {
        assert_ne!(key.fingerprint(), changed.fingerprint());
    }
    let mut encoded = StableEncoder::new(TyTerm::DOMAIN);
    encoded.u32(14);
    encoded.record(&key);
    encoded.sequence(&[GenericArg::Type(TyTerm::Param(p))], |e, a| e.record(a));
    assert_eq!(source.fingerprint(), encoded.finish());
    assert_ne!(
        source.fingerprint(),
        TyTerm::Nominal(key.fingerprint(), vec![GenericArg::Type(TyTerm::Param(p))]).fingerprint()
    );
    validate_closed(&source).unwrap();
    assert!(OpaqueKey::new(p.definition, 0, vec![(BoundKind::Type, p); 2]).is_err());
    let (typed, _) = typed_key();
    let mut reversed = typed.captures().to_vec();
    reversed.reverse();
    assert_eq!(typed, OpaqueKey::new(p.definition, 0, reversed).unwrap());
    // This does not prove the original's 'static diagnostic or hidden type.
}

#[test]
fn original_precise_empty_capture_does_not_capture_higher_ranked_region() {
    let f = function(PRECISE, "hello");
    let result = f.ret_type().unwrap().ty().unwrap();
    let opaques: Vec<_> = result
        .syntax()
        .descendants()
        .filter_map(ast::ImplTraitType::cast)
        .collect();
    assert_eq!(opaques.len(), 2);
    let precise = opaques[1]
        .syntax()
        .descendants()
        .find_map(ast::UseBoundGenericArgs::cast)
        .unwrap();
    assert_eq!(precise.use_bound_generic_args().count(), 0);
    let binder = opaques[0]
        .syntax()
        .descendants()
        .find_map(ast::ForBinder::cast)
        .unwrap();
    assert_eq!(
        binder
            .generic_param_list()
            .unwrap()
            .generic_params()
            .count(),
        1
    );
    let definition = Digest::of_bytes(f.syntax().text().to_string().as_bytes());
    let inner = OpaqueKey::new(definition, 1, vec![]).unwrap();
    let source = Binder {
        variables: vec![BoundKind::Region],
        value: TyTerm::Opaque(inner, vec![]),
    };
    let opened = instantiate_binder(&source, definition, 1).unwrap();
    assert_eq!(opened, source.value);
    assert_ne!(
        source.value,
        TyTerm::Opaque(OpaqueKey::new(definition, 0, vec![]).unwrap(), vec![])
    );
    // Only original nested return/capture positions are adapted. IntoIterator,
    // associated bindings and the full check-pass outcome remain unexecuted.
}

#[test]
fn original_dyn_schema_identity_is_distinct_and_requires_later_admission() {
    let mut schemas = Vec::new();
    for name in ["foo", "bar"] {
        let f = function(DYN, name);
        let schema = f
            .syntax()
            .descendants()
            .find_map(ast::DynTraitType::cast)
            .unwrap();
        schemas.push(DynSchemaKey(Digest::of_bytes(
            schema.syntax().text().to_string().as_bytes(),
        )));
    }
    assert_ne!(schemas[0], schemas[1]);
    let term = TyTerm::Dyn(schemas[0]);
    let mut encoded = StableEncoder::new(TyTerm::DOMAIN);
    encoded.u32(13);
    encoded.record(&schemas[0]);
    assert_eq!(term.fingerprint(), encoded.finish());
    assert_ne!(
        term.fingerprint(),
        TyTerm::Nominal(schemas[0].0, vec![]).fingerprint()
    );
    assert_eq!(Substitution::default().apply(&term).unwrap(), term);
    validate_closed(&term).unwrap();
    let transaction = Digest::of_bytes(DYN.as_bytes());
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let mut solver = Unifier::new(transaction);
    for candidate in [
        term.clone(),
        TyTerm::Tuple(vec![term.clone()]),
        TyTerm::Nominal(schemas[0].0, vec![GenericArg::Type(term.clone())]),
    ] {
        assert_eq!(
            solver
                .unify(&TyTerm::Infer(variable), &candidate)
                .unwrap_err()
                .code,
            "LRR-E-GENERIC-BINDING"
        );
        assert!(solver.binding(variable).is_none());
    }
    assert!(solver.unify(&term, &term).is_err());
    solver.unify(&TyTerm::Infer(variable), &unit()).unwrap();
    // Constructing source keys does not accept either invalid dyn schema or
    // satisfy the original required-associated-type diagnostics.
}

#[test]
fn original_opaque_arguments_use_checked_typed_capture_avoiding_substitution() {
    let (key, p) = typed_key();
    let mut subst = Substitution::default();
    subst
        .insert(p, GenericArg::Type(TyTerm::Bound { depth: 0, index: 0 }))
        .unwrap();
    subst
        .insert(
            p,
            GenericArg::Region(RegionTerm::Bound { depth: 0, index: 1 }),
        )
        .unwrap();
    subst
        .insert(
            p,
            GenericArg::Const(ConstTerm::Bound { depth: 0, index: 2 }),
        )
        .unwrap();
    let term = TyTerm::Function(Box::new(Binder {
        variables: vec![BoundKind::Type, BoundKind::Region, BoundKind::Const],
        value: FunctionSignature {
            parameters: vec![],
            result: Box::new(TyTerm::Opaque(key.clone(), args(p))),
        },
    }));
    validate_closed(&term).unwrap();
    let changed = subst.apply(&term).unwrap();
    let TyTerm::Function(f) = &changed else {
        panic!("function missing")
    };
    assert_eq!(
        *f.value.result,
        TyTerm::Opaque(
            key.clone(),
            vec![
                GenericArg::Type(TyTerm::Bound { depth: 1, index: 0 }),
                GenericArg::Region(RegionTerm::Bound { depth: 1, index: 1 }),
                GenericArg::Const(ConstTerm::Bound { depth: 1, index: 2 })
            ]
        )
    );
    assert!(validate_closed(&changed).is_err());
    assert_eq!(
        key.captures(),
        &[
            (BoundKind::Type, p),
            (BoundKind::Region, p),
            (BoundKind::Const, p)
        ]
    );
    let transaction = p.definition;
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let mut solver = Unifier::new(transaction);
    let opaque = TyTerm::Opaque(key.clone(), args(p));
    assert!(solver.unify(&TyTerm::Infer(variable), &opaque).is_err());
    assert!(solver.binding(variable).is_none());
    assert!(solver.unify(&opaque, &opaque).is_err());
    for bad in [vec![], vec![GenericArg::Const(ConstTerm::Value(0)); 3]] {
        let malformed = TyTerm::Opaque(key.clone(), bad);
        assert_eq!(
            validate_closed(&malformed).unwrap_err().code,
            "LRR-E-GENERIC-BINDING"
        );
        assert_eq!(
            subst.apply(&malformed).unwrap_err().code,
            "LRR-E-GENERIC-BINDING"
        );
    }
}

#[test]
fn original_opaque_arguments_instantiate_typed_binders_and_reject_definition_leaks() {
    let (key, p) = typed_key();
    let template = Binder {
        variables: vec![BoundKind::Type, BoundKind::Region, BoundKind::Const],
        value: TyTerm::Opaque(
            key.clone(),
            vec![
                GenericArg::Type(TyTerm::Bound { depth: 0, index: 0 }),
                GenericArg::Region(RegionTerm::Bound { depth: 0, index: 1 }),
                GenericArg::Const(ConstTerm::Bound { depth: 0, index: 2 }),
            ],
        ),
    };
    let opened = instantiate_binder(&template, p.definition, 1).unwrap();
    let TyTerm::Opaque(open_key, actual) = &opened else {
        panic!("opaque lost")
    };
    assert_eq!(open_key, &key);
    let placeholder = |index| PlaceholderKey {
        transaction: p.definition,
        universe: 1,
        index,
    };
    assert_eq!(
        actual,
        &vec![
            GenericArg::Type(TyTerm::Placeholder(placeholder(0))),
            GenericArg::Region(RegionTerm::Placeholder(placeholder(1))),
            GenericArg::Const(ConstTerm::Placeholder(placeholder(2)))
        ]
    );
    let fact = |ty| Binder {
        variables: vec![],
        value: TraitRef {
            trait_key: Digest::of_bytes(b"Any"),
            self_ty: ty,
            args: vec![],
        },
    };
    let parameters = key.captures().to_vec();
    let original = TyTerm::Opaque(key.clone(), args(p));
    let env = ParamEnv::new(
        p.definition,
        parameters.clone(),
        vec![fact(original.clone())],
    )
    .unwrap();
    for index in 0..3 {
        let mut changed = args(p);
        changed[index] = actual[index].clone();
        assert_eq!(
            ParamEnv::new(
                p.definition,
                parameters.clone(),
                vec![fact(TyTerm::Opaque(key.clone(), changed))]
            )
            .unwrap_err()
            .code,
            "LRR-E-GENERIC-BINDING"
        );
        let mut missing = parameters.clone();
        missing.remove(index);
        assert!(ParamEnv::new(p.definition, missing, vec![fact(original.clone())]).is_err());
    }
    assert_eq!(env.predicates(), &[fact(original)]);
    let mut invalid = template.clone();
    let TyTerm::Opaque(_, a) = &mut invalid.value else {
        unreachable!()
    };
    a[1] = GenericArg::Region(RegionTerm::Bound { depth: 0, index: 0 });
    assert!(instantiate_binder(&invalid, p.definition, 1).is_err());
}

#[test]
fn original_opaque_owned_metadata_is_bounded_before_copying() {
    let (key, p) = capture();
    let source = TyTerm::Opaque(key.clone(), vec![GenericArg::Type(unit())]);
    assert!(
        source.heap_bytes().unwrap()
            >= std::mem::size_of::<(BoundKind, ParamKey)>() + std::mem::size_of::<GenericArg>()
    );
    let mut captures = Vec::with_capacity(100_001);
    captures.push((BoundKind::Type, p));
    assert_eq!(
        OpaqueKey::new(p.definition, 0, captures).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    let huge = TyTerm::Opaque(
        key,
        vec![GenericArg::Type(TyTerm::Tuple(vec![unit(); 100_000]))],
    );
    assert!(huge.heap_bytes().is_none());
    let mut subst = Substitution::default();
    assert_eq!(
        subst.insert(p, GenericArg::Type(huge)).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    subst.insert(p, GenericArg::Type(unit())).unwrap();
    assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), unit());
}

fn original_frame_contracts() -> (
    lrr::syntax::signatures::FunctionSignature,
    lrr::frontend::call_contracts::PreparedCallContracts,
    ParamKey,
) {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{OutlineKey, QueryKey},
            memo::Database,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        frontend::call_contracts::PreparedCallContracts,
        runtime::shape::TypeId,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let blob = SourceBlob::new(CAPTURE.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
    let crate_key = Digest::of_bytes(CAPTURE.as_bytes());
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
    let definition = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "foo")
        .unwrap()
        .def_key
        .fingerprint();
    // A frame metadata control consumes the original declaration directly.
    // The original's unresolved std import remains a discovery frontier; this
    // adapter does not assert completed callable lookup or body execution.
    let value = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("original foo signature: {error}"),
        QueryPoll::NeedInput(input) => panic!("original foo signature needs input: {input:?}"),
    };
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("original foo query returned another record")
    };
    let signature = signature.as_ref().clone();
    let p = ParamKey {
        definition: signature.definition,
        ordinal: 0,
    };
    assert_eq!(signature.definition, definition);
    assert_eq!(signature.parameters.len(), 1);
    assert_eq!(signature.parameters[0].symbolic, Some(TyTerm::Param(p)));
    assert!(signature.result_symbolic.is_none());
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::Constant(_)
    )));
    let contracts = PreparedCallContracts {
        definition: p.definition,
        source_parameters: vec![TyTerm::Param(p)],
        parameter_descriptors: vec![TypeId(1)],
        source_substitution: [(p, unit())].into(),
        runtime_substitution: [(p, TypeId(1))].into(),
        source_result: signature.result_symbolic.clone(),
        result_descriptor: None,
        normalized_deferred_return: None,
    };
    (signature, contracts, p)
}

#[test]
fn original_opaque_frame_quarantines_each_transaction_argument_before_capture() {
    use lrr::vm::context::GenericEnv;
    let (signature, mut contracts, p) = original_frame_contracts();
    let signature = &signature;
    let (key, _) = typed_key();
    let placeholder = PlaceholderKey {
        transaction: p.definition,
        universe: 1,
        index: 0,
    };
    let infer = InferVarKey {
        transaction: p.definition,
        ordinal: 0,
    };
    let before = GenericEnv::capture(signature, &contracts)
        .unwrap()
        .signature_key();
    let valid = vec![
        GenericArg::Type(unit()),
        GenericArg::Region(RegionTerm::Static),
        GenericArg::Const(ConstTerm::Value(0)),
    ];
    for (index, arg) in [
        (0, GenericArg::Type(TyTerm::Infer(infer))),
        (0, GenericArg::Type(TyTerm::Placeholder(placeholder))),
        (1, GenericArg::Region(RegionTerm::Placeholder(placeholder))),
        (2, GenericArg::Const(ConstTerm::Placeholder(placeholder))),
    ] {
        let mut a = valid.clone();
        a[index] = arg;
        let term = TyTerm::Opaque(key.clone(), a);
        contracts.source_substitution.insert(p, term.clone());
        assert_eq!(
            GenericEnv::capture(signature, &contracts).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        contracts.source_substitution.insert(p, unit());
        let mut declaration = signature.clone();
        declaration.parameters[0].symbolic = Some(term.clone());
        assert_eq!(
            GenericEnv::capture(&declaration, &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        declaration = signature.clone();
        declaration.result_symbolic = Some(term.clone());
        contracts.source_result = Some(term.clone());
        assert_eq!(
            GenericEnv::capture(&declaration, &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        contracts.source_result = signature.result_symbolic.clone();
        contracts.source_parameters[0] = term;
        assert_eq!(
            GenericEnv::capture(signature, &contracts).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        contracts.source_parameters[0] = TyTerm::Param(p);
    }
    assert_eq!(
        GenericEnv::capture(signature, &contracts)
            .unwrap()
            .signature_key(),
        before
    );
    // Header capture plus direct malformed-contract controls only; the original
    // foo body, hidden descriptor, trait bounds and rustc diagnostic are not run.
}

// All wrappers are LRR structural-admission controls around the same
// original-derived opaque return identity, not extra source-language fixtures.
fn nested_opaque_shapes(term: TyTerm, key: &OpaqueKey) -> Vec<TyTerm> {
    use lrr::traits::term::ProjectionKey;
    let trait_key = Digest::of_bytes(b"Any");
    let projection = |self_ty, args, item_args| {
        TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key,
                self_ty,
                args,
            },
            item: key.fingerprint(),
            item_args,
        }))
    };
    vec![
        term.clone(),
        TyTerm::Tuple(vec![term.clone()]),
        TyTerm::Slice(Box::new(term.clone())),
        TyTerm::Array(Box::new(term.clone()), ConstTerm::Value(1)),
        TyTerm::Ref(RegionTerm::Static, false, Box::new(term.clone())),
        TyTerm::Nominal(key.definition(), vec![GenericArg::Type(term.clone())]),
        TyTerm::Opaque(key.clone(), vec![GenericArg::Type(term.clone())]),
        TyTerm::Function(Box::new(Binder {
            variables: vec![],
            value: FunctionSignature {
                parameters: vec![term.clone()],
                result: Box::new(unit()),
            },
        })),
        TyTerm::Function(Box::new(Binder {
            variables: vec![],
            value: FunctionSignature {
                parameters: vec![],
                result: Box::new(term.clone()),
            },
        })),
        projection(term.clone(), vec![], vec![]),
        projection(unit(), vec![GenericArg::Type(term.clone())], vec![]),
        projection(unit(), vec![], vec![GenericArg::Type(term)]),
    ]
}

#[test]
fn original_opaque_frame_rejects_malformed_capture_shapes_before_owning_clone() {
    use lrr::vm::context::GenericEnv;
    let (signature, mut contracts, p) = original_frame_contracts();
    let initial = GenericEnv::capture(&signature, &contracts).unwrap();
    let original_signature = signature.fingerprint();
    let (key, _) = capture();
    for wrong in [
        vec![],
        vec![GenericArg::Type(unit()); 2],
        vec![GenericArg::Region(RegionTerm::Static)],
        vec![GenericArg::Const(ConstTerm::Value(0))],
    ] {
        let malformed = TyTerm::Opaque(key.clone(), wrong);
        // The V15 witness: this is not a resource failure or a placeholder.
        assert!(malformed.heap_bytes().is_some());
        assert_eq!(
            validate_closed(&malformed).unwrap_err().code,
            "LRR-E-GENERIC-BINDING"
        );
        for term in nested_opaque_shapes(malformed, &key) {
            assert_eq!(
                term.validate_opaque_shape().unwrap_err().code,
                "LRR-E-GENERIC-BINDING"
            );
            contracts.source_substitution.insert(p, term.clone());
            assert_eq!(
                GenericEnv::capture(&signature, &contracts)
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
            contracts.source_substitution.insert(p, unit());
            let mut declaration = signature.clone();
            declaration.parameters[0].symbolic = Some(term.clone());
            assert_eq!(
                GenericEnv::capture(&declaration, &contracts)
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
            declaration = signature.clone();
            declaration.result_symbolic = Some(term.clone());
            contracts.source_result = Some(term.clone());
            assert_eq!(
                GenericEnv::capture(&declaration, &contracts)
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
            contracts.source_result = signature.result_symbolic.clone();
            contracts.source_parameters[0] = term;
            assert_eq!(
                GenericEnv::capture(&signature, &contracts)
                    .unwrap_err()
                    .code,
                "LRR-E-CALL-CONTEXT"
            );
            contracts.source_parameters[0] = TyTerm::Param(p);
            assert_eq!(
                GenericEnv::capture(&signature, &contracts)
                    .unwrap()
                    .signature_key(),
                original_signature
            );
            assert_eq!(initial.signature_key(), original_signature);
            assert_eq!(initial.symbolic_binding(p), Some(&unit()));
        }
    }
    assert_eq!(signature.fingerprint(), original_signature);
}

#[test]
fn original_opaque_frame_shape_admission_preserves_abstract_and_enclosing_bound_terms() {
    use lrr::vm::context::GenericEnv;
    let (signature, mut contracts, p) = original_frame_contracts();
    let (key, source_parameter) = typed_key();
    let enclosing = TyTerm::Opaque(
        key.clone(),
        vec![
            GenericArg::Type(TyTerm::Bound { depth: 2, index: 0 }),
            GenericArg::Region(RegionTerm::Bound { depth: 2, index: 1 }),
            GenericArg::Const(ConstTerm::Bound { depth: 2, index: 2 }),
        ],
    );
    assert!(validate_closed(&enclosing).is_err());
    for source in [TyTerm::Opaque(key, args(source_parameter)), enclosing] {
        source.validate_opaque_shape().unwrap();
        let mut declaration = signature.clone();
        declaration.parameters[0].symbolic = Some(source.clone());
        declaration.result_symbolic = Some(source.clone());
        contracts.source_result = Some(source.clone());
        contracts.source_parameters[0] = source.clone();
        contracts.source_substitution.insert(p, source.clone());
        let env = GenericEnv::capture(&declaration, &contracts).unwrap();
        assert_eq!(env.symbolic_binding(p), Some(&source));
        assert_eq!(env.definition(), &declaration);
        assert_eq!(contracts.source_substitution.get(&p), Some(&source));
    }
    // A shape guard must not reinterpret permitted enclosing binder context as
    // standalone closed-term admission or prove source/world schema authority.
}

#[test]
fn original_opaque_capture_identity_rejects_out_of_domain_parameter_ordinals() {
    let (_, mut p) = capture();
    for ordinal in [100_000, u32::MAX] {
        p.ordinal = ordinal;
        for kind in [BoundKind::Type, BoundKind::Region, BoundKind::Const] {
            assert_eq!(
                OpaqueKey::new(p.definition, 0, vec![(kind, p)])
                    .unwrap_err()
                    .code,
                "LRR-E-GENERIC-BINDING"
            );
        }
    }
    p.ordinal = 99_999;
    let key = OpaqueKey::new(p.definition, 0, vec![(BoundKind::Type, p)]).unwrap();
    assert_eq!(key.captures(), &[(BoundKind::Type, p)]);
    // LRR domain-boundary mutation of the original T anchor; the upstream
    // function still has its single unchanged ordinal-zero type parameter.
}

#[test]
fn original_opaque_substitution_rejects_malformed_shape_before_claiming_typed_key() {
    let (key, p) = capture();
    let retained = ParamKey {
        definition: p.definition,
        ordinal: 1,
    };
    for wrong in [
        vec![],
        vec![GenericArg::Type(unit()); 2],
        vec![GenericArg::Region(RegionTerm::Static)],
        vec![GenericArg::Const(ConstTerm::Value(0))],
    ] {
        for term in nested_opaque_shapes(TyTerm::Opaque(key.clone(), wrong), &key) {
            let mut subst = Substitution::default();
            subst.insert(retained, GenericArg::Type(unit())).unwrap();
            assert!(term.heap_bytes().is_some());
            assert_eq!(
                subst.insert(p, GenericArg::Type(term)).unwrap_err().code,
                "LRR-E-GENERIC-BINDING"
            );
            assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), TyTerm::Param(p));
            assert_eq!(subst.apply(&TyTerm::Param(retained)).unwrap(), unit());
            // The rejected type key remains available, including when the
            // same owner/ordinal is already present in the other namespaces.
            subst
                .insert(p, GenericArg::Region(RegionTerm::Static))
                .unwrap();
            subst
                .insert(p, GenericArg::Const(ConstTerm::Value(0)))
                .unwrap();
            subst.insert(p, GenericArg::Type(unit())).unwrap();
            assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), unit());
            let duplicate = subst
                .insert(p, GenericArg::Type(TyTerm::Primitive(PrimitiveType::Bool)))
                .unwrap_err();
            assert_eq!(duplicate.code, "LRR-E-GENERIC-BINDING");
            assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), unit());
        }
    }
    let free = TyTerm::Opaque(
        key,
        vec![GenericArg::Type(TyTerm::Bound { depth: 2, index: 0 })],
    );
    assert!(validate_closed(&free).is_err());
    let mut subst = Substitution::default();
    subst.insert(p, GenericArg::Type(free.clone())).unwrap();
    assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), free);
}

#[test]
fn original_opaque_rejected_substitution_preserves_exact_accumulated_work_budget() {
    let (key, p) = capture();
    let retained = ParamKey {
        definition: p.definition,
        ordinal: 1,
    };
    let filling = ParamKey {
        definition: p.definition,
        ordinal: 2,
    };
    let mut subst = Substitution::default();
    subst.insert(retained, GenericArg::Type(unit())).unwrap();
    let malformed = TyTerm::Opaque(key, vec![GenericArg::Type(unit()); 45_000]);
    assert!(malformed.heap_bytes().is_some());
    assert_eq!(
        subst
            .insert(p, GenericArg::Type(malformed))
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    // One existing entry/type costs two work units. This admitted tuple costs
    // 99,995 (entry + tuple + elements), corrected type retry costs two, and
    // the const entry costs one: exactly the existing 100,000-unit budget.
    subst
        .insert(
            filling,
            GenericArg::Type(TyTerm::Tuple(vec![unit(); 99_993])),
        )
        .unwrap();
    subst.insert(p, GenericArg::Type(unit())).unwrap();
    subst
        .insert(p, GenericArg::Const(ConstTerm::Value(0)))
        .unwrap();
    assert_eq!(
        subst
            .insert(p, GenericArg::Region(RegionTerm::Static))
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(subst.apply(&TyTerm::Param(retained)).unwrap(), unit());
    assert_eq!(subst.apply(&TyTerm::Param(p)).unwrap(), unit());
    // This is a controlled metadata-budget adaptation, not 45,000 source
    // captures or a full upstream function/diagnostic execution.
}

#[test]
fn original_precise_capture_outer_binder_checks_combined_work_and_owned_capacity() {
    use lrr::traits::subst::validate_binder_type;
    let f = function(PRECISE, "hello");
    let precise = f
        .syntax()
        .descendants()
        .find_map(ast::UseBoundGenericArgs::cast)
        .unwrap();
    assert_eq!(precise.use_bound_generic_args().count(), 0);
    let definition = Digest::of_bytes(f.syntax().text().to_string().as_bytes());
    let source = TyTerm::Opaque(OpaqueKey::new(definition, 1, vec![]).unwrap(), vec![]);
    // The source's empty capture remains independent of its outer universal
    // region. Extra unused slots/capacity below are LRR budget corruption.
    let exact_work = Binder {
        variables: vec![BoundKind::Region; 99_999],
        value: source.clone(),
    };
    validate_binder_type(&exact_work).unwrap();
    assert_eq!(
        instantiate_binder(&exact_work, definition, 1).unwrap(),
        source
    );
    let excess_work = Binder {
        variables: vec![BoundKind::Region; 100_000],
        value: source.clone(),
    };
    assert_eq!(
        validate_binder_type(&excess_work).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(
        instantiate_binder(&excess_work, definition, 1)
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
    let inline = std::mem::size_of::<Binder<TyTerm>>();
    let owned = source.heap_bytes().unwrap();
    let capacity = (32 * 1024 * 1024 - inline - owned) / std::mem::size_of::<BoundKind>();
    let mut variables = Vec::with_capacity(capacity);
    variables.push(BoundKind::Region);
    let exact_bytes = Binder {
        variables,
        value: source.clone(),
    };
    validate_binder_type(&exact_bytes).unwrap();
    assert_eq!(
        instantiate_binder(&exact_bytes, definition, 1).unwrap(),
        source
    );
    drop(exact_bytes);
    let mut variables = Vec::with_capacity(capacity + 1);
    variables.push(BoundKind::Region);
    let excess_bytes = Binder {
        variables,
        value: source,
    };
    assert_eq!(
        validate_binder_type(&excess_bytes).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(
        instantiate_binder(&excess_bytes, definition, 1)
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
}

fn original_context_exact_table_bytes(
    signature: &lrr::syntax::signatures::FunctionSignature,
    contracts: &lrr::frontend::call_contracts::PreparedCallContracts,
) -> usize {
    use lrr::{runtime::shape::TypeId, vm::context::GenericEnv};
    signature.estimated_bytes()
        + std::mem::size_of::<GenericEnv>()
        + contracts.source_substitution.len() * std::mem::size_of::<(ParamKey, TyTerm)>()
        + contracts.runtime_substitution.len() * std::mem::size_of::<(ParamKey, TypeId)>()
        + contracts
            .source_substitution
            .values()
            .map(|ty| ty.heap_bytes().unwrap())
            .sum::<usize>()
}

#[test]
fn original_generic_frame_tables_account_exact_owned_entries_and_preserve_lookup() {
    use lrr::vm::context::GenericEnv;
    let (signature, contracts, p) = original_frame_contracts();
    let expected = original_context_exact_table_bytes(&signature, &contracts);
    assert_eq!(
        GenericEnv::checked_bytes(&signature, &contracts).unwrap(),
        expected
    );
    let env = GenericEnv::capture(&signature, &contracts).unwrap();
    assert_eq!(env.retained_bytes(), expected);
    assert_eq!(env.symbolic_binding(p), Some(&unit()));
    assert_eq!(
        env.runtime_binding(p),
        contracts.runtime_substitution.get(&p).copied()
    );
    let absent = ParamKey {
        definition: p.definition,
        ordinal: 99_999,
    };
    assert_eq!(env.symbolic_binding(absent), None);
    assert_eq!(env.runtime_binding(absent), None);
    assert_eq!(env.definition(), &signature);
}

#[test]
fn original_opaque_frame_tables_account_nested_heap_without_a_flat_binding_guess() {
    use lrr::vm::context::GenericEnv;
    let (signature, mut contracts, p) = original_frame_contracts();
    let (key, _) = capture();
    let source = TyTerm::Opaque(key, vec![GenericArg::Type(unit())]);
    assert!(source.heap_bytes().unwrap() > 0);
    contracts.source_substitution.insert(p, source.clone());
    let expected = original_context_exact_table_bytes(&signature, &contracts);
    assert_eq!(
        GenericEnv::checked_bytes(&signature, &contracts).unwrap(),
        expected
    );
    let env = GenericEnv::capture(&signature, &contracts).unwrap();
    assert_eq!(env.retained_bytes(), expected);
    assert_eq!(env.symbolic_binding(p), Some(&source));
    assert_eq!(
        env.runtime_binding(p),
        contracts.runtime_substitution.get(&p).copied()
    );
    // Original foo header/return identity plus LRR metadata retention controls;
    // hidden-type, source-world admission and the original body are not executed.
}

#[test]
fn original_generic_frame_empty_tables_do_not_clone_vacant_tree_nodes() {
    use lrr::vm::context::GenericEnv;
    let (signature, mut contracts, p) = original_frame_contracts();
    contracts.source_substitution.remove(&p);
    contracts.runtime_substitution.remove(&p);
    assert!(contracts.source_substitution.is_empty());
    assert!(contracts.runtime_substitution.is_empty());
    let expected = original_context_exact_table_bytes(&signature, &contracts);
    let env = GenericEnv::capture(&signature, &contracts).unwrap();
    assert_eq!(env.retained_bytes(), expected);
    assert_eq!(env.symbolic_binding(p), None);
    assert_eq!(env.runtime_binding(p), None);
    // An unforced source parameter remains unbound. This does not certify an
    // executed call's generic fulfillment or the actual foo program.
}
