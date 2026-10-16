//! Reached constructor selection through the existing source/hygiene namespace.
//! It performs only typed query demands; no guest allocation or expansion effect.
use super::{
    names::{NameDefinition, NameLookup, NameNamespaces},
    source_locals::OrdinaryNameSite,
    source_macros::EnteredMacroIndex,
    types::{BoundNominalConstructor, NominalConstructorBinding, SchemaNames},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::{
        body_index::BodyIndex,
        constructors::{ConstructorPlan, RecordPathSelection},
        imports::ImportSegment,
        outline::ScopeKey,
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecordConstructorDemand {
    Name(NameLookup),
    /// A local/generated/alias/variant definition requires its source type view.
    Definition(NameDefinition),
    TypeContext,
    OwnerContext(ScopeKey),
    Parameter(crate::traits::term::ParamKey),
    Fields(Box<NominalConstructorBinding>),
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecordConstructorBinding {
    Ready(Box<BoundNominalConstructor>),
    Deferred(Box<RecordConstructorDemand>),
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
fn deferred(demand: RecordConstructorDemand) -> QueryPoll<RecordConstructorBinding> {
    QueryPoll::Ready(Ok(RecordConstructorBinding::Deferred(Box::new(demand))))
}

/// Reached preparation from the memoized owning source environment. Local
/// declaration authority is supplied by the same indexed Type occurrence used
/// by other source type consumers, never by searching module outline rows.
pub(crate) fn bind_prepared_record_constructor(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    body: &BodyIndex,
    plan: &ConstructorPlan,
) -> QueryPoll<RecordConstructorBinding> {
    use super::source_locals::owned_plan::type_declarations::BodyTypeOutcome;
    if sources.len() > 100_000
        || plan.origin().body_key != *body.body()
        || plan.profile() != body.profile()
    {
        return QueryPoll::Ready(Err(fault("constructor plan/owning body differs")));
    }
    let outcome = match db.fetch(
        &SyntaxProvider,
        QueryKey::SourceLocalPlan(body.body().clone()),
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::SourceLocalPlan(locals) = outcome.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("constructor owning local plan outcome differs")));
    };
    if locals.context().source().index().body() != body.body() || locals.environment().is_none() {
        return QueryPoll::Ready(Err(fault("constructor owning environment differs")));
    }
    let selected = match locals.prepare_body_type(db, plan.node()) {
        QueryPoll::Ready(Ok(BodyTypeOutcome::Ready(selected))) => selected,
        QueryPoll::Ready(Ok(BodyTypeOutcome::Pending(_))) => {
            return deferred(RecordConstructorDemand::TypeContext);
        }
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let declaration = selected.declaration();
    if declaration.definition().item_kind != ItemKind::Struct {
        return deferred(RecordConstructorDemand::TypeContext);
    }
    let schema = match db.fetch(
        &SyntaxProvider,
        QueryKey::TypeSchema(declaration.definition_identity()),
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("constructor selected schema outcome differs")));
    };
    let names = match SchemaNames::from_body_type(db, graph, schema, &selected) {
        QueryPoll::Ready(Ok(names)) => names,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    match names.bind_record_constructor_current(
        db,
        sources,
        None,
        plan,
        locals.context().source().scope(),
    ) {
        QueryPoll::Ready(Ok(NominalConstructorBinding::Ready(bound))) => {
            QueryPoll::Ready(Ok(RecordConstructorBinding::Ready(Box::new(bound))))
        }
        QueryPoll::Ready(Ok(pending)) => {
            deferred(RecordConstructorDemand::Fields(Box::new(pending)))
        }
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}

/// The supplied entered index must retain the actual published source/hygiene
/// context. Pending producers/local declarations never fall back to module names.
pub fn bind_record_constructor(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    source: &PreparedSourceView,
    entered: &EnteredMacroIndex<'_>,
    body: &BodyIndex,
    plan: &ConstructorPlan,
) -> QueryPoll<RecordConstructorBinding> {
    match bind_record_inner(db, graph, sources, source, entered, body, plan) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}

fn bind_record_inner(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    source: &PreparedSourceView,
    entered: &EnteredMacroIndex<'_>,
    body: &BodyIndex,
    plan: &ConstructorPlan,
) -> Result<QueryPoll<RecordConstructorBinding>, Diagnostic> {
    if sources.len() > 100_000
        || entered.body != *body.body()
        || plan.origin().body_key != *body.body()
        || plan.profile() != body.profile()
        || !sources
            .iter()
            .any(|s| s.selection() == source.selection() && s.same_publication(source))
    {
        return Err(fault("constructor entered source/body catalogue mismatch"));
    }
    entered.parent.require_publication(source)?;
    for view in sources {
        graph.require_source(view.selection(), view.outline())?;
    }
    let caller = body.body().definition;
    if !matches!(db.input(&InputKey::DefinitionSource(caller)).as_deref(),
        Some(InputValue::DefinitionSource(key)) if key == &source.selection().outline.parse)
        || !matches!(db.input(&InputKey::DefinitionBody(caller)).as_deref(),
            Some(InputValue::DefinitionBody(handle)) if &handle.key() == body.body())
        || !matches!(db.input(&InputKey::CrateCfg(source.selection().outline.crate_key)).as_deref(),
            Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint() == source.selection().outline.cfg)
    {
        return Err(fault(
            "constructor caller source revision differs from database",
        ));
    }
    let context = db
        .input(&InputKey::DeclarationContext(caller))
        .ok_or_else(|| fault("constructor caller context absent"))?;
    let InputValue::DeclarationContext(context) = context.as_ref() else {
        return Err(fault("constructor caller context variant differs"));
    };
    if context.scope != entered.module_scope {
        return Err(fault("constructor caller declaration scope differs"));
    }
    if context.scope.owner.is_some() {
        return Ok(deferred(RecordConstructorDemand::OwnerContext(
            context.scope.clone(),
        )));
    }
    let RecordPathSelection::Named(path) = source.record_constructor_path(body, plan)? else {
        return Ok(deferred(RecordConstructorDemand::TypeContext));
    };
    let caller_item = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.fingerprint() == caller)
        .ok_or_else(|| fault("constructor caller declaration absent"))?;
    if context.range != caller_item.declaration_span
        || !matches!(db.input(&InputKey::DefinitionHeader(caller)).as_deref(),
            Some(InputValue::DefinitionHeader(header)) if header == &caller_item.header)
    {
        return Err(fault(
            "constructor caller header/context differs from source",
        ));
    }
    // A generic source parameter must not resolve to a same-spelled module item.
    // Only functions demand FunctionSignature; initializer generic syntax is
    // captured from its exact declaration without constant/body evaluation.
    let signature = match caller_item.def_key.item_kind {
        ItemKind::Function => {
            let outcome = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(caller)) {
                QueryPoll::Ready(Ok(outcome)) => outcome,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            };
            Some(outcome)
        }
        ItemKind::Constant | ItemKind::Static => None,
        _ => return Ok(deferred(RecordConstructorDemand::TypeContext)),
    };
    let initializer_parameters = if signature.is_none() {
        source.initializer_parameters(body.body())?
    } else {
        Vec::new()
    };
    let parameters = if let Some(outcome) = &signature {
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return Err(fault("constructor caller signature outcome differs"));
        };
        signature.generics.as_slice()
    } else {
        initializer_parameters.as_slice()
    };
    if !path.absolute()
        && let Some(ImportSegment::Name(first)) = path.segments().first()
    {
        let wanted = crate::expand::scopes::MacroName::parse(first, plan.profile().edition)?;
        for parameter in parameters {
            if parameter.kind == crate::syntax::signatures::GenericKind::Type
                && crate::expand::scopes::MacroName::parse(&parameter.name, plan.profile().edition)?
                    == wanted
            {
                return Ok(deferred(RecordConstructorDemand::Parameter(
                    crate::traits::term::ParamKey {
                        definition: caller,
                        ordinal: parameter.ordinal,
                    },
                )));
            }
        }
    }
    let locals = entered.local_index(source.selection(), body.body(), &Default::default(), &[])?;
    let site = OrdinaryNameSite {
        source: source.selection(),
        body: body.body(),
        at: u64::from(path.at()),
        name: path.name(),
        context: locals.context_at(if path.segments().len() == 1 {
            path.name_range()
        } else {
            path.first_name_range()
        })?,
        namespace: Namespace::Type,
    };
    let selected = if !path.absolute() && path.segments().len() == 1 {
        locals.lookup_ordinary(site, graph, None)?
    } else {
        locals.lookup_qualified_module(site, path.segments(), path.absolute(), graph)?
    };
    let NameLookup::Defined(definition) = selected else {
        return Ok(deferred(RecordConstructorDemand::Name(selected)));
    };
    let mut candidates = sources.iter().flat_map(|view| {
        view.outline()
            .items
            .iter()
            .filter(|item| item.def_key.fingerprint() == definition.identity)
            .map(|item| (*view, item))
    });
    let Some((selected_source, item)) = candidates.next() else {
        return Ok(deferred(RecordConstructorDemand::Definition(definition)));
    };
    if candidates.next().is_some() {
        return Err(fault(
            "constructor source catalogue contains duplicate definition owners",
        ));
    }
    if item.def_key.item_kind != ItemKind::Struct {
        return Ok(deferred(RecordConstructorDemand::Definition(definition)));
    }
    let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition.identity)) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
        return Err(fault("constructor selected schema outcome differs"));
    };
    let names = match SchemaNames::from_database(db, graph, selected_source, schema) {
        QueryPoll::Ready(Ok(names)) => names,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    Ok(
        match names.bind_record_constructor_current(db, sources, None, plan, &context.scope) {
            QueryPoll::Ready(Ok(NominalConstructorBinding::Ready(bound))) => {
                QueryPoll::Ready(Ok(RecordConstructorBinding::Ready(Box::new(bound))))
            }
            QueryPoll::Ready(Ok(pending)) => {
                deferred(RecordConstructorDemand::Fields(Box::new(pending)))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        frontend::{hygiene::HygieneFacts, source_macros::PublishedMacroIndex},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::{constructors::ConstructorKind, parse::ParseKey},
    };
    #[test]
    fn rustc_const_record_binding_keeps_source_frontier_without_function_signature() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-struct.rs"
        );
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(b"rustc-const-record-source-context");
        let path = SourcePath::new("const-struct.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("original const record source absent")
        };
        let caller = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "y")
            .unwrap();
        let key = caller.body.as_ref().unwrap().key();
        let body = view
            .index_body(view.selection(), &key, Default::default())
            .unwrap();
        let outcome = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ConstructorPlan(
                    body.pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
                        .unwrap(),
                ),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::ConstructorPlan(plan) = outcome.value.as_ref() else {
            panic!("original constructor plan absent")
        };
        let root = crate::expand::source_hygiene::source_root(unit, 2024).unwrap();
        let facts = HygieneFacts::new(root.contexts, root.rows)
            .unwrap()
            .with_name_contexts(root.names)
            .unwrap();
        let index = PublishedMacroIndex::from_view(view.share(), root.context, facts).unwrap();
        let entered = index.enter_body(view.selection(), &key).unwrap();
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let result =
            bind_record_constructor(&mut db, &graph, &[&view], &view, &entered, &body, plan)
                .into_ready()
                .unwrap()
                .unwrap();
        assert!(matches!(result, RecordConstructorBinding::Deferred(demand)
            if matches!(demand.as_ref(), RecordConstructorDemand::Name(NameLookup::NeedInput(_)))));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_)
                | QueryKey::Constant(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::Thunk(_)
                | QueryKey::ExpansionInput(_)
        )));
        // Actual std::cmp/Debug frontiers remain; neither function context nor
        // a bound nominal witness, guest guard or full stock run is invented.
    }
    #[test]
    fn rustc_import_free_const_record_binds_checked_nominal_witness_without_function_signature() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs"
        );
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(b"rustc-import-free-const-record-source-context");
        let path = SourcePath::new("issue-19244.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("original const record source absent")
        };
        let caller = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "STRUCT")
            .unwrap();
        let key = caller.body.as_ref().unwrap().key();
        let body = view
            .index_body(view.selection(), &key, Default::default())
            .unwrap();
        let outcome = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ConstructorPlan(
                    body.pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
                        .unwrap(),
                ),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::ConstructorPlan(plan) = outcome.value.as_ref() else {
            panic!("original constructor plan absent")
        };
        let root = crate::expand::source_hygiene::source_root(unit, 2024).unwrap();
        let facts = HygieneFacts::new(root.contexts, root.rows)
            .unwrap()
            .with_name_contexts(root.names)
            .unwrap();
        let index = PublishedMacroIndex::from_view(view.share(), root.context, facts).unwrap();
        let entered = index.enter_body(view.selection(), &key).unwrap();
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let result =
            bind_record_constructor(&mut db, &graph, &[&view], &view, &entered, &body, plan)
                .into_ready()
                .unwrap()
                .unwrap();
        let RecordConstructorBinding::Ready(bound) = result else {
            panic!("original import-free const must bind: {result:?}")
        };
        let expected = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "MyStruct")
            .unwrap();
        assert_eq!(bound.mapping().definition(), expected.def_key.fingerprint());
        assert_eq!(bound.mapping().constructor(), plan.fingerprint());
        assert_eq!(bound.mapping().written_to_declaration(), &[0]);
        assert_eq!(
            bound
                .variant()
                .fields()
                .iter()
                .map(|field| field.primitive())
                .collect::<Vec<_>>(),
            [Some(crate::ir::op::PrimitiveType::Usize)]
        );
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_)
                | QueryKey::Constant(_)
                | QueryKey::Thunk(_)
                | QueryKey::ExpansionInput(_)
        )));
        // Same unchanged upstream constructor: exact source path ranges are
        // parser data, and a foreign caller must fail privacy before Type work.
        let RecordPathSelection::Named(path) = view.record_constructor_path(&body, plan).unwrap()
        else {
            panic!("unchanged upstream record path missing")
        };
        assert_eq!(
            &ORIGINAL[path.name_range().start as usize..path.name_range().end as usize],
            path.name()
        );
        let schema = db
            .fetch(
                &SyntaxProvider,
                QueryKey::TypeSchema(expected.def_key.fingerprint()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            panic!()
        };
        let names = SchemaNames::from_current_database(&mut db, &graph, &[&view], schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let mut foreign_caller = caller.scope.clone();
        foreign_caller.crate_key = Digest([226; 32]);
        let before = db.trace.events.len();
        assert!(
            matches!(names.bind_record_constructor_current(&mut db, &[&view], None, plan, &foreign_caller),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-TYPE-SCHEMA")
        );
        assert_eq!(
            db.trace.events.len(),
            before,
            "privacy must precede current Type lookup"
        );
        // Actual nominal source/schema witness only. Constant evaluation,
        // nested constants, array lengths and the original main are unexecuted.
    }
    #[test]
    fn rustc_const_record_phases_transfer_original_nominal_result_once() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs"
        );
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(b"rustc-import-free-const-record-source-context");
        let path = SourcePath::new("issue-19244.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("original const record source absent")
        };
        let caller = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "STRUCT")
            .unwrap();
        let key = caller.body.as_ref().unwrap().key();
        let body = view
            .index_body(view.selection(), &key, Default::default())
            .unwrap();
        let outcome = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ConstructorPlan(
                    body.pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
                        .unwrap(),
                ),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::ConstructorPlan(plan) = outcome.value.as_ref() else {
            panic!("original constructor plan absent")
        };
        let root = crate::expand::source_hygiene::source_root(unit, 2024).unwrap();
        let facts = HygieneFacts::new(root.contexts, root.rows)
            .unwrap()
            .with_name_contexts(root.names)
            .unwrap();
        let index = PublishedMacroIndex::from_view(view.share(), root.context, facts).unwrap();
        let entered = index.enter_body(view.selection(), &key).unwrap();
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let result =
            bind_record_constructor(&mut db, &graph, &[&view], &view, &entered, &body, plan)
                .into_ready()
                .unwrap()
                .unwrap();
        let RecordConstructorBinding::Ready(bound) = result else {
            panic!("original import-free const must bind: {result:?}")
        };
        let expected = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "MyStruct")
            .unwrap();
        assert_eq!(bound.mapping().definition(), expected.def_key.fingerprint());
        assert_eq!(bound.mapping().constructor(), plan.fingerprint());
        assert_eq!(bound.mapping().written_to_declaration(), &[0]);
        assert_eq!(
            bound
                .variant()
                .fields()
                .iter()
                .map(|field| field.primitive())
                .collect::<Vec<_>>(),
            [Some(crate::ir::op::PrimitiveType::Usize)]
        );
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        use crate::{
            ir::pc::Phase,
            runtime::{cell::GuestStore, shape::PrimitiveIdentity},
            syntax::body_index::ScopeId,
            vm::{
                frame::{Frame, FrameStatus},
                step::{StepOutcome, step},
            },
        };
        let expected_field =
            PrimitiveIdentity::from_recipe(bound.variant().fields()[0].primitive().unwrap())
                .unwrap()
                .id();
        let entry = body.pc(plan.node(), Phase::Entry).unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
        // Explicit component setup only: the constant evaluator does not enter
        // original initializers yet. Do not hide an implicit scope entry in IR.
        frame.enter(ScopeId(0)).unwrap();
        frame
            .prepare_nominal_constructor(plan, *bound, &mut store)
            .unwrap();
        let mut returned = None;
        for _ in 0..5 {
            let outcome = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("actual original initializer thunk absent")
            };
            match step(&mut frame, &mut store, recipe).unwrap() {
                StepOutcome::Next(_) => {}
                StepOutcome::Return(ticket) => {
                    returned = Some((ticket, recipe.clone()));
                    break;
                }
                other => panic!("unexpected initializer phase: {other:?}"),
            }
        }
        let (ticket, return_recipe) =
            returned.expect("original constructor must return through its checked root exit");
        assert_eq!(frame.status(), FrameStatus::Returned);
        assert_eq!(frame.counters.operations_started, 5);
        assert_eq!(frame.counters.operations_completed, 5);
        let field = store.aggregate_field(ticket, 0).unwrap();
        let actual = store.read_scalar(field).unwrap();
        assert_eq!(actual.type_id, expected_field.0);
        assert_eq!(actual.payload, 42);
        let root_cell = frame.cell(body.nodes()[0].value.unwrap()).unwrap();
        assert_eq!(
            store.state(root_cell).unwrap(),
            crate::runtime::cell::CellState::Moved
        );
        frame.finish_return(&mut store).unwrap();
        assert!(frame.active_scopes().is_empty());
        assert_eq!(store.read_scalar(field).unwrap(), actual);
        assert!(step(&mut frame, &mut store, &return_recipe).is_err());
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::Constant(_) | QueryKey::ExpansionInput(_)
        )));
        // Actual original STRUCT expression only with explicit frame-scope
        // setup. No Constant query, public evaluator, nested constants, arrays,
        // source member projection, aggregate Drop or original main ran.
    }
    const LARGE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/large-records.rs"
    );
    #[test]
    fn rustc_large_record_binding_selects_original_source_before_guard_allocation() {
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(LARGE.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(b"rustc-large-record-source-binding");
        let path = SourcePath::new("main.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("stock source absent")
        };
        let caller = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "f")
            .unwrap();
        let body_key = caller.body.as_ref().unwrap().key();
        let body = view
            .index_body(view.selection(), &body_key, Default::default())
            .unwrap();
        let plan = body
            .nodes()
            .iter()
            .find_map(|node| {
                let pc = body.pc(node.id, crate::ir::pc::Phase::Entry).unwrap();
                node.constructor_guard?;
                let result = db
                    .fetch(&SyntaxProvider, QueryKey::ConstructorPlan(pc))
                    .into_ready()
                    .unwrap()
                    .unwrap();
                let QueryValue::ConstructorPlan(plan) = result.value.as_ref() else {
                    panic!("constructor plan absent")
                };
                Some(plan.as_ref().clone())
            })
            .unwrap();
        let ConstructorKind::Record { fields, .. } = plan.kind() else {
            panic!("stock record absent")
        };
        assert_eq!(fields.len(), 12);
        let root = crate::expand::source_hygiene::source_root(unit, 2024).unwrap();
        let facts = HygieneFacts::new(root.contexts, root.rows)
            .unwrap()
            .with_name_contexts(root.names)
            .unwrap();
        let index = PublishedMacroIndex::from_view(view.share(), root.context, facts).unwrap();
        let entered = index.enter_body(view.selection(), &body_key).unwrap();
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let result =
            bind_record_constructor(&mut db, &graph, &[&view], &view, &entered, &body, &plan)
                .into_ready()
                .unwrap()
                .unwrap();
        let RecordConstructorBinding::Ready(bound) = result else {
            panic!("stock Large must bind: {result:?}")
        };
        let expected = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "Large")
            .unwrap();
        assert_eq!(bound.mapping().definition(), expected.def_key.fingerprint());
        assert_eq!(bound.mapping().constructor(), plan.fingerprint());
        assert_eq!(
            bound.mapping().written_to_declaration(),
            (0..12).collect::<Vec<u32>>()
        );
        assert!(
            bound
                .variant()
                .fields()
                .iter()
                .all(|ty| ty.primitive() == Some(crate::ir::op::PrimitiveType::Isize))
        );
        let duplicate = bind_record_constructor(
            &mut db,
            &graph,
            &[&view, &view],
            &view,
            &entered,
            &body,
            &plan,
        )
        .into_ready()
        .unwrap()
        .unwrap_err();
        assert_eq!(
            duplicate.message,
            "constructor source catalogue contains duplicate definition owners"
        );
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::Constant(_)))
        );
        // Source selection and type binding only; f() execution, the annotated
        // let, aggregate cleanup and full run-pass acceptance remain pending.
    }
}
