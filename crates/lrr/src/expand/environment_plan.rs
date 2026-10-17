//! Pure eager environment operand resumption; all child execution stays outside providers.
use super::{
    builtins::FrozenCompileEnvironment,
    compile_input_identity::{CompileInputDescriptor, CompileInputKind},
    environment_arguments::{EnvironmentArguments, EnvironmentExpansion, EnvironmentExpansionPoll},
    identity::{ExpansionKey, GrammarSlot},
    include_arguments::NestedStringInvocation,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{ExpansionRequest, InputRequest},
    },
    syntax::token_wire::TokenForest,
};
use std::collections::BTreeMap;
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
pub struct EnvironmentArgumentOperand {
    tokens: TokenForest,
    nested: Option<NestedStringInvocation>,
}
impl EnvironmentArgumentOperand {
    pub fn nested(&self) -> Option<&NestedStringInvocation> {
        self.nested.as_ref()
    }
}
pub struct EnvironmentArgumentPlan {
    definition: Digest,
    original: Digest,
    operands: Vec<EnvironmentArgumentOperand>,
}
pub enum EnvironmentMaterializationPoll {
    Ready(Result<EnvironmentArguments, Diagnostic>),
    NeedSelection { operand: usize },
    NeedInput(InputRequest),
}
pub enum AdoptedEnvironmentExpansionPoll {
    Output(EnvironmentExpansionPoll),
    NeedSelection { operand: usize },
    NeedInput(InputRequest),
}
impl EnvironmentArgumentPlan {
    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .operands
            .capacity()
            .checked_mul(std::mem::size_of::<EnvironmentArgumentOperand>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(|| fault("operand plan metadata overflow"))?;
        for operand in &self.operands {
            bytes = bytes
                .checked_add(
                    operand
                        .tokens
                        .allocated_bytes()
                        .map_err(|e| fault(e.to_string()))?,
                )
                .ok_or_else(|| fault("operand plan metadata overflow"))?;
            if let Some(nested) = &operand.nested {
                bytes = bytes
                    .checked_add(nested.allocated_bytes()?)
                    .ok_or_else(|| fault("operand plan metadata overflow"))?;
            }
        }
        Ok(bytes)
    }
    pub fn operands(&self) -> &[EnvironmentArgumentOperand] {
        &self.operands
    }
    pub fn parse(
        descriptor: &CompileInputDescriptor,
        original: &TokenForest,
    ) -> Result<Self, Diagnostic> {
        EnvironmentArguments::parse(descriptor, original)?;
        let wire = original.encode().map_err(|e| fault(e.to_string()))?;
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
            .map_err(|e| fault(e.to_string()))?;
        let parsed = lrr_expand_adapter::string_arguments::owned_operands(
            &input,
            if descriptor.kind() == CompileInputKind::Env {
                2
            } else {
                1
            },
        )
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let mut identity = StableEncoder::new("lrr/v2/environment-operand-plan");
        identity.digest(descriptor.definition_identity());
        identity.bytes(&wire);
        let identity = identity.finish();
        let mut operands = Vec::with_capacity(parsed.len());
        for (index, operand) in parsed.into_iter().enumerate() {
            let mut parent = StableEncoder::new("lrr/v2/environment-operand-parent");
            parent.digest(identity);
            parent.u32(u32::try_from(index).map_err(|_| fault("environment ordinal overflow"))?);
            let nested = operand
                .nested
                .map(|n| NestedStringInvocation::for_parent(parent.finish(), n))
                .transpose()?;
            let tokens =
                TokenForest::decode(&operand.tokens.encode().map_err(|e| fault(e.to_string()))?)
                    .map_err(|e| fault(e.to_string()))?;
            operands.push(EnvironmentArgumentOperand { tokens, nested });
        }
        Ok(Self {
            definition: descriptor.definition_identity(),
            original: Digest::of_bytes(&wire),
            operands,
        })
    }
    pub fn materialize_adopted(
        &self,
        db: &mut Database,
        descriptor: &CompileInputDescriptor,
        original: &TokenForest,
        environment: Digest,
        selections: &BTreeMap<usize, ExpansionKey>,
    ) -> EnvironmentMaterializationPoll {
        self.materialize_resolved(
            db,
            descriptor,
            original,
            environment,
            selections,
            &std::collections::BTreeMap::new(),
        )
    }
    pub(crate) fn materialize_resolved(
        &self,
        db: &mut crate::db::memo::Database,
        descriptor: &CompileInputDescriptor,
        original: &TokenForest,
        environment: Digest,
        selections: &std::collections::BTreeMap<usize, crate::expand::identity::ExpansionKey>,
        chains: &std::collections::BTreeMap<usize, super::string_chain::StringExpansionChain>,
    ) -> EnvironmentMaterializationPoll {
        let mut work = || -> Result<EnvironmentMaterializationPoll, Diagnostic> {
            let wire = original.encode().map_err(|e| fault(e.to_string()))?;
            if self.definition != descriptor.definition_identity()
                || self.original != Digest::of_bytes(&wire)
            {
                return Err(fault("environment plan belongs to another invocation"));
            }
            if selections
                .keys()
                .any(|i| self.operands.get(*i).is_none_or(|o| o.nested.is_none()))
            {
                return Err(fault("environment selection is not a nested operand"));
            }
            let mut outputs = Vec::with_capacity(self.operands.len());
            let mut bytes = 0usize;
            for (index, operand) in self.operands.iter().enumerate() {
                let tokens = match &operand.nested {
                    None => operand.tokens.clone(),
                    Some(nested) => {
                        let Some(key) = selections.get(&index) else {
                            return Ok(EnvironmentMaterializationPoll::NeedSelection {
                                operand: index,
                            });
                        };
                        key.validate()?;
                        if key.invocation != nested.identity()
                            || key.input_tokens != nested.argument_identity()
                            || key.consumer_compile_env != environment
                            || key.grammar != GrammarSlot::Expression
                        {
                            return Err(fault(
                                "environment child selection input/context mismatch",
                            ));
                        }
                        let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint()))
                        else {
                            return Ok(EnvironmentMaterializationPoll::NeedInput(
                                InputRequest::Expand(Box::new(ExpansionRequest {
                                    key: key.clone(),
                                })),
                            ));
                        };
                        let InputValue::ExpansionRecord(record) = input.as_ref() else {
                            return Err(fault("environment child input variant mismatch"));
                        };
                        if record.key() != key {
                            return Err(fault("environment child record key mismatch"));
                        }
                        match chains.get(&index) {
                            Some(chain) => match chain.tokens(db, key, environment) {
                                crate::db::poll::QueryPoll::Ready(result) => result?,
                                crate::db::poll::QueryPoll::NeedInput(input) => {
                                    return Ok(EnvironmentMaterializationPoll::NeedInput(input));
                                }
                            },
                            None => record.tokens()?,
                        }
                    }
                };
                bytes = bytes
                    .checked_add(tokens.allocated_bytes().map_err(|e| fault(e.to_string()))?)
                    .ok_or_else(|| fault("environment child aggregate metadata limit"))?;
                let owned = lrr_expand_adapter::token_wire::TokenForest::decode(
                    &tokens.encode().map_err(|e| fault(e.to_string()))?,
                )
                .map_err(|e| fault(e.to_string()))?;
                // Each child must be one string; never allow a child to inject a new argument.
                if !matches!(
                    lrr_expand_adapter::string_arguments::parse_single(&owned)
                        .map_err(|e| { Diagnostic::new(e.code, e.message) })?,
                    lrr_expand_adapter::string_arguments::StringOperand::Literal { .. }
                ) {
                    return Err(fault("environment child output still requires expansion"));
                }
                outputs.push(owned);
            }
            let owned = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
                .map_err(|e| fault(e.to_string()))?;
            let joined = lrr_expand_adapter::concat_arguments::join(&owned, &outputs)
                .map_err(|e| Diagnostic::new(e.code, e.message))?;
            let joined = TokenForest::decode(&joined.encode().map_err(|e| fault(e.to_string()))?)
                .map_err(|e| fault(e.to_string()))?;
            Ok(EnvironmentMaterializationPoll::Ready(
                EnvironmentArguments::parse(descriptor, &joined),
            ))
        };
        match work() {
            Ok(p) => p,
            Err(e) => EnvironmentMaterializationPoll::Ready(Err(e)),
        }
    }
}
impl FrozenCompileEnvironment {
    pub fn expand_environment_adopted(
        &self,
        db: &mut Database,
        plan: &EnvironmentArgumentPlan,
        request: EnvironmentExpansion<'_>,
        selections: &BTreeMap<usize, ExpansionKey>,
    ) -> Result<AdoptedEnvironmentExpansionPoll, Diagnostic> {
        self.expand_environment_resolved(db, plan, request, selections, &BTreeMap::new())
    }
    pub(crate) fn expand_environment_resolved(
        &self,
        db: &mut Database,
        plan: &EnvironmentArgumentPlan,
        request: EnvironmentExpansion<'_>,
        selections: &BTreeMap<usize, ExpansionKey>,
        chains: &BTreeMap<usize, super::string_chain::StringExpansionChain>,
    ) -> Result<AdoptedEnvironmentExpansionPoll, Diagnostic> {
        let descriptor = request
            .catalog
            .select(request.selected)?
            .ok_or_else(|| fault("environment output requires a compiler definition"))?;
        match plan.materialize_resolved(
            db,
            descriptor,
            request.original,
            self.fingerprint(),
            selections,
            chains,
        ) {
            EnvironmentMaterializationPoll::Ready(Ok(args)) => self
                .expand_environment_arguments(request, Some(&args))
                .map(AdoptedEnvironmentExpansionPoll::Output),
            EnvironmentMaterializationPoll::Ready(Err(e)) => Err(e),
            EnvironmentMaterializationPoll::NeedSelection { operand } => {
                Ok(AdoptedEnvironmentExpansionPoll::NeedSelection { operand })
            }
            EnvironmentMaterializationPoll::NeedInput(input) => {
                Ok(AdoptedEnvironmentExpansionPoll::NeedInput(input))
            }
        }
    }
}
