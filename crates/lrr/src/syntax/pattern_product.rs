//! Retained structural decisions for the ordinary pattern transaction engine.
//! This captures source edges, not Cartesian alternatives or executable arms.
use super::pattern_tests::ScalarTestPlan;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::pc::{NodeId, PcKey, Phase, SlotId},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        pattern_bindings::{
            BindingConflicts, BindingContext, BindingUncertainty, WrittenBindingMode,
        },
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;
#[path = "pattern_lower.rs"]
mod lowering;

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "retained structural pattern decision budget".into(),
    )
}

#[derive(Debug)]
pub(crate) struct BindingIntent {
    pub(crate) canonical: Option<NodeId>,
    pub(crate) context: BindingContext,
    pub(crate) conflicts: BindingConflicts,
    pub(crate) mode: WrittenBindingMode,
    pub(crate) edition: Option<u16>,
    pub(crate) uncertainty: BindingUncertainty,
    pub(crate) requires_namespace: bool,
    pub(crate) subpattern: Option<NodeId>,
}

#[derive(Debug)]
pub(crate) enum DecisionKind {
    Wildcard,
    Scalar(ScalarTestPlan),
    Binding(BindingIntent),
    Tuple(Box<[NodeId]>),
    Or(Box<[NodeId]>),
    Group(NodeId),
    /// Deferred until this actual indexed node is reached. An unsupported cold
    /// alternative cannot prevent a preceding successful decision from running.
    Unsupported(Diagnostic),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DecisionContinuation {
    Enter(NodeId),
    Commit(NodeId),
}

#[derive(Debug)]
pub(crate) struct DecisionRow {
    pub(crate) node: NodeId,
    pub(crate) parent: Option<NodeId>,
    pub(crate) success: Option<DecisionContinuation>,
    /// Actual written tuple ordinals from the root source to this node. These
    /// select source members only after ordinary runtime source admission.
    pub(crate) source_path: Box<[u32]>,
    pub(crate) kind: DecisionKind,
}
impl DecisionRow {
    pub(crate) fn scalar_upper_pending(&self) -> Option<bool> {
        match &self.kind {
            DecisionKind::Scalar(ScalarTestPlan::Range { lower, upper, .. }) => {
                Some(lower.is_some() && upper.is_some())
            }
            DecisionKind::Scalar(_) => Some(false),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct PatternProductPlan {
    root: NodeId,
    owner: NodeId,
    arm: Option<NodeId>,
    source: SlotId,
    matched: PcKey,
    unmatched: PcKey,
    unmatched_inactive: PcKey,
    index: Arc<BodyIndex>,
    body: crate::source::manifest::BodyKey,
    profile: Digest,
    source_identity: Digest,
    body_range: SourceRange,
    body_structure: Digest,
    rows: Vec<DecisionRow>,
    lookup: Vec<(NodeId, usize)>,
    bytes: usize,
    identity: Digest,
    choice_bound: usize,
}

fn indexed_identity(
    body: &BodyHandle,
    index: &BodyIndex,
    node: &ra_ap_syntax::SyntaxNode,
    role: NodeRole,
) -> Result<NodeId, Diagnostic> {
    let range = node.text_range();
    let range = SourceRange::checked(
        u32::from(range.start())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
        u32::from(range.end())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
    )?;
    index
        .nodes_at_range(range)
        .find(|node| node.role == role)
        .map(|node| node.id)
        .ok_or_else(|| invalid("pattern owner source indexed location absent"))
}
/// A staged root remains independently selectable. Unsupported preparation of
/// one arm cannot become a global executable-shape requirement for cold arms.
#[derive(Debug)]
pub(crate) struct ProductRoot {
    pub(crate) root: NodeId,
    pub(crate) product: Result<Arc<PatternProductPlan>, Diagnostic>,
}
impl ProductRoot {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        match &self.product {
            Ok(_) => Some(0), // The query owner charges each Product allocation.
            Err(error) => error.heap_bytes(),
        }
    }
    pub(crate) fn encode(&self, e: &mut StableEncoder) {
        e.u32(self.root.0);
        match &self.product {
            Ok(plan) => {
                e.u8(0);
                e.digest(plan.identity());
            }
            Err(error) => {
                e.u8(1);
                e.string(error.code);
                e.string(&error.message);
            }
        }
    }
}

struct Preparation<'a> {
    body: &'a BodyHandle,
    index: &'a BodyIndex,
    rows: Vec<DecisionRow>,
    remaining_work: &'a mut usize,
    reserved: usize,
    byte_limit: usize,
}
impl Preparation<'_> {
    fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        *self.remaining_work = self.remaining_work.checked_sub(1).ok_or_else(limit)?;
        // Include preparation's red-node/edge vectors and normal Vec growth
        // before allocating. Final retained accounting uses actual capacities.
        self.reserved = self
            .reserved
            .checked_add(bytes.checked_mul(4).ok_or_else(limit)?)
            .filter(|bytes| *bytes <= self.byte_limit)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn identity(&self, pattern: &ast::Pat) -> Result<NodeId, Diagnostic> {
        let source = pattern.syntax().text_range();
        let range = SourceRange::checked(
            u32::from(source.start())
                .checked_sub(self.body.range.start)
                .ok_or_else(limit)?,
            u32::from(source.end())
                .checked_sub(self.body.range.start)
                .ok_or_else(limit)?,
        )?;
        self.index
            .nodes_at_range(range)
            .find(|node| node.role == NodeRole::Pattern)
            .map(|node| node.id)
            .ok_or_else(|| invalid("structural pattern indexed location absent"))
    }
    fn children(
        &mut self,
        children: impl Iterator<Item = ast::Pat>,
    ) -> Result<Vec<ast::Pat>, Diagnostic> {
        let mut result = Vec::new();
        for child in children {
            self.charge(std::mem::size_of::<ast::Pat>() + std::mem::size_of::<NodeId>())?;
            result.try_reserve(1).map_err(|_| limit())?;
            result.push(child);
        }
        Ok(result)
    }
    fn capture(
        &mut self,
        pattern: ast::Pat,
        parent: Option<NodeId>,
        success: Option<DecisionContinuation>,
        path: &mut Vec<u32>,
        depth: usize,
    ) -> Result<NodeId, Diagnostic> {
        self.charge(
            std::mem::size_of::<DecisionRow>()
                .checked_add(
                    path.len()
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(limit)?,
                )
                .ok_or_else(limit)?,
        )?;
        if depth >= 256 {
            return Err(limit());
        }
        let node = self.identity(&pattern)?;
        let mut source_path = Vec::new();
        source_path
            .try_reserve_exact(path.len())
            .map_err(|_| limit())?;
        source_path.extend_from_slice(path);
        let position = self.rows.len();
        self.rows.try_reserve(1).map_err(|_| limit())?;
        self.rows.push(DecisionRow {
            node,
            parent,
            success,
            source_path: source_path.into_boxed_slice(),
            kind: DecisionKind::Wildcard,
        });
        let kind = if pattern
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
        {
            DecisionKind::Unsupported(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "reached structural pattern attributes require source admission".into(),
            ))
        } else {
            match pattern {
                ast::Pat::WildcardPat(_) => DecisionKind::Wildcard,
                scalar @ (ast::Pat::LiteralPat(_) | ast::Pat::RangePat(_)) => {
                    match ScalarTestPlan::from_source(&scalar) {
                        Ok(plan) => DecisionKind::Scalar(plan),
                        Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                            DecisionKind::Unsupported(error)
                        }
                        Err(error) => return Err(error),
                    }
                }
                ast::Pat::ParenPat(group) => match group.pat() {
                    Some(inner) => DecisionKind::Group(self.capture(
                        inner,
                        Some(node),
                        Some(DecisionContinuation::Commit(node)),
                        path,
                        depth + 1,
                    )?),
                    None => return Err(invalid("grouped pattern source absent")),
                },
                ast::Pat::IdentPat(binding) => {
                    let group = self
                        .index
                        .pattern_bindings()
                        .group_for(node)
                        .ok_or_else(|| invalid("retained canonical binding candidate absent"))?;
                    let occurrence = group
                        .occurrence(node)
                        .ok_or_else(|| invalid("canonical binding occurrence absent"))?;
                    // Copy fixed source facts before recursively retaining the
                    // @ subpattern; never perform a second binding walk.
                    let mut intent = BindingIntent {
                        canonical: group.canonical_candidate(),
                        context: group.context(),
                        conflicts: group.conflicts(),
                        mode: occurrence.mode,
                        edition: occurrence.edition,
                        uncertainty: occurrence.uncertainty,
                        requires_namespace: group.requires_namespace(),
                        subpattern: None,
                    };
                    if let Some(inner) = binding.pat() {
                        intent.subpattern = Some(self.capture(
                            inner,
                            Some(node),
                            Some(DecisionContinuation::Commit(node)),
                            path,
                            depth + 1,
                        )?);
                    }
                    DecisionKind::Binding(intent)
                }
                ast::Pat::TuplePat(tuple) => {
                    let children = self.children(tuple.fields())?;
                    if children
                        .iter()
                        .any(|child| matches!(child, ast::Pat::RestPat(_)))
                    {
                        DecisionKind::Unsupported(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "tuple rest requires its source arity continuation".into(),
                        ))
                    } else {
                        let mut rows = Vec::new();
                        rows.try_reserve_exact(children.len())
                            .map_err(|_| limit())?;
                        let mut children_ids = Vec::new();
                        children_ids
                            .try_reserve_exact(children.len())
                            .map_err(|_| limit())?;
                        for child in &children {
                            children_ids.push(self.identity(child)?);
                        }
                        for (field, child) in children.into_iter().enumerate() {
                            self.charge(std::mem::size_of::<u32>())?;
                            path.try_reserve(1).map_err(|_| limit())?;
                            path.push(u32::try_from(field).map_err(|_| limit())?);
                            let next = children_ids
                                .get(field + 1)
                                .copied()
                                .map(DecisionContinuation::Enter)
                                .unwrap_or(DecisionContinuation::Commit(node));
                            rows.push(self.capture(
                                child,
                                Some(node),
                                Some(next),
                                path,
                                depth + 1,
                            )?);
                            path.pop();
                        }
                        DecisionKind::Tuple(rows.into_boxed_slice())
                    }
                }
                ast::Pat::OrPat(or) => {
                    let children = self.children(or.pats())?;
                    if children.is_empty() {
                        return Err(invalid("or-pattern alternatives absent"));
                    }
                    let mut rows = Vec::new();
                    rows.try_reserve_exact(children.len())
                        .map_err(|_| limit())?;
                    for child in children {
                        rows.push(self.capture(
                            child,
                            Some(node),
                            Some(DecisionContinuation::Commit(node)),
                            path,
                            depth + 1,
                        )?);
                    }
                    DecisionKind::Or(rows.into_boxed_slice())
                }
                _ => DecisionKind::Unsupported(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "reached pattern requires its ordinary source decision integration".into(),
                )),
            }
        };
        if let DecisionKind::Unsupported(error) = &kind {
            self.charge(error.heap_bytes().ok_or_else(limit)?)?;
        }
        self.rows[position].kind = kind;
        Ok(node)
    }
}

impl PatternProductPlan {
    /// Test adapter uses the same complete cold owner capture as the query.
    /// Production phase selection can only consume retained source-plan roots.
    #[cfg(test)]
    pub(crate) fn prepare(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &Arc<BodyIndex>,
        root: NodeId,
    ) -> Result<Self, Diagnostic> {
        if index.body() != &body.key() || index.profile() != &parsed.profile {
            return Err(invalid("structural pattern source body/profile differs"));
        }
        parsed.entered_body_root(body)?;
        let syntax = parsed
            .entered_node_syntax(body, index, root)?
            .and_then(ast::Pat::cast)
            .ok_or_else(|| invalid("structural root is not an indexed pattern"))?;
        let parent = syntax
            .syntax()
            .parent()
            .ok_or_else(|| invalid("structural pattern owner absent"))?;
        let owner = if ast::MatchArm::can_cast(parent.kind()) {
            parent
                .ancestors()
                .find_map(ast::MatchExpr::cast)
                .map(|owner| owner.syntax().clone())
                .ok_or_else(|| invalid("structural match owner absent"))?
        } else if ast::LetExpr::can_cast(parent.kind()) {
            parent
        } else {
            return Err(invalid(
                "structural root is not an arm or condition pattern",
            ));
        };
        let roots = Self::prepare_owner(parsed, body, index, &owner, 32 * 1024 * 1024)?;
        let product = roots
            .into_iter()
            .find(|row| row.root == root)
            .ok_or_else(|| invalid("structural root excluded by actual owner source"))?
            .product?;
        Arc::try_unwrap(product).map_err(|_| invalid("new structural root unexpectedly shared"))
    }
    fn capture_selected(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        retained_index: Arc<BodyIndex>,
        root: NodeId,
        syntax: ast::Pat,
        next_arm: Option<NodeId>,
        byte_limit: usize,
        remaining_work: &mut usize,
    ) -> Result<Self, Diagnostic> {
        let (owner, arm, source, matched, unmatched, unmatched_inactive) =
            Self::owner_receipt(body, index, &syntax, next_arm)?;
        let mut preparation = Preparation {
            body,
            index,
            rows: Vec::new(),
            remaining_work,
            reserved: std::mem::size_of::<Self>(),
            byte_limit,
        };
        preparation.capture(syntax, None, None, &mut Vec::new(), 0)?;
        preparation.charge(
            preparation
                .rows
                .len()
                .checked_mul(std::mem::size_of::<(NodeId, usize)>())
                .ok_or_else(limit)?,
        )?;
        let mut lookup = Vec::new();
        lookup
            .try_reserve_exact(preparation.rows.len())
            .map_err(|_| limit())?;
        lookup.extend(
            preparation
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| (row.node, i)),
        );
        lookup.sort_unstable_by_key(|row| row.0);
        if lookup.windows(2).any(|rows| rows[0].0 == rows[1].0) {
            return Err(invalid("structural decision node captured twice"));
        }
        let mut source_identity = StableEncoder::new("lrr/v1/structural-pattern-source");
        source_identity.digest(parsed.content);
        source_identity.record(&parsed.profile);
        source_identity.record(&body.key());
        let choice_bound = preparation
            .rows
            .iter()
            .filter(|row| matches!(row.kind, DecisionKind::Or(_)))
            .count();
        let mut plan = Self {
            root,
            owner,
            arm,
            source,
            matched,
            unmatched,
            unmatched_inactive,
            index: retained_index,
            body: body.key(),
            profile: parsed.profile.fingerprint(),
            source_identity: source_identity.finish(),
            body_range: body.range,
            body_structure: index.semantic_identity(),
            rows: preparation.rows,
            lookup,
            bytes: 0,
            identity: Digest::of_bytes(b"unpublished-pattern-plan"),
            choice_bound,
        };
        plan.bytes = plan
            .measure_bytes()
            .filter(|bytes| *bytes <= byte_limit)
            .ok_or_else(limit)?;
        plan.identity = plan.fingerprint();
        Ok(plan)
    }
    /// One cold owner capture. Body entry is validated by the source-plan owner
    /// before this call. Exact indexed root attachment does not rehash the body.
    /// Header collection is linear, and the reverse fold supplies each next-arm
    /// edge without repeatedly searching all following disabled siblings.
    pub(crate) fn prepare_owner(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &Arc<BodyIndex>,
        syntax: &ra_ap_syntax::SyntaxNode,
        byte_limit: usize,
    ) -> Result<Vec<ProductRoot>, Diagnostic> {
        if index.body() != &body.key() || index.profile() != &parsed.profile {
            return Err(invalid("staged product owner body/profile differs"));
        }
        let mut headers = Vec::new();
        let mut budget = 100_000usize;
        let mut bytes = 0usize;
        let mut append =
            |pattern: ast::Pat, arm: Option<NodeId>, attributed: bool| -> Result<(), Diagnostic> {
                bytes = bytes
                    .checked_add(
                        std::mem::size_of::<(NodeId, ast::Pat, Option<NodeId>, bool)>()
                            .checked_mul(4)
                            .ok_or_else(limit)?,
                    )
                    .filter(|n| *n <= byte_limit)
                    .ok_or_else(limit)?;
                let root = indexed_identity(body, index, pattern.syntax(), NodeRole::Pattern)?;
                headers.try_reserve(1).map_err(|_| limit())?;
                headers.push((root, pattern, arm, attributed));
                Ok(())
            };
        if let Some(expression) = ast::MatchExpr::cast(syntax.clone()) {
            let arms = expression
                .match_arm_list()
                .ok_or_else(|| invalid("staged match arms absent"))?;
            for arm in arms.arms() {
                budget = budget.checked_sub(1).ok_or_else(limit)?;
                let (enabled, attributed) = super::statement_cfg(arm.syntax(), index.cfg())?;
                if enabled {
                    let id = indexed_identity(body, index, arm.syntax(), NodeRole::MatchArm)?;
                    append(
                        arm.pat()
                            .ok_or_else(|| invalid("staged arm pattern absent"))?,
                        Some(id),
                        attributed,
                    )?;
                }
            }
        } else if let Some(condition) = ast::LetExpr::cast(syntax.clone()) {
            append(
                condition
                    .pat()
                    .ok_or_else(|| invalid("staged condition pattern absent"))?,
                None,
                false,
            )?;
        } else {
            return Err(invalid(
                "staged product owner requires actual match or let condition",
            ));
        }
        // Keep errors per root. Metadata exhaustion is an owner preparation
        // refusal; it cannot allocate unbounded independent per-arm budgets.
        bytes
            .checked_add(
                headers
                    .len()
                    .checked_mul(std::mem::size_of::<ProductRoot>())
                    .ok_or_else(limit)?,
            )
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(headers.len())
            .map_err(|_| limit())?;
        bytes = bytes
            .checked_add(
                result
                    .capacity()
                    .checked_mul(std::mem::size_of::<ProductRoot>())
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        let mut next = None;
        while let Some((root, pattern, arm, attributed)) = headers.pop() {
            let remaining = byte_limit
                .checked_sub(bytes)
                .and_then(|n| n.checked_sub(2 * std::mem::size_of::<usize>()))
                .ok_or_else(limit)?;
            let product = if attributed {
                Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "reached pattern arm attributes require ordinary macro integration".into(),
                ))
            } else {
                Self::capture_selected(
                    parsed,
                    body,
                    index,
                    Arc::clone(index),
                    root,
                    pattern,
                    next,
                    remaining,
                    &mut budget,
                )
                .map(Arc::new)
            };
            let charge = match &product {
                Ok(plan) => plan
                    .metadata_bytes()
                    .checked_add(2 * std::mem::size_of::<usize>())
                    .ok_or_else(limit)?,
                Err(error) => error.heap_bytes().ok_or_else(limit)?,
            };
            bytes = bytes
                .checked_add(charge)
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
            result.push(ProductRoot { root, product });
            if let Some(arm) = arm {
                next = Some(arm);
            }
        }
        result.sort_unstable_by_key(|row| row.root);
        if result.windows(2).any(|rows| rows[0].root == rows[1].root) {
            return Err(invalid("staged product root captured twice"));
        }
        Ok(result)
    }
    pub(crate) fn choice_bound(&self) -> usize {
        self.choice_bound
    }
    fn owner_receipt(
        body: &BodyHandle,
        index: &BodyIndex,
        root: &ast::Pat,
        next: Option<NodeId>,
    ) -> Result<(NodeId, Option<NodeId>, SlotId, PcKey, PcKey, PcKey), Diagnostic> {
        fn expression(
            body: &BodyHandle,
            index: &BodyIndex,
            expr: &ast::Expr,
        ) -> Result<NodeId, Diagnostic> {
            let range = expr.syntax().text_range();
            let range = SourceRange::checked(
                u32::from(range.start())
                    .checked_sub(body.range.start)
                    .ok_or_else(limit)?,
                u32::from(range.end())
                    .checked_sub(body.range.start)
                    .ok_or_else(limit)?,
            )?;
            index
                .nodes_at_range(range)
                .find(|node| matches!(node.role, NodeRole::Expression(_)))
                .map(|node| node.id)
                .ok_or_else(|| invalid("pattern scrutinee indexed location absent"))
        }
        let parent = root
            .syntax()
            .parent()
            .ok_or_else(|| invalid("pattern owner syntax absent"))?;
        if let Some(arm) = ast::MatchArm::cast(parent.clone()) {
            let owner = arm
                .syntax()
                .ancestors()
                .find_map(ast::MatchExpr::cast)
                .ok_or_else(|| invalid("pattern match owner absent"))?;
            let owner_id = indexed_identity(
                body,
                index,
                owner.syntax(),
                NodeRole::Expression(crate::syntax::body_index::ExpressionKind::Match),
            )?;
            let arm_id = indexed_identity(body, index, arm.syntax(), NodeRole::MatchArm)?;
            let source = expression(
                body,
                index,
                &owner
                    .expr()
                    .ok_or_else(|| invalid("pattern scrutinee absent"))?,
            )?;
            let source = index.nodes()[source.0 as usize]
                .value
                .ok_or_else(|| invalid("pattern scrutinee value absent"))?;
            let inactive = index.pc(
                next.unwrap_or(owner_id),
                if next.is_some() {
                    Phase::Entry
                } else {
                    Phase::Exit
                },
            )?;
            Ok((
                owner_id,
                Some(arm_id),
                source,
                index.pc(
                    arm_id,
                    if index.nodes()[arm_id.0 as usize].guard_scope.is_some() {
                        Phase::Operand(0)
                    } else {
                        Phase::Commit
                    },
                )?,
                index.pc(arm_id, Phase::PatternCleanup)?,
                inactive,
            ))
        } else if let Some(condition) = ast::LetExpr::cast(parent) {
            let owner = indexed_identity(
                body,
                index,
                condition.syntax(),
                NodeRole::Expression(crate::syntax::body_index::ExpressionKind::Other),
            )?;
            let source = expression(
                body,
                index,
                &condition
                    .expr()
                    .ok_or_else(|| invalid("condition scrutinee absent"))?,
            )?;
            let source = index.nodes()[source.0 as usize]
                .value
                .ok_or_else(|| invalid("condition scrutinee value absent"))?;
            let matched = index.pc(owner, Phase::Commit)?;
            let unmatched = index.pc(owner, Phase::PatternCleanup)?;
            Ok((owner, None, source, matched, unmatched.clone(), unmatched))
        } else {
            Err(invalid(
                "decision capture requires actual root arm/condition pattern",
            ))
        }
    }
    pub(crate) fn owner(&self) -> NodeId {
        self.owner
    }
    pub(crate) fn arm(&self) -> Option<NodeId> {
        self.arm
    }
    pub(crate) fn source(&self) -> SlotId {
        self.source
    }
    pub(crate) fn matched(&self) -> &PcKey {
        &self.matched
    }
    pub(crate) fn unmatched(&self) -> &PcKey {
        &self.unmatched
    }
    pub(crate) fn unmatched_inactive(&self) -> &PcKey {
        &self.unmatched_inactive
    }
    pub(crate) fn index(&self) -> &Arc<BodyIndex> {
        &self.index
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn require_runtime(
        &self,
        body: &crate::source::manifest::BodyKey,
        profile: &crate::core::profile::ProfileIdentity,
    ) -> Result<(), Diagnostic> {
        if body != &self.body || profile.fingerprint() != self.profile {
            return Err(invalid("pattern decision runtime body/profile differs"));
        }
        Ok(())
    }
    pub(crate) fn require_structure(&self, identity: Digest) -> Result<(), Diagnostic> {
        if identity != self.body_structure {
            return Err(invalid(
                "pattern decision runtime index/source structure differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn require_source(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
    ) -> Result<(), Diagnostic> {
        self.require_runtime(&body.key(), &parsed.profile)?;
        let mut source = StableEncoder::new("lrr/v1/structural-pattern-source");
        source.digest(parsed.content);
        source.record(&parsed.profile);
        source.record(&body.key());
        if source.finish() != self.source_identity || body.range != self.body_range {
            return Err(invalid("pattern decision lowering source/range differs"));
        }
        Ok(())
    }
    pub(crate) fn require_index(&self, index: &BodyIndex) -> Result<(), Diagnostic> {
        self.require_runtime(index.body(), index.profile())?;
        if (!std::ptr::eq(self.index.as_ref(), index)
            && index.semantic_identity() != self.body_structure)
            || index
                .nodes()
                .get(self.root.0 as usize)
                .is_none_or(|node| node.role != NodeRole::Pattern)
        {
            return Err(invalid("pattern decision prepared index/root differs"));
        }
        Ok(())
    }
    pub(crate) fn root(&self) -> NodeId {
        self.root
    }
    pub(crate) fn rows(&self) -> &[DecisionRow] {
        &self.rows
    }
    pub(crate) fn row(&self, node: NodeId) -> Option<&DecisionRow> {
        let row = self.lookup.binary_search_by_key(&node, |row| row.0).ok()?;
        self.rows.get(self.lookup[row].1)
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn measure_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.rows
                    .capacity()
                    .checked_mul(std::mem::size_of::<DecisionRow>())?,
            )?
            .checked_add(
                self.lookup
                    .capacity()
                    .checked_mul(std::mem::size_of::<(NodeId, usize)>())?,
            )?;
        for row in &self.rows {
            bytes = bytes.checked_add(
                row.source_path
                    .len()
                    .checked_mul(std::mem::size_of::<u32>())?,
            )?;
            bytes = bytes.checked_add(match &row.kind {
                DecisionKind::Tuple(children) | DecisionKind::Or(children) => {
                    children.len().checked_mul(std::mem::size_of::<NodeId>())?
                }
                DecisionKind::Unsupported(error) => error.heap_bytes()?,
                _ => 0,
            })?;
        }
        Some(bytes)
    }
}

impl PartialEq for PatternProductPlan {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for PatternProductPlan {}

impl StableEncode for PatternProductPlan {
    const DOMAIN: &'static str = "lrr/v1/structural-pattern-product-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.root.0);
        e.u32(self.owner.0);
        e.option(&self.arm, |e, arm| e.u32(arm.0));
        e.u32(self.source.0);
        e.record(&self.matched);
        e.record(&self.unmatched);
        e.record(&self.unmatched_inactive);
        e.u64(self.choice_bound as u64);
        e.record(&self.body);
        e.digest(self.profile);
        e.digest(self.source_identity);
        e.u32(self.body_range.start);
        e.u32(self.body_range.end);
        e.digest(self.body_structure);
        e.sequence(&self.rows, |e, row| {
            e.u32(row.node.0);
            e.option(&row.parent, |e, node| e.u32(node.0));
            e.option(&row.success, |e, next| match next {
                DecisionContinuation::Enter(node) => {
                    e.u8(0);
                    e.u32(node.0);
                }
                DecisionContinuation::Commit(node) => {
                    e.u8(1);
                    e.u32(node.0);
                }
            });
            e.sequence(&row.source_path, |e, field| e.u32(*field));
            match &row.kind {
                DecisionKind::Wildcard => e.u8(0),
                DecisionKind::Scalar(ScalarTestPlan::Literal(range)) => {
                    e.u8(1);
                    e.u32(range.start);
                    e.u32(range.end);
                }
                DecisionKind::Scalar(ScalarTestPlan::Range {
                    lower,
                    upper,
                    inclusive,
                }) => {
                    e.u8(2);
                    e.option(lower, |e, r| {
                        e.u32(r.start);
                        e.u32(r.end);
                    });
                    e.option(upper, |e, r| {
                        e.u32(r.start);
                        e.u32(r.end);
                    });
                    e.bool(*inclusive);
                }
                DecisionKind::Binding(intent) => {
                    e.u8(3);
                    e.option(&intent.canonical, |e, node| e.u32(node.0));
                    match intent.context {
                        BindingContext::Original => e.u8(0),
                        BindingContext::Normalized(context) => {
                            e.u8(1);
                            e.digest(Digest(context.0.0));
                        }
                        BindingContext::Unresolved(node) => {
                            e.u8(2);
                            e.u32(node.0);
                        }
                    }
                    e.bool(intent.conflicts.conjunction_duplicate);
                    e.bool(intent.conflicts.alternative_missing);
                    e.bool(intent.conflicts.written_mode);
                    e.bool(intent.conflicts.context_unresolved);
                    e.bool(intent.mode.by_reference);
                    e.bool(intent.mode.mutable);
                    e.option(&intent.edition, |e, edition| e.u16(*edition));
                    e.u8(match intent.uncertainty {
                        BindingUncertainty::ExplicitBinding => 0,
                        BindingUncertainty::BindingOrConstant => 1,
                    });
                    e.bool(intent.requires_namespace);
                    e.option(&intent.subpattern, |e, node| e.u32(node.0));
                }
                DecisionKind::Tuple(children) => {
                    e.u8(4);
                    e.sequence(children, |e, node| e.u32(node.0));
                }
                DecisionKind::Or(children) => {
                    e.u8(5);
                    e.sequence(children, |e, node| e.u32(node.0));
                }
                DecisionKind::Group(inner) => {
                    e.u8(6);
                    e.u32(inner.0);
                }
                DecisionKind::Unsupported(error) => {
                    e.u8(7);
                    e.string(error.code);
                    e.string(&error.message);
                }
            }
        });
        e.sequence(&self.lookup, |e, (node, row)| {
            e.u32(node.0);
            e.u64(*row as u64);
        });
    }
}

#[cfg(test)]
#[path = "pattern_product_tests.rs"]
mod tests;
