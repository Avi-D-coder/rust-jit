//! One shared pure local-source plan per actual published owning environment.
use super::{
    dependency::{InputKey, InputValue},
    memo::Database,
    poll::QueryPoll,
    provider::Provided,
    value::QueryValue,
};
use crate::{frontend::source_locals::PreparedSourceLocalPlan, source::manifest::BodyKey};
use std::sync::Arc;
pub(super) fn source_local_plan(db: &mut Database, body: &BodyKey) -> QueryPoll<Provided> {
    let Some(input) = db.input(&InputKey::BodyEnvironment(body.clone())) else {
        return Provided::error(
            "LRR-E-BODY-ENVIRONMENT",
            "owning source environment is not published",
        );
    };
    let InputValue::BodyEnvironment(environment) = input.as_ref() else {
        return Provided::error(
            "LRR-E-INTERNAL-SNAPSHOT",
            "owning environment input variant differs",
        );
    };
    if environment.body() != body {
        return Provided::error("LRR-E-BODY-ENVIRONMENT", "owning environment body differs");
    }
    match environment.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
    }
    let context = match Arc::clone(environment.source()).prepare(db) {
        QueryPoll::Ready(Ok(v)) => v,
        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
    };
    let plan = match PreparedSourceLocalPlan::prepare(db, environment.macros().index(), context) {
        QueryPoll::Ready(Ok(v)) => v,
        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
    };
    match plan.attach_environment(Arc::clone(environment)) {
        Ok(plan) => {
            let checked = db.preflight_source_local_plan(&plan);
            match checked {
                Ok(()) => Provided::pure(QueryValue::SourceLocalPlan(plan)),
                Err(error) => QueryPoll::Ready(Err(error)),
            }
        }
        Err(e) => QueryPoll::Ready(Err(e)),
    }
}
