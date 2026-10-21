//! Pure, retained structural facts for demand-driven source type constraints.
//! This table grants no name/type authority and prepares no executable children.
use super::{
    body_index::{BodyIndex, ExpressionKind, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
    pattern_tree::{ConstructorForm, LocalItems, NameForm, PatternNode, PatternTree},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::{op::PrimitiveType, pc::NodeId},
    source::manifest::BodyKey,
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{
        self, ArithOp, BinaryOp, HasArgList, HasAttrs, HasGenericArgs, HasLoopBody, HasName, UnaryOp,
    },
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) const SOURCE_CONSTRAINT_SCHEMA: u32 = 16;
fn overflow() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "source constraint plan ordinal overflow".into(),
    )
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}

/// Syntax categories only. For example Binary does not assert primitive
/// arithmetic, and Path does not certify either a binding or an item.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FactKind {
    Opaque,
    Path,
    Integer,
    Float,
    Literal,
    Parenthesis,
    Prefix,
    Binary,
    Call,
    MethodCall,
    Let,
    Block,
    Return,
    Cast,
    Borrow,
    /// `&raw const e` / `&raw mut e`: a raw pointer to the place of operand 0;
    /// `flag` is the mutable form.
    RawBorrow,
    SuffixedInteger,
    SuffixedFloat,
    /// `expression;`: the statement discards the value. Operand 0 only.
    Discard,
    /// `if condition then else`: operands are condition, then, optional else.
    If,
    /// Builtin `format_args!` syntax: operand 0 is the template, the rest are
    /// the explicit arguments in written order.
    FormatArguments,
    /// An identifier pattern (its spelling row; its shape is the pattern
    /// engine's retained `PatternTree`).
    BindingPattern,
    /// `(a, b)`: operands are the elements.
    Tuple,
    /// `match`: operand 0 is the scrutinee, then each arm's pattern and body.
    Match,
    /// A closure expression: operand 0 is its body, whose value its calls
    /// return (not a value of the expression's own frame), then each
    /// parameter's root pattern.
    Closure,
    /// `e.0` / `e.name`: operand 0 is the base; `field` is the written
    /// position, `member` the written field name.
    Field,
    /// `[a, b]` / `[a; n]`: operands are the elements (`[a; n]` has one,
    /// and `flag`).
    Array,
    /// `base[index]`: operands are the base and the index; `flag` is an
    /// integer literal index (certainly one element, never a range).
    Index,
    /// `Path { name: e, .. }`: operands are the written field initializers
    /// (shorthand fields included), then the `..base` expression when `flag`.
    /// The written field names are the plan's `record_fields` row. A range
    /// expression (`a..b`, `a..`, `..b`) is its lang-item record.
    Record,
    /// `for PAT in HEAD BODY`: operands are the head, the pattern root (whose
    /// retained tree is the loop's `Some(PAT)` test) and the body.
    For,
    /// `base[a..b]` (a written range index): operands are the base and the
    /// range; its Type is the slice of the base's element Type.
    IndexRange,
    /// A language-item call (`a..=b` is `RangeInclusive::new(a, b)`):
    /// operands are the call's arguments in order; there is no callee
    /// operand.
    LangCall,
    /// A destructuring assignee's `_` or `..`: a part of the matched value
    /// that nothing constrains. It states no relation and cannot hold a
    /// variable open.
    Wildcard,
}
impl FactKind {
    fn tag(self) -> u8 {
        match self {
            Self::Opaque => 0,
            Self::Path => 1,
            Self::Integer => 2,
            Self::Float => 3,
            Self::Literal => 4,
            Self::Parenthesis => 5,
            Self::Prefix => 6,
            Self::Binary => 7,
            Self::Call => 8,
            Self::MethodCall => 9,
            Self::Let => 10,
            Self::Block => 11,
            Self::Return => 12,
            Self::Cast => 13,
            Self::Borrow => 14,
            Self::SuffixedInteger => 15,
            Self::SuffixedFloat => 16,
            Self::Discard => 17,
            Self::If => 18,
            Self::FormatArguments => 19,
            Self::BindingPattern => 20,
            Self::Tuple => 22,
            Self::Match => 23,
            Self::Closure => 24,
            Self::Field => 25,
            Self::Array => 26,
            Self::Index => 27,
            Self::Record => 28,
            Self::For => 29,
            Self::LangCall => 30,
            Self::IndexRange => 31,
            Self::RawBorrow => 32,
            Self::Wildcard => 33,
        }
    }
}

/// The written operator's typing family, from the actual operator token. It
/// selects which operand/result relation the Rust built-in operator rules
/// state; it never asserts that the operands are primitive.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceOperator {
    /// `+ - * / % & | ^`: built-in operands and result share one type.
    Arithmetic,
    /// `<< >>`: the result has the left operand's type; the right is independent.
    Shift,
    /// `== != < <= > >=`: built-in operands share one type; the result is bool.
    Comparison,
    /// `&& ||`: bool operands and result.
    Lazy,
    /// `=`: the right operand coerces to the left place's type; result unit.
    Assign,
    /// `+= -= *= /= %= &= |= ^=`: built-in operands share one type; result unit.
    CompoundArithmetic,
    /// `<<= >>=`: no relation between the operands; result unit.
    CompoundShift,
    /// Unary `-`: built-in operand and result share one type.
    Negate,
    /// Unary `!`: built-in operand and result share one type.
    Not,
    /// Unary `*`.
    Deref,
}
impl SourceOperator {
    fn tag(self) -> u8 {
        match self {
            Self::Arithmetic => 0,
            Self::Shift => 1,
            Self::Comparison => 2,
            Self::Lazy => 3,
            Self::Assign => 4,
            Self::CompoundArithmetic => 5,
            Self::CompoundShift => 6,
            Self::Negate => 7,
            Self::Not => 8,
            Self::Deref => 9,
        }
    }
    fn binary(op: BinaryOp) -> Self {
        match op {
            BinaryOp::ArithOp(ArithOp::Shl | ArithOp::Shr) => Self::Shift,
            BinaryOp::ArithOp(_) => Self::Arithmetic,
            BinaryOp::CmpOp(_) => Self::Comparison,
            BinaryOp::LogicOp(_) => Self::Lazy,
            BinaryOp::Assignment { op: None } => Self::Assign,
            BinaryOp::Assignment {
                op: Some(ArithOp::Shl | ArithOp::Shr),
            } => Self::CompoundShift,
            BinaryOp::Assignment { op: Some(_) } => Self::CompoundArithmetic,
        }
    }
    fn unary(op: UnaryOp) -> Self {
        match op {
            UnaryOp::Neg => Self::Negate,
            UnaryOp::Not => Self::Not,
            UnaryOp::Deref => Self::Deref,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SourceFact {
    kind: FactKind,
    // Preserve missing children and their ordinal. A Call's first edge is the
    // callee; a MethodCall's first edge is the receiver. Let uses pattern, init.
    operands: Vec<Option<NodeId>>,
    annotation: Option<SourceRange>,
    // The canonical indexed Syntax node of `annotation`, the same occurrence
    // selected by the ordinary entered Type goal.
    annotation_node: Option<NodeId>,
    // Exact operator token location, not its printed text or a type judgment.
    operator: Option<SourceRange>,
    operation: Option<SourceOperator>,
    numeric_prefix: bool,
    // A literal suffix is lexical (rustc's LitIntType/LitFloatType), not a name.
    suffix: Option<PrimitiveType>,
    // The decoded lexical length of a byte string, for its array Type.
    byte_length: Option<u64>,
    // Path and binding-pattern spelling row; candidate filter only.
    name: Option<u32>,
    // Field: the written tuple position.
    field: Option<u32>,
    // Field / MethodCall: the written field or method name's row in the
    // plan's member table.
    member: Option<u32>,
    // FormatArguments: the template may name a count argument (`$` or `.*`).
    flag: bool,
    // A closure parameter's root pattern: bound from its call argument, with
    // its written Type (if any) in `annotation`.
    closure_parameter: bool,
    // A parameter root pattern of a pattern function (E-R5-010), plain
    // identifier or not: the signature ordinal (receiver first) of the
    // argument it matches, whose declared Type is the matched value's Type.
    parameter: Option<u32>,
}

/// One structural step from a matched value's Type into a subpattern's.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum PatternStep {
    Deref,
    Field(u32),
    /// The element of an array or slice.
    Element,
    /// The `written`th field pattern of the constructor pattern whose name
    /// is the root tree's resolution `slot` (a tuple-struct item before any
    /// `..`, or a record field in written order). Which declared field that
    /// is needs the slot's ordinary namespace resolution.
    Constructor {
        slot: u32,
        written: u32,
    },
    /// A tuple position counted from the end: the `k`th item from the last
    /// one (a pattern after `..`; the arity is the matched Type's).
    TupleFromEnd(u32),
    /// A tuple-struct or variant field counted from the end of the
    /// declaration (a pattern after `..`; the arity is the declaration's).
    ConstructorFromEnd {
        slot: u32,
        from_end: u32,
    },
    /// The subsequence an array or slice pattern's `rest @ ..` binds: a
    /// sequence of the matched Type's own element Type (of another length), so
    /// it projects no component. Like any non-reference pattern step it
    /// states the references the default binding mode dereferences first.
    Rest,
}
/// The written structure of an annotation or cast target Type node: which
/// child Type a projection step selects. Named Types stay with the entered
/// Type service.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum TypeShape {
    /// `dyn Trait`: the principal trait `PathType`'s canonical Syntax node
    /// (the first trait bound, as the local annotation service reads it).
    Dyn(NodeId),
    Reference(NodeId),
    Tuple(Vec<NodeId>),
    Parenthesis(NodeId),
    /// `[T; N]` or `[T]`.
    Element(NodeId),
    /// `Base<A, ..>`: a single-segment generic application; the PathType's
    /// own node (its base selection) and each written Type argument.
    Generic(NodeId, Vec<NodeId>),
    /// `_`: a written hole the initializer's Type fills.
    Infer,
    /// `*const T` / `*mut T` (E-R5-008): mutability and the pointee node.
    RawPointer(bool, NodeId),
}

/// Where a binding occurs inside its owner's retained `PatternTree`: the
/// root pattern and the steps from the root's matched Type to the bound one.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PatternOccurrence {
    pub(crate) root: NodeId,
    pub(crate) steps: Vec<PatternStep>,
    pub(crate) by_reference: bool,
    /// `mut x`: a by-value binding even under a reference default binding mode.
    pub(crate) mutable: bool,
}

/// Derived reverse edges permit a demand to visit only its connected syntax.
/// Ordinals retain each parent's semantic position without an AST search.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct SourceUse {
    pub(crate) child: NodeId,
    pub(crate) parent: NodeId,
    pub(crate) ordinal: u32,
}
impl SourceFact {
    pub(crate) fn kind(&self) -> FactKind {
        self.kind
    }
    pub(crate) fn operands(&self) -> &[Option<NodeId>] {
        &self.operands
    }
    pub(crate) fn operand(&self, ordinal: usize) -> Option<NodeId> {
        self.operands.get(ordinal).copied().flatten()
    }
    pub(crate) fn annotation(&self) -> Option<SourceRange> {
        self.annotation
    }
    pub(crate) fn annotation_node(&self) -> Option<NodeId> {
        self.annotation_node
    }
    #[cfg(test)]
    pub(crate) fn operator(&self) -> Option<SourceRange> {
        self.operator
    }
    pub(crate) fn operation(&self) -> Option<SourceOperator> {
        self.operation
    }
    pub(crate) fn numeric_prefix(&self) -> bool {
        self.numeric_prefix
    }
    pub(crate) fn byte_length(&self) -> Option<u64> {
        self.byte_length
    }
    pub(crate) fn suffix(&self) -> Option<PrimitiveType> {
        self.suffix
    }
    pub(crate) fn field(&self) -> Option<u32> {
        self.field
    }
    /// A named field access's member-table row.
    pub(crate) fn member(&self) -> Option<u32> {
        self.member
    }
    /// An `Index` whose index is an integer literal, a `Record` with a
    /// trailing `..base` operand, a repeat `Array`, a `Call` whose callee
    /// path writes explicit generic arguments, a string `Literal`, or a
    /// `&mut` `Borrow`.
    pub(crate) fn flag(&self) -> bool {
        self.flag
    }
    pub(crate) fn counted_format(&self) -> bool {
        self.kind == FactKind::FormatArguments && self.flag
    }
    pub(crate) fn closure_parameter(&self) -> bool {
        self.closure_parameter
    }
    pub(crate) fn parameter(&self) -> Option<u32> {
        self.parameter
    }
    fn opaque() -> Self {
        Self {
            kind: FactKind::Opaque,
            operands: Vec::new(),
            annotation: None,
            annotation_node: None,
            operator: None,
            operation: None,
            numeric_prefix: false,
            suffix: None,
            byte_length: None,
            name: None,
            field: None,
            member: None,
            flag: false,
            closure_parameter: false,
            parameter: None,
        }
    }
}

/// One spelling. All three lists are candidate filters only: equal spelling
/// never establishes a binding, and hidden regions are only possible uses.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct SourceName {
    paths: Vec<NodeId>,
    bindings: Vec<NodeId>,
    hidden: Vec<NodeId>,
}
impl SourceName {
    /// Unqualified path expressions with this spelling.
    pub(crate) fn paths(&self) -> &[NodeId] {
        &self.paths
    }
    /// Identifier patterns with this spelling.
    pub(crate) fn bindings(&self) -> &[NodeId] {
        &self.bindings
    }
    /// Unindexed regions (closures, async/gen blocks, unexpanded macro calls)
    /// and format templates whose tokens contain this spelling, so they may
    /// read a same-named local without an indexed path node.
    pub(crate) fn hidden(&self) -> &[NodeId] {
        &self.hidden
    }
}

/// Owned compact facts, with no parser/index/environment Arc to deep-copy or
/// retire. Real source dependencies are retained by the existing query memo.
#[derive(Clone, Debug)]
pub(crate) enum PathTypeArguments {
    Types(Box<[NodeId]>),
    Unsupported,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct SourcePathArguments {
    pub(crate) final_segment: Option<PathTypeArguments>,
    pub(crate) owner_segment: Option<PathTypeArguments>,
}
#[derive(Debug, Clone)]
pub(crate) struct SourceConstraintPlan {
    body: BodyKey,
    structure: Digest,
    facts: Vec<SourceFact>,
    uses: Vec<SourceUse>,
    relations: super::source_constraint_relations::SourceRelationPlan,
    names: Vec<(String, SourceName)>,
    // The pattern engine's own retained tree for each `let` and match-arm
    // root pattern, and every binding occurrence's position in it.
    trees: BTreeMap<NodeId, Arc<PatternTree>>,
    pattern_names: BTreeMap<NodeId, (NodeId, u32)>,
    occurrences: BTreeMap<NodeId, PatternOccurrence>,
    type_shapes: BTreeMap<NodeId, TypeShape>,
    path_arguments: BTreeMap<NodeId, SourcePathArguments>,
    // Sorted written field names (field accesses, record expressions and
    // record patterns); facts refer to rows. Candidate spelling only.
    members: Vec<String>,
    // Each record expression's written field names, in operand order.
    record_fields: BTreeMap<NodeId, Box<[u32]>>,
    // Count of the expressions and patterns that each add one level of
    // nesting to a Type (borrows, dereferences, tuples, constructor and
    // method calls, loop heads, tuple/reference patterns): a finite Type
    // built by this body's syntax, projected to a part, has fewer levels
    // than this times the nesting of the declared Types those sites relate.
    structural: usize,
    bytes: usize,
}
impl SourceConstraintPlan {
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn structure(&self) -> Digest {
        self.structure
    }
    pub(crate) fn relations(&self) -> &super::source_constraint_relations::SourceRelationPlan {
        &self.relations
    }
    #[cfg(test)]
    pub(crate) fn facts(&self) -> &[SourceFact] {
        &self.facts
    }
    pub(crate) fn fact(&self, node: NodeId) -> Option<&SourceFact> {
        self.facts.get(node.0 as usize)
    }
    pub(crate) fn facts_len(&self) -> usize {
        self.facts.len()
    }
    pub(crate) fn uses(&self, node: NodeId) -> &[SourceUse] {
        let start = self.uses.partition_point(|edge| edge.child < node);
        let count = self.uses[start..].partition_point(|edge| edge.child == node);
        &self.uses[start..start + count]
    }
    #[cfg(test)]
    fn name_row(&self, name: &str) -> Option<&SourceName> {
        let name = name.strip_prefix("r#").unwrap_or(name);
        self.names
            .binary_search_by(|(row, _)| row.as_str().cmp(name))
            .ok()
            .map(|index| &self.names[index].1)
    }
    /// Candidate filter only. Equal spelling does not establish a binding;
    /// every selected use still requires the common hygienic Value goal.
    #[cfg(test)]
    pub(crate) fn paths_named(&self, name: &str) -> &[NodeId] {
        self.name_row(name).map_or(&[], SourceName::paths)
    }
    #[cfg(test)]
    pub(crate) fn named(&self, name: &str) -> Option<&SourceName> {
        self.name_row(name)
    }
    /// The spelling row of a Path or BindingPattern fact.
    pub(crate) fn name_of(&self, node: NodeId) -> Option<(&str, &SourceName)> {
        let row = self.fact(node)?.name?;
        self.names
            .get(row as usize)
            .map(|(name, row)| (name.as_str(), row))
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn structural_depth(&self) -> usize {
        self.structural
    }
    /// The retained pattern tree of a `let` or match-arm root pattern.
    pub(crate) fn tree(&self, root: NodeId) -> Option<&PatternTree> {
        self.trees.get(&root).map(Arc::as_ref)
    }
    /// Direct handle to the pattern engine's own canonical name row.
    pub(crate) fn pattern_name(&self, node: NodeId) -> Option<&crate::syntax::pattern_tree::PatternName> {
        let (root,slot)=self.pattern_names.get(&node)?;
        self.trees.get(root)?.names().get(*slot as usize)
    }
    pub(crate) fn path_arguments(&self, node: NodeId) -> Option<&SourcePathArguments> {
        self.path_arguments.get(&node)
    }
    /// The written structure of an annotation Type node, if structural.
    pub(crate) fn type_shape(&self, node: NodeId) -> Option<&TypeShape> {
        self.type_shapes.get(&node)
    }
    /// A binding occurrence's root pattern and its steps within that root.
    pub(crate) fn occurrence(&self, binding: NodeId) -> Option<&PatternOccurrence> {
        self.occurrences.get(&binding)
    }
    /// A written field name's member-table row.
    pub(crate) fn member_row(&self, name: &str) -> Option<u32> {
        let name = name.strip_prefix("r#").unwrap_or(name);
        self.members
            .binary_search_by(|row| row.as_str().cmp(name))
            .ok()
            .and_then(|row| u32::try_from(row).ok())
    }
    pub(crate) fn member(&self, row: u32) -> Option<&str> {
        self.members.get(row as usize).map(String::as_str)
    }
    /// A record expression's written field names (member rows), in operand order.
    pub(crate) fn record_fields(&self, record: NodeId) -> Option<&[u32]> {
        self.record_fields.get(&record).map(|row| &**row)
    }

    pub(crate) fn prepare(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
    ) -> Result<Self, Diagnostic> {
        if index.body() != &body.key() || index.profile() != &parsed.profile {
            return Err(invalid("source constraint body/profile differs"));
        }
        // Genuine entry validation once. Subsequent attachment follows retained
        // indexed locations, not a whole-file descendants search per node.
        parsed.entered_body_root(body)?;
        let mut facts = Vec::with_capacity(index.nodes().len());
        let mut names: BTreeMap<String, SourceName> = BTreeMap::new();
        let mut type_shapes = BTreeMap::new();
        let mut spelled: Vec<(NodeId, String)> = Vec::new();
        let mut written = WrittenMembers::default();
        // Destructuring assignments: their assignees spell constructor names.
        let mut assignees: Vec<ast::Expr> = Vec::new();
        for node in index.nodes() {
            if node.id.0 as usize != facts.len() {
                return Err(invalid("source constraint node order differs"));
            }
            let mut fact = SourceFact::opaque();
            let indexed = matches!(
                node.role,
                NodeRole::Expression(_)
                    | NodeRole::Statement
                    | NodeRole::Pattern
                    | NodeRole::OpaqueItem
            );
            if indexed && let Some(syntax) = parsed.entered_node_syntax(body, index, node.id)? {
                if hidden_region(node.role, &syntax) {
                    for name in hidden_names(&syntax) {
                        let row = names.entry(name).or_default();
                        if row.hidden.last() != Some(&node.id) {
                            row.hidden.push(node.id);
                        }
                    }
                } else if let Some(name) = unqualified_path_name(&syntax) {
                    let name = canonical(name.text());
                    names.entry(name.clone()).or_default().paths.push(node.id);
                    spelled.push((node.id, name));
                }
                if node.role == NodeRole::Pattern {
                    classify_pattern(&syntax, &mut fact, &mut names, &mut spelled, node.id);
                    if let Some(parameter) = syntax.parent().and_then(ast::Param::cast)
                        && parameter
                            .syntax()
                            .parent()
                            .and_then(|list| list.parent())
                            .is_some_and(|owner| ast::ClosureExpr::can_cast(owner.kind()))
                    {
                        fact.closure_parameter = true;
                        annotate(body, index, parameter.ty(), &mut fact, &mut type_shapes)?;
                    }
                    // A pattern function's parameter: the root pattern of the
                    // argument with that signature ordinal.
                    if let Some(parameter) = syntax.parent().and_then(ast::Param::cast)
                        && let Some(list) = parameter.syntax().parent().and_then(ast::ParamList::cast)
                        && index.parameter_owner().is_some_and(|owner| {
                            node.parent
                                .and_then(|param| index.nodes().get(param.0 as usize))
                                .and_then(|param| param.parent)
                                == Some(owner)
                        })
                    {
                        let position = list
                            .params()
                            .position(|candidate| candidate.syntax() == parameter.syntax())
                            .and_then(|position| u32::try_from(position).ok());
                        fact.parameter = position
                            .and_then(|position| {
                                position.checked_add(u32::from(list.self_param().is_some()))
                            });
                    }
                }
                if let Some(lhs) = ast::BinExpr::cast(syntax.clone())
                    .and_then(|binary| super::assignee::destructuring_lhs(&binary))
                {
                    assignees.push(lhs);
                }
                if !matches!(node.role, NodeRole::OpaqueItem) {
                    classify(
                        body,
                        index,
                        syntax,
                        node.id,
                        &mut fact,
                        &mut type_shapes,
                        &mut written,
                    )?;
                }
            }
            facts.push(fact);
        }
        // Deterministic sorted rows; facts retain only their row ordinal.
        let names: Vec<(String, SourceName)> = names.into_iter().collect();
        for (node, name) in spelled {
            let row = names
                .binary_search_by(|(row, _)| row.as_str().cmp(name.as_str()))
                .map_err(|_| invalid("source spelling row absent"))?;
            facts[node.0 as usize].name = Some(u32::try_from(row).map_err(|_| overflow())?);
        }
        let mut uses = Vec::new();
        for (parent, fact) in facts.iter().enumerate() {
            let parent = NodeId(u32::try_from(parent).map_err(|_| overflow())?);
            for (ordinal, child) in fact.operands.iter().enumerate() {
                if let Some(child) = child {
                    if child.0 as usize >= facts.len() {
                        return Err(invalid("source constraint edge node absent"));
                    }
                    uses.push(SourceUse {
                        child: *child,
                        parent,
                        ordinal: u32::try_from(ordinal).map_err(|_| overflow())?,
                    });
                }
            }
        }
        uses.sort_unstable();
        // Approximate retained size for the memo's metadata report. This is an
        // estimate, never an admission limit (E-R5-006).
        let bytes = std::mem::size_of::<Self>()
            .saturating_add(
                facts
                    .capacity()
                    .saturating_mul(std::mem::size_of::<SourceFact>()),
            )
            .saturating_add(facts.iter().fold(0usize, |n, fact| {
                n.saturating_add(
                    fact.operands
                        .capacity()
                        .saturating_mul(std::mem::size_of::<Option<NodeId>>()),
                )
            }))
            .saturating_add(
                uses.capacity()
                    .saturating_mul(std::mem::size_of::<SourceUse>()),
            )
            .saturating_add(names.iter().fold(0usize, |n, (name, row)| {
                n.saturating_add(std::mem::size_of::<(String, SourceName)>())
                    .saturating_add(name.capacity())
                    .saturating_add(
                        (row.paths.capacity() + row.bindings.capacity() + row.hidden.capacity())
                            .saturating_mul(std::mem::size_of::<NodeId>()),
                    )
            }));
        let bytes = written.path_arguments.values().fold(bytes, |bytes, arguments| {
            [arguments.final_segment.as_ref(), arguments.owner_segment.as_ref()]
                .into_iter().flatten().fold(bytes.saturating_add(
                    std::mem::size_of::<(NodeId, SourcePathArguments)>()).saturating_add(128),
                    |bytes, argument| match argument {
                        PathTypeArguments::Types(nodes) => bytes.saturating_add(
                            nodes.len().saturating_mul(std::mem::size_of::<NodeId>())),
                        PathTypeArguments::Unsupported => bytes,
                    })
        });
        // Owner root patterns: a `let` pattern, every match-arm pattern and a
        // `for` pattern (whose tree is the loop's `Some(PAT)` test).
        let roots: Vec<(NodeId, bool, bool)> = facts
            .iter()
            .flat_map(|fact| match fact.kind {
                FactKind::Let => fact
                    .operands
                    .first()
                    .copied()
                    .flatten()
                    .map(|root| (root, false, false))
                    .into_iter()
                    .collect(),
                FactKind::For => fact
                    .operands
                    .get(1)
                    .copied()
                    .flatten()
                    .map(|root| (root, true, false))
                    .into_iter()
                    .collect(),
                FactKind::Match => fact
                    .operands
                    .iter()
                    .skip(1)
                    .step_by(2)
                    .filter_map(|root| root.map(|root| (root, false, false)))
                    .collect(),
                // A closure's parameter patterns, matched against its
                // call arguments (the body is operand 0).
                FactKind::Closure => fact
                    .operands
                    .iter()
                    .skip(1)
                    .filter_map(|root| root.map(|root| (root, false, true)))
                    .collect(),
                _ => Vec::new(),
            })
            // A pattern function's parameter root patterns (E-R5-010).
            .chain(
                facts
                    .iter()
                    .enumerate()
                    .filter(|(_, fact)| fact.parameter.is_some())
                    .filter_map(|(node, _)| Some((NodeId(u32::try_from(node).ok()?), false, true))),
            )
            .collect();
        let mut trees = BTreeMap::new();
        let mut occurrences = BTreeMap::new();
        let mut structural = facts
            .iter()
            .filter(|fact| {
                fact.kind == FactKind::Borrow
                    || fact.kind == FactKind::Tuple
                    || fact.kind == FactKind::Array
                    || fact.kind == FactKind::Field
                    || fact.kind == FactKind::Index
                    || fact.kind == FactKind::IndexRange
                    || fact.kind == FactKind::Record
                    // A call may construct a tuple struct or variant.
                    || fact.kind == FactKind::Call
                    // A method call, a loop head's `Item`, a closure and a
                    // `match` each add the nesting of the declared Type they
                    // relate.
                    || fact.kind == FactKind::MethodCall
                    || fact.kind == FactKind::For
                    || fact.kind == FactKind::Closure
                    || fact.kind == FactKind::Match
                    || fact.operation == Some(SourceOperator::Deref)
            })
            .count();
        if !roots.is_empty() {
            let items = LocalItems::collect(parsed, body)?;
            for (root, for_loop, closure_parameter) in roots {
                let Some(pattern) = parsed
                    .entered_node_syntax(body, index, root)?
                    .and_then(ast::Pat::cast)
                else {
                    continue;
                };
                // A plain identifier parameter is its own declaration
                // (`closure_parameter`), not an occurrence in a root pattern.
                if closure_parameter
                    && matches!(&pattern, ast::Pat::IdentPat(ident)
                        if ident.ref_token().is_none() && ident.at_token().is_none())
                {
                    continue;
                }
                // A root the engine cannot index has no tree; its bindings
                // stay unanswered rather than re-derived here.
                let tree = if for_loop {
                    PatternTree::build_for(parsed, body, index, &items, &pattern)
                } else {
                    PatternTree::build(parsed, body, index, &items, &pattern)
                };
                let Ok(tree) = tree else {
                    continue;
                };
                let mut steps = Vec::new();
                record_occurrences(
                    tree.root(),
                    root,
                    &mut steps,
                    &mut occurrences,
                    &mut structural,
                );
                trees.insert(root, Arc::new(tree));
            }
        }
        // A destructuring assignee is a pattern: its constructor names are
        // selected by the same pattern-name service. Its leaves are places,
        // not bindings, so it records no occurrences.
        if !assignees.is_empty() {
            let items = LocalItems::collect(parsed, body)?;
            for lhs in &assignees {
                if let Ok(tree) = PatternTree::build_assignee(parsed, body, index, &items, lhs) {
                    trees.insert(tree.pattern(), Arc::new(tree));
                }
            }
        }
        let mut pattern_names = BTreeMap::new();
        for (root,tree) in &trees {
            for (slot,name) in tree.names().iter().enumerate() {
                let Some(node)=name.source else {continue};
                let slot=u32::try_from(slot).map_err(|_| overflow())?;
                if let Some((prior_root,prior_slot))=pattern_names.insert(node,(*root,slot)) {
                    if trees.get(&prior_root).and_then(|tree|tree.names().get(prior_slot as usize))!=Some(name) {
                        return Err(invalid("pattern source name has conflicting retained rows"));
                    }
                }
            }
        }
        // The member table: every written field name of a field access,
        // record expression or record pattern, sorted.
        let mut members: Vec<String> = written
            .fields
            .iter()
            .map(|(_, name)| name.clone())
            .chain(
                written
                    .records
                    .iter()
                    .flat_map(|(_, names)| names.iter().cloned()),
            )
            .chain(trees.values().flat_map(|tree| {
                tree.names()
                    .iter()
                    .filter_map(|name| match &name.form {
                        NameForm::Record(fields) => Some(fields.iter().cloned()),
                        _ => None,
                    })
                    .flatten()
                    .collect::<Vec<_>>()
            }))
            .collect();
        members.sort_unstable();
        members.dedup();
        let row = |name: &str| -> Result<u32, Diagnostic> {
            members
                .binary_search_by(|row| row.as_str().cmp(name))
                .map_err(|_| invalid("source member row absent"))
                .and_then(|row| u32::try_from(row).map_err(|_| overflow()))
        };
        for (node, name) in &written.fields {
            facts[node.0 as usize].member = Some(row(name)?);
        }
        let mut record_fields = BTreeMap::new();
        for (node, names) in &written.records {
            let rows = names
                .iter()
                .map(|name| row(name))
                .collect::<Result<Vec<_>, _>>()?;
            record_fields.insert(*node, rows.into_boxed_slice());
        }
        let relations = super::source_constraint_relations::SourceRelationPlan::prepare(
            body.key(), index.semantic_identity(), &facts, &uses,
        )?;
        Ok(Self {
            body: body.key(),
            structure: index.semantic_identity(),
            facts,
            uses,
            relations,
            names,
            trees,
            pattern_names,
            occurrences,
            type_shapes,
            path_arguments: written.path_arguments,
            members,
            record_fields,
            structural,
            bytes,
        })
    }
}

fn capture_path_arguments(
    body: &BodyHandle,
    index: &BodyIndex,
    list: Option<ast::GenericArgList>,
) -> Result<Option<PathTypeArguments>, Diagnostic> {
    let Some(list) = list else { return Ok(None) };
    let mut nodes = Vec::new();
    for argument in list.generic_args() {
        let ty = match argument {
            ast::GenericArg::LifetimeArg(_) => continue,
            ast::GenericArg::TypeArg(argument) => argument.ty(),
            _ => return Ok(Some(PathTypeArguments::Unsupported)),
        };
        let Some(node) = ty
            .as_ref()
            .map(|ty| type_node(body, index, ty))
            .transpose()?
            .flatten()
        else {
            return Ok(Some(PathTypeArguments::Unsupported));
        };
        nodes.push(node);
    }
    Ok(Some(PathTypeArguments::Types(nodes.into_boxed_slice())))
}
fn canonical(raw: &str) -> String {
    raw.strip_prefix("r#").unwrap_or(raw).to_owned()
}
fn unqualified_path_name(syntax: &SyntaxNode) -> Option<ast::NameRef> {
    let path = ast::PathExpr::cast(syntax.clone())?.path()?;
    if path.qualifier().is_some() {
        return None;
    }
    path.segment()?.name_ref()
}
/// Indexed nodes whose contents are not indexed but may still read a local:
/// closures, async/gen blocks and unexpanded macro calls. Items and inline
/// const blocks cannot name an enclosing local.
fn hidden_region(role: NodeRole, syntax: &SyntaxNode) -> bool {
    match role {
        // A closure body is indexed: its uses are ordinary source uses.
        NodeRole::Expression(
            ExpressionKind::Async | ExpressionKind::Gen | ExpressionKind::Macro,
        ) => true,
        NodeRole::OpaqueItem => ast::MacroCall::can_cast(syntax.kind()),
        NodeRole::Expression(_) => ast::FormatArgsExpr::can_cast(syntax.kind()),
        _ => false,
    }
}
/// Every identifier token spelling in an unindexed region. For a builtin
/// format template, only implicit count captures (`name$`) can constrain a
/// local; its explicit arguments are indexed expressions.
fn hidden_names(syntax: &SyntaxNode) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(format) = ast::FormatArgsExpr::cast(syntax.clone()) {
        if let Some(ast::Expr::Literal(template)) = format.template() {
            let text = template.token().text().to_owned();
            let bytes = text.as_bytes();
            let mut start = None;
            for (at, byte) in bytes.iter().enumerate() {
                let word = byte.is_ascii_alphanumeric() || *byte == b'_';
                match (start, word) {
                    (None, true) => start = Some(at),
                    (Some(from), false) => {
                        if *byte == b'$' {
                            found.push(text[from..at].to_owned());
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        found.sort();
        found.dedup();
        return found;
    }
    for token in syntax
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind() == SyntaxKind::IDENT {
            found.push(canonical(token.text()));
        }
    }
    found.sort();
    found.dedup();
    found
}

fn relative(body: &BodyHandle, range: ra_ap_syntax::TextRange) -> Result<SourceRange, Diagnostic> {
    let absolute = super::ast_adapter::range(range);
    if !body.range.contains(absolute) {
        return Err(invalid("source constraint child outside body"));
    }
    Ok(SourceRange {
        start: absolute
            .start
            .checked_sub(body.range.start)
            .ok_or_else(overflow)?,
        end: absolute
            .end
            .checked_sub(body.range.start)
            .ok_or_else(overflow)?,
    })
}
fn edge(
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: Option<SyntaxNode>,
    fact: &mut SourceFact,
) -> Result<(), Diagnostic> {
    let node = if let Some(syntax) = syntax {
        let range = relative(body, syntax.text_range())?;
        index
            .nodes_at_range(range)
            .find(|node| {
                if ast::Expr::can_cast(syntax.kind()) {
                    matches!(node.role, NodeRole::Expression(_))
                } else if ast::Pat::can_cast(syntax.kind()) {
                    node.role == NodeRole::Pattern
                } else {
                    false
                }
            })
            .map(|node| node.id)
    } else {
        None
    };
    fact.operands.push(node);
    Ok(())
}
fn expr_edge(
    body: &BodyHandle,
    index: &BodyIndex,
    expression: Option<ast::Expr>,
    fact: &mut SourceFact,
) -> Result<(), Diagnostic> {
    edge(body, index, expression.map(|e| e.syntax().clone()), fact)
}
/// Exactly the canonical Syntax node used by PreparedBodyTypeOccurrences and
/// the ordinary cast Type consumer.
fn annotate(
    body: &BodyHandle,
    index: &BodyIndex,
    ty: Option<ast::Type>,
    fact: &mut SourceFact,
    shapes: &mut BTreeMap<NodeId, TypeShape>,
) -> Result<(), Diagnostic> {
    let Some(ty) = ty else {
        return Ok(());
    };
    let range = relative(body, ty.syntax().text_range())?;
    fact.annotation = Some(range);
    fact.annotation_node = type_node(body, index, &ty)?;
    // Record the structural Type nodes below it (reference, tuple, parens).
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let Some(node) = type_node(body, index, &ty)? else {
            continue;
        };
        let shape = match &ty {
            ast::Type::RefType(reference) => match reference.ty() {
                Some(inner) => {
                    let child = type_node(body, index, &inner)?;
                    pending.push(inner);
                    child.map(TypeShape::Reference)
                }
                None => None,
            },
            ast::Type::PtrType(pointer) => match pointer.ty() {
                Some(inner) => {
                    let child = type_node(body, index, &inner)?;
                    pending.push(inner);
                    let mutable = pointer.mut_token().is_some();
                    child.map(|child| TypeShape::RawPointer(mutable, child))
                }
                None => None,
            },
            ast::Type::ParenType(group) => match group.ty() {
                Some(inner) => {
                    let child = type_node(body, index, &inner)?;
                    pending.push(inner);
                    child.map(TypeShape::Parenthesis)
                }
                None => None,
            },
            ast::Type::ArrayType(array) => match array.ty() {
                Some(inner) => {
                    let child = type_node(body, index, &inner)?;
                    pending.push(inner);
                    child.map(TypeShape::Element)
                }
                None => None,
            },
            ast::Type::SliceType(slice) => match slice.ty() {
                Some(inner) => {
                    let child = type_node(body, index, &inner)?;
                    pending.push(inner);
                    child.map(TypeShape::Element)
                }
                None => None,
            },
            ast::Type::InferType(_) => Some(TypeShape::Infer),
            ast::Type::DynTraitType(object) => {
                let principal = crate::syntax::source_bound_polarity::positive_principal(object.type_bound_list())
                    .map(ast::Type::PathType);
                match principal {
                    Some(principal) => type_node(body, index, &principal)?.map(TypeShape::Dyn),
                    None => None,
                }
            }
            ast::Type::PathType(path) => {
                // Type arguments on the only segment, as the local annotation
                // service's generic application reads them.
                let written = path.path();
                let arguments = written
                    .as_ref()
                    .filter(|written| written.qualifier().is_none())
                    .and_then(|written| written.segment())
                    .and_then(|segment| super::ast_adapter::nonempty_generic_arguments(&segment));
                match arguments {
                    Some(arguments) => {
                        let mut nodes = Vec::new();
                        let mut complete = true;
                        for argument in arguments.generic_args() {
                            match argument {
                                ast::GenericArg::LifetimeArg(_) => {}
                                ast::GenericArg::TypeArg(argument) => match argument.ty() {
                                    Some(inner) => match type_node(body, index, &inner)? {
                                        Some(child) => {
                                            nodes.push(child);
                                            pending.push(inner);
                                        }
                                        None => complete = false,
                                    },
                                    None => complete = false,
                                },
                                _ => complete = false,
                            }
                        }
                        complete.then_some(TypeShape::Generic(node, nodes))
                    }
                    None => None,
                }
            }
            ast::Type::TupleType(tuple) => {
                let mut elements = Vec::new();
                for element in tuple.fields() {
                    match type_node(body, index, &element)? {
                        Some(child) => elements.push(child),
                        None => {
                            elements.clear();
                            break;
                        }
                    }
                    pending.push(element);
                }
                (!elements.is_empty()).then_some(TypeShape::Tuple(elements))
            }
            _ => None,
        };
        if let Some(shape) = shape {
            shapes.insert(node, shape);
        }
    }
    Ok(())
}
/// The canonical indexed Syntax node of a written Type.
fn type_node(
    body: &BodyHandle,
    index: &BodyIndex,
    ty: &ast::Type,
) -> Result<Option<NodeId>, Diagnostic> {
    let range = relative(body, ty.syntax().text_range())?;
    Ok(index
        .nodes_at_range(range)
        .find(|node| node.role == NodeRole::Syntax)
        .map(|node| node.id))
}
fn suffix_type(suffix: &str) -> Option<PrimitiveType> {
    Some(match suffix {
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "i128" => PrimitiveType::I128,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "u128" => PrimitiveType::U128,
        "usize" => PrimitiveType::Usize,
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        _ => return None,
    })
}
/// Binding positions within one retained tree. Steps follow tuple fields
/// (a position after `..` is counted from the end), reference patterns,
/// array and slice elements, and constructor fields (resolved to the
/// declaration by the ordinary namespace resolution of the slot); an
/// or-pattern alternative or an `@` subpattern matches the same Type. The
/// references a non-reference pattern dereferences (default binding modes)
/// are not written: the matched value's Type states them.
fn record_occurrences(
    node: &PatternNode,
    root: NodeId,
    steps: &mut Vec<PatternStep>,
    occurrences: &mut BTreeMap<NodeId, PatternOccurrence>,
    structural: &mut usize,
) {
    match node {
        PatternNode::Binding {
            occurrence,
            by_reference,
            mutable,
            subpattern,
            ..
        } => {
            occurrences.insert(
                *occurrence,
                PatternOccurrence {
                    root,
                    steps: steps.clone(),
                    by_reference: *by_reference,
                    mutable: *mutable,
                },
            );
            if let Some(subpattern) = subpattern {
                record_occurrences(subpattern, root, steps, occurrences, structural);
            }
        }
        PatternNode::Tuple { items, rest } => {
            *structural += 1;
            for (ordinal, item) in items.iter().enumerate() {
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    return;
                };
                // A position after `..` is counted from the end.
                if rest.is_some_and(|rest| ordinal >= rest) {
                    let Ok(from_end) = u32::try_from(items.len() - 1 - ordinal as usize) else {
                        return;
                    };
                    steps.push(PatternStep::TupleFromEnd(from_end));
                } else {
                    steps.push(PatternStep::Field(ordinal));
                }
                record_occurrences(item, root, steps, occurrences, structural);
                steps.pop();
            }
        }
        PatternNode::Reference { inner, .. } => {
            *structural += 1;
            steps.push(PatternStep::Deref);
            record_occurrences(inner, root, steps, occurrences, structural);
            steps.pop();
        }
        PatternNode::Array {
            items,
            rest_binding,
            ..
        } => {
            *structural += 1;
            steps.push(PatternStep::Element);
            for item in items {
                record_occurrences(item, root, steps, occurrences, structural);
            }
            steps.pop();
            if let Some(binding) = rest_binding {
                steps.push(PatternStep::Rest);
                record_occurrences(binding, root, steps, occurrences, structural);
                steps.pop();
            }
        }
        PatternNode::Or(alternatives) => {
            for alternative in alternatives {
                record_occurrences(alternative, root, steps, occurrences, structural);
            }
        }
        PatternNode::Constructor { slot, form } => {
            let (items, rest) = match form {
                ConstructorForm::Path => return,
                ConstructorForm::Tuple { items, rest } => (items, *rest),
                ConstructorForm::Record { fields, .. } => (fields, None),
            };
            *structural += 1;
            for (written, item) in items.iter().enumerate() {
                let Ok(written) = u32::try_from(written) else {
                    return;
                };
                // A position after `..` is counted from the declared end.
                if rest.is_some_and(|rest| written >= rest) {
                    let Ok(from_end) = u32::try_from(items.len() - 1 - written as usize) else {
                        return;
                    };
                    steps.push(PatternStep::ConstructorFromEnd {
                        slot: *slot,
                        from_end,
                    });
                } else {
                    steps.push(PatternStep::Constructor {
                        slot: *slot,
                        written,
                    });
                }
                record_occurrences(item, root, steps, occurrences, structural);
                steps.pop();
            }
        }
        _ => {}
    }
}
fn classify_pattern(
    syntax: &SyntaxNode,
    fact: &mut SourceFact,
    names: &mut BTreeMap<String, SourceName>,
    spelled: &mut Vec<(NodeId, String)>,
    node: NodeId,
) {
    match ast::Pat::cast(syntax.clone()) {
        Some(ast::Pat::IdentPat(pattern)) => {
            fact.kind = FactKind::BindingPattern;
            if let Some(name) = pattern.name() {
                let name = canonical(name.text());
                names.entry(name.clone()).or_default().bindings.push(node);
                spelled.push((node, name));
            }
        }
        _ => {}
    }
}
/// Written field names gathered while classifying, before the member table.
#[derive(Default)]
struct WrittenMembers {
    path_arguments: BTreeMap<NodeId, SourcePathArguments>,
    fields: Vec<(NodeId, String)>,
    records: Vec<(NodeId, Vec<String>)>,
}
fn classify(
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: SyntaxNode,
    node: NodeId,
    fact: &mut SourceFact,
    shapes: &mut BTreeMap<NodeId, TypeShape>,
    written: &mut WrittenMembers,
) -> Result<(), Diagnostic> {
    if let Some(statement) = ast::LetStmt::cast(syntax.clone()) {
        fact.kind = FactKind::Let;
        edge(
            body,
            index,
            statement.pat().map(|p| p.syntax().clone()),
            fact,
        )?;
        expr_edge(body, index, statement.initializer(), fact)?;
        annotate(body, index, statement.ty(), fact, shapes)?;
        return Ok(());
    }
    if let Some(statement) = ast::ExprStmt::cast(syntax.clone()) {
        // Only an explicit `;` discards. A block-like expression statement
        // without one is a unit-typed context instead.
        if statement.semicolon_token().is_some() {
            fact.kind = FactKind::Discard;
            expr_edge(body, index, statement.expr(), fact)?;
        }
        return Ok(());
    }
    let pattern_path = match ast::Pat::cast(syntax.clone()) {
        Some(ast::Pat::RecordPat(pattern)) => pattern.path(),
        Some(ast::Pat::TupleStructPat(pattern)) => pattern.path(),
        Some(ast::Pat::PathPat(pattern)) => pattern.path(),
        _ => None,
    };
    if let Some(path) = pattern_path {
        let final_segment = path.segment().and_then(|segment|segment.generic_arg_list());
        let owner_segment = path.qualifier().and_then(|path|path.segment()).and_then(|segment|segment.generic_arg_list());
        if final_segment.is_some() || owner_segment.is_some() {
            written.path_arguments.insert(node,SourcePathArguments {
                final_segment:capture_path_arguments(body,index,final_segment)?,
                owner_segment:capture_path_arguments(body,index,owner_segment)?,
            });
        }
    }
    let Some(expression) = ast::Expr::cast(syntax) else {
        return Ok(());
    };
    let path = match &expression {
        ast::Expr::PathExpr(value) => value.path(),
        ast::Expr::RecordExpr(record) => record.path(),
        _ => None,
    };
    if let Some(path) = path {
        let final_segment = path.segment().and_then(|p| p.generic_arg_list());
        let owner_segment = path.qualifier().and_then(|p| p.segment())
            .and_then(|p| p.generic_arg_list());
        if final_segment.is_some() || owner_segment.is_some() {
            written.path_arguments.insert(node, SourcePathArguments {
                final_segment: capture_path_arguments(body, index, final_segment)?,
                owner_segment: capture_path_arguments(body, index, owner_segment)?,
            });
        }
    }
    match expression {
        ast::Expr::ClosureExpr(closure) => {
            fact.kind = FactKind::Closure;
            expr_edge(body, index, closure.body(), fact)?;
            // Then each parameter's root pattern, in written order.
            if let Some(parameters) = closure.param_list() {
                for parameter in parameters.params() {
                    edge(
                        body,
                        index,
                        parameter.pat().map(|pattern| pattern.syntax().clone()),
                        fact,
                    )?;
                }
            }
        }
        ast::Expr::PathExpr(_) => fact.kind = FactKind::Path,
        ast::Expr::Literal(literal) => {
            fact.kind = match literal.token().kind() {
                SyntaxKind::INT_NUMBER => match ast::IntNumber::cast(literal.token()) {
                    Some(number) => match number.suffix() {
                        None => FactKind::Integer,
                        Some(suffix) => {
                            fact.suffix = suffix_type(suffix);
                            FactKind::SuffixedInteger
                        }
                    },
                    None => FactKind::Literal,
                },
                SyntaxKind::FLOAT_NUMBER => match ast::FloatNumber::cast(literal.token()) {
                    Some(number) => match number.suffix() {
                        None => FactKind::Float,
                        Some(suffix) => {
                            fact.suffix = suffix_type(suffix);
                            FactKind::SuffixedFloat
                        }
                    },
                    None => FactKind::Literal,
                },
                // A non-numeric literal has a fixed lexical Type: `bool`,
                // `char`, `u8` (a byte), or `&'static str` (`flag`).
                SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => {
                    fact.suffix = Some(PrimitiveType::Bool);
                    FactKind::Literal
                }
                SyntaxKind::CHAR => {
                    fact.suffix = Some(PrimitiveType::Char);
                    FactKind::Literal
                }
                SyntaxKind::BYTE => {
                    fact.suffix = Some(PrimitiveType::U8);
                    FactKind::Literal
                }
                SyntaxKind::STRING => {
                    fact.flag = true;
                    FactKind::Literal
                }
                SyntaxKind::BYTE_STRING => {
                    let bytes = ast::ByteString::cast(literal.token())
                        .ok_or_else(|| invalid("byte string token kind differs"))?;
                    let value = bytes.value().map_err(|_| invalid("invalid byte string literal"))?;
                    fact.byte_length = Some(u64::try_from(value.len()).map_err(|_| overflow())?);
                    FactKind::Literal
                }
                _ => FactKind::Literal,
            }
        }
        ast::Expr::ParenExpr(group) => {
            fact.kind = FactKind::Parenthesis;
            expr_edge(body, index, group.expr(), fact)?;
        }
        ast::Expr::PrefixExpr(prefix) => {
            fact.kind = FactKind::Prefix;
            fact.numeric_prefix = prefix
                .op_token()
                .is_some_and(|token| matches!(token.text(), "-" | "!"));
            fact.operation = prefix.op_kind().map(SourceOperator::unary);
            fact.operator = prefix
                .op_token()
                .map(|t| relative(body, t.text_range()))
                .transpose()?;
            expr_edge(body, index, prefix.expr(), fact)?;
        }
        ast::Expr::BinExpr(binary) => {
            fact.kind = FactKind::Binary;
            fact.operation = binary.op_kind().map(SourceOperator::binary);
            fact.operator = binary
                .op_token()
                .map(|t| relative(body, t.text_range()))
                .transpose()?;
            expr_edge(body, index, binary.lhs(), fact)?;
            expr_edge(body, index, binary.rhs(), fact)?;
        }
        ast::Expr::CallExpr(call) => {
            fact.kind = FactKind::Call;
            // A callee path with explicit generic arguments (`f::<u8>(..)`).
            fact.flag = matches!(call.expr(), Some(ast::Expr::PathExpr(callee))
            if callee.path().is_some_and(|path| {
                std::iter::successors(Some(path), |path| path.qualifier()).any(|path| {
                    path.segment()
                        .is_some_and(|segment| segment.generic_arg_list().is_some())
                })
            }));
            expr_edge(body, index, call.expr(), fact)?;
            if let Some(args) = call.arg_list() {
                for argument in args.args() {
                    expr_edge(body, index, Some(argument), fact)?;
                }
            }
        }
        ast::Expr::MethodCallExpr(call) => {
            fact.kind = FactKind::MethodCall;
            // The written method name (candidate spelling for selection);
            // explicit generic arguments leave the call opaque.
            if call.generic_arg_list().is_some() {
                fact.kind = FactKind::Opaque;
                return Ok(());
            }
            if let Some(name) = call.name_ref() {
                written.fields.push((node, canonical(name.text())));
            }
            expr_edge(body, index, call.receiver(), fact)?;
            if let Some(args) = call.arg_list() {
                for argument in args.args() {
                    expr_edge(body, index, Some(argument), fact)?;
                }
            }
        }
        ast::Expr::BlockExpr(block) => {
            // A label, async/gen/try or `move` changes the block's
            // result relation (break values, futures, Try wrapping); those
            // stay opaque. `unsafe` and `const` keep the ordinary tail relation.
            if block.label().is_none()
                && block.async_token().is_none()
                && block.gen_token().is_none()
                && block.move_token().is_none()
                && block.try_block_modifier().is_none()
            {
                fact.kind = FactKind::Block;
                expr_edge(
                    body,
                    index,
                    block.stmt_list().and_then(|s| s.tail_expr()),
                    fact,
                )?;
            }
        }
        ast::Expr::IfExpr(conditional) => {
            fact.kind = FactKind::If;
            expr_edge(body, index, conditional.condition(), fact)?;
            expr_edge(
                body,
                index,
                conditional.then_branch().map(ast::Expr::BlockExpr),
                fact,
            )?;
            let otherwise = match conditional.else_branch() {
                Some(ast::ElseBranch::Block(block)) => Some(ast::Expr::BlockExpr(block)),
                Some(ast::ElseBranch::IfExpr(nested)) => Some(ast::Expr::IfExpr(nested)),
                None => None,
            };
            expr_edge(body, index, otherwise, fact)?;
        }
        ast::Expr::TupleExpr(tuple) => {
            fact.kind = FactKind::Tuple;
            if super::assignee::enclosing(tuple.syntax()).is_some() {
                // A destructuring assignee: its operands align with the
                // matched tuple (`..` spans the unwritten positions).
                match super::assignee::fact_operands(&ast::Expr::TupleExpr(tuple.clone())) {
                    Some(operands) => {
                        for element in operands {
                            expr_edge(body, index, Some(element), fact)?;
                        }
                    }
                    None => fact.kind = FactKind::Opaque,
                }
            } else {
                for element in tuple.fields() {
                    expr_edge(body, index, Some(element), fact)?;
                }
            }
        }
        ast::Expr::UnderscoreExpr(_) if super::assignee::enclosing(expression.syntax()).is_some() => {
            fact.kind = FactKind::Wildcard;
        }
        ast::Expr::RangeExpr(range)
            if super::assignee::is_rest(&expression)
                && super::assignee::enclosing(range.syntax()).is_some() =>
        {
            fact.kind = FactKind::Wildcard;
        }
        ast::Expr::FieldExpr(access) => {
            // A written position (`.0`) or field name (`.x`); which declared
            // field it selects needs the base's Type.
            if let Some(name) = access.name_ref() {
                fact.kind = FactKind::Field;
                match name.text().parse::<u32>() {
                    Ok(position) => fact.field = Some(position),
                    Err(_) => written.fields.push((node, canonical(name.text()))),
                }
                expr_edge(body, index, access.expr(), fact)?;
            }
        }
        ast::Expr::RecordExpr(record) => {
            // Field attributes (`#[cfg]`) change which initializers exist;
            // such a record stays opaque.
            if let Some(list) = record.record_expr_field_list()
                && list.fields().all(|field| field.attrs().next().is_none())
                // `S { a, .. }` without a base takes default field values.
                && (list.spread().is_some() || list.dotdot_token().is_none())
            {
                let mut names = Vec::new();
                let mut complete = true;
                for field in list.fields() {
                    match (field.field_name(), field.expr()) {
                        (Some(name), Some(initializer)) => {
                            names.push(canonical(name.text()));
                            expr_edge(body, index, Some(initializer), fact)?;
                        }
                        _ => complete = false,
                    }
                }
                if complete {
                    fact.kind = FactKind::Record;
                    if let Some(base) = list.spread() {
                        fact.flag = true;
                        expr_edge(body, index, Some(base), fact)?;
                    }
                    written.records.push((node, names));
                } else {
                    fact.operands.clear();
                }
            }
        }
        ast::Expr::ArrayExpr(array) => {
            fact.kind = FactKind::Array;
            match array.kind() {
                ast::ArrayExprKind::ElementList(elements) => {
                    if super::assignee::enclosing(array.syntax()).is_some() {
                        // A destructuring assignee: `..` names no element.
                        for element in
                            super::assignee::fact_operands(&ast::Expr::ArrayExpr(array.clone()))
                                .unwrap_or_default()
                        {
                            expr_edge(body, index, Some(element), fact)?;
                        }
                    } else {
                        for element in elements {
                            expr_edge(body, index, Some(element), fact)?;
                        }
                    }
                }
                ast::ArrayExprKind::Repeat { initializer, .. } => {
                    fact.flag = true;
                    expr_edge(body, index, initializer, fact)?;
                }
            }
        }
        ast::Expr::IndexExpr(access) => {
            // Only an integer literal index certainly selects one element
            // (a range index selects a slice).
            fact.flag = matches!(
                access.index(),
                Some(ast::Expr::Literal(literal))
                    if literal.token().kind() == SyntaxKind::INT_NUMBER
            );
            fact.kind = if matches!(access.index(), Some(ast::Expr::RangeExpr(_))) {
                FactKind::IndexRange
            } else {
                FactKind::Index
            };
            expr_edge(body, index, access.base(), fact)?;
            expr_edge(body, index, access.index(), fact)?;
        }
        ast::Expr::MatchExpr(selection) => {
            fact.kind = FactKind::Match;
            expr_edge(body, index, selection.expr(), fact)?;
            for arm in selection
                .match_arm_list()
                .into_iter()
                .flat_map(|arms| arms.arms())
            {
                edge(body, index, arm.pat().map(|p| p.syntax().clone()), fact)?;
                expr_edge(body, index, arm.expr(), fact)?;
            }
        }
        // An `if let` / `while let` / let-chain clause: the scrutinee is
        // matched against the pattern as a `let` statement's initializer is
        // (the shared pattern plan's transactional test).
        ast::Expr::LetExpr(clause) => {
            fact.kind = FactKind::Let;
            edge(body, index, clause.pat().map(|p| p.syntax().clone()), fact)?;
            expr_edge(body, index, clause.expr(), fact)?;
        }
        ast::Expr::ForExpr(looped) => {
            fact.kind = FactKind::For;
            expr_edge(body, index, looped.iterable(), fact)?;
            edge(body, index, looped.pat().map(|p| p.syntax().clone()), fact)?;
            expr_edge(body, index, looped.loop_body().map(ast::Expr::BlockExpr), fact)?;
        }
        ast::Expr::RangeExpr(range) => {
            // The lang-item record a range expression constructs (an
            // inclusive `a..=b` is a `RangeInclusive::new` call instead).
            if let Some((_, start, end)) = super::constructors::LangRecord::of(&range) {
                let mut names = Vec::new();
                if let Some(start) = start {
                    names.push("start".to_owned());
                    expr_edge(body, index, Some(start), fact)?;
                }
                if let Some(end) = end {
                    names.push("end".to_owned());
                    expr_edge(body, index, Some(end), fact)?;
                }
                fact.kind = FactKind::Record;
                written.records.push((node, names));
            } else if let Some((start, end)) = super::constructors::range_inclusive_bounds(&range) {
                // `a..=b` is the call `RangeInclusive::new(a, b)`.
                fact.kind = FactKind::LangCall;
                expr_edge(body, index, Some(start), fact)?;
                expr_edge(body, index, Some(end), fact)?;
            }
        }
        ast::Expr::ReturnExpr(ret) => {
            fact.kind = FactKind::Return;
            expr_edge(body, index, ret.expr(), fact)?;
        }
        ast::Expr::CastExpr(cast) => {
            fact.kind = FactKind::Cast;
            expr_edge(body, index, cast.expr(), fact)?;
            annotate(body, index, cast.ty(), fact, shapes)?;
        }
        // `&raw const e` / `&raw mut e` is a raw pointer to the place, not a
        // reference to its type.
        ast::Expr::RefExpr(reference) if reference.raw_token().is_some() => {
            fact.kind = FactKind::RawBorrow;
            fact.flag = reference.mut_token().is_some();
            expr_edge(body, index, reference.expr(), fact)?;
        }
        ast::Expr::RefExpr(reference) => {
            fact.kind = FactKind::Borrow;
            // `&mut e`: the operand place is used mutably.
            fact.flag = reference.mut_token().is_some();
            expr_edge(body, index, reference.expr(), fact)?;
        }
        ast::Expr::FormatArgsExpr(format) => {
            fact.kind = FactKind::FormatArguments;
            let template = format.template();
            // A count (`{:1$}`, `{:w$}`, `{:.*}`) requires usize; anything
            // else is a generic formatting-trait use with no numeric relation.
            fact.flag = match &template {
                Some(ast::Expr::Literal(literal)) => {
                    let text = literal.token().text().to_owned();
                    text.contains('$') || text.contains(".*")
                }
                _ => true,
            };
            expr_edge(body, index, template, fact)?;
            for argument in format
                .syntax()
                .children()
                .filter_map(ast::FormatArgsArg::cast)
            {
                expr_edge(body, index, argument.expr(), fact)?;
            }
        }
        // Incomplete source relationships are opaque until demanded by the
        // shared solver. Never lower unselected branches to populate this table.
        _ => {}
    }
    Ok(())
}

impl StableEncode for SourceConstraintPlan {
    const DOMAIN: &'static str = "lrr/v1/source-constraint-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(SOURCE_CONSTRAINT_SCHEMA);
        e.record(&self.body);
        e.digest(self.structure);
        e.record(&self.relations);
        e.sequence(&self.facts, |e, fact| {
            e.u8(fact.kind.tag());
            e.sequence(&fact.operands, |e, node| e.option(node, |e, n| e.u32(n.0)));
            e.option(&fact.annotation, encode_range);
            e.option(&fact.annotation_node, |e, n| e.u32(n.0));
            e.option(&fact.operator, encode_range);
            e.option(&fact.operation, |e, op| e.u8(op.tag()));
            e.bool(fact.numeric_prefix);
            e.option(&fact.suffix, |e, primitive| e.u32(primitive.tag()));
            e.option(&fact.byte_length, |e, length| e.u64(*length));
            e.option(&fact.name, |e, row| e.u32(*row));
            e.option(&fact.field, |e, position| e.u32(*position));
            e.option(&fact.member, |e, row| e.u32(*row));
            e.bool(fact.flag);
            e.bool(fact.closure_parameter);
            e.option(&fact.parameter, |e, ordinal| e.u32(*ordinal));
        });
        e.sequence(&self.trees.iter().collect::<Vec<_>>(), |e, (root, tree)| {
            e.u32(root.0);
            e.digest(tree.identity());
        });
        e.sequence(
            &self.occurrences.iter().collect::<Vec<_>>(),
            |e, (binding, occurrence)| {
                e.u32(binding.0);
                e.u32(occurrence.root.0);
                e.sequence(&occurrence.steps, |e, step| match step {
                    PatternStep::Deref => e.u8(0),
                    PatternStep::Field(ordinal) => {
                        e.u8(1);
                        e.u32(*ordinal);
                    }
                    PatternStep::Element => e.u8(2),
                    PatternStep::Constructor { slot, written } => {
                        e.u8(3);
                        e.u32(*slot);
                        e.u32(*written);
                    }
                    PatternStep::TupleFromEnd(from_end) => {
                        e.u8(4);
                        e.u32(*from_end);
                    }
                    PatternStep::ConstructorFromEnd { slot, from_end } => {
                        e.u8(5);
                        e.u32(*slot);
                        e.u32(*from_end);
                    }
                    PatternStep::Rest => e.u8(6),
                });
                e.bool(occurrence.by_reference);
                e.bool(occurrence.mutable);
            },
        );
        e.sequence(
            &self.type_shapes.iter().collect::<Vec<_>>(),
            |e, (node, shape)| {
                e.u32(node.0);
                match shape {
                    TypeShape::Reference(inner) => {
                        e.u8(0);
                        e.u32(inner.0);
                    }
                    TypeShape::Tuple(elements) => {
                        e.u8(1);
                        e.sequence(elements, |e, element| e.u32(element.0));
                    }
                    TypeShape::Parenthesis(inner) => {
                        e.u8(2);
                        e.u32(inner.0);
                    }
                    TypeShape::Element(inner) => {
                        e.u8(3);
                        e.u32(inner.0);
                    }
                    TypeShape::Infer => e.u8(5),
                    TypeShape::RawPointer(mutable, inner) => {
                        e.u8(7);
                        e.bool(*mutable);
                        e.u32(inner.0);
                    }
                    TypeShape::Dyn(principal) => {
                        e.u8(6);
                        e.u32(principal.0);
                    }
                    TypeShape::Generic(base, arguments) => {
                        e.u8(4);
                        e.u32(base.0);
                        e.sequence(arguments, |e, argument| e.u32(argument.0));
                    }
                }
            },
        );
        e.sequence(&self.pattern_names.iter().collect::<Vec<_>>(), |e,(node,(root,slot))| {e.u32(node.0);e.u32(root.0);e.u32(*slot);});
        e.sequence(&self.path_arguments.iter().collect::<Vec<_>>(), |e, (node, args)| {
            e.u32(node.0);
            for list in [&args.final_segment, &args.owner_segment] {
                match list {
                    None => e.u8(0),
                    Some(PathTypeArguments::Unsupported) => e.u8(2),
                    Some(PathTypeArguments::Types(nodes)) => {
                        e.u8(1);
                        e.sequence(nodes, |e, node| e.u32(node.0));
                    }
                }
            }
        });
        e.sequence(&self.members, |e, member| e.string(member));
        e.sequence(
            &self.record_fields.iter().collect::<Vec<_>>(),
            |e, (node, rows)| {
                e.u32(node.0);
                e.sequence(rows, |e, row| e.u32(*row));
            },
        );
        e.u64(self.structural as u64);
        e.sequence(&self.names, |e, (name, row)| {
            e.string(name);
            e.sequence(&row.paths, |e, n| e.u32(n.0));
            e.sequence(&row.bindings, |e, n| e.u32(n.0));
            e.sequence(&row.hidden, |e, n| e.u32(n.0));
        });
    }
}
fn encode_range(e: &mut StableEncoder, range: &SourceRange) {
    e.u32(range.start);
    e.u32(range.end);
}
