//! Configured facade language items resolved to actual ordinary declarations.
//! The compiler facet supplies a crate, not a guessed trait identity.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{BucketKey, CurrentModuleNameKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::names::NameLookup,
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::outline::ScopeKey,
};

pub(crate) fn sized(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    profile: &ProfileIdentity,
    core: Option<Digest>,
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
            name: "Sized".into(),
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
    // Verify the selected actual declaration; a reexport or same-named user
    // trait cannot mint this language item. No executable body is demanded.
    let actual = source.outline().declaration_rows().find(|row| {
        row.scope == scope
            && row.def_key.item_kind == ItemKind::Trait
            && row.def_key.fingerprint() == selected.identity
            && selected.defining_unit == core
            && selected.definition_edition == profile.edition
    });
    QueryPoll::Ready(Ok(actual.map(|_| selected.identity)))
}
