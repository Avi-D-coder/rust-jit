//! Owned original-token environment operands and typed eager expansion demands.
use super::{
    builtins::{CompileStringPoll, FrozenCompileEnvironment},
    compile_input_identity::{CompileInputDescriptor, CompileInputKind},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    syntax::token_wire::{Key, TokenForest},
};
#[derive(Debug)]
pub enum StringOperand {
    Literal { value: String, span: Key },
    NeedExpansion { first_span: Key, last_span: Key },
}
pub struct EnvironmentArguments {
    descriptor: Digest,
    kind: CompileInputKind,
    input: Digest,
    name: StringOperand,
    message: Option<StringOperand>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
impl EnvironmentArguments {
    pub fn name(&self) -> &StringOperand {
        &self.name
    }
    pub fn missing_message(&self) -> Option<&StringOperand> {
        self.message.as_ref()
    }
    pub fn fingerprint(&self) -> Digest {
        self.input
    }
    pub fn parse(
        descriptor: &CompileInputDescriptor,
        input: &TokenForest,
    ) -> Result<Self, Diagnostic> {
        let required = match descriptor.kind() {
            CompileInputKind::Env => true,
            CompileInputKind::OptionEnv => false,
            _ => return Err(fault("selected builtin is not an environment macro")),
        };
        let bytes = input.encode().map_err(|e| fault(&e.to_string()))?;
        let forest = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
            .map_err(|e| fault(&e.to_string()))?;
        let parsed = lrr_expand_adapter::environment_arguments::parse(&forest, required)
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let convert = |operand| match operand {
            lrr_expand_adapter::environment_arguments::StringOperand::Literal { value, span } => {
                StringOperand::Literal {
                    value,
                    span: Key(span.0),
                }
            }
            lrr_expand_adapter::environment_arguments::StringOperand::NeedExpansion {
                first_span,
                last_span,
            } => StringOperand::NeedExpansion {
                first_span: Key(first_span.0),
                last_span: Key(last_span.0),
            },
        };
        let mut e = StableEncoder::new("lrr/v2/environment-token-arguments");
        e.digest(descriptor.definition_identity());
        e.bytes(&bytes);
        Ok(Self {
            descriptor: descriptor.definition_identity(),
            kind: descriptor.kind(),
            input: e.finish(),
            name: convert(parsed.name),
            message: parsed.missing_message.map(convert),
        })
    }
}
#[derive(Debug)]
pub enum EnvironmentOperandPoll<'a> {
    Ready(Result<Option<&'a str>, Diagnostic>),
    NeedCapture {
        unit: Digest,
        name: &'a str,
        environment: Digest,
        span: Key,
    },
    NeedExpansion {
        operand: usize,
        first_span: Key,
        last_span: Key,
    },
}
impl FrozenCompileEnvironment {
    pub fn evaluate_environment_arguments<'a>(
        &'a self,
        descriptor: &CompileInputDescriptor,
        args: &'a EnvironmentArguments,
    ) -> Result<EnvironmentOperandPoll<'a>, Diagnostic> {
        if args.descriptor != descriptor.definition_identity() || args.kind != descriptor.kind() {
            return Err(fault(
                "environment arguments belong to another builtin identity",
            ));
        }
        for (index, operand) in std::iter::once(&args.name)
            .chain(args.message.as_ref())
            .enumerate()
        {
            if let StringOperand::NeedExpansion {
                first_span,
                last_span,
            } = operand
            {
                return Ok(EnvironmentOperandPoll::NeedExpansion {
                    operand: index,
                    first_span: *first_span,
                    last_span: *last_span,
                });
            }
        }
        let StringOperand::Literal { value: name, span } = &args.name else {
            return Err(fault("environment operand is still unexpanded"));
        };
        Ok(match self.environment_operand(descriptor, name)? {
            CompileStringPoll::Ready(Err(mut error)) => {
                if let Some(StringOperand::Literal { value, .. }) = &args.message {
                    error.message = value.clone();
                }
                EnvironmentOperandPoll::Ready(Err(error))
            }
            CompileStringPoll::Ready(value) => EnvironmentOperandPoll::Ready(value),
            CompileStringPoll::NeedCapture {
                unit,
                name,
                environment,
            } => EnvironmentOperandPoll::NeedCapture {
                unit,
                name,
                environment,
                span: *span,
            },
        })
    }
}

/// Exact compiler selection and source site for pure environment output emission.
pub struct EnvironmentExpansion<'a> {
    pub catalog: &'a crate::frontend::compile_input_catalog::CompileInputCatalog,
    pub selected: &'a crate::expand::scopes::MacroDefinition,
    pub key: &'a crate::expand::identity::ExpansionKey,
    pub original: &'a TokenForest,
    pub site: Key,
    pub max_wire_bytes: usize,
}
pub enum EnvironmentExpansionPoll {
    Ready(Result<crate::expand::record::ExpansionRecord, Diagnostic>),
    NeedCapture {
        unit: Digest,
        name: String,
        environment: Digest,
        span: Key,
    },
    NeedExpansion {
        operand: usize,
        first_span: Key,
        last_span: Key,
    },
    /// Option output requires the exact standard facade; never fabricate Some/None names.
    NeedOptionFacade {
        defining_unit: Digest,
        value: Option<String>,
        site: Key,
    },
}
impl FrozenCompileEnvironment {
    /// No ambient lookup or process effect. Unknown names and nested operands suspend.
    pub fn expand_environment(
        &self,
        request: EnvironmentExpansion<'_>,
    ) -> Result<EnvironmentExpansionPoll, Diagnostic> {
        self.expand_environment_arguments(request, None)
    }
    pub(super) fn expand_environment_arguments(
        &self,
        request: EnvironmentExpansion<'_>,
        resolved: Option<&EnvironmentArguments>,
    ) -> Result<EnvironmentExpansionPoll, Diagnostic> {
        use crate::expand::identity::MacroIdentity;
        request.key.validate()?;
        if request.max_wire_bytes == 0 {
            return Err(fault("environment output resource limits are invalid"));
        }
        let descriptor = request
            .catalog
            .select(request.selected)?
            .ok_or_else(|| fault("environment output requires a compiler-facet definition"))?;
        let bytes = request
            .original
            .encode()
            .map_err(|e| fault(&e.to_string()))?;
        if !matches!(
            descriptor.kind(),
            CompileInputKind::Env | CompileInputKind::OptionEnv
        ) || request.key.consumer_compile_env != self.fingerprint()
            || request.key.input_tokens != Digest::of_bytes(&bytes)
            || request.key.definition_edition != request.selected.definition_edition
            || request.key.macro_identity
                != (MacroIdentity::Builtin {
                    definition: request.selected.identity,
                })
        {
            return Err(fault(
                "environment output definition/input/environment/grammar mismatch",
            ));
        }
        if !request
            .original
            .spans()
            .iter()
            .any(|s| s.key() == request.site)
        {
            return Err(fault(
                "environment output site is absent from original input",
            ));
        }
        let parsed;
        let arguments = match resolved {
            Some(arguments) => arguments,
            None => {
                parsed = EnvironmentArguments::parse(descriptor, request.original)?;
                &parsed
            }
        };
        Ok(
            match self.evaluate_environment_arguments(descriptor, arguments)? {
                EnvironmentOperandPoll::NeedCapture {
                    unit,
                    name,
                    environment,
                    span,
                } => EnvironmentExpansionPoll::NeedCapture {
                    unit,
                    name: name.into(),
                    environment,
                    span,
                },
                EnvironmentOperandPoll::NeedExpansion {
                    operand,
                    first_span,
                    last_span,
                } => EnvironmentExpansionPoll::NeedExpansion {
                    operand,
                    first_span,
                    last_span,
                },
                EnvironmentOperandPoll::Ready(Err(error)) => {
                    EnvironmentExpansionPoll::Ready(Err(error))
                }
                EnvironmentOperandPoll::Ready(Ok(value))
                    if descriptor.kind() == CompileInputKind::OptionEnv =>
                {
                    EnvironmentExpansionPoll::NeedOptionFacade {
                        defining_unit: request.selected.defining_unit,
                        value: value.map(str::to_owned),
                        site: request.site,
                    }
                }
                EnvironmentOperandPoll::Ready(Ok(Some(value))) => {
                    let input = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
                        .map_err(|e| fault(&e.to_string()))?;
                    let output = lrr_expand_adapter::string_output::emit(
                        &input,
                        lrr_expand_adapter::token_wire::Key(request.site.0),
                        value,
                        request.max_wire_bytes,
                    )
                    .map_err(|e| Diagnostic::new(e.code, e.message))?;
                    let wire = output.encode().map_err(|e| fault(&e.to_string()))?;
                    EnvironmentExpansionPoll::Ready(
                        crate::expand::record::ExpansionRecord::from_wire(
                            request.key.clone(),
                            wire,
                        ),
                    )
                }
                EnvironmentOperandPoll::Ready(Ok(None)) => {
                    return Err(fault("required environment output is absent"));
                }
            },
        )
    }
}
