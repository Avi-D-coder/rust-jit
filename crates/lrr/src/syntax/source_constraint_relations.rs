//! Retained unsolved source relations, shared by source-constraint consumers.
//! Endpoints describe source requirements, never inferred Types or solved goals.
use super::source_constraints::{FactKind, SourceFact, SourceUse};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::{op::PrimitiveType, pc::NodeId},
    source::manifest::BodyKey,
};

/// A leaf's written syntax, not a selected numeric width or fallback answer.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceNumericLeaf {
    Integer,
    Float,
    WrittenSuffix(Option<PrimitiveType>),
}

/// Ordinary services still needed to interpret this source occurrence. A
/// boundary is an explicit hole in this first extraction, never zero relations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceRelationBoundary {
    ValueLookup,
    Pattern,
    Call,
    Method,
    LangCall,
    Return,
    Discard,
    WrittenType,
    Operator,
    Structural,
    Unrecorded,
}

#[derive(Debug, Clone)]
pub(crate) enum SourceRelationInput {
    Numeric(SourceNumericLeaf),
    /// Parentheses or the block's value operand.
    Pass(Option<NodeId>),
    /// Source result coercions, not successful equality/unification. The
    /// condition is retained, but does not share the result Type.
    If {
        condition: Option<NodeId>,
        then_value: Option<NodeId>,
        else_value: Option<NodeId>,
    },
    /// Exact original operand order: scrutinee, then pattern/body pairs.
    /// Pattern interpretation remains a common-service boundary.
    Match {
        operands: Box<[Option<NodeId>]>,
    },
    Boundary {
        kind: SourceRelationBoundary,
        operands: Box<[Option<NodeId>]>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceRelationConsumer {
    Pass,
    IfValue { has_else: bool },
    MatchValue,
    Discard,
    LetInitializer { pattern: Option<NodeId> },
    ReturnValue,
    Boundary(SourceRelationBoundary),
}

/// Every interpretation retains the original exact child/parent/ordinal edge,
/// including the elementary relations consumed without another source query.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceRelationUse {
    pub(crate) edge: SourceUse,
    pub(crate) consumer: SourceRelationConsumer,
}

#[derive(Debug, Clone)]
struct SourceRelationRow {
    input: SourceRelationInput,
    uses: Vec<SourceRelationUse>,
}

/// Prepared exactly once in the existing SourceConstraints memo. There is no
/// closed-component, inferred-answer, source-authority or root-ready accessor.
#[derive(Debug, Clone)]
pub(crate) struct SourceRelationPlan {
    body: BodyKey,
    structure: Digest,
    rows: Box<[SourceRelationRow]>,
}
impl SourceRelationPlan {
    pub(super) fn prepare(
        body: BodyKey,
        structure: Digest,
        facts: &[SourceFact],
        uses: &[SourceUse],
    ) -> Result<Self, Diagnostic> {
        let mut rows = facts
            .iter()
            .map(|fact| {
                let input = match fact.kind() {
                    FactKind::Integer => SourceRelationInput::Numeric(SourceNumericLeaf::Integer),
                    FactKind::Float => SourceRelationInput::Numeric(SourceNumericLeaf::Float),
                    FactKind::SuffixedInteger | FactKind::SuffixedFloat => {
                        SourceRelationInput::Numeric(SourceNumericLeaf::WrittenSuffix(
                            fact.suffix(),
                        ))
                    }
                    FactKind::Parenthesis | FactKind::Block => {
                        SourceRelationInput::Pass(fact.operand(0))
                    }
                    FactKind::If => SourceRelationInput::If {
                        condition: fact.operand(0),
                        then_value: fact.operand(1),
                        else_value: fact.operand(2),
                    },
                    FactKind::Match => SourceRelationInput::Match {
                        operands: fact.operands().into(),
                    },
                    _ => SourceRelationInput::Boundary {
                        kind: boundary(fact.kind()),
                        operands: fact.operands().into(),
                    },
                };
                SourceRelationRow {
                    input,
                    uses: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        for &edge in uses {
            let parent = facts.get(edge.parent.0 as usize).ok_or_else(invalid)?;
            if parent.operand(edge.ordinal as usize) != Some(edge.child) {
                return Err(invalid());
            }
            let consumer = match parent.kind() {
                FactKind::Parenthesis | FactKind::Block => SourceRelationConsumer::Pass,
                FactKind::If if edge.ordinal > 0 => SourceRelationConsumer::IfValue {
                    has_else: parent.operand(2).is_some(),
                },
                FactKind::Match if edge.ordinal > 0 && edge.ordinal % 2 == 0 => {
                    SourceRelationConsumer::MatchValue
                }
                FactKind::Discard => SourceRelationConsumer::Discard,
                FactKind::Let if edge.ordinal == 1 => SourceRelationConsumer::LetInitializer {
                    pattern: parent.operand(0),
                },
                FactKind::Return => SourceRelationConsumer::ReturnValue,
                _ => SourceRelationConsumer::Boundary(boundary(parent.kind())),
            };
            rows.get_mut(edge.child.0 as usize)
                .ok_or_else(invalid)?
                .uses
                .push(SourceRelationUse { edge, consumer });
        }
        Ok(Self {
            body,
            structure,
            rows: rows.into_boxed_slice(),
        })
    }
    pub(crate) fn require_source(
        &self,
        body: &BodyKey,
        structure: Digest,
    ) -> Result<(), Diagnostic> {
        if &self.body != body || self.structure != structure {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn inward(&self, node: NodeId) -> Option<&SourceRelationInput> {
        self.rows.get(node.0 as usize).map(|row| &row.input)
    }
    /// None means an invalid node. Some(empty) states only that the source
    /// node has no recorded consumers; it does not certify constraint closure.
    pub(crate) fn outward(&self, node: NodeId) -> Option<&[SourceRelationUse]> {
        self.rows
            .get(node.0 as usize)
            .map(|row| row.uses.as_slice())
    }
}
fn invalid() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-BODY-INDEX",
        "source relation owner or endpoint differs".into(),
    )
}
fn boundary(kind: FactKind) -> SourceRelationBoundary {
    match kind {
        FactKind::Path => SourceRelationBoundary::ValueLookup,
        FactKind::Let | FactKind::BindingPattern | FactKind::Match => {
            SourceRelationBoundary::Pattern
        }
        FactKind::Call => SourceRelationBoundary::Call,
        FactKind::MethodCall => SourceRelationBoundary::Method,
        FactKind::LangCall => SourceRelationBoundary::LangCall,
        FactKind::Return => SourceRelationBoundary::Return,
        FactKind::Discard => SourceRelationBoundary::Discard,
        FactKind::Cast => SourceRelationBoundary::WrittenType,
        FactKind::Prefix | FactKind::Binary => SourceRelationBoundary::Operator,
        FactKind::Borrow
        | FactKind::Tuple
        | FactKind::Field
        | FactKind::Array
        | FactKind::Index
        | FactKind::Record
        | FactKind::For
        | FactKind::IndexRange => SourceRelationBoundary::Structural,
        _ => SourceRelationBoundary::Unrecorded,
    }
}
impl SourceRelationBoundary {
    fn tag(self) -> u8 {
        match self {
            Self::ValueLookup => 0,
            Self::Pattern => 1,
            Self::Call => 2,
            Self::Method => 3,
            Self::LangCall => 4,
            Self::Return => 5,
            Self::Discard => 6,
            Self::WrittenType => 7,
            Self::Operator => 8,
            Self::Structural => 9,
            Self::Unrecorded => 10,
        }
    }
}
fn node(e: &mut StableEncoder, value: &Option<NodeId>) {
    e.option(value, |e, node| e.u32(node.0));
}
impl StableEncode for SourceRelationPlan {
    const DOMAIN: &'static str = "lrr/v1/source-relation-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.body);
        e.digest(self.structure);
        e.sequence(&self.rows, |e, row| {
            match &row.input {
                SourceRelationInput::Numeric(leaf) => {
                    e.u8(0);
                    match leaf {
                        SourceNumericLeaf::Integer => e.u8(0),
                        SourceNumericLeaf::Float => e.u8(1),
                        SourceNumericLeaf::WrittenSuffix(suffix) => {
                            e.u8(2);
                            e.option(suffix, |e, value| e.u32(value.tag()));
                        }
                    }
                }
                SourceRelationInput::Pass(child) => {
                    e.u8(1);
                    node(e, child);
                }
                SourceRelationInput::If {
                    condition,
                    then_value,
                    else_value,
                } => {
                    e.u8(2);
                    node(e, condition);
                    node(e, then_value);
                    node(e, else_value);
                }
                SourceRelationInput::Match { operands } => {
                    e.u8(3);
                    e.sequence(operands, node);
                }
                SourceRelationInput::Boundary { kind, operands } => {
                    e.u8(4);
                    e.u8(kind.tag());
                    e.sequence(operands, node);
                }
            }
            e.sequence(&row.uses, |e, relation| {
                e.u32(relation.edge.child.0);
                e.u32(relation.edge.parent.0);
                e.u32(relation.edge.ordinal);
                match &relation.consumer {
                    SourceRelationConsumer::Pass => e.u8(0),
                    SourceRelationConsumer::IfValue { has_else } => {
                        e.u8(1);
                        e.bool(*has_else);
                    }
                    SourceRelationConsumer::MatchValue => e.u8(2),
                    SourceRelationConsumer::Discard => e.u8(3),
                    SourceRelationConsumer::LetInitializer { pattern } => {
                        e.u8(4);
                        node(e, pattern);
                    }
                    SourceRelationConsumer::ReturnValue => e.u8(5),
                    SourceRelationConsumer::Boundary(kind) => {
                        e.u8(6);
                        e.u8(kind.tag());
                    }
                }
            });
        });
    }
}
