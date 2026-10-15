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
#[path = "primitive_float_methods.rs"]
mod primitive_methods;
pub(crate) use primitive_methods::PrimitiveFloatAdmission;
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
    /// Original-body agreement is replayed before every scalar plan/phase.
    fn scalar_assertion_body<'a>(
        &self,
        source: &'a crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
    ) -> Result<&'a crate::syntax::outline::BodyHandle, Diagnostic> {
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
        body.pc(
            crate::ir::pc::NodeId(key.node),
            crate::ir::pc::Phase::from_code(key.phase),
        )?;
        Ok(handle)
    }

    /// Replayed from the same selected intrinsic/source plan, never reconstructed
    /// from a cached opcode or a caller-supplied completeness claim.
    pub(crate) fn scalar_call_witness(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
    ) -> Result<
        Option<std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>,
        Diagnostic,
    > {
        use crate::{
            ir::pc::Phase,
            syntax::runtime_macro_scalars::{RuntimeMacroCallPhase, RuntimeScalarStep},
        };
        let handle = self.scalar_assertion_body(source, body, key)?;
        let ordinal = match Phase::from_code(key.phase) {
            Phase::Entry => 0,
            Phase::Operand(n) => usize::from(n) + 1,
            Phase::Commit | Phase::Exit => return Ok(None),
        };
        let plan = source.runtime_macro_scalar_plan(handle, body, key, &self.arguments)?;
        let (call, phase) = match plan.steps().get(ordinal) {
            Some(RuntimeScalarStep::CallPrepare(call)) => (*call, RuntimeMacroCallPhase::Prepare),
            Some(RuntimeScalarStep::CallAdvance(call, arg)) => {
                (*call, RuntimeMacroCallPhase::Advance(*arg))
            }
            Some(RuntimeScalarStep::CallSeal(call)) => (*call, RuntimeMacroCallPhase::Seal),
            Some(
                RuntimeScalarStep::Operation(_)
                | RuntimeScalarStep::PrimitiveMethod(_)
                | RuntimeScalarStep::Fault(_)
                | RuntimeScalarStep::Control(_),
            ) => return Ok(None),
            None => {
                return Err(Diagnostic::new(
                    "LRR-E-RUNTIME-MACRO",
                    "scalar call phase outside source plan".into(),
                ));
            }
        };
        let call = plan.call(call)?;
        call.require(&key.body, key, phase)?;
        if matches!(phase, RuntimeMacroCallPhase::Prepare) {
            source.require_runtime_macro_call(handle, body, key, call)?;
        }
        Ok(Some(std::sync::Arc::clone(call)))
    }

    pub(crate) fn scalar_primitive_method_witness(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
    ) -> Result<Option<crate::syntax::runtime_macro_scalars::RuntimePrimitiveMethod>, Diagnostic>
    {
        use crate::{ir::pc::Phase, syntax::runtime_macro_scalars::RuntimeScalarStep};
        let handle = self.scalar_assertion_body(source, body, key)?;
        let ordinal = match Phase::from_code(key.phase) {
            Phase::Entry => 0,
            Phase::Operand(n) => usize::from(n) + 1,
            Phase::Commit | Phase::Exit => return Ok(None),
        };
        let plan = source.runtime_macro_scalar_plan(handle, body, key, &self.arguments)?;
        match plan.steps().get(ordinal) {
            Some(RuntimeScalarStep::PrimitiveMethod(method)) => {
                method.require(&source.selection().outline.parse, key)?;
                Ok(Some(method.as_ref().clone()))
            }
            Some(_) => Ok(None),
            None => Err(Diagnostic::new(
                "LRR-E-RUNTIME-MACRO",
                "primitive method phase outside source plan".into(),
            )),
        }
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
        self.lower_scalar_assertion_with_method(source, body, key, next, None)
    }
    pub(crate) fn lower_scalar_assertion_with_method(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        body: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        next: crate::ir::pc::PcKey,
        admission: Option<&PrimitiveFloatAdmission>,
    ) -> Result<StaticPanicRecipeLowering, Diagnostic> {
        use crate::ir::{
            op::Op,
            pc::{NodeId, Phase},
            recipe::{Origin, RecipeDraft, RecipeExit},
            validate::verify,
        };
        let invalid = |message: &str| Diagnostic::new("LRR-E-RUNTIME-MACRO", message.into());
        let handle = self.scalar_assertion_body(source, body, key)?;
        let node = &body.nodes()[key.node as usize];
        let condition = node.runtime_temporaries[0]
            .ok_or_else(|| invalid("scalar assertion condition absent"))?;
        let destination = node
            .value
            .ok_or_else(|| invalid("scalar assertion result absent"))?;
        let call_witness = self.scalar_call_witness(source, body, key)?;
        let scalar_plan = if matches!(
            Phase::from_code(key.phase),
            Phase::Entry | Phase::Operand(_)
        ) {
            Some(source.runtime_macro_scalar_plan(handle, body, key, &self.arguments)?)
        } else {
            None
        };
        let id = NodeId(key.node);
        let (operations, exit) = match Phase::from_code(key.phase) {
            Phase::Entry | Phase::Operand(_) => {
                use crate::syntax::runtime_macro_scalars::{
                    RuntimeMacroCallPhase, RuntimeScalarStep,
                };
                let plan = scalar_plan
                    .as_ref()
                    .ok_or_else(|| invalid("scalar operand plan absent"))?;
                let ordinal = match Phase::from_code(key.phase) {
                    Phase::Entry => 0,
                    Phase::Operand(ordinal) => usize::from(ordinal) + 1,
                    _ => unreachable!(),
                };
                let step = plan
                    .steps()
                    .get(ordinal)
                    .ok_or_else(|| invalid("scalar assertion phase outside operand plan"))?;
                let following = || -> Result<_, Diagnostic> {
                    if ordinal + 1 == plan.steps().len() {
                        Ok(RecipeExit::Branch {
                            condition,
                            yes: body.pc(id, Phase::Exit)?,
                            no: body.pc(id, Phase::Commit)?,
                        })
                    } else {
                        let next = u16::try_from(ordinal)
                            .map_err(|_| invalid("scalar assertion phase overflow"))?;
                        Ok(RecipeExit::Next(body.pc(id, Phase::Operand(next))?))
                    }
                };
                match step {
                    RuntimeScalarStep::Operation(operation) => {
                        (vec![operation.clone()], following()?)
                    }
                    RuntimeScalarStep::PrimitiveMethod(method) => {
                        let admission = admission.ok_or_else(|| {
                            Diagnostic::new(
                                "LRR-E-TYPE-CONTEXT",
                                "primitive method requires reached receiver admission".into(),
                            )
                        })?;
                        (
                            vec![admission.operation(method, source, key)?],
                            following()?,
                        )
                    }
                    RuntimeScalarStep::Control(control) => {
                        control.require(&source.selection().outline.parse, body, key)?;
                        (vec![], control.recipe_exit())
                    }
                    RuntimeScalarStep::Fault(diagnostic) => {
                        plan.fault_span(key)
                            .ok_or_else(|| invalid("scalar reached fault source origin absent"))?;
                        // Existing LRIR has a typed reached-unsupported template.
                        // Do not erase another authority/currentness/source error.
                        if !matches!(
                            diagnostic,
                            Diagnostic {
                                code: "LRR-E-UNSUPPORTED",
                                message: _,
                                macro_origin: None,
                                rendered_macro_origin: None,
                                generated_origin: None,
                                rendered_generated_origin: None,
                                guest_stack: None,
                                rendered_guest_stack: None,
                                origin_attachment_failure: None,
                                failure_context: None,
                            }
                        ) {
                            return Err(diagnostic.clone());
                        }
                        (
                            vec![Op::Unsupported(crate::ir::op::DiagnosticTemplate {
                                message: diagnostic.message.clone(),
                            })],
                            following()?,
                        )
                    }
                    RuntimeScalarStep::CallPrepare(ordinal) => {
                        let call = plan.call(*ordinal)?;
                        call.require(&key.body, key, RuntimeMacroCallPhase::Prepare)?;
                        let next = if call.arguments().is_empty() {
                            call.seal_key()
                        } else {
                            call.argument_key(0)?
                        };
                        (
                            vec![scalar_call_operation(call, true)?],
                            RecipeExit::Next(next.clone()),
                        )
                    }
                    RuntimeScalarStep::CallAdvance(call_ordinal, argument) => {
                        let call = plan.call(*call_ordinal)?;
                        call.require(&key.body, key, RuntimeMacroCallPhase::Advance(*argument))?;
                        let advance = Op::CallAdvance {
                            site: crate::ir::recipe::CallSiteId(key.node),
                            argument: *argument,
                        };
                        let count = u32::try_from(call.arguments().len())
                            .map_err(|_| invalid("scalar call argument count overflow"))?;
                        if argument.checked_add(1) == Some(count) {
                            call.require(&key.body, key, RuntimeMacroCallPhase::Seal)?;
                            if !matches!(plan.steps().get(ordinal + 1),
                                Some(RuntimeScalarStep::CallSeal(seal)) if seal == call_ordinal)
                            {
                                return Err(invalid(
                                    "final scalar advance lacks its following seal",
                                ));
                            }
                            (
                                vec![advance, scalar_call_operation(call, false)?],
                                RecipeExit::Call {
                                    site: crate::ir::recipe::CallSiteId(key.node),
                                    result: call.result(),
                                    resume: call.resume_key().clone(),
                                },
                            )
                        } else {
                            let next = argument
                                .checked_add(1)
                                .ok_or_else(|| invalid("scalar call argument ordinal overflow"))?;
                            (
                                vec![advance],
                                RecipeExit::Next(call.argument_key(next)?.clone()),
                            )
                        }
                    }
                    RuntimeScalarStep::CallSeal(ordinal) => {
                        let call = plan.call(*ordinal)?;
                        call.require(&key.body, key, RuntimeMacroCallPhase::Seal)?;
                        if !call.arguments().is_empty() {
                            return Err(invalid(
                                "nonempty scalar seal must follow its final advance",
                            ));
                        }
                        (
                            vec![scalar_call_operation(call, false)?],
                            RecipeExit::Call {
                                site: crate::ir::recipe::CallSiteId(key.node),
                                result: call.result(),
                                resume: call.resume_key().clone(),
                            },
                        )
                    }
                }
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
        let control_span = scalar_plan.as_ref().and_then(|plan| {
            let ordinal = match Phase::from_code(key.phase) {
                Phase::Entry => 0,
                Phase::Operand(n) => usize::from(n) + 1,
                _ => return None,
            };
            match plan.steps().get(ordinal) {
                Some(crate::syntax::runtime_macro_scalars::RuntimeScalarStep::Control(control)) => {
                    Some(control.span().clone())
                }
                _ => None,
            }
        });
        let fault_span = scalar_plan
            .as_ref()
            .and_then(|plan| plan.fault_span(key))
            .cloned();
        let method_span = self
            .scalar_primitive_method_witness(source, body, key)?
            .map(|method| method.span().clone());
        let origin_span = if let Some(span) = method_span {
            span
        } else if let Some(span) = control_span {
            span
        } else if let Some(span) = fault_span {
            span
        } else if let Some(call) = &call_witness {
            call.span().clone()
        } else {
            body.span(id)?
        };
        let draft = crate::frontend::thunks::with_temporary_scope_entry(
            body,
            RecipeDraft {
                key: key.clone(),
                origin: Origin {
                    span: origin_span,
                    expansion_ancestry: vec![],
                },
                operations,
                exits: vec![exit],
                dependencies: vec![],
            },
        )?;
        let exhaustion = scalar_plan.as_ref().and_then(|plan| {
            let ordinal = match Phase::from_code(key.phase) {
                Phase::Entry => 0,
                Phase::Operand(n) => usize::from(n) + 1,
                _ => return None,
            };
            match plan.steps().get(ordinal) {
                Some(crate::syntax::runtime_macro_scalars::RuntimeScalarStep::Control(control)) => {
                    control.exhaustion()
                }
                _ => None,
            }
        });
        let recipe = if let Some(exhaustion) = exhaustion {
            crate::ir::validate::verify_runtime_scalar_match_exhaustion(draft, body, &exhaustion)?
        } else if let Some(plan) = &scalar_plan
            && plan.argument_borrow_at(key)?.is_some()
        {
            crate::ir::validate::verify_runtime_scalar_borrow(draft, body, plan)?
        } else if let Some(call) = &call_witness {
            crate::ir::validate::verify_runtime_scalar_call(draft, body, call)?
        } else {
            verify(draft, body)?
        };
        Ok(StaticPanicRecipeLowering::Recipe(Box::new(recipe)))
    }
    pub fn message_timing(&self) -> MessageTiming {
        self.message_timing
    }
}
fn scalar_call_operation(
    call: &crate::syntax::runtime_macro_scalars::RuntimeMacroCall,
    prepare: bool,
) -> Result<crate::ir::op::Op, Diagnostic> {
    fn text(value: &str) -> Result<String, Diagnostic> {
        let mut out = String::new();
        out.try_reserve_exact(value.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        out.push_str(value);
        Ok(out)
    }
    let name = text(call.name())?;
    let mut explicit = Vec::new();
    explicit
        .try_reserve_exact(call.explicit().len())
        .map_err(|_| crate::runtime::arena::resource())?;
    for ty in call.explicit() {
        explicit.push(text(ty)?);
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(call.arguments().len())
        .map_err(|_| crate::runtime::arena::resource())?;
    arguments.extend_from_slice(call.arguments());
    let site = crate::ir::recipe::CallSiteId(call.prepare_key().node);
    Ok(if prepare {
        crate::ir::op::Op::CallPrepare {
            site,
            name,
            explicit,
            arguments,
        }
    } else {
        crate::ir::op::Op::CallSeal {
            site,
            name,
            explicit,
            arguments,
        }
    })
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

#[cfg(test)]
#[path = "runtime_macro_call_recipe_tests.rs"]
mod call_recipe_tests;

#[path = "runtime_macro_generated_break.rs"]
mod generated_break;
pub(crate) use generated_break::{
    DeclarativeControlInput, GeneratedBreakAdmission, prepare_declarative_break,
};

#[cfg(test)]
#[path = "runtime_macro_control_recipe_tests.rs"]
mod control_recipe_tests;
