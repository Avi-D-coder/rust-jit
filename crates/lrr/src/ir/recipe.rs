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
    /// Roll back the selected pattern alternative and enter its retained retry
    /// continuation. The source slot stays live and is never evaluated again.
    PatternRetry {
        owner: super::pc::NodeId,
    },
    /// The checked ordinary transaction selected this continuation from its
    /// retained source graph. No runtime syntax or type planning occurs here.
    PatternNext {
        owner: super::pc::NodeId,
    },
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
    pub(super) namespace_witness: Option<std::sync::Arc<crate::db::memo::NamespaceWitness>>,
}
impl VerifiedRecipe {
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    /// Conservative metadata owned by this recipe allocation. Inline operation
    /// Arc handles remain included; shared Product payload/control allocations
    /// are charged once through product_roots by the query/memo owner instead.
    /// metadata_bytes retains the aggregate verifier/install admission bound.
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.metadata_bytes - self.shared_product_admission_bytes
    }
    /// Each actual immutable handle owned by the flat recipe operations. Memo
    /// ownership counts handles; allocation deduplication belongs to its caller.
    pub(crate) fn product_roots(
        &self,
    ) -> impl Iterator<Item = &std::sync::Arc<crate::syntax::pattern_product::PatternProductPlan>>
    {
        self.recipe.operations.iter().filter_map(|op| match op {
            Op::Pattern {
                operation: super::op::PatternOperation::Decision { plan, .. },
            } => Some(plan),
            _ => None,
        })
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
            .filter(|bytes| *bytes <= 64 * 1024 * 1024)
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
    pub(crate) fn revalidate_namespace(
        &self,
        db: &crate::db::memo::Database,
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        if let Some(witness) = &self.namespace_witness {
            witness.require_current(db)?;
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
fn hint(e: &mut StableEncoder, h: &HintRecipe) {
    match h {
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
    }
}
impl StableEncode for Recipe {
    const DOMAIN: &'static str = "lrr/v12/lrir-recipe";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        span(e, &self.origin.span);
        e.sequence(&self.origin.expansion_ancestry, |e, d| e.digest(*d));
        e.sequence(&self.operations, |e, op| match op {
            Op::Pattern { operation } => {
                use super::op::PatternOperation;
                e.u32(45);
                match operation {
                    PatternOperation::Decision {
                        owner,
                        plan,
                        transition,
                    } => {
                        use super::op::PatternDecision;
                        e.u32(5);
                        e.u32(owner.0);
                        e.digest(plan.identity());
                        match transition {
                            PatternDecision::Begin {
                                source,
                                matched,
                                unmatched,
                                unmatched_inactive,
                            } => {
                                e.u32(0);
                                e.u32(source.0);
                                e.record(matched);
                                e.record(unmatched);
                                e.record(unmatched_inactive);
                            }
                            PatternDecision::Observe => e.u32(1),
                            PatternDecision::Enter => e.u32(2),
                            PatternDecision::Success => e.u32(3),
                            PatternDecision::Fail { node } => {
                                e.u32(4);
                                e.option(node, |e, node| e.u32(node.0));
                            }
                            PatternDecision::Cleanup => e.u32(5),
                            PatternDecision::CheckTuple { place, arity } => {
                                e.u32(6);
                                e.u32(place.0);
                                e.u32(*arity);
                            }
                            PatternDecision::StartSource => e.u32(7),
                            PatternDecision::SourceStep => e.u32(8),
                            PatternDecision::EnterAttempt { scope } => {
                                e.u32(9);
                                e.u32(scope.0);
                            }
                            PatternDecision::Test {
                                result,
                                first,
                                upper_pending,
                            } => {
                                e.u32(10);
                                e.u32(result.0);
                                e.bool(*first);
                                e.bool(*upper_pending);
                            }
                        }
                    }
                    PatternOperation::Begin {
                        owner,
                        alternative,
                        source,
                        retry,
                    } => {
                        e.u32(0);
                        e.u32(owner.0);
                        e.u32(alternative.0);
                        e.u32(source.0);
                        e.record(retry);
                    }
                    PatternOperation::StageBinding {
                        owner,
                        binding,
                        source,
                        writable,
                    } => {
                        e.u32(1);
                        e.u32(owner.0);
                        e.u32(binding.0);
                        e.u32(source.0);
                        e.bool(*writable);
                    }
                    PatternOperation::Preview { owner } => {
                        e.u32(2);
                        e.u32(owner.0);
                    }
                    PatternOperation::Finish { owner, matched } => {
                        e.u32(3);
                        e.u32(owner.0);
                        e.option(matched, |e, slot| e.u32(slot.0));
                    }
                    PatternOperation::Rollback { owner } => {
                        e.u32(4);
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
            Op::ConstructFinish { guard, destination } => {
                e.u32(29);
                e.u32(guard.0);
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
                    super::op::DecodedLiteral::Char(value) => {
                        e.u32(1);
                        e.u32(*value as u32);
                    }
                    super::op::DecodedLiteral::Byte(value) => {
                        e.u32(2);
                        e.u32(u32::from(*value));
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
            RecipeExit::PatternRetry { owner } => {
                e.u32(8);
                e.u32(owner.0);
            }
            RecipeExit::PatternNext { owner } => {
                e.u32(9);
                e.u32(owner.0);
            }
        }
        e.sequence(&self.dependencies, |e, d| e.record(d));
    }
}

#[cfg(test)]
#[path = "definition_numeric_wire_tests.rs"]
mod definition_numeric_wire_tests;
#[cfg(test)]
#[path = "pattern_wire_tests.rs"]
mod pattern_wire_tests;
