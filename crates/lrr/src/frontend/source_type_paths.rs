//! Qualified declaration Type paths through ordinary source namespaces.
//! `$crate` uses its actual token context, shared with ordinary call lowering.
use super::{
    names::{NameLookup, NameNamespaces, NamePath},
    type_name_namespace::SourceTypeOwner,
};
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::manifest::Namespace,
};
use ra_ap_syntax::{AstNode, ast};

pub(super) fn lookup(
    db: &mut Database,
    graph: &NameNamespaces,
    declaration: SourceTypeOwner<'_>,
    path: &ast::Path,
) -> QueryPoll<Option<NameLookup>> {
    let Some((segments, absolute)) = super::member_index::path_segments(path.clone()) else {
        return QueryPoll::Ready(Ok(None));
    };
    if !matches!(segments.first(), Some(crate::syntax::imports::ImportSegment::Name(name)) if name == "$crate")
    {
        return QueryPoll::Ready(
            graph
                .lookup_source_path(
                    Namespace::Type,
                    &segments,
                    absolute,
                    declaration.scope,
                    declaration.header.profile.edition,
                )
                .map(Some),
        );
    }
    let Some(source) = db.input(&InputKey::DefinitionSource(declaration.definition)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let Some(context) = db.input(&InputKey::DeclarationContext(declaration.definition)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let (InputValue::DefinitionSource(parse), InputValue::DeclarationContext(context)) =
        (source.as_ref(), context.as_ref())
    else {
        return QueryPoll::Ready(Err(fault("Type path declaration inputs differ")));
    };
    if context.scope != *declaration.scope {
        return QueryPoll::Ready(Err(fault("Type path owning scope differs")));
    }
    let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
        return QueryPoll::Ready(Err(fault("Type path parse differs")));
    };
    let mut components = Vec::new();
    let mut cursor = Some(path.clone());
    while let Some(path) = cursor {
        let Some(name) = path.segment().and_then(|segment| segment.name_ref()) else {
            return QueryPoll::Ready(Ok(None));
        };
        components.push(name);
        cursor = path.qualifier();
    }
    components.reverse();
    let Some(root) = components.first() else {
        return QueryPoll::Ready(Ok(None));
    };
    let root = match parsed.declaration_type_name(
        declaration.definition,
        declaration.header,
        context,
        root,
    ) {
        Ok(root) => root,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if let Err(error) = root.require(
        parse,
        declaration.definition,
        declaration.header,
        context,
        "$crate",
    ) {
        return QueryPoll::Ready(Err(error));
    }
    let (unit, _) = match parsed.dollar_crate_root(root.range()) {
        Ok(Some(root)) => root,
        Ok(None) => return QueryPoll::Ready(Ok(None)),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let tail = components
        .iter()
        .skip(1)
        .map(|name| {
            let range = crate::syntax::ast_adapter::range(name.syntax().text_range());
            crate::expand::scopes::MacroName::parse(name.text(), parsed.token_edition(range)?)
        })
        .collect::<Result<Vec<_>, Diagnostic>>();
    let route = match tail.and_then(|tail| NamePath::new(unit, tail)) {
        Ok(route) => route,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    QueryPoll::Ready(
        graph
            .lookup(Namespace::Type, &route, declaration.scope)
            .map(Some),
    )
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
