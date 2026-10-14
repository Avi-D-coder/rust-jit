//! Stock generic header plus LRR return-packet corruption, not a compiletest pass.
use lrr::{
    core::{
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
    frontend::call_contracts::{ArgumentType, PreparedCallContracts},
    runtime::shape::TypeId,
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, signatures::FunctionSignature},
    traits::term::{ParamKey, TyTerm},
    vm::context::GenericEnv,
};

const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-fn-infer.rs");

fn original_headers() -> Vec<FunctionSignature> {
    let profile = ProfileIdentity {
        edition: 2015,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
    let consumer = Digest::of_bytes(b"LRR return identity declaration consumer");
    let providers = [
        Digest::of_bytes(b"LRR original id definition world one"),
        Digest::of_bytes(b"LRR original id definition world two"),
    ];
    let mut units = Vec::new();
    let mut dependencies = Vec::new();
    for (index, provider) in providers.iter().enumerate() {
        let input = SourceUnitInput {
            path: SourcePath::new(format!("src/provider{index}.rs")).unwrap(),
            crate_key: *provider,
            module_path: vec![],
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob: blob.clone(),
        };
        PreparedSourceInputs::refresh(None, vec![input.clone()], Default::default()).unwrap();
        units.push(input);
        dependencies.push(SourceDependency {
            consumer,
            alias: format!("original{index}"),
            provider: *provider,
        });
    }
    units.push(SourceUnitInput {
        path: SourcePath::new("src/consumer.rs").unwrap(),
        crate_key: consumer,
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: cfg.clone(),
        blob: SourceBlob::new(Vec::new()).unwrap(),
    });
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        dependencies,
    )
    .unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut signatures = Vec::new();
    for provider in providers {
        let outline = match db.fetch(
            &SyntaxProvider,
            QueryKey::Outline(OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: provider,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            }),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            _ => panic!("original declaration outline unavailable"),
        };
        let QueryValue::Outline(outline) = outline.value.as_ref() else {
            panic!("original outline query returned another value");
        };
        let definition = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "id")
            .unwrap()
            .def_key
            .fingerprint();
        let value = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
            QueryPoll::Ready(Ok(value)) => value,
            _ => panic!("original generic id signature unavailable"),
        };
        let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
            panic!("original signature query returned another value");
        };
        assert_eq!(signature.header.profile.edition, 2015);
        assert_eq!(signature.generics.len(), 1);
        assert_eq!(signature.generics[0].name, "T");
        assert_eq!(signature.parameters.len(), 1);
        let parameter = ParamKey {
            definition,
            ordinal: signature.generics[0].ordinal,
        };
        assert_eq!(
            signature.parameters[0].symbolic,
            Some(TyTerm::Param(parameter))
        );
        assert_eq!(signature.result_symbolic, Some(TyTerm::Param(parameter)));
        signatures.push(signature.as_ref().clone());
    }
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::Constant(_)
    )));
    signatures
}

#[test]
fn original_generic_return_rebinding_preserves_source_identity_at_equal_descriptor() {
    let signatures = original_headers();
    assert_ne!(signatures[0].definition, signatures[1].definition);
    let template = ParamKey {
        definition: signatures[0].definition,
        ordinal: signatures[0].generics[0].ordinal,
    };
    let caller = ParamKey {
        definition: signatures[1].definition,
        ordinal: signatures[1].generics[0].ordinal,
    };
    // This is an LRR packet admission control. Unit is a checked concrete
    // descriptor shared by two distinct rigid source views, not a type proof.
    let mut contracts = PreparedCallContracts {
        definition: template.definition,
        source_parameters: vec![TyTerm::Param(template)],
        parameter_descriptors: vec![TypeId(1)],
        source_substitution: [(template, TyTerm::Param(caller))].into(),
        runtime_substitution: [(template, TypeId(1))].into(),
        source_result: Some(TyTerm::Param(template)),
        result_descriptor: Some(TypeId(1)),
        normalized_deferred_return: None,
    };
    let before = GenericEnv::capture(&signatures[0], &contracts)
        .unwrap()
        .signature_key();
    let accepted = ArgumentType {
        source: TyTerm::Param(caller),
        descriptor: TypeId(1),
    };
    contracts.bind_return_parameter(&accepted).unwrap();
    let wrong_identity = ArgumentType {
        source: TyTerm::Param(template),
        descriptor: TypeId(1),
    };
    assert_eq!(
        contracts
            .bind_return_parameter(&wrong_identity)
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(
        contracts.source_substitution.get(&template),
        Some(&TyTerm::Param(caller))
    );
    assert_eq!(
        contracts.runtime_substitution.get(&template),
        Some(&TypeId(1))
    );
    assert_eq!(contracts.result_descriptor, Some(TypeId(1)));
    assert_eq!(contracts.source_result, signatures[0].result_symbolic);
    assert_eq!(
        GenericEnv::capture(&signatures[0], &contracts)
            .unwrap()
            .signature_key(),
        before
    );
    contracts.bind_return_parameter(&accepted).unwrap();
    // A deferred result descriptor must not erase an already retained source
    // binding either. This is a second LRR packet-state corruption, outside
    // the original source; both owning substitution maps still match.
    contracts.result_descriptor = None;
    assert_eq!(
        contracts
            .bind_return_parameter(&wrong_identity)
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(contracts.result_descriptor, None);
    assert_eq!(
        contracts.source_substitution.get(&template),
        Some(&TyTerm::Param(caller))
    );
    assert_eq!(
        contracts.runtime_substitution.get(&template),
        Some(&TypeId(1))
    );
    contracts.bind_return_parameter(&accepted).unwrap();
    assert_eq!(contracts.result_descriptor, Some(TypeId(1)));
}
