//! Pure output facts: no invocation-result lookup and no procedural execution authority.
use super::record::ExpansionRecord;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    service::expansion::DemandContext,
    syntax::token_wire::TokenForest,
};
use std::{collections::BTreeMap, sync::Arc};

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", message.into())
}
/// Eligibility belongs to this exact request context, independently of semantic identity.
pub struct ValidatedExpansionOutput {
    context: Digest,
    producer: Digest,
    semantic: Digest,
    tokens: Arc<TokenForest>,
    reused: bool,
}
impl ValidatedExpansionOutput {
    pub fn is_current(&self, context: &DemandContext) -> bool {
        self.context == context.fingerprint()
    }
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn semantic(&self) -> Digest {
        self.semantic
    }
    pub fn tokens(&self) -> &Arc<TokenForest> {
        &self.tokens
    }
    pub fn reused(&self) -> bool {
        self.reused
    }
}
/// This cache exposes no operation to obtain an expansion before current-session validation.
pub struct ExpansionOutputCache {
    outputs: BTreeMap<Digest, Arc<TokenForest>>,
    bytes: usize,
    limit: usize,
}
impl ExpansionOutputCache {
    pub fn new(limit: usize) -> Result<Self, Diagnostic> {
        if limit == 0 || limit > 256 * 1024 * 1024 {
            return Err(fault("invalid expansion output cache limit"));
        }
        Ok(Self {
            outputs: BTreeMap::new(),
            bytes: 0,
            limit,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }
    /// Called only after the actual matched session has fulfilled/admitted this exact request.
    pub(crate) fn validate(
        &mut self,
        context: &DemandContext,
        record: &ExpansionRecord,
    ) -> Result<ValidatedExpansionOutput, Diagnostic> {
        record.key().validate()?;
        if record.key().procedural_epoch != Some(context.epoch)
            || record.key().consumer_compile_env != context.compile_environment
        {
            return Err(fault("output validation epoch/environment mismatch"));
        }
        let mut key = record.key().clone();
        // Epoch affects freshness, not the semantic identity of validated unchanged output.
        key.procedural_epoch = Some(0);
        let mut encoded = StableEncoder::new("lrr/v2/validated-expansion-semantics");
        encoded.digest(context.unit);
        encoded.record(&key);
        encoded.digest(record.output());
        let semantic = encoded.finish();
        let parsed = record.tokens()?;
        let (tokens, reused) = if let Some(previous) = self.outputs.get(&semantic) {
            if previous.as_ref() != &parsed {
                return Err(fault("output identity collision"));
            }
            (Arc::clone(previous), true)
        } else {
            let charged = parsed
                .allocated_bytes()
                .map_err(|_| fault("output cache size overflow"))?
                .checked_add(128)
                .ok_or_else(|| fault("output cache size overflow"))?;
            let next = self
                .bytes
                .checked_add(charged)
                .filter(|n| *n <= self.limit)
                .ok_or_else(|| fault("output cache storage limit"))?;
            if self.outputs.len() >= 100_000 {
                return Err(fault("output cache count limit"));
            }
            let tokens = Arc::new(parsed);
            self.outputs.insert(semantic, Arc::clone(&tokens));
            self.bytes = next;
            (tokens, false)
        };
        Ok(ValidatedExpansionOutput {
            context: context.fingerprint(),
            producer: record.key().fingerprint(),
            semantic,
            tokens,
            reused,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind};
    #[test]
    fn validated_output_reuse_preserves_all_semantic_identity_and_hygiene_inputs() {
        let output = |origin: u8| {
            let parsed = lrr_expand_adapter::token_wire::parse_source(
                "11_u64",
                lrr_expand_adapter::token_wire::SourceIdentity {
                    unit: lrr_expand_adapter::token_wire::Key([1; 32]),
                    content: lrr_expand_adapter::token_wire::Key([2; 32]),
                    path: "fixture.rs".into(),
                },
                lrr_expand_adapter::token_wire::ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
                    defining_unit: lrr_expand_adapter::token_wire::Key([1; 32]),
                    edition: lrr_expand_adapter::token_wire::Edition::E2024,
                },
                lrr_expand_adapter::token_wire::Key([origin; 32]),
            )
            .unwrap();
            crate::syntax::token_wire::TokenForest::decode(&parsed.encode().unwrap()).unwrap()
        };
        let tokens = output(3);
        let context = DemandContext {
            request: Digest([4; 32]),
            session: Digest([5; 32]),
            unit: Digest([1; 32]),
            revision: 1,
            epoch: 1,
            source_generation: Digest([6; 32]),
            native_generation: None,
            compile_environment: Digest([7; 32]),
        };
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact: Digest([8; 32]),
                export: "controlled".into(),
                kind: ProceduralKind::Function,
            },
            invocation: Digest([9; 32]),
            input_tokens: Digest([10; 32]),
            consumer_compile_env: context.compile_environment,
            parent_context: Digest([11; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([12; 32]),
            procedural_epoch: Some(1),
        };
        let mut cache = ExpansionOutputCache::new(256 * 1024 * 1024).unwrap();
        let record = ExpansionRecord::from_tokens(key.clone(), &tokens).unwrap();
        let first = cache.validate(&context, &record).unwrap();
        assert!(!first.reused());
        let mut next_context = context.clone();
        next_context.epoch = 2;
        next_context.request = Digest([13; 32]);
        let mut next_key = key.clone();
        next_key.procedural_epoch = Some(2);
        let next_record = ExpansionRecord::from_tokens(next_key.clone(), &tokens).unwrap();
        let repeated = cache.validate(&next_context, &next_record).unwrap();
        assert!(repeated.reused());
        assert_eq!(first.semantic(), repeated.semantic());
        assert_ne!(first.producer(), repeated.producer());
        assert!(Arc::ptr_eq(first.tokens(), repeated.tokens()));
        assert!(!first.is_current(&next_context));
        let mut variants = Vec::new();
        let mut changed = next_key.clone();
        changed.macro_identity = MacroIdentity::Procedural {
            artifact: Digest([14; 32]),
            export: "controlled".into(),
            kind: ProceduralKind::Function,
        };
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.macro_identity = MacroIdentity::Procedural {
            artifact: Digest([8; 32]),
            export: "other".into(),
            kind: ProceduralKind::Function,
        };
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.macro_identity = MacroIdentity::Procedural {
            artifact: Digest([8; 32]),
            export: "controlled".into(),
            kind: ProceduralKind::Attribute,
        };
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.invocation = Digest([15; 32]);
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.input_tokens = Digest([16; 32]);
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.consumer_compile_env = Digest([17; 32]);
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.parent_context = Digest([18; 32]);
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.definition_edition = 2021;
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.call_edition = 2021;
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.grammar = GrammarSlot::Statements;
        variants.push(changed);
        let mut changed = next_key.clone();
        changed.engine_version = Digest([19; 32]);
        variants.push(changed);
        for changed in variants {
            let mut ctx = next_context.clone();
            ctx.compile_environment = changed.consumer_compile_env;
            let record = ExpansionRecord::from_tokens(changed, &tokens).unwrap();
            let fresh = cache.validate(&ctx, &record).unwrap();
            assert!(!fresh.reused());
            assert_ne!(fresh.semantic(), first.semantic());
        }
        let mut foreign = next_context.clone();
        foreign.unit = Digest([20; 32]);
        let fresh = cache.validate(&foreign, &next_record).unwrap();
        assert!(!fresh.reused());
        assert_ne!(fresh.semantic(), first.semantic());
        let changed_spans = ExpansionRecord::from_tokens(next_key.clone(), &output(21)).unwrap();
        let fresh = cache.validate(&next_context, &changed_spans).unwrap();
        assert!(!fresh.reused());
        assert_ne!(fresh.semantic(), first.semantic());
        let retained = cache.retained_bytes();
        assert!(cache.validate(&context, &next_record).is_err());
        assert_eq!(retained, cache.retained_bytes());
        let mut wrong_env = next_context;
        wrong_env.compile_environment = Digest([22; 32]);
        assert!(cache.validate(&wrong_env, &next_record).is_err());
        assert_eq!(retained, cache.retained_bytes());
    }
}
