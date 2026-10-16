//! Exact namespace projection from a genuine parent discovery publication.
//! Contained Type facets preserve that parent's identity and source closure.
use super::{
    dependency::{InputKey, InputValue},
    key::{QueryKey, SealedNameKey},
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    seals::SealDomain,
    value::QueryValue,
};

pub(super) fn execute(db: &mut Database, key: &SealedNameKey) -> QueryPoll<Provided> {
    // Domain-to-bucket equality is checked on the actual primary/contained
    // certificate below. A Scope parent alone cannot certify an arbitrary bucket.
    if !matches!(key.seal.domain, SealDomain::Scope { .. })
        || key.bucket.name.len() > 64 * 1024
        || key.bucket.scope.owner.is_some()
    {
        return Provided::error(
            "LRR-E-QUERY-REQUEST",
            "sealed name query domain/owner differs",
        );
    }
    match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(key.seal.clone())) {
        QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(), QueryValue::DiscoverySeal(seal) if seal.key() == key.seal) =>
            {}
        QueryPoll::Ready(Ok(_)) => {
            return Provided::error(
                "LRR-E-INTERNAL-SNAPSHOT",
                "sealed name certificate variant differs",
            );
        }
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    }
    let input = db.input(&InputKey::DiscoverySeal(key.seal.clone()));
    let Some(InputValue::DiscoverySeal(input)) = input.as_deref() else {
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "sealed name input absent after verification",
        );
    };
    let scope = match input.bucket_certificate(&key.bucket) {
        Ok(Some(scope)) => scope,
        Ok(None) => {
            return Provided::error(
                "LRR-E-QUERY-REQUEST",
                "sealed name has no actual scope facet",
            );
        }
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match scope.lookup(&key.bucket) {
        Ok(lookup) => Provided::pure(QueryValue::SealedName {
            key: Box::new(key.clone()),
            lookup,
        }),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
