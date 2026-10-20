//! Configured facade language items resolved to actual ordinary declarations.
//! The compiler facet supplies a crate, not a guessed trait identity.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest, profile::ProfileIdentity},
    db::{
        key::{BucketKey, CurrentModuleNameKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::names::NameLookup,
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::outline::ScopeKey,
};

pub(crate) fn sized(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    profile: &ProfileIdentity,
    core: Option<Digest>,
) -> QueryPoll<Option<Digest>> {
    marker_trait(db, sources, profile, core, "Sized")
}

/// Select an actual configured core::marker declaration through the same
/// source namespace service. This proves source ownership, not a lang role;
/// callers needing a compiler lang item must validate SourceLangIdentity.
pub(crate) fn marker_trait(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    profile: &ProfileIdentity,
    core: Option<Digest>,
    name: &str,
) -> QueryPoll<Option<Digest>> {
    let Some(core) = core else {
        return QueryPoll::Ready(Ok(None));
    };
    let scope = ScopeKey {
        crate_key: core,
        module_path: vec!["marker".into()],
        owner: None,
    };
    let mut owners = sources.iter().copied().filter(|source| {
        let selection = &source.selection().outline;
        selection.crate_key == core
            && selection.module_path == scope.module_path
            && selection.parse.profile == *profile
    });
    let Some(source) = owners.next() else {
        return QueryPoll::Ready(Ok(None));
    };
    if owners.next().is_some() {
        return QueryPoll::Ready(Ok(None));
    }
    let key = CurrentModuleNameKey {
        source: source.selection().outline.clone(),
        path: source.selection().path.as_str().to_owned(),
        bucket: BucketKey {
            scope: scope.clone(),
            namespace: Namespace::Type,
            name: name.into(),
        },
    };
    let outcome = match db.fetch(
        &SyntaxProvider,
        QueryKey::CurrentModuleName(Box::new(key.clone())),
    ) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::CurrentModuleName(observation) = outcome.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-LANGUAGE-ITEM",
            "language-item namespace query differs".into(),
        )));
    };
    if !observation.matches_request(&key) {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-LANGUAGE-ITEM",
            "language-item namespace request differs".into(),
        )));
    }
    let Some(NameLookup::Defined(selected)) = observation.lookup() else {
        return QueryPoll::Ready(Ok(None));
    };
    if selected.defining_unit != core || selected.definition_edition != profile.edition {
        return QueryPoll::Ready(Ok(None));
    }
    let identity = selected.identity;
    let actual = match db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(identity)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::TraitDeclaration(declaration) = actual.value.as_ref() else {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-LANGUAGE-ITEM",
            "language-item trait declaration query differs".into(),
        )));
    };
    QueryPoll::Ready(Ok(declaration
        .matches_source(&source.selection().outline.parse, &scope)
        .then_some(identity)))
}
