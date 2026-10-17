//! T054 coordinator admission and pure provider on the existing query engine.
use super::{
    ConstLimits, evaluate, memo::ConstMemoPayload, query_failure::ConstantQueryFailure,
    query_value::ConstantQueryValue, wire::ConstantWireType,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::{
        key::{ConstantKey, QueryKey},
        memo::{Database, Outcome},
        poll::QueryPoll,
        provider::{Provided, Provider, SyntaxProvider},
        value::QueryValue,
    },
    frontend::{
        constant_result_type::{CurrentConstantResultType, ExpectedResultReadiness},
        constant_source_admission::{ConstantRequest, ConstantSourceAdmission},
    },
    source::changes::PreparedSourceView,
};
fn uncached(error: Diagnostic) -> QueryPoll<Provided> {
    QueryPoll::Ready(Ok(Provided {
        value: QueryValue::Error(error),
        cacheable: false,
    }))
}
struct LengthQuery {
    key: ConstantKey,
    proof: CurrentConstantResultType,
    expected_count: u64,
}
struct CurrentConstantProvider<'a> {
    key: ConstantKey,
    admission: &'a ConstantSourceAdmission,
    length: Option<LengthQuery>,
    limits: ConstLimits,
    byte_limit: usize,
}
impl CurrentConstantProvider<'_> {
    fn evaluate_selected(
        &self,
        db: &mut Database,
        key: &ConstantKey,
        admission: &ConstantSourceAdmission,
    ) -> QueryPoll<Provided> {
        // Actual source/seal/epoch/compiler facts enter the existing collector.
        // Database blocks input publication while a provider is active.
        for (input, fingerprint) in admission.dependencies() {
            if db
                .input(input)
                .is_none_or(|value| value.fingerprint() != *fingerprint)
            {
                return uncached(Diagnostic::new(
                    "LRR-E-CONST-SOURCE",
                    "constant admitted dependency changed".into(),
                ));
            }
        }
        let evaluation = match evaluate(admission.plan(), self.limits) {
            Ok(value) => value,
            Err(failure) => {
                if failure.required_macro().is_some() {
                    return match super::query_macro_demand::ConstantMacroDemand::capture(
                        key, admission, &failure,
                    ) {
                        Ok(demand) => QueryPoll::Ready(Ok(Provided {
                            value: QueryValue::ConstantMacroDemand(Box::new(demand)),
                            cacheable: false,
                        })),
                        Err(error) => uncached(error),
                    };
                }
                return match ConstantQueryFailure::capture(key, admission, &failure) {
                    Ok(value) => Provided::pure(QueryValue::ConstantFailure(Box::new(value))),
                    // Preserve the actual evaluation error if deterministic memo
                    // admission is incomplete or its resource budget refuses.
                    Err(_) => uncached(failure.diagnostic().clone()),
                };
            }
        };
        let payload =
            match ConstMemoPayload::capture(admission.plan(), &evaluation, self.byte_limit) {
                Ok(payload) => payload,
                Err(error) => return uncached(error),
            };
        match ConstantQueryValue::capture(key, payload) {
            Ok(value) => Provided::pure(QueryValue::Constant(Box::new(value))),
            Err(error) => uncached(error),
        }
    }
}
impl Provider for CurrentConstantProvider<'_> {
    fn execute(&self, db: &mut Database, key: &QueryKey) -> QueryPoll<Provided> {
        if let Some(length) = &self.length
            && matches!(key, QueryKey::Constant(actual) if actual == &length.key)
        {
            return self.evaluate_selected(db, &length.key, length.proof.admission());
        }
        if !matches!(key, QueryKey::Constant(actual) if actual == &self.key) {
            return SyntaxProvider.execute(db, key);
        }
        // A source-owned array length is a genuine child query. Its dependency
        // is observed before initializer evaluation, including child memo hits.
        if let Some(length) = &self.length {
            let result = match db.fetch(self, QueryKey::Constant(length.key.clone())) {
                QueryPoll::Ready(Ok(result)) => result,
                QueryPoll::Ready(Err(error)) => return uncached(error),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if let QueryValue::ConstantMacroDemand(child) = result.value.as_ref() {
                return match super::query_macro_demand::ConstantMacroDemand::forward_length(
                    &self.key,
                    self.admission,
                    &length.key,
                    length.proof.admission(),
                    child,
                ) {
                    Ok(demand) => QueryPoll::Ready(Ok(Provided {
                        value: QueryValue::ConstantMacroDemand(Box::new(demand)),
                        cacheable: false,
                    })),
                    Err(error) => uncached(error),
                };
            }
            let QueryValue::Constant(value) = result.value.as_ref() else {
                return uncached(Diagnostic::new(
                    "LRR-E-CONST-TYPE",
                    "source array length query returned another value variant".into(),
                ));
            };
            let actual = match value.payload().decode_value() {
                Ok(value) => value,
                Err(error) => return uncached(error),
            };
            if actual
                .scalar()
                .is_none_or(|scalar| scalar.payload != length.expected_count)
            {
                return uncached(Diagnostic::new(
                    "LRR-E-CONST-TYPE",
                    "caller array count differs from its exact source length".into(),
                ));
            }
        }
        self.evaluate_selected(db, &self.key, self.admission)
    }
}
fn metadata_limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "constant query transient metadata limit".into(),
    )
}
fn make_key(
    db: &Database,
    admission: &ConstantSourceAdmission,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> Result<ConstantKey, Diagnostic> {
    super::wire::admit_result_limits(expected, byte_limit, limits.result_bytes)?;
    Ok(
        ConstantKey::capture(admission.plan(), expected, admission.identity())?
            .bind_universe(db.constant_input_universe()?),
    )
}
/// Coordinator-only source/header admission occurs before every provider or memo
/// access. No provider discovers inputs or performs macro/build preparation.
fn fetch_prepared_constant_mode(
    db: &mut Database,
    source: &PreparedSourceView,
    proof: CurrentConstantResultType,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
    typed_failures: bool,
) -> QueryPoll<Outcome> {
    let prepare = || -> Result<(ConstantKey, Option<LengthQuery>), Diagnostic> {
        let readiness = proof.check_current_expected(db, source, expected)?;
        let (concrete, length) = match readiness {
            ExpectedResultReadiness::ReadyScalar(scalar) => (scalar.wire_type(), None),
            ExpectedResultReadiness::RequiredArrayLength(demand) => {
                let ConstantWireType::Array { length, .. } = expected else {
                    unreachable!("checked array readiness")
                };
                let remaining = usize::MAX;
                let admission = proof
                    .admission()
                    .reselect_array_length_current(db, source, demand, remaining)?;
                let length_proof =
                    CurrentConstantResultType::capture_current(db, source, admission, remaining)?;
                let usize_type = demand.expected_wire();
                if !matches!(
                    length_proof.check_current_expected(db, source, &usize_type)?,
                    ExpectedResultReadiness::ReadyScalar(_)
                ) {
                    return Err(Diagnostic::new(
                        "LRR-E-CONST-TYPE",
                        "array length source is not scalar usize".into(),
                    ));
                }
                let key = make_key(
                    db,
                    length_proof.admission(),
                    &usize_type,
                    limits,
                    byte_limit,
                )?;
                let (element, _) = proof.array().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-CONST-TYPE",
                        "array readiness lost its source element proof".into(),
                    )
                })?;
                (
                    ConstantWireType::Array {
                        element: Box::new(element.wire_type()),
                        length: *length,
                    },
                    Some(LengthQuery {
                        key,
                        proof: length_proof,
                        expected_count: *length,
                    }),
                )
            }
        };
        let key = make_key(db, proof.admission(), &concrete, limits, byte_limit)?;
        Ok((key, length))
    };
    let (key, length) = match prepare() {
        Ok(ready) => ready,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let provider = CurrentConstantProvider {
        key: key.clone(),
        admission: proof.admission(),
        length,
        limits,
        byte_limit,
    };
    let query_key = QueryKey::Constant(key.clone());
    let poll = db.fetch(&provider, query_key.clone());
    // Database::fetch preserves its diagnostic-facing contract. The reached
    // source-use coordinator may also inspect the exact real cached failure as
    // typed data. This cannot turn an error into a value or recover another key.
    if typed_failures
        && let QueryPoll::Ready(Err(error)) = &poll
        && let Some(memo) = db.memo(&query_key)
        && memo.verified_revision == db.revision()
        && memo.fingerprint == memo.value.fingerprint()
        && let QueryValue::ConstantFailure(failure) = memo.value.as_ref()
        && failure.key() == &key
        && failure.diagnostic().code == error.code
        && failure.diagnostic().message == error.message
    {
        return QueryPoll::Ready(Ok(Outcome {
            value: std::sync::Arc::clone(&memo.value),
            fingerprint: memo.fingerprint,
            cacheable: true,
        }));
    }
    poll
}
/// Coordinator-only reached macro preparation. A provider merely emits data;
/// only this caller remints current source proof and repeats the pure query.
pub(crate) fn fetch_constant_preparing_macros(
    db: &mut Database,
    source: &PreparedSourceView,
    mut admission: ConstantSourceAdmission,
    expected: &ConstantWireType,
    mut limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<(ConstantSourceAdmission, Outcome)> {
    for _ in 0usize.. {
        let outcome =
            match fetch_current_constant_data(db, source, &admission, expected, limits, byte_limit)
            {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        let QueryValue::ConstantMacroDemand(demand) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Ok((admission, outcome)));
        };
        if outcome.cacheable || outcome.fingerprint != outcome.value.fingerprint() {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "constant macro demand was cached or changed".into(),
            )));
        }
        limits = match limits.after_steps(demand.evaluation_steps()) {
            Ok(limits) => limits,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        admission = match admission.prepare_reached_macro(db, source, demand) {
            Ok(admission) => admission,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
    }
    QueryPoll::Ready(Err(Diagnostic::new(
        "LRR-E-CONST-LIMIT",
        "constant macro retry count exhausted".into(),
    )))
}
/// Read-only adapters reserve the full additional admission clone before copying
/// retained BTreeMap stamps. Production entry points consume the owned packet.
// Historical diagnostic-facing borrowed wrapper retained by regression models.
#[cfg(test)]
pub(crate) fn fetch_current_constant(
    db: &mut Database,
    source: &PreparedSourceView,
    admission: &ConstantSourceAdmission,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    fetch_current_constant_mode(db, source, admission, expected, limits, byte_limit, false)
}
/// Typed current failures remain failures. No synthesis from a diagnostic alone.
pub(crate) fn fetch_current_constant_data(
    db: &mut Database,
    source: &PreparedSourceView,
    admission: &ConstantSourceAdmission,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    fetch_current_constant_mode(db, source, admission, expected, limits, byte_limit, true)
}
fn fetch_current_constant_mode(
    db: &mut Database,
    source: &PreparedSourceView,
    admission: &ConstantSourceAdmission,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
    typed_failures: bool,
) -> QueryPoll<Outcome> {
    let proof =
        match CurrentConstantResultType::capture_current(db, source, admission.clone(), usize::MAX) {
            Ok(proof) => proof,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
    fetch_prepared_constant_mode(
        db,
        source,
        proof,
        expected,
        limits,
        byte_limit,
        typed_failures,
    )
}
fn capture_and_fetch(
    db: &mut Database,
    source: &PreparedSourceView,
    definition: Digest,
    request: ConstantRequest,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    let admission =
        match ConstantSourceAdmission::capture_current(db, source, definition, request, usize::MAX) {
            Ok(admission) => admission,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
    match fetch_constant_preparing_macros(db, source, admission, expected, limits, byte_limit) {
        QueryPoll::Ready(Ok((_, outcome))) => match outcome.value.as_ref() {
            QueryValue::ConstantFailure(failure) => {
                QueryPoll::Ready(Err(failure.diagnostic().clone()))
            }
            _ => QueryPoll::Ready(Ok(outcome)),
        },
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}
/// Source-selected named constant. Missing current source/macro/preparation
/// evidence remains an explicit prerequisite; this never performs discovery.
pub fn constant(
    db: &mut Database,
    source: &PreparedSourceView,
    definition: Digest,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    capture_and_fetch(
        db,
        source,
        definition,
        ConstantRequest::Constant,
        expected,
        limits,
        byte_limit,
    )
}
/// Exact source const fn with immutable concrete arguments, no guest handles.
pub fn const_call(
    db: &mut Database,
    source: &PreparedSourceView,
    definition: Digest,
    arguments: Vec<super::ConstValue>,
    expected: &ConstantWireType,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    capture_and_fetch(
        db,
        source,
        definition,
        ConstantRequest::ConstCall(arguments),
        expected,
        limits,
        byte_limit,
    )
}
/// Exact selected array-length operand, always in concrete usize context.
pub fn array_length(
    db: &mut Database,
    source: &PreparedSourceView,
    definition: Digest,
    range: crate::syntax::spans::SourceRange,
    limits: ConstLimits,
    byte_limit: usize,
) -> QueryPoll<Outcome> {
    capture_and_fetch(
        db,
        source,
        definition,
        ConstantRequest::ArrayLength(range),
        &ConstantWireType::Scalar(crate::ir::op::PrimitiveType::Usize),
        limits,
        byte_limit,
    )
}
