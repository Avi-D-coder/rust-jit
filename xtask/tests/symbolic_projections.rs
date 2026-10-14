//! Original-header recipe controls, not normalization or compiletest acceptance.
use lrr::{
    core::digest::{Digest, StableEncode},
    ir::op::PrimitiveType,
    traits::{
        subst::{Substitution, instantiate_binder, validate_closed},
        term::{
            Binder, BoundKind, FunctionSignature, GenericArg, ParamKey, ProjectionKey, RegionTerm,
            TraitRef, TyTerm,
        },
        unify::Unifier,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasGenericParams, HasName},
};
const ORIGINAL: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/higher-ranked/trait-bounds/rigid-equate-projections-in-higher-ranked-fn-signature.rs"
);
// TypeAnchor::path_type selects the first PathType child, which may be
// the self type T. The pinned PathSegment extension separates the two slots.
fn projection_trait(path: &ast::Path) -> ast::PathType {
    let segment = path.qualifier().unwrap().segment().unwrap();
    let Some(ast::PathSegmentKind::Type {
        type_ref: Some(_),
        trait_ref: Some(trait_type),
    }) = segment.kind()
    else {
        panic!("original qualified trait projection missing")
    };
    trait_type
}
fn original_projection() -> (ProjectionKey, ParamKey) {
    let parsed = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let function = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().unwrap().text() == "foo")
        .unwrap();
    let ast::Type::ForType(bound) = function.ret_type().unwrap().ty().unwrap() else {
        panic!("original return binder lost")
    };
    let lifetimes: Vec<_> = bound
        .for_binder()
        .unwrap()
        .generic_param_list()
        .unwrap()
        .generic_params()
        .collect();
    assert_eq!(lifetimes.len(), 1);
    let ast::GenericParam::LifetimeParam(lifetime) = &lifetimes[0] else {
        panic!("original lifetime binder lost")
    };
    let written = lifetime.lifetime().unwrap().syntax().text().to_string();
    let ast::Type::FnPtrType(pointer) = bound.ty().unwrap() else {
        panic!("original function pointer lost")
    };
    let ast::Type::PathType(projected) = pointer
        .param_list()
        .unwrap()
        .params()
        .next()
        .unwrap()
        .ty()
        .unwrap()
    else {
        panic!("original projection lost")
    };
    let path = projected.path().unwrap();
    let member = path.segment().unwrap();
    let anchor = path
        .qualifier()
        .unwrap()
        .segment()
        .unwrap()
        .type_anchor()
        .unwrap();
    assert!(anchor.as_token().is_some());
    let self_name = anchor.ty().unwrap().syntax().text().to_string();
    let generic=function.generic_param_list().unwrap().generic_params().enumerate().find(|(_,parameter)|matches!(parameter,ast::GenericParam::TypeParam(p) if p.name().unwrap().text()==self_name.as_str())).unwrap();
    let parameter = ParamKey {
        definition: Digest::of_bytes(function.syntax().text().to_string().as_bytes()),
        ordinal: u32::try_from(generic.0).unwrap(),
    };
    let trait_type = projection_trait(&path);
    let trait_path = trait_type.path().unwrap();
    let segment = trait_path.segment().unwrap();
    let args: Vec<_> = segment.generic_arg_list().unwrap().generic_args().collect();
    assert_eq!(args.len(), 1);
    let ast::GenericArg::LifetimeArg(arg) = &args[0] else {
        panic!("original trait region lost")
    };
    assert_eq!(arg.lifetime().unwrap().syntax().text().to_string(), written);
    // These keys identify the source-derived recipe positions. M019 source
    // declaration/world resolution is separate; no selected impl is invented.
    let trait_key = Digest::of_bytes(segment.name_ref().unwrap().text().as_bytes());
    let item = Digest::of_bytes(member.name_ref().unwrap().text().as_bytes());
    (
        ProjectionKey {
            trait_ref: TraitRef {
                trait_key,
                self_ty: TyTerm::Param(parameter),
                args: vec![GenericArg::Region(RegionTerm::Bound { depth: 0, index: 0 })],
            },
            item,
            item_args: vec![],
        },
        parameter,
    )
}
#[test]
fn original_projection_substitution_keeps_trait_identity_and_bound_region() {
    let (projection, parameter) = original_projection();
    let source = TyTerm::Function(Box::new(Binder {
        variables: vec![BoundKind::Region],
        value: FunctionSignature {
            parameters: vec![TyTerm::Projection(Box::new(projection.clone()))],
            result: Box::new(TyTerm::Primitive(PrimitiveType::Unit)),
        },
    }));
    validate_closed(&source).unwrap();
    let mut subst = Substitution::default();
    subst
        .insert(
            parameter,
            GenericArg::Type(TyTerm::Ref(
                RegionTerm::Bound { depth: 0, index: 0 },
                false,
                Box::new(TyTerm::Primitive(PrimitiveType::Unit)),
            )),
        )
        .unwrap();
    let changed = subst.apply(&source).unwrap();
    let TyTerm::Function(pointer) = &changed else {
        panic!("function lost")
    };
    let TyTerm::Projection(changed_projection) = &pointer.value.parameters[0] else {
        panic!("projection lost")
    };
    assert_eq!(
        changed_projection.trait_ref.self_ty,
        TyTerm::Ref(
            RegionTerm::Bound { depth: 1, index: 0 },
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Unit))
        )
    );
    assert_eq!(changed_projection.trait_ref.args, projection.trait_ref.args);
    assert_eq!(
        changed_projection.trait_ref.trait_key,
        projection.trait_ref.trait_key
    );
    assert_eq!(changed_projection.item, projection.item);
    assert!(validate_closed(&changed).is_err()); // free replacement is not captured.
    let mut subst = Substitution::default();
    subst
        .insert(
            parameter,
            GenericArg::Type(TyTerm::Primitive(PrimitiveType::Unit)),
        )
        .unwrap();
    validate_closed(&subst.apply(&source).unwrap()).unwrap();
}
#[test]
fn original_projection_placeholder_instantiation_is_not_legacy_unification() {
    use lrr::traits::term::InferVarKey;
    let (projection, _) = original_projection();
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let opened = instantiate_binder(
        &Binder {
            variables: vec![BoundKind::Region],
            value: TyTerm::Projection(Box::new(projection)),
        },
        transaction,
        1,
    )
    .unwrap();
    let TyTerm::Projection(key) = &opened else {
        panic!("projection lost")
    };
    assert!(
        matches!(key.trait_ref.args[0],GenericArg::Region(RegionTerm::Placeholder(p)) if p.transaction==transaction && p.universe==1 && p.index==0)
    );
    let variable = InferVarKey {
        transaction,
        ordinal: 0,
    };
    let mut unifier = Unifier::with_universe(transaction, 1).unwrap();
    assert!(unifier.unify(&TyTerm::Infer(variable), &opened).is_err());
    assert!(unifier.binding(variable).is_none());
    unifier
        .unify(
            &TyTerm::Infer(variable),
            &TyTerm::Primitive(PrimitiveType::Unit),
        )
        .unwrap();
}
#[test]
fn original_gat_item_arguments_preserve_bound_and_static_regions() {
    const ORIGINAL_GAT: &str = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/generic-associated-types/construct_with_other_type.rs"
    );
    let parsed = ast::SourceFile::parse(ORIGINAL_GAT, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let projected = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Path::cast)
        .find(|path| {
            path.segment()
                .is_some_and(|s| s.name_ref().is_some_and(|n| n.text() == "Bar"))
                && path
                    .qualifier()
                    .and_then(|p| p.segment())
                    .and_then(|s| s.type_anchor())
                    .and_then(|a| a.ty())
                    .is_some_and(|t| t.syntax().text() == "T")
        })
        .unwrap();
    let segment = projected.segment().unwrap();
    let lifetimes: Vec<_> = segment
        .generic_arg_list()
        .unwrap()
        .generic_args()
        .map(|arg| {
            let ast::GenericArg::LifetimeArg(arg) = arg else {
                panic!("original GAT argument lost")
            };
            arg.lifetime().unwrap().syntax().text().to_string()
        })
        .collect();
    assert_eq!(lifetimes.len(), 2);
    assert_eq!(lifetimes[1], "'static");
    assert_ne!(lifetimes[0], lifetimes[1]);
    let impl_owner = projected
        .syntax()
        .ancestors()
        .find_map(ast::Impl::cast)
        .unwrap();
    let parameter = impl_owner
        .generic_param_list()
        .unwrap()
        .generic_params()
        .enumerate()
        .find(|(_, p)| matches!(p,ast::GenericParam::TypeParam(p) if p.name().unwrap().text()=="T"))
        .unwrap();
    let projection_self = ParamKey {
        definition: Digest::of_bytes(impl_owner.syntax().text().to_string().as_bytes()),
        ordinal: u32::try_from(parameter.0).unwrap(),
    };
    let mut projection = ProjectionKey {
        trait_ref: TraitRef {
            trait_key: Digest::of_bytes(
                projection_trait(&projected)
                    .path()
                    .unwrap()
                    .segment()
                    .unwrap()
                    .name_ref()
                    .unwrap()
                    .text()
                    .as_bytes(),
            ),
            self_ty: TyTerm::Param(projection_self),
            args: vec![],
        },
        item: Digest::of_bytes(segment.name_ref().unwrap().text().as_bytes()),
        item_args: vec![],
    };
    projection.item_args = vec![
        GenericArg::Region(RegionTerm::Bound { depth: 0, index: 0 }),
        GenericArg::Region(RegionTerm::Static),
    ];
    let transaction = Digest::of_bytes(ORIGINAL_GAT.as_bytes());
    let template = Binder {
        variables: vec![BoundKind::Region],
        value: TyTerm::Projection(Box::new(projection)),
    };
    let original_fingerprint = template.fingerprint();
    let opened = instantiate_binder(&template, transaction, 1).unwrap();
    let TyTerm::Projection(key) = opened else {
        panic!("GAT projection lost")
    };
    assert!(matches!(
        key.item_args[0],
        GenericArg::Region(RegionTerm::Placeholder(_))
    ));
    assert_eq!(key.item_args[1], GenericArg::Region(RegionTerm::Static));
    assert_eq!(template.fingerprint(), original_fingerprint);
    let mut different = template.clone();
    let TyTerm::Projection(key) = &mut different.value else {
        panic!("projection lost")
    };
    key.item_args[0] = GenericArg::Region(RegionTerm::Static);
    assert_ne!(template.fingerprint(), different.fingerprint());
    // Recipe adaptation tests associated-item argument traversal, not this
    // original's Deref implementation, GAT bounds or body/typechecking result.
}

#[test]
fn original_projected_definition_assumption_rejects_transaction_placeholders() {
    use lrr::traits::term::ParamEnv;
    let (projection, parameter) = original_projection();
    let fact = Binder {
        variables: vec![BoundKind::Region],
        value: TraitRef {
            trait_key: projection.trait_ref.trait_key,
            self_ty: TyTerm::Projection(Box::new(projection)),
            args: vec![],
        },
    };
    let env = ParamEnv::new(
        parameter.definition,
        vec![(BoundKind::Type, parameter)],
        vec![fact.clone()],
    )
    .unwrap();
    assert_eq!(env.predicates(), std::slice::from_ref(&fact));
    let opened = instantiate_binder(
        &Binder {
            variables: fact.variables.clone(),
            value: fact.value.self_ty.clone(),
        },
        Digest::of_bytes(ORIGINAL.as_bytes()),
        1,
    )
    .unwrap();
    let invalid = Binder {
        variables: vec![],
        value: TraitRef {
            trait_key: fact.value.trait_key,
            self_ty: opened,
            args: vec![],
        },
    };
    assert_eq!(
        ParamEnv::new(
            parameter.definition,
            vec![(BoundKind::Type, parameter)],
            vec![invalid]
        )
        .unwrap_err()
        .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(env.predicates(), &[fact]);
    // An LRR definition-admission control over original-derived terms. This
    // does not assert the original had this extra bound or fulfill any goal.
}
