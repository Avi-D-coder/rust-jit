//! Owned concat operand planning; macro execution remains an explicit demand.
use super::{
    compile_input_identity::{CompileInputDescriptor, CompileInputKind},
    include_arguments::NestedStringInvocation,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    syntax::token_wire::TokenForest,
};
pub struct ConcatOperand {
    tokens: TokenForest,
    nested: Option<NestedStringInvocation>,
}
impl ConcatOperand {
    pub fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
    pub fn nested(&self) -> Option<&NestedStringInvocation> {
        self.nested.as_ref()
    }
}
pub struct ConcatArguments {
    identity: Digest,
    operands: Vec<ConcatOperand>,
    definition: Digest,
    original: Digest,
}
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
impl ConcatArguments {
    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .operands
            .capacity()
            .checked_mul(std::mem::size_of::<ConcatOperand>())
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
    pub fn fingerprint(&self) -> Digest {
        self.identity
    }
    pub fn operands(&self) -> &[ConcatOperand] {
        &self.operands
    }
    pub fn next_expansion(&self) -> Option<(usize, &NestedStringInvocation)> {
        self.operands
            .iter()
            .enumerate()
            .find_map(|(index, operand)| operand.nested.as_ref().map(|nested| (index, nested)))
    }
    pub fn parse(
        descriptor: &CompileInputDescriptor,
        input: &TokenForest,
    ) -> Result<Self, Diagnostic> {
        if descriptor.kind() != CompileInputKind::Concat {
            return Err(fault("selected builtin is not concat"));
        }
        let wire = input.encode().map_err(|e| fault(e.to_string()))?;
        let original = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
            .map_err(|e| fault(e.to_string()))?;
        let mut identity = StableEncoder::new("lrr/v2/concat-operand-plan");
        identity.digest(descriptor.definition_identity());
        identity.bytes(&wire);
        let identity = identity.finish();
        let parsed = lrr_expand_adapter::concat_arguments::parse(&original)
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let mut operands = Vec::with_capacity(parsed.len());
        for (index, operand) in parsed.into_iter().enumerate() {
            let mut parent = StableEncoder::new("lrr/v2/concat-operand-parent");
            parent.digest(identity);
            parent.u32(u32::try_from(index).map_err(|_| fault("concat operand ordinal overflow"))?);
            let nested = operand
                .nested
                .map(|n| NestedStringInvocation::for_parent(parent.finish(), n))
                .transpose()?;
            let wire = operand.tokens.encode().map_err(|e| fault(e.to_string()))?;
            let tokens = TokenForest::decode(&wire).map_err(|e| fault(e.to_string()))?;
            operands.push(ConcatOperand { tokens, nested });
        }
        Ok(Self {
            identity,
            operands,
            definition: descriptor.definition_identity(),
            original: Digest::of_bytes(&wire),
        })
    }
}

pub enum ConcatMaterializationPoll {
    Ready(Result<TokenForest, Diagnostic>),
    NeedSelection { operand: usize },
    NeedInput(crate::db::poll::InputRequest),
}
impl ConcatArguments {
    /// Pure child-output join. Missing selection/input never becomes a literal result.
    pub fn materialize_adopted(
        &self,
        db: &mut crate::db::memo::Database,
        descriptor: &CompileInputDescriptor,
        original: &TokenForest,
        environment: Digest,
        selections: &std::collections::BTreeMap<usize, crate::expand::identity::ExpansionKey>,
    ) -> ConcatMaterializationPoll {
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
    ) -> ConcatMaterializationPoll {
        use crate::{
            core::digest::StableEncode,
            db::{
                dependency::{InputKey, InputValue},
                poll::{ExpansionRequest, InputRequest},
            },
            expand::identity::GrammarSlot,
        };
        let mut work = || -> Result<ConcatMaterializationPoll, Diagnostic> {
            let original_wire = original.encode().map_err(|e| fault(e.to_string()))?;
            if self.definition != descriptor.definition_identity()
                || self.original != Digest::of_bytes(&original_wire)
            {
                return Err(fault("concat plan belongs to another invocation"));
            }
            if selections.len() > self.operands.len() {
                return Err(fault("concat selection count exceeds operand count"));
            }
            if selections
                .keys()
                .any(|index| self.operands.get(*index).is_none_or(|o| o.nested.is_none()))
            {
                return Err(fault("concat selection is not a nested operand"));
            }
            let mut outputs = Vec::with_capacity(self.operands.len());
            let mut bytes = 0usize;
            for (index, operand) in self.operands.iter().enumerate() {
                let tokens = match &operand.nested {
                    None => operand.tokens.clone(),
                    Some(nested) => {
                        let Some(key) = selections.get(&index) else {
                            return Ok(ConcatMaterializationPoll::NeedSelection { operand: index });
                        };
                        key.validate()?;
                        if key.invocation != nested.identity()
                            || key.input_tokens != nested.argument_identity()
                            || key.consumer_compile_env != environment
                            || key.grammar != GrammarSlot::Expression
                        {
                            return Err(fault("concat child selection input/context mismatch"));
                        }
                        let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint()))
                        else {
                            return Ok(ConcatMaterializationPoll::NeedInput(InputRequest::Expand(
                                Box::new(ExpansionRequest { key: key.clone() }),
                            )));
                        };
                        let InputValue::ExpansionRecord(record) = input.as_ref() else {
                            return Err(fault("concat child input variant mismatch"));
                        };
                        if record.key() != key {
                            return Err(fault("concat child record key mismatch"));
                        }
                        match chains.get(&index) {
                            Some(chain) => match chain.tokens(db, key, environment) {
                                crate::db::poll::QueryPoll::Ready(result) => result?,
                                crate::db::poll::QueryPoll::NeedInput(input) => {
                                    return Ok(ConcatMaterializationPoll::NeedInput(input));
                                }
                            },
                            None => record.tokens()?,
                        }
                    }
                };
                bytes = bytes
                    .checked_add(tokens.allocated_bytes().map_err(|e| fault(e.to_string()))?)
                    .ok_or_else(|| fault("concat child aggregate metadata limit"))?;
                outputs.push(
                    lrr_expand_adapter::token_wire::TokenForest::decode(
                        &tokens.encode().map_err(|e| fault(e.to_string()))?,
                    )
                    .map_err(|e| fault(e.to_string()))?,
                );
            }
            let original = lrr_expand_adapter::token_wire::TokenForest::decode(&original_wire)
                .map_err(|e| fault(e.to_string()))?;
            let joined = lrr_expand_adapter::concat_arguments::join(&original, &outputs)
                .map_err(|e| Diagnostic::new(e.code, e.message))?;
            Ok(ConcatMaterializationPoll::Ready(
                TokenForest::decode(&joined.encode().map_err(|e| fault(e.to_string()))?)
                    .map_err(|e| fault(e.to_string())),
            ))
        };
        match work() {
            Ok(poll) => poll,
            Err(error) => ConcatMaterializationPoll::Ready(Err(error)),
        }
    }
}

pub struct ConcatExpansion<'a> {
    pub catalog: &'a crate::frontend::compile_input_catalog::CompileInputCatalog,
    pub selected: &'a crate::expand::scopes::MacroDefinition,
    pub key: &'a crate::expand::identity::ExpansionKey,
    pub original: &'a TokenForest,
    pub site: crate::syntax::token_wire::Key,
    pub selections: &'a std::collections::BTreeMap<usize, crate::expand::identity::ExpansionKey>,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_wire_bytes: usize,
}
pub enum ConcatExpansionPoll {
    Ready(Result<crate::expand::record::ExpansionRecord, Diagnostic>),
    NeedSelection { operand: usize },
    NeedInput(crate::db::poll::InputRequest),
}
impl ConcatArguments {
    /// Final pure parent output preserves the original invocation/input key.
    pub fn expand_adopted(
        &self,
        db: &mut crate::db::memo::Database,
        request: ConcatExpansion<'_>,
    ) -> ConcatExpansionPoll {
        self.expand_resolved(db, request, &std::collections::BTreeMap::new())
    }
    pub(crate) fn expand_resolved(
        &self,
        db: &mut crate::db::memo::Database,
        request: ConcatExpansion<'_>,
        chains: &std::collections::BTreeMap<usize, super::string_chain::StringExpansionChain>,
    ) -> ConcatExpansionPoll {
        use crate::expand::identity::MacroIdentity;
        let mut work = || -> Result<ConcatExpansionPoll, Diagnostic> {
            request.key.validate()?;
            let descriptor = request
                .catalog
                .select(request.selected)?
                .ok_or_else(|| fault("eager concat requires a compiler-facet definition"))?;
            if descriptor.kind() != CompileInputKind::Concat
                || self.definition != descriptor.definition_identity()
                || request.key.input_tokens != self.original
                || request.key.definition_edition != request.selected.definition_edition
                || request.key.macro_identity
                    != (MacroIdentity::Builtin {
                        definition: request.selected.identity,
                    })
            {
                return Err(fault(
                    "eager concat parent definition/input/grammar mismatch",
                ));
            }
            if request.max_input_bytes == 0
                || request.max_output_bytes == 0
                || request.max_wire_bytes == 0
            {
                return Err(fault("eager concat resource limits are invalid"));
            }
            if request
                .original
                .allocated_bytes()
                .map_err(|e| fault(e.to_string()))?
                > request.max_input_bytes
            {
                return Err(fault("eager concat original metadata exceeds limit"));
            }
            if !request
                .original
                .spans()
                .iter()
                .any(|s| s.key() == request.site)
            {
                return Err(fault("eager concat site is absent from original input"));
            }
            let joined = match self.materialize_resolved(
                db,
                descriptor,
                request.original,
                request.key.consumer_compile_env,
                request.selections,
                chains,
            ) {
                ConcatMaterializationPoll::NeedSelection { operand } => {
                    return Ok(ConcatExpansionPoll::NeedSelection { operand });
                }
                ConcatMaterializationPoll::NeedInput(input) => {
                    return Ok(ConcatExpansionPoll::NeedInput(input));
                }
                ConcatMaterializationPoll::Ready(result) => result?,
            };
            let value = crate::expand::concat::literal_value(
                descriptor,
                &joined,
                request.site,
                request.max_input_bytes,
                request.max_output_bytes,
            )?;
            let original = lrr_expand_adapter::token_wire::TokenForest::decode(
                &request
                    .original
                    .encode()
                    .map_err(|e| fault(e.to_string()))?,
            )
            .map_err(|e| fault(e.to_string()))?;
            let output = lrr_expand_adapter::string_output::emit(
                &original,
                lrr_expand_adapter::token_wire::Key(request.site.0),
                value.value(),
                request.max_wire_bytes,
            )
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
            let wire = output.encode().map_err(|e| fault(e.to_string()))?;
            Ok(ConcatExpansionPoll::Ready(
                crate::expand::record::ExpansionRecord::from_wire(request.key.clone(), wire),
            ))
        };
        match work() {
            Ok(poll) => poll,
            Err(error) => ConcatExpansionPoll::Ready(Err(error)),
        }
    }
}
