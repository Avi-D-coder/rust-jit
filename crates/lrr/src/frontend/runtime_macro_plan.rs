//! Pure source intrinsic preparation after lexical/path/hygiene resolution.
//! User and unresolved macros retain typed expansion/resolution outcomes; no
//! spelling fallback, source evaluation, formatting or query-provider effect.
use super::{macro_names::PathLookup, runtime_macro_catalog::RuntimeMacroCatalog};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroDefinition,
    facade::macros::RuntimeMacroKind,
    syntax::{
        macro_calls::SourceMacroCall, parse::ParsedFile,
        runtime_macro_arguments::RuntimeMacroArguments,
    },
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperandUse {
    Predicate,
    SharedComparison,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MessageTiming {
    Always,
    OnAssertionFailure,
}
#[derive(Debug)]
pub struct RuntimeIntrinsicPlan {
    definition: MacroDefinition,
    invocation: SourceMacroCall,
    arguments: RuntimeMacroArguments,
    operand_use: Option<OperandUse>,
    message_timing: MessageTiming,
}
/// Original-source certificate: the exact compiler intrinsic consumes syntax
/// containing no declaration, nested macro or transforming attribute producer.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuntimeNoItemProof {
    source: crate::syntax::parse::ParseKey,
    body: crate::source::manifest::BodyKey,
    invocation: crate::syntax::spans::SourceRange,
    definition: MacroDefinition,
}
impl RuntimeNoItemProof {
    pub(crate) fn body(&self) -> &crate::source::manifest::BodyKey {
        &self.body
    }
    pub(crate) fn invocation(&self) -> crate::syntax::spans::SourceRange {
        self.invocation
    }
    pub(crate) fn source(&self) -> &crate::syntax::parse::ParseKey {
        &self.source
    }
    pub(crate) fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub(crate) fn allocated_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>().checked_add(self.source.profile.semantics.capacity())
    }
}
impl crate::core::digest::StableEncode for RuntimeNoItemProof {
    const DOMAIN: &'static str = "lrr/v2/original-runtime-macro-no-item-proof";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.record(&self.source);
        e.record(&self.body);
        e.u32(self.invocation.start);
        e.u32(self.invocation.end);
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u32(u32::from(self.definition.definition_edition));
    }
}
impl RuntimeIntrinsicPlan {
    pub fn no_item_proof(
        &self,
        body: &crate::source::manifest::BodyKey,
    ) -> Option<RuntimeNoItemProof> {
        self.arguments.no_item_syntax().then(|| RuntimeNoItemProof {
            source: self.arguments.source().clone(),
            body: body.clone(),
            invocation: self.invocation.range(),
            definition: self.definition.clone(),
        })
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.invocation.allocated_bytes()?)
            .and_then(|n| n.checked_add(self.arguments.allocated_bytes().ok()?))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "runtime intrinsic plan metadata overflow".into(),
                )
            })
    }
    pub fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub fn invocation(&self) -> &SourceMacroCall {
        &self.invocation
    }
    pub fn arguments(&self) -> &RuntimeMacroArguments {
        &self.arguments
    }
    pub fn operand_use(&self) -> Option<OperandUse> {
        self.operand_use
    }
    /// Static, brace-free messages can directly use the existing guest panic op.
    /// Formatting/implicit captures remain a typed demand, never a guessed message.
    pub fn lower_static_panic(&self) -> Result<StaticPanicLowering, Diagnostic> {
        let kind = self.arguments.kind();
        let default = match kind {
            RuntimeMacroKind::Panic => "explicit panic",
            RuntimeMacroKind::Unreachable => "internal error: entered unreachable code",
            RuntimeMacroKind::Unimplemented => "not implemented",
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-RUNTIME-MACRO",
                    "static panic lowering requires its resolved panic-family definition".into(),
                ));
            }
        };
        let literal = match self.arguments.message_argument_count() {
            0 => None,
            1 => match self.arguments.literal_message() {
                Some(value) if !value.contains(['{', '}']) => Some(value),
                _ => return Ok(StaticPanicLowering::NeedsFormatting),
            },
            _ => return Ok(StaticPanicLowering::NeedsFormatting),
        };
        let prefix = if kind == RuntimeMacroKind::Panic || literal.is_none() {
            ""
        } else {
            default
        };
        let payload = literal.unwrap_or(default);
        let separator = if prefix.is_empty() { "" } else { ": " };
        let bytes = prefix
            .len()
            .checked_add(separator.len())
            .and_then(|n| n.checked_add(payload.len()))
            .filter(|n| *n <= 65_536)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "static guest panic message exceeds its owned limit".into(),
                )
            })?;
        let mut message = String::new();
        message.try_reserve_exact(bytes).map_err(|_| {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "static guest panic message allocation".into(),
            )
        })?;
        message.push_str(prefix);
        message.push_str(separator);
        message.push_str(payload);
        Ok(StaticPanicLowering::Operation(Box::new(
            crate::ir::op::Op::Panic { message },
        )))
    }
    pub fn lower_static_panic_recipe(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        next: crate::ir::pc::PcKey,
    ) -> Result<StaticPanicRecipeLowering, Diagnostic> {
        use crate::{
            ir::{
                pc::NodeId,
                recipe::{Origin, RecipeDraft, RecipeExit},
                validate::verify,
            },
            syntax::body_index::{ExpressionKind, NodeRole},
        };
        let invalid = |message: &str| Diagnostic::new("LRR-E-RUNTIME-MACRO", message.into());
        self.arguments
            .require_source(&source.selection().outline.parse)?;
        if &key.body != body.body() || key.phase != 0 {
            return Err(invalid(
                "static panic recipe requires its body and entry phase",
            ));
        }
        let handle = source
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|handle| handle.key() == key.body)
            .ok_or_else(|| invalid("static panic body is absent from source publication"))?;
        handle.require_entry()?;
        let node = body
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("static panic node is absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro) {
            return Err(invalid(
                "static panic recipe requires its macro expression node",
            ));
        }
        let start = handle
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| invalid("static panic source offset overflow"))?;
        let end = handle
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| invalid("static panic source offset overflow"))?;
        if (crate::syntax::spans::SourceRange { start, end }) != self.invocation.range()
            || !handle.range.contains(self.invocation.range())
        {
            return Err(invalid(
                "static panic node differs from prepared invocation",
            ));
        }
        let StaticPanicLowering::Operation(operation) = self.lower_static_panic()? else {
            return Ok(StaticPanicRecipeLowering::NeedsFormatting);
        };
        let recipe = verify(
            crate::frontend::thunks::with_temporary_scope_entry(
                body,
                RecipeDraft {
                    key: key.clone(),
                    origin: Origin {
                        span: body.span(NodeId(key.node))?,
                        expansion_ancestry: vec![],
                    },
                    operations: vec![*operation],
                    exits: vec![RecipeExit::Next(next)],
                    dependencies: vec![],
                },
            )?,
            body,
        )?;
        Ok(StaticPanicRecipeLowering::Recipe(Box::new(recipe)))
    }
    /// Literal and admitted source-call predicates use the ordinary VM machinery.
    /// Failure message demand is a separate reached phase, never a success recipe.
    pub fn lower_assertion_recipe(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        next: crate::ir::pc::PcKey,
    ) -> Result<StaticPanicRecipeLowering, Diagnostic> {
        use crate::{
            ir::{
                op::{HintRecipe, LiteralKind, LiteralRecipe, Op, PrimitiveType},
                pc::{NodeId, Phase},
                recipe::{Origin, RecipeDraft, RecipeExit},
                validate::verify,
            },
            syntax::{
                body_index::{ExpressionKind, NodeRole},
                spans::BodySpan,
            },
        };
        let invalid = |message: &str| Diagnostic::new("LRR-E-RUNTIME-MACRO", message.into());
        self.arguments
            .require_source(&source.selection().outline.parse)?;
        if self.arguments.kind() != RuntimeMacroKind::Assert || key.body != *body.body() {
            return Err(invalid(
                "literal assertion requires its resolved definition and body",
            ));
        }
        let literal = self.arguments.literal_predicate();
        let call = self.arguments.zero_arg_predicate_call();
        let binding = self.arguments.binding_predicate();
        if literal.is_none() && call.is_none() && binding.is_none() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "reached assertion expression awaits operand expansion integration".into(),
            ));
        }
        let handle = source
            .outline()
            .items
            .iter()
            .filter_map(|i| i.body.as_ref())
            .find(|handle| handle.key() == key.body)
            .ok_or_else(|| invalid("literal assertion body absent from source"))?;
        handle.require_entry()?;
        let node = body
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("literal assertion node absent"))?;
        let start = handle
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| invalid("literal assertion range overflow"))?;
        let end = handle
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| invalid("literal assertion range overflow"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro)
            || (crate::syntax::spans::SourceRange { start, end }) != self.invocation.range()
        {
            return Err(invalid("literal assertion source invocation differs"));
        }
        let condition = node.runtime_temporaries[0]
            .ok_or_else(|| invalid("literal assertion temporary absent"))?;
        let destination = node
            .value
            .ok_or_else(|| invalid("literal assertion value absent"))?;
        let id = NodeId(key.node);
        let (operations, exit) = match Phase::from_code(key.phase) {
            Phase::Entry if call.is_some() => {
                let name = call.ok_or_else(|| invalid("assertion predicate call absent"))?;
                let site = crate::ir::recipe::CallSiteId(key.node);
                (
                    vec![
                        Op::SetHint {
                            child: condition,
                            hint: HintRecipe::Primitive(PrimitiveType::Bool),
                        },
                        Op::CallSeal {
                            site,
                            name: name.into(),
                            explicit: vec![],
                            arguments: vec![],
                        },
                    ],
                    RecipeExit::Call {
                        site,
                        resume: body.pc(id, Phase::Operand(0))?,
                        result: condition,
                    },
                )
            }
            Phase::Operand(0) if call.is_some() => (
                vec![Op::Unit {
                    destination: node.runtime_temporaries[1]
                        .ok_or_else(|| invalid("assertion branch phase temporary absent"))?,
                }],
                RecipeExit::Branch {
                    condition,
                    yes: body.pc(id, Phase::Exit)?,
                    no: body.pc(id, Phase::Commit)?,
                },
            ),
            Phase::Entry if binding.is_some() => (
                vec![source.runtime_macro_binding_read(
                    handle,
                    body,
                    key,
                    binding.ok_or_else(|| invalid("assertion binding absent"))?,
                    condition,
                )?],
                RecipeExit::Branch {
                    condition,
                    yes: body.pc(id, Phase::Exit)?,
                    no: body.pc(id, Phase::Commit)?,
                },
            ),
            Phase::Entry => {
                let operand = self
                    .arguments
                    .operands()
                    .first()
                    .ok_or_else(|| invalid("literal assertion operand absent"))?;
                let relative_start = operand
                    .start
                    .checked_sub(handle.range.start)
                    .ok_or_else(|| invalid("literal assertion operand precedes body"))?;
                let relative_end = operand
                    .end
                    .checked_sub(handle.range.start)
                    .ok_or_else(|| invalid("literal assertion operand precedes body"))?;
                (
                    vec![Op::Literal {
                        literal: LiteralRecipe {
                            decoded: None,
                            kind: LiteralKind::Bool,
                            spelling: literal
                                .ok_or_else(|| invalid("assertion literal absent"))?
                                .into(),
                            source: BodySpan {
                                body_key: key.body.clone(),
                                relative_start,
                                relative_end,
                            },
                        },
                        hint: HintRecipe::Primitive(PrimitiveType::Bool),
                        destination: condition,
                    }],
                    RecipeExit::Branch {
                        condition,
                        yes: body.pc(id, Phase::Exit)?,
                        no: body.pc(id, Phase::Commit)?,
                    },
                )
            }
            Phase::Exit => (vec![Op::Unit { destination }], RecipeExit::Next(next)),
            Phase::Commit => {
                let message = if self.arguments.message_argument_count() == 0 {
                    if let Some(spelling) = literal {
                        format!("assertion failed: {spelling}")
                    } else if let Some(name) = binding {
                        let canonical = crate::expand::scopes::MacroName::parse(
                            name,
                            self.arguments.source().profile.edition,
                        )?;
                        format!(
                            "assertion failed: {}{}",
                            if name.starts_with("r#") { "r#" } else { "" },
                            canonical.as_str()
                        )
                    } else {
                        let name =
                            call.ok_or_else(|| invalid("assertion predicate call absent"))?;
                        let canonical = crate::expand::scopes::MacroName::parse(
                            name,
                            self.arguments.source().profile.edition,
                        )?;
                        format!(
                            "assertion failed: {}{}()",
                            if name.starts_with("r#") { "r#" } else { "" },
                            canonical.as_str()
                        )
                    }
                } else if self.arguments.message_argument_count() == 1 {
                    match self.arguments.literal_message() {
                        Some(value) if !value.contains(['{', '}']) => value.into(),
                        _ => return Ok(StaticPanicRecipeLowering::NeedsFormatting),
                    }
                } else {
                    return Ok(StaticPanicRecipeLowering::NeedsFormatting);
                };
                if message.len() > 65_536 {
                    return Err(Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "assertion message exceeds owned panic budget".into(),
                    ));
                }
                (
                    vec![Op::Assert {
                        condition,
                        destination,
                        message,
                    }],
                    RecipeExit::Next(next),
                )
            }
            _ => return Err(invalid("literal assertion phase invalid")),
        };
        let recipe = verify(
            crate::frontend::thunks::with_temporary_scope_entry(
                body,
                RecipeDraft {
                    key: key.clone(),
                    origin: Origin {
                        span: body.span(id)?,
                        expansion_ancestry: vec![],
                    },
                    operations,
                    exits: vec![exit],
                    dependencies: vec![],
                },
            )?,
            body,
        )?;
        Ok(StaticPanicRecipeLowering::Recipe(Box::new(recipe)))
    }
    /// Reached scalar operands execute as ordinary bounded source-PC phases.
    /// Success records the Assert opcode; failure alone demands message syntax.
    pub(crate) fn lower_scalar_assertion_recipe(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        next: crate::ir::pc::PcKey,
    ) -> Result<StaticPanicRecipeLowering, Diagnostic> {
        use crate::ir::{
            op::Op,
            pc::{NodeId, Phase},
            recipe::{Origin, RecipeDraft, RecipeExit},
            validate::verify,
        };
        let invalid = |message: &str| Diagnostic::new("LRR-E-RUNTIME-MACRO", message.into());
        self.arguments
            .require_source(&source.selection().outline.parse)?;
        let handle = source
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|handle| handle.key() == key.body)
            .ok_or_else(|| invalid("scalar assertion body absent"))?;
        handle.require_entry()?;
        let node = body
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("scalar assertion node absent"))?;
        let condition = node.runtime_temporaries[0]
            .ok_or_else(|| invalid("scalar assertion condition absent"))?;
        let destination = node
            .value
            .ok_or_else(|| invalid("scalar assertion result absent"))?;
        // Revalidate source identity for every phase, including success/failure.
        if key.body != *body.body()
            || node.role
                != crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Macro,
                )
            || handle.range.start.checked_add(node.range.start)
                != Some(self.invocation.range().start)
            || handle.range.start.checked_add(node.range.end) != Some(self.invocation.range().end)
        {
            return Err(invalid("scalar assertion source invocation differs"));
        }
        let id = NodeId(key.node);
        let (operations, exit) = match Phase::from_code(key.phase) {
            Phase::Entry | Phase::Operand(_) => {
                let operations =
                    source.runtime_macro_scalar_operations(handle, body, key, &self.arguments)?;
                let ordinal = match Phase::from_code(key.phase) {
                    Phase::Entry => 0,
                    Phase::Operand(ordinal) => usize::from(ordinal) + 1,
                    _ => unreachable!(),
                };
                let operation = operations
                    .get(ordinal)
                    .ok_or_else(|| invalid("scalar assertion phase outside operand plan"))?
                    .clone();
                let exit = if ordinal + 1 == operations.len() {
                    RecipeExit::Branch {
                        condition,
                        yes: body.pc(id, Phase::Exit)?,
                        no: body.pc(id, Phase::Commit)?,
                    }
                } else {
                    let following = u16::try_from(ordinal)
                        .map_err(|_| invalid("scalar assertion phase overflow"))?;
                    RecipeExit::Next(body.pc(id, Phase::Operand(following))?)
                };
                (vec![operation], exit)
            }
            Phase::Exit => (
                vec![Op::Assert {
                    condition,
                    destination,
                    message: String::new(),
                }],
                RecipeExit::Next(next),
            ),
            Phase::Commit => {
                // Comparison failure requires Debug/formatting integration; do
                // not forge a panic string or evaluate a message on success.
                if self.arguments.kind() != RuntimeMacroKind::Assert {
                    return Ok(StaticPanicRecipeLowering::NeedsFormatting);
                }
                let message = if self.arguments.message_argument_count() == 0 {
                    format!(
                        "assertion failed: {}",
                        source.runtime_assertion_predicate_text(&self.arguments)?
                    )
                } else if self.arguments.message_argument_count() == 1 {
                    match self.arguments.literal_message() {
                        Some(message) if !message.contains(['{', '}']) => message.to_string(),
                        _ => return Ok(StaticPanicRecipeLowering::NeedsFormatting),
                    }
                } else {
                    return Ok(StaticPanicRecipeLowering::NeedsFormatting);
                };
                (
                    vec![Op::Assert {
                        condition,
                        destination,
                        message,
                    }],
                    RecipeExit::Next(next),
                )
            }
        };
        let recipe = verify(
            crate::frontend::thunks::with_temporary_scope_entry(
                body,
                RecipeDraft {
                    key: key.clone(),
                    origin: Origin {
                        span: body.span(id)?,
                        expansion_ancestry: vec![],
                    },
                    operations,
                    exits: vec![exit],
                    dependencies: vec![],
                },
            )?,
            body,
        )?;
        Ok(StaticPanicRecipeLowering::Recipe(Box::new(recipe)))
    }
    pub fn message_timing(&self) -> MessageTiming {
        self.message_timing
    }
}
#[derive(Debug)]
pub enum RuntimeMacroPreparation {
    Intrinsic(Box<RuntimeIntrinsicPlan>),
    SourceExpansion {
        definition: MacroDefinition,
        invocation: SourceMacroCall,
    },
    Resolution {
        lookup: PathLookup,
        invocation: SourceMacroCall,
    },
}
pub fn prepare_runtime_macro(
    parsed: &ParsedFile,
    invocation: SourceMacroCall,
    lookup: PathLookup,
    catalog: &RuntimeMacroCatalog,
) -> Result<RuntimeMacroPreparation, Diagnostic> {
    if invocation.key() != &parsed.key() {
        return Err(Diagnostic::new(
            "LRR-E-RUNTIME-MACRO",
            "runtime macro preparation source/profile is stale".into(),
        ));
    }
    let PathLookup::Defined(definition) = lookup else {
        return Ok(RuntimeMacroPreparation::Resolution { lookup, invocation });
    };
    let Some(kind) = catalog.select(&definition)? else {
        return Ok(RuntimeMacroPreparation::SourceExpansion {
            definition,
            invocation,
        });
    };
    let arguments = parsed.runtime_macro_arguments(&invocation, kind)?;
    let (operand_use, message_timing) = match kind {
        RuntimeMacroKind::Panic
        | RuntimeMacroKind::Println
        | RuntimeMacroKind::Vec
        | RuntimeMacroKind::Unreachable
        | RuntimeMacroKind::Unimplemented => (None, MessageTiming::Always),
        RuntimeMacroKind::Assert => (
            Some(OperandUse::Predicate),
            MessageTiming::OnAssertionFailure,
        ),
        RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe => (
            Some(OperandUse::SharedComparison),
            MessageTiming::OnAssertionFailure,
        ),
    };
    Ok(RuntimeMacroPreparation::Intrinsic(Box::new(
        RuntimeIntrinsicPlan {
            definition,
            invocation,
            arguments,
            operand_use,
            message_timing,
        },
    )))
}

#[derive(Debug)]
pub enum StaticPanicLowering {
    Operation(Box<crate::ir::op::Op>),
    NeedsFormatting,
}

#[derive(Debug)]
pub enum StaticPanicRecipeLowering {
    Recipe(Box<crate::ir::recipe::VerifiedRecipe>),
    NeedsFormatting,
}

/// Verified reached expression and retained output provenance, without macro authority.
#[derive(Debug)]
pub struct PreparedDeclarativeExpression {
    pub(crate) recipe: Box<crate::ir::recipe::VerifiedRecipe>,
    pub(crate) tokens: crate::syntax::token_wire::TokenForest,
    pub(crate) origins: Vec<crate::syntax::spans::TokenOrigin>,
    pub(crate) metadata_bytes: usize,
}
impl PreparedDeclarativeExpression {
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub fn recipe(&self) -> &crate::ir::recipe::VerifiedRecipe {
        &self.recipe
    }
    pub fn tokens(&self) -> &crate::syntax::token_wire::TokenForest {
        &self.tokens
    }
    pub fn origins(&self) -> &[crate::syntax::spans::TokenOrigin] {
        &self.origins
    }
}
