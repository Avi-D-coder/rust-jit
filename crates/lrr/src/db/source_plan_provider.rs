//! Pure shared source plans. Callers bind namespace and runtime authority later.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{core::digest::Digest, source::manifest::PcKey};
use std::sync::Arc;

pub(super) fn reference_header(
    provider: &SyntaxProvider,
    db: &mut Database,
    definition: Digest,
) -> QueryPoll<Provided> {
    let signature = match db.fetch(provider, QueryKey::FunctionSignature(definition)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
        if let QueryValue::Error(error) = signature.value.as_ref() {
            return QueryPoll::Ready(Err(error.clone()));
        }
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "reference signature query differs",
        );
    };
    let source = db.input(&InputKey::DefinitionSource(definition));
    let context = db.input(&InputKey::DeclarationContext(definition));
    let body = db.input(&InputKey::DefinitionBody(definition));
    let (Some(InputValue::DefinitionSource(parse)), Some(InputValue::DeclarationContext(context))) =
        (source.as_deref(), context.as_deref())
    else {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "reference header inputs absent");
    };
    let body = match body.as_deref() {
        Some(InputValue::DefinitionBody(body)) => Some(body),
        None => None,
        _ => return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "reference body input differs"),
    };
    let parsed = match db.fetch(provider, QueryKey::ParseFile(parse.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "reference parser query differs");
    };
    match parsed.reference_header_plan(signature, context, body) {
        Ok(plan) => Provided::pure(QueryValue::ReferenceHeader(Arc::new(plan))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}

pub(super) fn pattern_plan(
    provider: &SyntaxProvider,
    db: &mut Database,
    owner: &PcKey,
) -> QueryPoll<Provided> {
    if owner.phase != 0 {
        return Provided::error("LRR-E-QUERY-REQUEST", "pattern plan requires entry phase");
    }
    let body = match db.fetch(provider, QueryKey::BodyStructure(owner.body.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::BodyStructure(index) = body.value.as_ref() else {
        if let QueryValue::Error(error) = body.value.as_ref() {
            return QueryPoll::Ready(Err(error.clone()));
        }
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "pattern body query differs");
    };
    let handle = db.input(&InputKey::DefinitionBody(owner.body.definition));
    let source = db.input(&InputKey::DefinitionSource(owner.body.definition));
    let (Some(InputValue::DefinitionBody(handle)), Some(InputValue::DefinitionSource(parse))) =
        (handle.as_deref(), source.as_deref())
    else {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "pattern source inputs absent");
    };
    if handle.key() != owner.body {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "pattern body key is stale");
    }
    let parsed = match db.fetch(provider, QueryKey::ParseFile(parse.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Provided::error("LRR-E-INTERNAL-SNAPSHOT", "pattern parser query differs");
    };
    match parsed.prepare_pattern_plan(handle, Arc::clone(index), owner) {
        Ok(plan) => Provided::pure(QueryValue::PatternPlan(Arc::new(plan))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
