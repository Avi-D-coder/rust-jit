//! Atomic coordinator publication of an already checked typed syntax input.
//! Definition/member/source lease projection is a separate publication step.
use super::*;
use crate::db::retained_syntax::RetainedSyntaxInput;
impl Database {
    pub fn publish_retained_syntax(&mut self, input: Arc<RetainedSyntaxInput>) -> QueryPoll<()> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        match input.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        let result = (|| {
            let key = InputKey::RetainedSyntax(input.key().content);
            let value = InputValue::RetainedSyntax(input);
            let fingerprint = value.fingerprint();
            if let Some(old) = self.inputs.get(&key) {
                return if old.fingerprint == fingerprint {
                    Ok(())
                } else {
                    Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "immutable retained syntax input conflict",
                    ))
                };
            }
            let bytes = input_metadata_bytes(&key, &value)?;
            let total = account_add(self.cache_usage()?.memo_bytes, bytes)?;
            if total > self.run_budgets.unwrap_or_default().query_bytes {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "retained syntax input exceeds query metadata cap",
                ));
            }
            let revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
            self.inputs.insert(
                key,
                InputRecord {
                    value: Arc::new(value),
                    fingerprint,
                },
            );
            self.revision = revision;
            Ok(())
        })();
        QueryPoll::Ready(result)
    }
}
