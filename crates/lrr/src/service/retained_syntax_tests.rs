//! Source/seal admission controls around unchanged upstream syntax. These do
//! not execute a guest or count an upstream runtime contract as passed.
use crate::{
    core::digest::Digest,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
        retained_syntax::RetainedSyntaxInput,
        value::QueryValue,
    },
    expand::{
        source_declarative::{
            self,
            assembly::{PreparedSourceSyntax, SourceSyntaxAssembly, tests as assembly},
        },
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    },
    source::{changes::PreparedSourceInputs, manifest::Namespace},
    syntax::body_index::BodyIndexLimits,
};
use std::{collections::BTreeMap, sync::Arc};

const STOCK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);

pub(super) struct Fixture {
    inputs: PreparedSourceInputs,
    pub(super) prepared: Arc<PreparedSourceSyntax>,
    pub(super) discovery: SourceDiscovery,
    pub(super) db: Database,
}
pub(super) fn fixture(edition: u16, complete: bool) -> Fixture {
    let (provider, index, inputs) = assembly::world(STOCK, edition);
    let outer = assembly::request(&provider, &index);
    let generated = assembly::output(&provider, index.scan(), &outer, 1);
    let mut syntax = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&outer, generated.clone())
        .unwrap();
    if complete {
        let nested = GeneratedDeclarationMacroIndex::new(
            generated.clone(),
            DeclarationMacroParent::Source(index),
        )
        .unwrap();
        let invocation = generated
            .scan()
            .producers()
            .iter()
            .find(|producer| {
                producer.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            })
            .unwrap()
            .identity;
        let source_declarative::SourceDeclarativeDemand::Ready(inner) = provider
            .generated_declaration_demand(&nested, invocation, None)
            .unwrap()
        else {
            panic!("actual stock nested request absent")
        };
        let child = assembly::output(&provider, generated.scan(), &inner, 1);
        syntax = syntax.attach(&inner, child).unwrap();
    }
    let source = provider.module_source();
    let discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest([242; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([17; 32]),
        source_declarative::engine_version(),
        Arc::new(
            CompileInputCatalog::standalone(&source.selection().outline.parse.profile).unwrap(),
        ),
    )
    .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    Fixture {
        inputs,
        prepared: Arc::new(syntax.finish().unwrap()),
        discovery,
        db,
    }
}
pub(super) fn seal(f: &mut Fixture) {
    // Only actual producer replies can close discovery. Never interpret an
    // unsupported phase or an error as a completed namespace.
    for _ in 0..16 {
        match f.discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                f.discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                f.discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                f.discovery.publish(&mut f.db).unwrap();
                return;
            }
            _ => panic!("stock declaration discovery needs an unhandled actual phase"),
        }
    }
    panic!("stock declaration discovery did not close")
}
pub(super) fn admit(f: &mut Fixture) -> Arc<RetainedSyntaxInput> {
    match RetainedSyntaxInput::prepare(&mut f.db, f.prepared.clone(), f.discovery.key().clone()) {
        QueryPoll::Ready(Ok(input)) => input,
        QueryPoll::Ready(Err(error)) => panic!("retained admission failed: {error:?}"),
        QueryPoll::NeedInput(input) => {
            panic!("retained admission unexpectedly suspended: {input:?}")
        }
    }
}
#[test]
fn missing_actual_discovery_requests_input_without_publishing_syntax() {
    let mut f = fixture(2024, true);
    let before = f.db.revision();
    assert!(matches!(
        RetainedSyntaxInput::prepare(&mut f.db, f.prepared.clone(), f.discovery.key().clone()),
        QueryPoll::NeedInput(InputRequest::Discover { key }) if key == *f.discovery.key()
    ));
    assert_eq!(f.db.revision(), before);
    assert!(
        f.db.published_input(&InputKey::DiscoverySeal(f.discovery.key().clone()))
            .is_none()
    );
}
#[test]
fn admitted_stock_syntax_uses_pure_parse_and_ordinary_body_index_in_every_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition, true);
        seal(&mut f);
        let input = admit(&mut f);
        let source_revision = f.db.source_revision();
        let original =
            f.db.input(&InputKey::SourceBlob(
                f.prepared.source().outline.parse.content,
            ))
            .unwrap();
        assert!(matches!(
            f.db.publish_retained_syntax(input.clone()),
            QueryPoll::Ready(Ok(()))
        ));
        let revision = f.db.revision();
        assert!(matches!(
            f.db.publish_retained_syntax(input.clone()),
            QueryPoll::Ready(Ok(()))
        ));
        assert_eq!(f.db.revision(), revision);
        assert_eq!(f.db.source_revision(), source_revision);
        assert!(
            f.db.published_input(&InputKey::SourceBlob(input.key().content))
                .is_none()
        );
        assert_eq!(
            f.db.input(&InputKey::SourceBlob(
                f.prepared.source().outline.parse.content
            ))
            .unwrap()
            .fingerprint(),
            original.fingerprint()
        );
        let QueryPoll::Ready(Ok(result)) =
            f.db.fetch(&SyntaxProvider, QueryKey::ParseFile(input.key().clone()))
        else {
            panic!("admitted source parse failed")
        };
        let QueryValue::ParseFile(parsed) = result.value.as_ref() else {
            panic!("parse result variant differs")
        };
        assert_eq!(parsed.key(), *input.key());
        let outline = parsed
            .outline(
                f.prepared.source().outline.crate_key,
                vec![],
                &crate::source::snapshot::CfgSet::guest_default(),
            )
            .unwrap();
        let main = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let handle = main.body.as_ref().unwrap();
        let definition = InputKey::DefinitionSource(handle.key().definition);
        let Some((InputValue::DefinitionSource(selected), _)) = f.db.published_input(&definition)
        else {
            panic!("original definition source absent")
        };
        assert_eq!(selected, &f.prepared.source().outline.parse);
        let indexed = parsed
            .index_body(handle, BodyIndexLimits::default())
            .unwrap();
        assert!(!indexed.nodes().iter().any(|node| matches!(
            node.role,
            crate::syntax::body_index::NodeRole::Expression(
                crate::syntax::body_index::ExpressionKind::Macro
            )
        )));
    }
}
#[test]
fn incomplete_expansion_is_not_admissible_even_with_closed_discovery() {
    let mut f = fixture(2024, false);
    seal(&mut f);
    assert!(matches!(
        RetainedSyntaxInput::prepare(&mut f.db, f.prepared.clone(), f.discovery.key().clone()),
        QueryPoll::Ready(Err(_))
    ));
}
#[test]
fn cached_retained_parse_cannot_survive_epoch_replacement() {
    let mut f = fixture(2024, true);
    seal(&mut f);
    let input = admit(&mut f);
    assert!(matches!(
        f.db.publish_retained_syntax(input.clone()),
        QueryPoll::Ready(Ok(()))
    ));
    let key = QueryKey::ParseFile(input.key().clone());
    assert!(matches!(
        f.db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    f.db.begin_expansion_epoch(2).unwrap();
    assert!(matches!(
        f.db.fetch(&SyntaxProvider, key),
        QueryPoll::Ready(Err(_))
    ));
    assert!(matches!(
        f.db.publish_retained_syntax(input),
        QueryPoll::Ready(Err(_))
    ));
}
#[test]
fn source_universe_replacement_removes_retained_input_and_old_seal() {
    let mut f = fixture(2024, true);
    seal(&mut f);
    let input = admit(&mut f);
    assert!(matches!(
        f.db.publish_retained_syntax(input.clone()),
        QueryPoll::Ready(Ok(()))
    ));
    f.inputs.publish(&mut f.db).unwrap();
    assert!(
        f.db.published_input(&InputKey::RetainedSyntax(input.key().content))
            .is_none()
    );
    assert!(
        f.db.published_input(&InputKey::DiscoverySeal(f.discovery.key().clone()))
            .is_none()
    );
    assert!(matches!(
        f.db.fetch(&SyntaxProvider, QueryKey::ParseFile(input.key().clone())),
        QueryPoll::Ready(Err(_))
    ));
}
#[test]
fn generic_input_publication_cannot_admit_retained_syntax() {
    let mut f = fixture(2024, true);
    seal(&mut f);
    let input = admit(&mut f);
    let revision = f.db.revision();
    assert!(
        f.db.publish_inputs(BTreeMap::from([(
            InputKey::RetainedSyntax(input.key().content),
            InputValue::RetainedSyntax(input)
        )]))
        .is_err()
    );
    assert_eq!(f.db.revision(), revision);
}
