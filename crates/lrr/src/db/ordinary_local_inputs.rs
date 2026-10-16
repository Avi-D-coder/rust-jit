//! Coordinator publication from the ordinary entered Value/canonical-call proof.
//! Only the exact selected body locator is new; no executable body is prepared.
use super::*;
use crate::{
    core::digest::StableEncode, frontend::callables::ordinary_local::OrdinaryLocalAdmission,
};

impl Database {
    /// Root's reached ordinary-call coordinator consumes this source proof only
    /// after matching its real site to the current frame. This method grants
    /// body metadata, never call acceptance or guest/macro execution authority.
    pub(crate) fn publish_ordinary_local_function(
        &mut self,
        proof: &OrdinaryLocalAdmission,
    ) -> Result<(), Diagnostic> {
        self.require_coordinator()?;
        if !self.run_active {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "ordinary local admission requires its active run",
            ));
        }
        // This privately issued Q39 guard retains the actual source/namespace
        // input closure. An unrelated derived body publication may advance the
        // database revision without changing any of these dependencies. Never
        // reconstruct the proof or refresh its guard at this boundary.
        proof.revalidate(self)?;
        let parent = proof.parent();
        let source = parent.context().source();
        let caller = source.body().def_key;
        let definition = proof.item().def_key.fingerprint();
        if proof.site().body != source.body().key()
            || proof.source() != &source.parsed().key()
            || proof
                .item()
                .body
                .as_ref()
                .is_none_or(|body| body.def_key != definition)
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "ordinary local admission parent/source differs",
            ));
        }
        // The private guard owns semantic dependencies. Compare actual parent
        // attachments too, preserving exact source/header/body correspondence.
        let parent_rows_match = matches!(self.inputs.get(&InputKey::DefinitionSource(caller)).map(|row| row.value.as_ref()),
                Some(InputValue::DefinitionSource(parse)) if parse == proof.source())
            && matches!(self.inputs.get(&InputKey::DefinitionHeader(caller)).map(|row| row.value.as_ref()),
                Some(InputValue::DefinitionHeader(header)) if header == source.header())
            && matches!(self.inputs.get(&InputKey::DeclarationContext(caller)).map(|row| row.value.as_ref()),
                Some(InputValue::DeclarationContext(context)) if context == source.context())
            && matches!(self.inputs.get(&InputKey::DefinitionBody(caller)).map(|row| row.value.as_ref()),
                Some(InputValue::DefinitionBody(body)) if body == source.body());
        let Some(environment) = parent.environment() else {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "ordinary local admission owning environment absent",
            ));
        };
        if !parent_rows_match
            || !matches!(self.inputs.get(&InputKey::BodyEnvironment(environment.body().clone())).map(|row| row.value.as_ref()),
                Some(InputValue::BodyEnvironment(actual)) if actual.identity() == environment.identity()
                    && actual.body() == source.index().body())
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "ordinary local admission owning parent changed",
            ));
        }
        let owner_key = InputKey::LocalDeclarationOwner(definition);
        let expected_owner = InputValue::LocalDeclarationOwner(source.body().key());
        if !self.inputs.get(&owner_key).is_some_and(|row| {
            row.fingerprint == expected_owner.fingerprint()
                && matches!(row.value.as_ref(), InputValue::LocalDeclarationOwner(owner) if owner == source.index().body())
        }) {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "ordinary local admission actual declaration owner differs",
            ));
        }
        let mut staged = None;
        for (key, value) in proof.derived_inputs() {
            if !value.matches(&key) {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "ordinary local admission derived row variant differs",
                ));
            }
            if let Some(current) = self.inputs.get(&key) {
                let same = match (current.value.as_ref(), &value) {
                    (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => a == b,
                    (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => a == b,
                    (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => {
                        a == b
                    }
                    (InputValue::DefinitionBody(a), InputValue::DefinitionBody(b)) => a == b,
                    _ => false,
                };
                if !same || current.fingerprint != value.fingerprint() {
                    return Err(fault(
                        "LRR-E-CALL-CONTEXT",
                        "ordinary local admission immutable declaration row conflict",
                    ));
                }
            } else if key == InputKey::DefinitionBody(definition) {
                if staged.replace((key, value)).is_some() {
                    return Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "ordinary local admission repeated body locator",
                    ));
                }
            } else {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "ordinary local admission cold declaration row absent",
                ));
            }
        }
        let Some((key, value)) = staged else {
            return Ok(());
        };
        let extra = input_metadata_bytes(&key, &value)?;
        let usage = self.cache_usage()?;
        let budgets = self.run_budgets.unwrap_or_default();
        if account_add(usage.memo_bytes, extra)? > budgets.query_bytes
            || usage.source_input_bytes > budgets.source_input_bytes
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "ordinary local body publication exceeds cache cap",
            ));
        }
        let revision = self.revision.checked_add(1).ok_or_else(|| {
            fault(
                "LRR-E-ID-EXHAUSTED",
                "ordinary local publication revision exhausted",
            )
        })?;
        let record = InputRecord {
            fingerprint: value.fingerprint(),
            value: Arc::new(value),
        };
        // All fallible validation, quota and row construction precedes mutation.
        self.inputs.insert(key, record);
        self.revision = revision;
        Ok(())
    }
}

#[cfg(test)]
#[path = "ordinary_local_inputs_tests.rs"]
mod tests;
