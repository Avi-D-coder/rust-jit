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
    },
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
            Ok(PatternSelection::Constructor { identity, variant })
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
    let item_kind = sources
        .iter()
        .find_map(|source| source.outline_item(definition.identity))
        .map(|item| item.def_key.item_kind);
    QueryPoll::Ready(
        pattern_declaration(graph, &name, definition.identity, item_kind)
            .map(|selection| (name, selection)),
    )
}
