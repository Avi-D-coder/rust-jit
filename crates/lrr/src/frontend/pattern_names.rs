//! The module-graph meaning of a written pattern name, shared by the static
//! source-constraint service and the reached pattern dispatcher.
//!
//! rustc resolves an identifier pattern to a constant, unit struct or unit
//! variant when one is in scope and otherwise introduces a fresh binding; a
//! path, tuple-struct or record pattern names a constructor (or a constant).
//! A record pattern's name lives in the Type namespace, every other form in
//! the Value namespace, both looked up from the body's lexical module context
//! through `CallResolver`. The `for` desugaring's lang `Option::Some` is
//! looked up through `core`, else `std` (a 2015 crate names only `::std`).
//! Block-local (lexical) names are selected by their entered Value goal, not
//! here.
use super::{
    callables::CallResolver,
    names::{EnumVariantDefinition, NameLookup, NameNamespaces},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::{memo::Database, poll::QueryPoll},
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::pattern_tree::{NameForm, OPTION_SOME_LANG_PATH, PatternName},
};

/// What a selected pattern name means.
#[derive(Debug, Clone)]
pub(crate) enum PatternSelection {
    /// A fresh binding: no Value declaration answers the identifier, or it
    /// names a function item (which a binding shadows).
    Binding,
    /// A constant item, matched by value.
    Constant(Digest),
    /// A struct or enum-variant constructor.
    Constructor {
        identity: Digest,
        variant: Option<EnumVariantDefinition>,
        /// Exact source application when the written name is an applied alias.
        application: Option<super::source_types::SourceTypeApplication>,
    },
}

/// The same source selection for an indexed lexical pattern and a module
/// pattern. No value/initializer is evaluated, including during capture typing.
fn select_source_pattern_name_inner(
    resolver: &CallResolver<'_>,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    db: &mut Database,
    at: &crate::ir::pc::PcKey,
    name: &PatternName,
) -> QueryPoll<(PatternName, PatternSelection)> {
    use crate::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        frontend::source_value_goals::CanonicalValueKind,
    };
    let Some(node) = name.lexical.or(name.source) else {
        return select_module_pattern_name(resolver, graph, sources, db, at.body.definition, name, Some(at));
    };
    let key = crate::ir::pc::PcKey {
        body: at.body.clone(),
        node: node.0,
        phase: 0,
    };
    let fetched = match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::EnteredValue(goal) = fetched.value.as_ref() else {
        return QueryPoll::Ready(Err(unsupported(
            "pattern Value query variant differs".into(),
        )));
    };
    if goal.is_qualified_path() && goal.rooted().is_none() {
        return select_module_pattern_name(resolver, graph, sources, db, at.body.definition, name, Some(at));
    }
    if let Some(demand) = goal.pending() {
        return QueryPoll::Ready(Err(unsupported(format!(
            "pattern source selection remains pending: {demand:?}"
        ))));
    }
    let Some(declaration) = goal.declaration() else {
        if goal.local_binding().is_some()
            || (goal.facade().is_some() && name.form == NameForm::Identifier)
        {
            return QueryPoll::Ready(Ok((name.clone(), PatternSelection::Binding)));
        }
        return QueryPoll::Ready(Err(unsupported(
            "pattern selects no source declaration".into(),
        )));
    };
    let kind = match goal.declaration_kind(db) {
        QueryPoll::Ready(Ok(CanonicalValueKind::Ready(kind))) => kind,
        QueryPoll::Ready(Ok(_)) => {
            return QueryPoll::Ready(Err(unsupported(
                "pattern declaration kind remains pending".into(),
            )));
        }
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    if let Some(member) = goal.rooted() {
        if kind != ItemKind::Enum {
            return QueryPoll::Ready(Err(unsupported(
                "rooted pattern member requires enum/associated-item selection".into(),
            )));
        }
        let schema = match db.fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(declaration.identity()),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(unsupported(
                "pattern enum schema variant differs".into(),
            )));
        };
        if let Err(error) = schema.variant(Some(member)) {
            return QueryPoll::Ready(Err(error));
        }
        return QueryPoll::Ready(Ok((
            name.clone(),
            PatternSelection::Constructor {
                identity: declaration.identity(),
                variant: Some(EnumVariantDefinition {
                    enumeration: declaration.identity(),
                    name: member.to_owned(),
                }),
                application: None,
            },
        )));
    }
    if let Some(variant) = declaration.variant() {
        return QueryPoll::Ready(Ok((
            name.clone(),
            PatternSelection::Constructor {
                identity: declaration.identity(),
                variant: Some(variant.clone()),
                application: None,
            },
        )));
    }
    finish_pattern_declaration(db, graph, sources, name, declaration.identity(), Some(kind), Some(at))
        .map(|selected| (name.clone(), selected))
}

fn unsupported(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}

/// Look up `name` from `caller`'s lexical module context; the returned name
/// is the spelling actually selected (the lang `Option::Some` through `core`
/// or `std`).
pub(crate) fn lookup_pattern_name(
    resolver: &CallResolver<'_>,
    db: &mut Database,
    caller: Digest,
    name: &PatternName,
) -> QueryPoll<(PatternName, NameLookup)> {
    if name.spelling == OPTION_SOME_LANG_PATH {
        for spelling in [OPTION_SOME_LANG_PATH, "::std::option::Option::Some"] {
            match resolver.lookup_written(db, caller, spelling, Namespace::Value) {
                QueryPoll::Ready(Ok(found @ NameLookup::Defined(_))) => {
                    let selected = PatternName {
                        spelling: spelling.to_owned(),
                        form: NameForm::Tuple,
                        lexical: None,
                        source: None,
                    };
                    return QueryPoll::Ready(Ok((selected, found)));
                }
                QueryPoll::Ready(Ok(_)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-NAME-MISSING",
            "`for` requires the library's `Option::Some` (core::option)".into(),
        )));
    }
    let namespace = if matches!(name.form, NameForm::Record(_)) {
        Namespace::Type
    } else {
        Namespace::Value
    };
    match resolver.lookup_written(db, caller, &name.spelling, namespace) {
        QueryPoll::Ready(Ok(found)) => QueryPoll::Ready(Ok((name.clone(), found))),
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}

/// The pattern meaning of a selected declaration of kind `item_kind`.
pub(crate) fn pattern_declaration(
    graph: &NameNamespaces,
    name: &PatternName,
    identity: Digest,
    item_kind: Option<ItemKind>,
) -> Result<PatternSelection, Diagnostic> {
    let variant = graph.enum_variant(identity).cloned();
    match (item_kind, &variant) {
        // A function item in the Value namespace is shadowed by a new
        // binding; constants, statics and constructors are not.
        (Some(ItemKind::Function), _) if name.form == NameForm::Identifier => {
            Ok(PatternSelection::Binding)
        }
        (Some(ItemKind::Constant), _) if !matches!(name.form, NameForm::Record(_)) => {
            Ok(PatternSelection::Constant(identity))
        }
        (Some(ItemKind::Struct), _) | (_, Some(_)) => {
            Ok(PatternSelection::Constructor { identity, variant, application: None })
        }
        _ => Err(unsupported(format!(
            "pattern name `{}` names a Value item that is not a constant or constructor",
            name.spelling
        ))),
    }
}

/// Select a module-graph (non-lexical) pattern name: the name lookup, then
/// the selected declaration's pattern meaning.
pub(crate) fn select_pattern_name(
    resolver: &CallResolver<'_>,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    db: &mut Database,
    caller: Digest,
    name: &PatternName,
) -> QueryPoll<(PatternName, PatternSelection)> {
    select_module_pattern_name(resolver, graph, sources, db, caller, name, None)
}
fn select_module_pattern_name(
    resolver: &CallResolver<'_>, graph: &NameNamespaces, sources: &[&PreparedSourceView],
    db: &mut Database, caller: Digest, name: &PatternName, at: Option<&crate::ir::pc::PcKey>,
) -> QueryPoll<(PatternName, PatternSelection)> {
    let (name, found) = match lookup_pattern_name(resolver, db, caller, name) {
        QueryPoll::Ready(Ok(found)) => found,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let spelling = &name.spelling;
    let definition = match found {
        // An identifier no Value declaration (prelude included) answers is
        // a fresh binding, as under `#![no_implicit_prelude]`.
        NameLookup::Absent | NameLookup::LocalBinding(_) if name.form == NameForm::Identifier => {
            return QueryPoll::Ready(Ok((name, PatternSelection::Binding)));
        }
        NameLookup::Defined(definition) => definition,
        NameLookup::NeedInput(_) => {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                format!("pattern name `{spelling}` awaits unresolved namespace producers"),
            )));
        }
        _ => {
            // `Type::NAME` names an associated constant (inherent, trait impl
            // or trait default) of a nominal or primitive type, as value paths do.
            if name.form == NameForm::Path && spelling.contains("::") {
                match resolver.associated_value(db, caller, spelling) {
                    QueryPoll::Ready(Ok(Some(constant))) => {
                        return QueryPoll::Ready(Ok((name, PatternSelection::Constant(constant))));
                    }
                    QueryPoll::Ready(Ok(None)) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            return QueryPoll::Ready(Err(unsupported(format!(
                "pattern name `{spelling}` has no unique accessible declaration"
            ))));
        }
    };
    let item_kind =
        super::callables::selected_item_kind(db, sources.iter().copied(), definition.identity);
    finish_pattern_declaration(db, graph, sources, &name, definition.identity, item_kind, at)
        .map(|selection| (name, selection))
}

/// One retained ordinary pattern-name answer. No executable/body/parser Arc
/// is duplicated; the query's existing dependency closure owns currentness.
#[derive(Debug)]
pub struct PreparedSourcePatternName {
    key: crate::ir::pc::PcKey,
    name: PatternName,
    selection: PatternSelection,
}
impl PreparedSourcePatternName {
    pub(crate) fn key(&self) -> &crate::ir::pc::PcKey { &self.key }
    pub(crate) fn name(&self) -> &PatternName { &self.name }
    pub(crate) fn selection(&self) -> &PatternSelection { &self.selection }
}
impl crate::core::digest::StableEncode for PreparedSourcePatternName {
    const DOMAIN: &'static str = "lrr/v2/source-pattern-name";
    fn encode_fields(&self,e: &mut crate::core::digest::StableEncoder) {
        e.record(&self.key); self.name.encode(e);
        match &self.selection {
            PatternSelection::Binding => e.u8(0),
            PatternSelection::Constant(id) => {e.u8(1);e.digest(*id);},
            PatternSelection::Constructor {identity,variant,application} => {
                e.u8(2);e.digest(*identity);
                e.option(variant,|e,v|{e.digest(v.enumeration);e.string(&v.name);});
                e.option(application,|e,ty|e.record(ty));
            }
        }
    }
}
macro_rules! ready_name {
    ($value:expr) => {match $value {QueryPoll::Ready(Ok(value))=>value,QueryPoll::Ready(Err(error))=>return QueryPoll::Ready(Err(error)),QueryPoll::NeedInput(input)=>return QueryPoll::NeedInput(input)}};
}
/// Canonical actual-node preparation. Query callbacks never run guest code or
/// expand a producer. Source metadata and selected Type arguments are ordinary.
pub(crate) fn prepare_source_name(db: &mut Database,key: &crate::ir::pc::PcKey)
    -> QueryPoll<PreparedSourcePatternName> {
    use crate::db::{key::QueryKey,provider::SyntaxProvider,value::QueryValue};
    if key.phase!=0 {return QueryPoll::Ready(Err(unsupported("source pattern name requires entry key".into())));}
    let owner=ready_name!(db.fetch(&SyntaxProvider,QueryKey::SourceLocalPlan(key.body.clone())));
    let QueryValue::SourceLocalPlan(owner)=owner.value.as_ref() else {return QueryPoll::Ready(Err(unsupported("source pattern owner differs".into())));};
    ready_name!(owner.require_current(db));
    let facts=ready_name!(db.fetch(&SyntaxProvider,QueryKey::SourceConstraints(key.body.clone())));
    let QueryValue::SourceConstraints(facts)=facts.value.as_ref() else {return QueryPoll::Ready(Err(unsupported("source pattern index differs".into())));};
    let Some(name)=facts.pattern_name(crate::ir::pc::NodeId(key.node)) else {return QueryPoll::Ready(Err(unsupported("actual source pattern name absent".into())));};
    if facts.body()!=&key.body || name.source!=Some(crate::ir::pc::NodeId(key.node)) {
        return QueryPoll::Ready(Err(unsupported("source pattern indexed owner differs".into())));
    }
    let Some(environment)=owner.environment() else {return QueryPoll::Ready(Err(unsupported("source pattern environment absent".into())));};
    let completed=environment.macros().source_environment();let sources=completed.sources().iter().collect::<Vec<_>>();
    let resolver=match CallResolver::new(completed.names(),&sources) {Ok(resolver)=>resolver,Err(error)=>return QueryPoll::Ready(Err(error))};
    let (name,selection)=ready_name!(select_source_pattern_name_inner(&resolver,completed.names(),&sources,db,key,name));
    // The retained name answer also depends on the actual constructor schema;
    // installed target reuse must not outlive a field/variant declaration edit.
    if let PatternSelection::Constructor {identity,variant,..}=&selection {
        let owner=variant.as_ref().map_or(*identity,|v|v.enumeration);
        ready_name!(db.fetch(&SyntaxProvider,QueryKey::TypeSchema(owner)));
    }
    QueryPoll::Ready(Ok(PreparedSourcePatternName {key:key.clone(),name,selection}))
}
/// One service for the matcher, capture typing and source constraints.
pub(crate) fn select_source_pattern_name(
    resolver: &CallResolver<'_>, graph: &NameNamespaces, sources: &[&PreparedSourceView],
    db: &mut Database, at: &crate::ir::pc::PcKey, name: &PatternName,
) -> QueryPoll<(PatternName,PatternSelection)> {
    use crate::db::{key::QueryKey,provider::SyntaxProvider,value::QueryValue};
    let Some(node)=name.source else {return select_source_pattern_name_inner(resolver,graph,sources,db,at,name);};
    let key=crate::ir::pc::PcKey {body:at.body.clone(),node:node.0,phase:0};
    let selected=ready_name!(db.fetch(&SyntaxProvider,QueryKey::SourcePatternName(key.clone())));
    let QueryValue::SourcePatternName(selected)=selected.value.as_ref() else {return QueryPoll::Ready(Err(unsupported("source pattern name result differs".into())));};
    if selected.key()!=&key || selected.name()!=name {return QueryPoll::Ready(Err(unsupported("source pattern canonical name differs".into())));}
    QueryPoll::Ready(Ok((selected.name.clone(),selected.selection.clone())))
}
fn finish_pattern_declaration(
    db: &mut Database,graph: &NameNamespaces,sources: &[&PreparedSourceView],name: &PatternName,
    identity: Digest,kind: Option<ItemKind>,at: Option<&crate::ir::pc::PcKey>,
) -> QueryPoll<PatternSelection> {
    use crate::{db::{key::QueryKey,provider::SyntaxProvider,value::QueryValue},
        traits::term::TyTerm};
    if kind!=Some(ItemKind::TypeAlias) {return QueryPoll::Ready(pattern_declaration(graph,name,identity,kind));}
    let (Some(at),Some(node))=(at,name.source) else {return QueryPoll::Ready(Err(unsupported("pattern alias requires its actual source occurrence".into())));};
    let site=crate::ir::pc::PcKey {body:at.body.clone(),node:node.0,phase:0};
    let owner=ready_name!(db.fetch(&SyntaxProvider,QueryKey::SourceLocalPlan(at.body.clone())));
    let QueryValue::SourceLocalPlan(owner)=owner.value.as_ref() else {return QueryPoll::Ready(Err(unsupported("pattern alias owner differs".into())));};
    let selected=ready_name!(super::explicit_type_arguments::select_path_arguments(db,owner,&site,
        super::explicit_type_arguments::PathArgumentPosition::Constructor));
    let mut arguments=Vec::new();
    if let Some(selected)=selected {
        for guard in &selected.guards {if let Err(error)=guard.require_current(db) {return QueryPoll::Ready(Err(error));}}
        arguments.extend(selected.arguments.iter().map(|argument|argument.as_ref().map(|argument|argument.source().clone())));
    }
    let Some(application)=ready_name!(super::source_types::prepare_alias_application(db,graph,sources,identity,&arguments))else{
        return QueryPoll::Ready(Err(unsupported("pattern alias requires its source target application".into())));
    };
    let TyTerm::Nominal(target,_)=&application.term else {return QueryPoll::Ready(Err(unsupported("pattern alias target is not nominal".into())));};
    let definition=match super::source_types::nominal_definition(db,sources,*target) {Ok(definition)=>definition,Err(error)=>return QueryPoll::Ready(Err(error))};
    if definition.item_kind!=ItemKind::Struct {return QueryPoll::Ready(Err(unsupported("pattern alias target requires variant selection".into())));}
    QueryPoll::Ready(Ok(PatternSelection::Constructor {identity:*target,variant:None,application:Some(application)}))
}

/// Shared source field-name -> declaration-ordinal selection.
pub(crate) fn field_ordinals(
    db: &mut Database,
    definition: Digest,
    variant_name: Option<&str>,
    form: &NameForm,
    shape: super::types::FieldShape,
) -> QueryPoll<Box<[u32]>> {
    let ordinals: Box<[u32]> = match form {
        NameForm::Record(written) => {
            let schema = match db.fetch(
                &crate::db::provider::SyntaxProvider,
                crate::db::key::QueryKey::TypeSchema(definition),
            ) {
                QueryPoll::Ready(Ok(schema)) => schema,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let crate::db::value::QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
                return QueryPoll::Ready(Err(unsupported("record pattern schema differs".into())));
            };
            let recipe = match schema.variant(variant_name) {
                Ok(recipe) => recipe,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let mut ordinals = Vec::with_capacity(written.len());
            for field in written.iter() {
                // Named fields by name; tuple fields by their written index.
                let position = recipe
                    .fields
                    .iter()
                    .position(|declared| declared.name.as_deref() == Some(field.as_str()));
                let position = position.or_else(|| {
                    (shape == super::types::FieldShape::Tuple)
                        .then(|| field.parse::<usize>().ok())
                        .flatten()
                        .filter(|index| *index < recipe.fields.len())
                });
                let Some(position) = position else {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-PATTERN-TYPE",
                        format!("record pattern field `{field}` is not declared"),
                    )));
                };
                ordinals.push(u32::try_from(position).unwrap_or(u32::MAX));
            }
            ordinals.into()
        }
        _ => Box::new([]),
    };
    QueryPoll::Ready(Ok(ordinals))
}
