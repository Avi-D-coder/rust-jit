//! Immutable adopted expression producer proofs; no selection or execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{ExpansionRequest, InputRequest, QueryPoll},
    },
    expand::{
        identity::{ExpansionKey, GrammarSlot},
        include_arguments::NestedStringInvocation,
    },
    syntax::token_wire::TokenForest,
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
pub(crate) enum StringOutput {
    Literal,
    Nested(Box<NestedStringInvocation>),
}
/// One expression only: a returned macro cannot inject extra parent operands.
pub(crate) fn classify_output(
    key: &ExpansionKey,
    tokens: &TokenForest,
) -> Result<StringOutput, Diagnostic> {
    let wire = tokens.encode().map_err(|e| fault(&e.message))?;
    let owned = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
        .map_err(|e| fault(&e.message))?;
    let mut operands =
        lrr_expand_adapter::concat_arguments::parse(&owned).map_err(|e| fault(&e.message))?;
    if operands.len() != 1 {
        return Err(fault("eager child output requires exactly one expression"));
    }
    let operand = operands
        .pop()
        .ok_or_else(|| fault("eager child expression absent"))?;
    match operand.nested {
        Some(nested) => Ok(StringOutput::Nested(Box::new(
            NestedStringInvocation::for_parent(key.fingerprint(), nested)?,
        ))),
        None => Ok(StringOutput::Literal),
    }
}
pub(crate) struct StringExpansionChain {
    producers: Vec<(ExpansionKey, Digest)>,
}
impl StringExpansionChain {
    pub(crate) fn new() -> Self {
        Self {
            producers: Vec::new(),
        }
    }
    pub(crate) fn root(&self) -> Result<&ExpansionKey, Diagnostic> {
        self.producers
            .first()
            .map(|(key, _)| key)
            .ok_or_else(|| fault("eager chain root absent"))
    }
    pub(crate) fn push(&mut self, key: ExpansionKey, output: Digest) -> Result<(), Diagnostic> {
        if self.producers.len() >= 256 {
            return Err(fault("eager string producer depth limit"));
        }
        self.producers
            .try_reserve(1)
            .map_err(|_| fault("eager string producer allocation"))?;
        self.producers.push((key, output));
        self.allocated_bytes()?;
        Ok(())
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .producers
            .capacity()
            .checked_mul(std::mem::size_of::<(ExpansionKey, Digest)>())
            .ok_or_else(|| fault("eager string producer metadata overflow"))?;
        for (key, _) in &self.producers {
            bytes = bytes
                .checked_add(crate::expand::record::key_heap_bytes(key))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("eager string producer metadata limit"))?;
        }
        Ok(bytes)
    }
    /// Re-observe all exact records and each returned invocation before joining.
    pub(crate) fn tokens(
        &self,
        db: &mut Database,
        root: &ExpansionKey,
        environment: Digest,
    ) -> QueryPoll<TokenForest> {
        let mut work = || -> Result<QueryPoll<TokenForest>, Diagnostic> {
            if self.root()? != root {
                return Err(fault("eager string chain root differs"));
            }
            self.allocated_bytes()?;
            let mut expected: Option<NestedStringInvocation> = None;
            for (index, (key, output)) in self.producers.iter().enumerate() {
                key.validate()?;
                if key.consumer_compile_env != environment || key.grammar != GrammarSlot::Expression
                {
                    return Err(fault("eager string chain environment/grammar differs"));
                }
                if let Some(nested) = &expected
                    && (nested.identity() != key.invocation
                        || nested.argument_identity() != key.input_tokens)
                {
                    return Err(fault("eager string chain returned invocation differs"));
                }
                let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint())) else {
                    return Ok(QueryPoll::NeedInput(InputRequest::Expand(Box::new(
                        ExpansionRequest { key: key.clone() },
                    ))));
                };
                let InputValue::ExpansionRecord(record) = input.as_ref() else {
                    return Err(fault("eager string chain input variant differs"));
                };
                if record.key() != key || record.output() != *output {
                    return Err(fault("eager string chain adopted output differs"));
                }
                let tokens = record.tokens()?;
                match classify_output(key, &tokens)? {
                    StringOutput::Literal if index + 1 == self.producers.len() => {
                        return Ok(QueryPoll::Ready(Ok(tokens)));
                    }
                    StringOutput::Nested(nested) if index + 1 < self.producers.len() => {
                        expected = Some(*nested)
                    }
                    _ => return Err(fault("eager string chain completion differs")),
                }
            }
            Err(fault("eager string chain incomplete"))
        };
        match work() {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
