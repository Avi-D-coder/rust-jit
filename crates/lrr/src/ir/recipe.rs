//! Immutable symbolic recipes admitted only through structural verification.
use super::{
    op::{DiagnosticTemplate, HintRecipe, Op},
    pc::{PcKey, SlotId},
};
pub use crate::db::dependency::DependencyStamp as FactDependency;
use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    syntax::{body_index::LabelId, spans::BodySpan},
};
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub struct Origin {
    pub span: BodySpan,
    pub expansion_ancestry: Vec<Digest>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ControlRecipe {
    Return {
        value: SlotId,
    },
    Break {
        target: LabelId,
        value: Option<SlotId>,
        resume: PcKey,
    },
    Continue {
        target: LabelId,
    },
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CallSiteId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct EffectSiteId(pub u32);
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecipeExit {
    Backedge(PcKey),
    Call {
        site: CallSiteId,
        resume: PcKey,
        result: SlotId,
    },
    Effect {
        site: EffectSiteId,
        resume: PcKey,
        result: SlotId,
    },
    Next(PcKey),
    Branch {
        condition: SlotId,
        yes: PcKey,
        no: PcKey,
    },
    Control(ControlRecipe),
    Fault(DiagnosticTemplate),
    /// Reached exhaustion of a match: not compile-time exhaustiveness checking.
    MatchNoArm,
}
impl RecipeExit {
    pub fn requires_dispatch_boundary(&self) -> bool {
        !matches!(self, Self::Next(_))
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Recipe {
    pub key: PcKey,
    pub origin: Origin,
    pub operations: Box<[Op]>,
    pub exit: RecipeExit,
    pub dependencies: Box<[FactDependency]>,
}
#[derive(Debug, Clone)]
pub struct RecipeDraft {
    pub key: PcKey,
    pub origin: Origin,
    pub operations: Vec<Op>,
    pub exits: Vec<RecipeExit>,
    pub dependencies: Vec<FactDependency>,
}
/// No mutable accessor or unchecked public constructor. Runtime installation uses this witness.
#[derive(Debug, Clone)]
pub struct VerifiedRecipe {
    pub(super) recipe: Recipe,
    pub(super) metadata_bytes: usize,
    // Included in aggregate admission, separately owned/accounted by immutable
    // Product roots in the memo layer. Never recomputed in installed execution.
    pub(super) shared_product_admission_bytes: usize,
    pub(super) shared_string_literal_admission_bytes: usize,
    pub(super) namespace_witness: Option<std::sync::Arc<crate::db::memo::NamespaceWitness>>,
    // Owned relevant input currentness only, never name/type/operation grants.
    pub(super) prepared_inputs: Option<std::sync::Arc<crate::db::memo::PreparedInputGuard>>,
    // An ordinary call consumes its immutable source/Type selection at runtime;
    // cache eviction must not cause installed execution to plan source again.
    pub(super) source_call:
        Option<crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
    // Type arguments of the call that later code decided (see
    // `InferredArguments`); retained beside the recipe like `source_call`.
    pub(super) inferred_arguments: Option<std::sync::Arc<InferredArguments>>,
}
/// Type arguments a call leaves open that later code decides (rustc's
/// inferred turbofish): the callee declaration they belong to, then each
/// decided generic parameter (its ordinal in the callee's generics, the same
/// numbering a written turbofish uses) with its closed written Type. They
/// are inputs of the recipe's prepared-input guard, not operations.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InferredArguments {
    pub definition: Digest,
    pub arguments: Vec<(u32, std::sync::Arc<super::written_type::WrittenType>)>,
}
impl VerifiedRecipe {
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }
    pub(crate) fn source_call(
        &self,
    ) -> Option<&crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments> {
        self.source_call.as_ref()
    }
    /// Cold migration check for the ordinary source-call lowering route only.
    /// The actual old joined guard is retained unchanged; the opaque publisher
    /// receipt fences its exact target replacements. No recipe executes here.
    pub(crate) fn require_call_body_transition(
        &self,
        db: &crate::db::memo::Database,
        transition: &crate::db::memo::RetainedCallBodyTransition,
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        transition.require_before_plan(db, self.call_body_transition_plan(db)?)
    }
    pub(crate) fn require_call_body_observer(
        &self,
        db: &crate::db::memo::Database,
        transition: &crate::db::memo::RetainedCallBodyTransition,
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        transition.require_observer_plan(db, self.call_body_transition_plan(db)?)
    }
    fn call_body_transition_plan(
        &self,
        db: &crate::db::memo::Database,
    ) -> Result<
        &crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments,
        crate::core::diagnostic::Diagnostic,
    > {
        let invalid = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "recipe is not the actual prepared source call being renewed".into(),
            )
        };
        let plan = self.source_call.as_ref().ok_or_else(invalid)?;
        let guard = self.prepared_inputs.as_ref().ok_or_else(invalid)?;
        if !std::sync::Arc::ptr_eq(guard, plan.guard()) {
            return Err(invalid());
        }
        if let Some(witness) = &self.namespace_witness {
            witness.require_current(db)?;
        }
        Ok(plan)
    }
    pub(crate) fn with_source_call(
        mut self,
        plan: &crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        if self.source_call.is_some()
            || plan.key().body != self.recipe.key.body
            || plan.key().node != self.recipe.key.node
            || plan.key().phase != 0
        {
            return Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "ordinary recipe source call attachment differs".into(),
            ));
        }
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(plan.metadata_bytes())
            .ok_or_else(crate::runtime::arena::resource)?;
        self.source_call = Some(plan.clone());
        Ok(self)
    }
    /// This Entry recipe with the reached Type of its own result stated
    /// first: the same `SetHint` a parent writes for a child, here for a
    /// node whose Type the common source constraints decided from its later
    /// uses (`None`, `W::new()`). Verified again as an ordinary recipe; the
    /// cold attachments of the original are kept.
    pub(crate) fn with_entry_hint(
        self,
        index: &crate::syntax::body_index::BodyIndex,
        hint: HintRecipe,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        let invalid = |message: &str| {
            crate::core::diagnostic::Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
        };
        let key = self.recipe.key.clone();
        if key.phase != super::pc::Phase::Entry.code()? {
            return Err(invalid("entry hint requires an entry phase recipe"));
        }
        let child = index
            .nodes()
            .get(key.node as usize)
            .and_then(|node| node.value)
            .ok_or_else(|| invalid("entry hint node has no value slot"))?;
        let mut operations = Vec::with_capacity(self.recipe.operations.len() + 1);
        operations.push(Op::SetHint { child, hint });
        operations.extend(self.recipe.operations.iter().cloned());
        let draft = RecipeDraft {
            key,
            origin: self.recipe.origin.clone(),
            operations,
            exits: vec![self.recipe.exit.clone()],
            dependencies: self.recipe.dependencies.to_vec(),
        };
        let mut verified = super::validate::verify(draft, index)?;
        verified.namespace_witness = self.namespace_witness;
        verified.prepared_inputs = self.prepared_inputs;
        verified.source_call = self.source_call;
        verified.inferred_arguments = self.inferred_arguments;
        // Everything the original accounted beyond its verified core stays
        // accounted: keep the larger bound.
        verified.metadata_bytes = verified.metadata_bytes.max(self.metadata_bytes);
        Ok(verified)
    }
    /// The type arguments later code decided for this recipe's call, stated
    /// as written turbofish arguments would be (the dispatcher reads them
    /// like the explicit ones; no operation carries them).
    pub(crate) fn inferred_arguments(&self) -> Option<&std::sync::Arc<InferredArguments>> {
        self.inferred_arguments.as_ref()
    }
    pub(crate) fn with_inferred_arguments(
        mut self,
        inferred: std::sync::Arc<InferredArguments>,
    ) -> Self {
        // Only a phase that prepares or seals the call reads them.
        if self
            .recipe
            .operations
            .iter()
            .any(|op| matches!(op, Op::CallPrepare { .. } | Op::CallSeal { .. }))
        {
            self.inferred_arguments = Some(inferred);
        }
        self
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    /// Conservative metadata owned by this recipe allocation. Inline operation
    /// Arc handles remain included; shared pattern-tree payloads are charged
    /// once through pattern_roots by the query/memo owner instead.
    /// metadata_bytes retains the aggregate verifier/install admission bound.
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.metadata_bytes - self.shared_product_admission_bytes
    }
    /// Each actual immutable handle owned by the flat recipe operations. Memo
    /// ownership counts handles; allocation deduplication belongs to its caller.
    pub(crate) fn pattern_roots(
        &self,
    ) -> impl Iterator<Item = &std::sync::Arc<crate::syntax::pattern_tree::PatternTree>> {
        self.recipe.operations.iter().filter_map(|op| match op {
            Op::Pattern {
                operation: super::op::PatternOperation::Test { tree, .. },
            } => Some(tree),
            _ => None,
        })
    }
    /// Actual opaque UTF-8 receipt roots owned by flat literal operations.
    pub fn string_literal_roots(
        &self,
    ) -> impl Iterator<Item = &std::sync::Arc<crate::syntax::string_literal::PreparedStringLiteral>>
    {
        self.recipe.operations.iter().filter_map(|op| match op {
            Op::Literal { literal, .. } => match &literal.decoded {
                Some(super::op::DecodedLiteral::String(proof)) => Some(proof),
                _ => None,
            },
            _ => None,
        })
    }
    /// Included conservatively in current recipe exclusive accounting until
    /// the memo owner atomically takes over actual Arc allocation/pin charging.
    pub fn string_literal_admission_bytes(&self) -> usize {
        self.shared_string_literal_admission_bytes
    }
    /// Attach only after genuine source admission. Recording currentness does
    /// not authorize a pattern, macro, name lookup or any guest operation.
    /// A stale recipe can never be refreshed by simply recapturing its world.
    pub(crate) fn with_namespace_witness(
        mut self,
        db: &crate::db::memo::Database,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        if self.namespace_witness.is_some() {
            self.revalidate_namespace(db)?;
            return Ok(self);
        }
        let witness = db.capture_namespace_witness()?;
        let bytes = self
            .metadata_bytes
            .checked_add(witness.metadata_bytes())
            .ok_or_else(|| {
                crate::core::diagnostic::Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "recipe namespace witness exceeds metadata limit".into(),
                )
            })?;
        self.namespace_witness = Some(std::sync::Arc::new(witness));
        self.metadata_bytes = bytes;
        Ok(self)
    }
    /// Cold attachment only. A new guard cannot replace or refresh a retained
    /// one, and a relevant guard does not replace the legacy namespace witness.
    #[cfg(test)]
    pub(crate) fn with_prepared_inputs(
        self,
        inputs: crate::db::memo::PreparedInputGuard,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        self.with_shared_prepared_inputs(std::sync::Arc::new(inputs))
    }
    /// Phase preparation shares the one genuine cold query guard. Neither
    /// attachment nor recipe fingerprinting walks its input closure again.
    pub(crate) fn with_shared_prepared_inputs(
        mut self,
        inputs: std::sync::Arc<crate::db::memo::PreparedInputGuard>,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        if self.prepared_inputs.is_some() {
            return Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-INTERNAL-SNAPSHOT",
                "recipe already retains prepared input currentness".into(),
            ));
        }
        let bytes = self
            .metadata_bytes
            .checked_add(inputs.metadata_bytes())
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(|| {
                crate::core::diagnostic::Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "recipe prepared inputs exceed metadata limit".into(),
                )
            })?;
        self.prepared_inputs = Some(inputs);
        self.metadata_bytes = bytes;
        Ok(self)
    }
    /// Cached once at cold guard capture; memo fingerprinting never rehashes the
    /// input closure. Raw Recipe wire data remains separate from current use.
    pub(crate) fn prepared_inputs_fingerprint(&self) -> Option<Digest> {
        self.prepared_inputs
            .as_ref()
            .map(|inputs| inputs.fingerprint())
    }
    pub(crate) fn revalidate_namespace(
        &self,
        db: &crate::db::memo::Database,
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        if let Some(witness) = &self.namespace_witness {
            witness.require_current(db)?;
        }
        if let Some(inputs) = &self.prepared_inputs {
            inputs.require_current(db)?;
        }
        Ok(())
    }
}
fn span(e: &mut StableEncoder, s: &BodySpan) {
    e.record(&s.body_key);
    e.u32(s.relative_start);
    e.u32(s.relative_end);
}
fn definition_numeric_context(e: &mut StableEncoder, context: super::op::DefinitionNumericContext) {
    match context {
        super::op::DefinitionNumericContext::Parameter(ordinal) => {
            e.u32(0);
            e.u32(ordinal);
        }
        super::op::DefinitionNumericContext::Result => e.u32(1),
        super::op::DefinitionNumericContext::ResultOrDefault { floating } => {
            e.u32(2);
            e.bool(floating);
        }
    }
}
fn assign_kind(e: &mut StableEncoder, kind: &super::op::AssignKind) {
    match kind {
        super::op::AssignKind::Primitive { left, right } => {
            e.u32(0);
            e.u32(left.tag());
            e.u32(right.tag());
        }
        super::op::AssignKind::DefinitionNumeric { context, right } => {
            e.u32(1);
            definition_numeric_context(e, *context);
            e.option(right, |e, primitive| e.u32(primitive.tag()));
        }
    }
}
impl StableEncode for super::op::RelatedNumericPlan {
    const DOMAIN: &'static str = "lrr/v1/related-numeric-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.bool(self.floating);
        e.sequence(&self.sources, |e, source| {
            e.u32(source.node.0);
            e.sequence(&source.projections, |e, step| {
                use super::op::NumericTypeProjection as P;
                match step {
                    P::Deref => e.u8(0),
                    P::AutoDeref => e.u8(1),
                    P::TupleField(field) => {
                        e.u8(2);
                        e.u32(*field);
                    }
                    P::Element => e.u8(3),
                    P::TypeArgument(position) => {
                        e.u8(4);
                        e.u32(*position);
                    }
                }
            });
        });
    }
}
fn hint(e: &mut StableEncoder, h: &HintRecipe) {
    match h {
        HintRecipe::RelatedNumeric(plan) => {
            e.u32(12);
            e.record(plan.as_ref());
        }
        HintRecipe::DefinitionResult => e.u32(8),
        HintRecipe::DefinitionParameter(ordinal) => {
            e.u32(7);
            e.u32(*ordinal);
        }
        HintRecipe::DefinitionNumeric(context) => {
            e.u32(6);
            definition_numeric_context(e, *context);
        }
        HintRecipe::BorrowTarget { reference } => {
            e.u32(5);
            e.u32(reference.0);
        }
        HintRecipe::ArrayPrimitive { element, len } => {
            e.u32(4);
            e.u32(element.tag());
            e.u32(*len);
        }
        HintRecipe::None => e.u32(0),
        HintRecipe::Parameter(key) => {
            e.u32(2);
            e.digest(key.definition);
            e.u32(key.ordinal);
        }
        HintRecipe::Primitive(p) => {
            e.u32(1);
            e.u32(p.tag());
        }
        HintRecipe::Reference {
            region,
            mutable,
            target,
        } => {
            e.u32(3);
            e.record(region);
            e.bool(*mutable);
            e.u32(target.tag());
        }
        HintRecipe::Written(written) => {
            e.u32(9);
            e.record(written.as_ref());
        }
        HintRecipe::DefinitionWrittenResult => e.u32(10),
        HintRecipe::IndexOperand { base } => {
            e.u32(11);
            e.u32(base.0);
        }
    }
}
impl StableEncode for Recipe {
    const DOMAIN: &'static str = "lrr/v16/lrir-recipe";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        span(e, &self.origin.span);
        e.sequence(&self.origin.expansion_ancestry, |e, d| e.digest(*d));
        e.sequence(&self.operations, |e, op| match op {
            Op::Pattern { operation } => {
                use super::op::PatternOperation;
                e.u32(45);
                match operation {
                    PatternOperation::Open { owner, source } => {
                        e.u32(10);
                        e.u32(owner.0);
                        e.u32(source.0);
                    }
                    PatternOperation::Test {
                        owner,
                        tree,
                        result,
                        retry,
                    } => {
                        e.u32(11);
                        e.u32(owner.0);
                        e.digest(tree.identity());
                        e.u32(result.0);
                        e.bool(*retry);
                    }
                    PatternOperation::Expose { owner } => {
                        e.u32(12);
                        e.u32(owner.0);
                    }
                    PatternOperation::Conceal { owner } => {
                        e.u32(13);
                        e.u32(owner.0);
                    }
                    PatternOperation::Commit { owner } => {
                        e.u32(14);
                        e.u32(owner.0);
                    }
                    PatternOperation::Close { owner } => {
                        e.u32(15);
                        e.u32(owner.0);
                    }
                }
            }
            Op::SourceConstant {
                constant,
                destination,
            } => {
                e.u32(u32::from(super::op::SOURCE_CONSTANT_OPCODE));
                e.record(constant.as_ref());
                e.u32(destination.0);
            }
            Op::ValuePlace {
                source,
                destination,
            } => {
                e.u32(38);
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::PromotedPlace {
                source,
                destination,
            } => {
                e.u32(u32::from(super::op::PROMOTED_PLACE_OPCODE));
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::PlaceTransfer {
                source,
                destination,
            } => {
                e.u32(39);
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::ProjectField { base, destination } => {
                e.u32(37);
                e.u32(base.0);
                e.u32(destination.0);
            }
            Op::ProjectTuple {
                base,
                field,
                destination,
            } => {
                e.u32(35);
                e.u32(base.0);
                e.u32(*field);
                e.u32(destination.0);
            }
            Op::ProjectIndex {
                base,
                index,
                destination,
            } => {
                e.u32(36);
                e.u32(base.0);
                e.u32(index.0);
                e.u32(destination.0);
            }
            Op::DerefPlace {
                reference,
                destination,
            } => {
                e.u32(32);
                e.u32(reference.0);
                e.u32(destination.0);
            }
            Op::ReadPlace { place, destination } => {
                e.u32(33);
                e.u32(place.0);
                e.u32(destination.0);
            }
            Op::AssignPlace {
                place,
                source,
                destination,
            } => {
                e.u32(34);
                e.u32(place.0);
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::ParameterPlace {
                ordinal,
                destination,
            } => {
                e.u32(44);
                e.u32(*ordinal);
                e.u32(destination.0);
            }
            Op::MakePlace {
                binding,
                destination,
            } => {
                e.u32(30);
                e.u32(binding.0);
                e.u32(destination.0);
            }
            Op::Borrow {
                place,
                access,
                destination,
            } => {
                e.u32(31);
                e.u32(place.0);
                e.u32(match access {
                    super::op::BorrowAccess::Shared => 0,
                    super::op::BorrowAccess::Mutable => 1,
                });
                e.u32(destination.0);
            }
            Op::ConstructStart { guard } => {
                e.u32(27);
                e.u32(guard.0);
            }
            Op::ConstructField {
                guard,
                field,
                source,
            } => {
                e.u32(28);
                e.u32(guard.0);
                e.u32(*field);
                e.u32(source.0);
            }
            Op::ConstructRest { guard, base } => {
                e.u32(u32::from(super::op::CONSTRUCT_REST_OPCODE));
                e.u32(guard.0);
                e.u32(base.0);
            }
            Op::ConstructFinish { guard, destination } => {
                e.u32(29);
                e.u32(guard.0);
                e.u32(destination.0);
            }
            Op::ValuePath { path, destination } => {
                e.u32(u32::from(super::op::VALUE_PATH_OPCODE));
                e.string(path);
                e.u32(destination.0);
            }
            Op::MakeClosure {
                captures,
                parameters,
                destination,
            } => {
                e.u32(u32::from(super::op::MAKE_CLOSURE_OPCODE));
                e.sequence(captures, |e, capture| {
                    match capture.source {
                        super::op::CaptureSource::Binding(binding) => {
                            e.u8(0);
                            e.u32(binding.0);
                        }
                        super::op::CaptureSource::Parameter(ordinal) => {
                            e.u8(1);
                            e.u32(ordinal);
                        }
                    }
                    e.u32(match capture.mode {
                        super::op::CaptureMode::Shared => 0,
                        super::op::CaptureMode::Mutable => 1,
                        super::op::CaptureMode::Value => 2,
                        super::op::CaptureMode::Consumed => 3,
                    });
                });
                e.sequence(parameters, |e, parameter| {
                    e.bool(parameter.mutable);
                    e.option(&parameter.hint, |e, hint| e.u32(*hint as u32));
                });
                e.u32(destination.0);
            }
            Op::PrimitiveFloatingConstant {
                constant,
                destination,
            } => {
                e.u32(41);
                e.record(constant.as_ref());
                e.u32(destination.0);
            }
            Op::Literal {
                literal,
                hint: h,
                destination,
            } => {
                e.u32(0);
                e.u32(literal.kind.tag());
                e.option(&literal.decoded, |e, decoded| match decoded {
                    super::op::DecodedLiteral::Integer {
                        negative,
                        radix,
                        magnitude,
                        suffix,
                    } => {
                        e.u32(4);
                        e.u32(super::op::DECODED_INTEGER_SCHEMA);
                        e.bool(*negative);
                        e.u8(*radix);
                        e.u64(*magnitude as u64);
                        e.u64((*magnitude >> 64) as u64);
                        e.option(suffix, |e, ty| e.u32(ty.tag()));
                    }
                    super::op::DecodedLiteral::InvalidInteger {
                        negative,
                        radix,
                        suffix,
                        reason,
                    } => {
                        e.u32(5);
                        e.u32(super::op::DECODED_INTEGER_SCHEMA);
                        e.bool(*negative);
                        e.u8(*radix);
                        e.option(suffix, |e, ty| e.u32(ty.tag()));
                        e.u32(reason.tag());
                    }
                    super::op::DecodedLiteral::String(proof) => {
                        e.u32(6);
                        e.record(proof.as_ref());
                    }
                    super::op::DecodedLiteral::Char(value) => {
                        e.u32(1);
                        e.u32(*value as u32);
                    }
                    super::op::DecodedLiteral::Byte(value) => {
                        e.u32(2);
                        e.u32(u32::from(*value));
                    }
                    super::op::DecodedLiteral::ByteString(bytes) => {
                        e.u32(7);
                        e.bytes(bytes);
                    }
                    super::op::DecodedLiteral::Float { decimal, suffix } => {
                        e.u32(3);
                        e.string(decimal);
                        e.option(suffix, |e, ty| e.u32(ty.tag()));
                    }
                });
                e.string(&literal.spelling);
                span(e, &literal.source);
                hint(e, h);
                e.u32(destination.0);
            }
            Op::Unit { destination } => {
                e.u32(1);
                e.u32(destination.0);
            }
            Op::SetHint { child, hint: h } => {
                e.u32(2);
                e.u32(child.0);
                hint(e, h);
            }
            Op::ResultTransfer {
                source,
                destination,
            } => {
                e.u32(3);
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::ScopeEnter { scope } => {
                e.u32(4);
                e.u32(scope.0);
            }
            Op::ScopeLeave { scope } => {
                e.u32(5);
                e.u32(scope.0);
            }
            Op::CallPrepare {
                site,
                name,
                explicit,
                arguments,
            } => {
                e.u32(22);
                e.u32(site.0);
                e.string(name);
                e.sequence(explicit, |e, ty| e.string(ty));
                e.sequence(arguments, |e, slot| e.u32(slot.0));
            }
            Op::IteratorCall {
                call,
                source,
                destination,
            } => {
                e.u32(65);
                e.u32(call.tag());
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::ArrayRepeatElement {
                guard,
                source,
                count,
                more,
                initialize,
            } => {
                e.u32(guard.0);
                e.u32(source.0);
                e.u32(count.0);
                e.u32(more.0);
                e.bool(*initialize);
            }
            Op::ArrayRepeat {
                source,
                count,
                destination,
            } => {
                e.u32(63);
                e.u32(source.0);
                e.u32(count.0);
                e.u32(destination.0);
            }
            Op::CompoundAssignPlace {
                operator,
                place,
                right,
                destination,
            } => {
                e.u32(62);
                e.u32(*operator as u32);
                e.u32(place.0);
                e.u32(right.0);
                e.u32(destination.0);
            }
            Op::ComparisonOperand {
                place,
                read,
                destination,
            } => {
                e.u32(64);
                e.u32(place.0);
                match read {
                    crate::ir::op::OperandRead::Local(binding) => {
                        e.u32(0);
                        e.u32(binding.0);
                    }
                    crate::ir::op::OperandRead::Parameter(ordinal) => {
                        e.u32(1);
                        e.u32(*ordinal);
                    }
                    crate::ir::op::OperandRead::Place => e.u32(2),
                }
                e.u32(destination.0);
            }
            Op::CalleeDeref { place } => {
                e.u32(68);
                e.u32(place.0);
            }
            Op::MethodReceiver {
                site,
                place,
                scratch,
                destination,
            } => {
                e.u32(61);
                e.u32(site.0);
                e.u32(place.0);
                e.u32(scratch.0);
                e.u32(destination.0);
            }
            Op::FormatArguments {
                template,
                arguments,
                destination,
            } => {
                e.u32(60);
                template.encode(e);
                e.sequence(arguments, |e, slot| e.u32(slot.0));
                e.u32(destination.0);
            }
            Op::Assert {
                condition,
                destination,
                message,
            } => {
                e.u32(26);
                e.u32(condition.0);
                e.u32(destination.0);
                e.string(message);
            }
            Op::Declaration => e.u32(24),
            Op::Panic { message } => {
                e.u32(25);
                e.string(message);
            }
            Op::CallAdvance { site, argument } => {
                e.u32(23);
                e.u32(site.0);
                e.u32(*argument);
            }
            Op::CheckParameters { count } => {
                e.u32(20);
                e.u32(*count);
            }
            Op::ReadParameter {
                ordinal,
                destination,
            } => {
                e.u32(21);
                e.u32(*ordinal);
                e.u32(destination.0);
            }
            Op::CallSeal {
                site,
                name,
                explicit,
                arguments,
            } => {
                e.u32(19);
                e.u32(site.0);
                e.string(name);
                e.sequence(explicit, |e, ty| e.string(ty));
                e.sequence(arguments, |e, slot| e.u32(slot.0));
            }
            Op::CheckUnit { source } => {
                e.u32(18);
                e.u32(source.0);
            }
            Op::BranchHint {
                parent,
                condition,
                yes,
                no,
                boolean,
            } => {
                e.u32(17);
                e.u32(parent.0);
                e.u32(condition.0);
                e.option(yes, |e, s| e.u32(s.0));
                e.option(no, |e, s| e.u32(s.0));
                e.bool(*boolean);
            }
            Op::DeclareLocal {
                binding,
                hint: h,
                writable,
            } => {
                e.u32(14);
                e.u32(binding.0);
                hint(e, h);
                e.bool(*writable);
            }
            Op::CompoundHint {
                binding,
                child,
                kind,
            } => {
                e.u32(40);
                e.u32(binding.0);
                e.u32(child.0);
                assign_kind(e, kind);
            }
            Op::LocalHint { binding, child } => {
                e.u32(15);
                e.u32(binding.0);
                e.u32(child.0);
            }
            Op::AssignLocal {
                binding,
                source,
                destination,
            } => {
                e.u32(16);
                e.u32(binding.0);
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::BindLocal {
                binding,
                source,
                hint: h,
                writable,
            } => {
                e.u32(7);
                e.u32(binding.0);
                e.u32(source.0);
                hint(e, h);
                e.bool(*writable);
            }
            Op::ReadLocal {
                binding,
                destination,
            } => {
                e.u32(8);
                e.u32(binding.0);
                e.u32(destination.0);
            }
            Op::Discard { source } => {
                e.u32(9);
                e.u32(source.0);
            }
            Op::NumericBinary {
                operator,
                left,
                right,
                destination,
            } => {
                e.u32(10);
                e.u32(operator.tag());
                e.u32(left.0);
                e.u32(right.0);
                e.u32(destination.0);
            }
            Op::NumericUnary {
                operator,
                source,
                destination,
            } => {
                e.u32(11);
                e.u32(operator.tag());
                e.u32(source.0);
                e.u32(destination.0);
            }
            Op::PrimitiveCast {
                source,
                target,
                destination,
            } => {
                e.u32(12);
                e.u32(source.0);
                e.u32(target.tag());
                e.u32(destination.0);
            }
            Op::NumericHint {
                parent,
                child,
                left,
                operator,
            } => {
                e.u32(13);
                e.u32(parent.0);
                e.u32(child.0);
                e.option(left, |e, id| e.u32(id.0));
                e.option(operator, |e, op| e.u32(op.tag()));
            }
            Op::PointerCast {
                source,
                target,
                destination,
            } => {
                e.u32(67);
                e.u32(source.0);
                e.record(target.as_ref());
                e.u32(destination.0);
            }
            Op::Unsupported(d) => {
                e.u32(6);
                e.string(&d.message);
            }
        });
        match &self.exit {
            RecipeExit::Call {
                site,
                resume,
                result,
            } => {
                e.u32(4);
                e.u32(site.0);
                e.record(resume);
                e.u32(result.0);
            }
            RecipeExit::Effect {
                site,
                resume,
                result,
            } => {
                e.u32(5);
                e.u32(site.0);
                e.record(resume);
                e.u32(result.0);
            }
            RecipeExit::Backedge(pc) => {
                e.u32(6);
                e.record(pc);
            }
            RecipeExit::Next(pc) => {
                e.u32(0);
                e.record(pc);
            }
            RecipeExit::Branch { condition, yes, no } => {
                e.u32(1);
                e.u32(condition.0);
                e.record(yes);
                e.record(no);
            }
            RecipeExit::Control(c) => {
                e.u32(2);
                match c {
                    ControlRecipe::Return { value } => {
                        e.u32(0);
                        e.u32(value.0);
                    }
                    ControlRecipe::Break {
                        target,
                        value,
                        resume,
                    } => {
                        e.u32(1);
                        e.u32(target.0);
                        e.record(resume);
                        e.bool(value.is_some());
                        if let Some(v) = value {
                            e.u32(v.0);
                        }
                    }
                    ControlRecipe::Continue { target } => {
                        e.u32(2);
                        e.u32(target.0);
                    }
                }
            }
            RecipeExit::Fault(d) => {
                e.u32(3);
                e.string(&d.message);
            }
            RecipeExit::MatchNoArm => e.u32(7),
        }
        e.sequence(&self.dependencies, |e, d| e.record(d));
    }
}

#[cfg(test)]
#[path = "definition_numeric_wire_tests.rs"]
mod definition_numeric_wire_tests;

#[cfg(test)]
#[path = "prepared_input_recipe_tests.rs"]
mod prepared_input_recipe_tests;
