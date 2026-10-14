//! Original include operands and pure authority routing; capture remains a service effect.
use super::compile_input_identity::{CompileInputDescriptor, CompileInputKind};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::include_paths::{GeneratedSourceRequest, IncludeAuthorities, IncludeLocation},
    syntax::token_wire::{Key, TokenForest},
};
#[derive(Debug)]
pub enum IncludeOperand {
    Literal { value: String, span: Key },
    NeedExpansion { first_span: Key, last_span: Key },
}
pub struct IncludeArguments {
    definition: Digest,
    input: Digest,
    operand: IncludeOperand,
    nested: Option<NestedStringInvocation>,
    root_tokens: Digest,
    producers: Vec<(crate::expand::identity::ExpansionKey, Digest)>,
}
#[derive(Clone)]
pub struct NestedStringInvocation {
    identity: Digest,
    absolute: bool,
    segments: Vec<String>,
    arguments: TokenForest,
    argument_identity: Digest,
    reference_span: Key,
}
impl NestedStringInvocation {
    pub(crate) fn for_parent(
        parent: Digest,
        nested: lrr_expand_adapter::nested_string::NestedStringInvocation,
    ) -> Result<Self, Diagnostic> {
        let wire = nested
            .arguments
            .encode()
            .map_err(|e| fault(e.to_string()))?;
        let mut identity = StableEncoder::new("lrr/v2/nested-eager-string-invocation");
        identity.digest(parent);
        identity.bool(nested.absolute);
        identity.sequence(&nested.segments, |e, part| e.string(part));
        identity.bytes(&wire);
        Ok(Self {
            identity: identity.finish(),
            absolute: nested.absolute,
            segments: nested.segments,
            arguments: TokenForest::decode(&wire).map_err(|e| fault(e.to_string()))?,
            argument_identity: Digest::of_bytes(&wire),
            reference_span: Key(nested.first_span.0),
        })
    }
    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .arguments
            .allocated_bytes()
            .map_err(|e| fault(e.to_string()))?
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| {
                n.checked_add(
                    self.segments
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .ok_or_else(|| fault("nested invocation metadata overflow"))?;
        for segment in &self.segments {
            bytes = bytes
                .checked_add(segment.capacity())
                .ok_or_else(|| fault("nested invocation metadata overflow"))?;
        }
        Ok(bytes)
    }
    pub fn reference_span(&self) -> Key {
        self.reference_span
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn absolute(&self) -> bool {
        self.absolute
    }
    pub fn segments(&self) -> &[String] {
        &self.segments
    }
    pub fn arguments(&self) -> &TokenForest {
        &self.arguments
    }
    pub fn argument_identity(&self) -> Digest {
        self.argument_identity
    }
}
#[derive(Debug)]
pub enum IncludePathPoll {
    Capture(Box<GeneratedSourceRequest>),
    NeedExpansion { first_span: Key, last_span: Key },
}
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
impl IncludeArguments {
    pub(crate) fn producer_keys(
        &self,
    ) -> impl Iterator<Item = &crate::expand::identity::ExpansionKey> {
        self.producers.iter().map(|(key, _)| key)
    }

    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(match &self.operand {
                IncludeOperand::Literal { value, .. } => value.capacity(),
                IncludeOperand::NeedExpansion { .. } => 0,
            })
            .ok_or_else(|| fault("include argument metadata overflow"))?;
        if let Some(nested) = &self.nested {
            bytes = bytes
                .checked_add(nested.allocated_bytes()?)
                .ok_or_else(|| fault("include argument metadata overflow"))?;
        }
        bytes = bytes
            .checked_add(
                self.producers
                    .capacity()
                    .checked_mul(std::mem::size_of::<(
                        crate::expand::identity::ExpansionKey,
                        Digest,
                    )>())
                    .ok_or_else(|| fault("include producer metadata overflow"))?,
            )
            .ok_or_else(|| fault("include argument metadata overflow"))?;
        for (key, _) in &self.producers {
            bytes = bytes
                .checked_add(crate::expand::record::key_heap_bytes(key))
                .ok_or_else(|| fault("include producer metadata overflow"))?;
        }
        Ok(bytes)
    }

    pub fn nested_invocation(&self) -> Option<&NestedStringInvocation> {
        self.nested.as_ref()
    }
    pub fn operand(&self) -> &IncludeOperand {
        &self.operand
    }
    pub fn fingerprint(&self) -> Digest {
        self.input
    }
    pub fn parse(
        descriptor: &CompileInputDescriptor,
        input: &TokenForest,
    ) -> Result<Self, Diagnostic> {
        if input.allocated_bytes().map_err(|e| fault(e.to_string()))? > 64 * 1024 * 1024 {
            return Err(fault("include operand metadata exceeds limit"));
        }
        if !matches!(
            descriptor.kind(),
            CompileInputKind::Include
                | CompileInputKind::IncludeStr
                | CompileInputKind::IncludeBytes
        ) {
            return Err(fault("selected builtin is not an include macro"));
        }
        let bytes = input.encode().map_err(|e| fault(e.to_string()))?;
        let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
            .map_err(|e| fault(e.to_string()))?;
        let operand = lrr_expand_adapter::string_arguments::parse_single(&tokens)
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        use lrr_expand_adapter::string_arguments::StringOperand as O;
        let operand = match operand {
            O::Literal { value, span } => IncludeOperand::Literal {
                value,
                span: Key(span.0),
            },
            O::NeedExpansion {
                first_span,
                last_span,
            } => IncludeOperand::NeedExpansion {
                first_span: Key(first_span.0),
                last_span: Key(last_span.0),
            },
        };
        let mut e = StableEncoder::new("lrr/v2/include-token-arguments");
        e.digest(descriptor.definition_identity());
        e.bytes(&bytes);
        let identity = e.finish();
        let nested = if matches!(operand, IncludeOperand::NeedExpansion { .. }) {
            let nested = lrr_expand_adapter::nested_string::parse(&tokens)
                .map_err(|e| Diagnostic::new(e.code, e.message))?;
            let wire = nested
                .arguments
                .encode()
                .map_err(|e| fault(e.to_string()))?;
            let mut key = StableEncoder::new("lrr/v2/nested-include-string-invocation");
            key.digest(identity);
            key.bool(nested.absolute);
            key.sequence(&nested.segments, |e, segment| e.string(segment));
            key.bytes(&wire);
            Some(NestedStringInvocation {
                identity: key.finish(),
                absolute: nested.absolute,
                segments: nested.segments,
                arguments: TokenForest::decode(&wire).map_err(|e| fault(e.to_string()))?,
                argument_identity: Digest::of_bytes(&wire),
                reference_span: Key(nested.first_span.0),
            })
        } else {
            None
        };
        Ok(Self {
            definition: descriptor.definition_identity(),
            input: identity,
            operand,
            nested,
            root_tokens: Digest::of_bytes(&bytes),
            producers: vec![],
        })
    }
    /// Read only an adopted matching expansion input; absent output stays a demand.
    /// Name resolution and macro authority remain with their existing owners.
    pub fn resume_adopted(
        &self,
        db: &mut crate::db::memo::Database,
        descriptor: &CompileInputDescriptor,
        expected: &crate::expand::identity::ExpansionKey,
    ) -> crate::db::poll::QueryPoll<Self> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            poll::{ExpansionRequest, InputRequest, QueryPoll},
        };
        let work = || -> Result<Option<Self>, Diagnostic> {
            expected.validate()?;
            let nested = self
                .nested
                .as_ref()
                .ok_or_else(|| fault("include operand has no pending nested invocation"))?;
            if self.definition != descriptor.definition_identity()
                || expected.invocation != nested.identity
                || expected.input_tokens != nested.argument_identity
                || expected.grammar != crate::expand::identity::GrammarSlot::Expression
            {
                return Err(fault(
                    "nested include expansion key does not match original operand",
                ));
            }
            let Some(input) = db.input(&InputKey::ExpansionRecord(expected.fingerprint())) else {
                return Ok(None);
            };
            let InputValue::ExpansionRecord(record) = input.as_ref() else {
                return Err(fault("nested include expansion input variant mismatch"));
            };
            if record.key() != expected {
                return Err(fault("nested include expansion record key mismatch"));
            }
            let mut resumed = Self::parse(descriptor, &record.tokens()?)?;
            if self.producers.len() >= 256 {
                return Err(fault("include operand producer chain exceeds limit"));
            }
            let mut producer_bytes = (self.producers.len() + 1)
                * std::mem::size_of::<(crate::expand::identity::ExpansionKey, Digest)>();
            for key in self
                .producers
                .iter()
                .map(|(key, _)| key)
                .chain(std::iter::once(expected))
            {
                producer_bytes = producer_bytes
                    .checked_add(crate::expand::record::key_heap_bytes(key))
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| fault("include producer metadata exceeds limit"))?;
            }
            resumed.root_tokens = self.root_tokens;
            resumed.producers = self.producers.clone();
            resumed.producers.push((expected.clone(), record.output()));
            let mut identity = StableEncoder::new("lrr/v2/resumed-include-string-operand");
            identity.digest(self.input);
            identity.record(expected);
            identity.digest(record.output());
            identity.digest(resumed.input);
            resumed.input = identity.finish();
            Ok(Some(resumed))
        };
        let mut work = work;
        match work() {
            Ok(Some(value)) => QueryPoll::Ready(Ok(value)),
            Ok(None) => QueryPoll::NeedInput(InputRequest::Expand(Box::new(ExpansionRequest {
                key: expected.clone(),
            }))),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// Re-observe all adopted producers before consuming a retained resolved path.
    pub fn validate_resolution(
        &self,
        db: &mut crate::db::memo::Database,
        descriptor: &CompileInputDescriptor,
        original: &TokenForest,
        environment: Digest,
    ) -> crate::db::poll::QueryPoll<()> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            poll::{ExpansionRequest, InputRequest, QueryPoll},
        };
        match original.allocated_bytes() {
            Ok(bytes) if bytes <= 64 * 1024 * 1024 => {}
            _ => {
                return QueryPoll::Ready(Err(fault(
                    "resolved include original metadata exceeds limit",
                )));
            }
        }
        let wire = match original.encode() {
            Ok(wire) => wire,
            Err(e) => return QueryPoll::Ready(Err(fault(e.to_string()))),
        };
        if self.definition != descriptor.definition_identity()
            || self.root_tokens != Digest::of_bytes(&wire)
        {
            return QueryPoll::Ready(Err(fault(
                "resolved include operand belongs to another original invocation",
            )));
        }
        for (key, output) in &self.producers {
            if key.consumer_compile_env != environment {
                return QueryPoll::Ready(Err(fault(
                    "resolved include producer consumer environment mismatch",
                )));
            }
            match db
                .input(&InputKey::ExpansionRecord(key.fingerprint()))
                .as_deref()
            {
                Some(InputValue::ExpansionRecord(record))
                    if record.key() == key && record.output() == *output => {}
                None | Some(InputValue::ExpansionRecord(_)) => {
                    return QueryPoll::NeedInput(InputRequest::Expand(Box::new(
                        ExpansionRequest { key: key.clone() },
                    )));
                }
                _ => {
                    return QueryPoll::Ready(Err(fault(
                        "resolved include producer input variant mismatch",
                    )));
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
    pub fn route_from_included(
        &self,
        descriptor: &CompileInputDescriptor,
        authorities: &IncludeAuthorities,
        view: &crate::source::include_view::IncludedSourceView,
        include_origin: Key,
    ) -> Result<IncludePathPoll, Diagnostic> {
        if self.definition != descriptor.definition_identity() {
            return Err(fault("include operand definition mismatch"));
        }
        match &self.operand {
            IncludeOperand::Literal { value, .. } => Ok(IncludePathPoll::Capture(Box::new(
                authorities.request_from_included(descriptor, view, value, include_origin)?,
            ))),
            IncludeOperand::NeedExpansion {
                first_span,
                last_span,
            } => Ok(IncludePathPoll::NeedExpansion {
                first_span: *first_span,
                last_span: *last_span,
            }),
        }
    }
    /// This selects an immutable capture request and performs no filesystem operation.
    pub fn route(
        &self,
        descriptor: &CompileInputDescriptor,
        authorities: &IncludeAuthorities,
        from: &IncludeLocation,
        include_origin: Key,
    ) -> Result<IncludePathPoll, Diagnostic> {
        if self.definition != descriptor.definition_identity() {
            return Err(fault(
                "include arguments belong to another builtin identity",
            ));
        }
        Ok(match &self.operand {
            IncludeOperand::Literal { value, .. } => IncludePathPoll::Capture(Box::new(
                authorities.request(descriptor, from, value, include_origin)?,
            )),
            IncludeOperand::NeedExpansion {
                first_span,
                last_span,
            } => IncludePathPoll::NeedExpansion {
                first_span: *first_span,
                last_span: *last_span,
            },
        })
    }
}
