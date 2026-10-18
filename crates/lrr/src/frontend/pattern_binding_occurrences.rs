//! Immutable pattern-name source observations. These are not binding grants.
//! The final authority join needs current relevant scope selection, local-prefix
//! lookup and compiler-prelude proofs; existing scalar admission stays intact.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::DependencyStamp,
        key::{BucketKey, QueryKey, SealedNameKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::SealDomain,
        value::QueryValue,
    },
    facade::value_binding_prelude::{RetainedPreludeName, RetainedValueBindingPrelude},
    frontend::{names::NameLookup, source_body_context::PreparedBodyContext},
    ir::pc::{NodeId, PcKey, Phase},
    source::manifest::Namespace,
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        local_bindings::{BindingForm, LocalBindingRecipe, PatternNameKind},
        pattern_bindings::{
            BindingConflicts, BindingContext, BindingUncertainty, WrittenBindingMode,
        },
        spans::SourceRange,
    },
};
use std::{collections::BTreeMap, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "pattern binding occurrence budget".into(),
    )
}
const WORK: usize = usize::MAX;
fn add(bytes: usize, amount: usize, _cap: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(amount)
        .ok_or_else(limit)
}
fn storage<T>(count: usize) -> Result<usize, Diagnostic> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(limit)
}

/// Exact source-local join status. Disabled/unavailable source candidates are
/// retained per occurrence rather than rejecting an otherwise cold body plan.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PatternLocalOccurrence {
    Joined {
        identity: Digest,
        scope: u32,
        visible_from: u64,
    },
    Missing,
    Conflicting,
}
/// One actual written occurrence, distinct from the canonical candidate. No
/// constructor accepts caller-provided spelling, lookup or Boolean absence.
#[derive(Debug)]
pub(crate) struct PatternBindingOccurrence {
    pattern: NodeId,
    root: NodeId,
    owner: NodeId,
    arm: Option<NodeId>,
    canonical: Option<NodeId>,
    range: SourceRange,
    name_range: SourceRange,
    name: String,
    context: BindingContext,
    conflicts: BindingConflicts,
    mode: WrittenBindingMode,
    edition: Option<u16>,
    uncertainty: BindingUncertainty,
    local: PatternLocalOccurrence,
}
impl PatternBindingOccurrence {
    pub(crate) fn pattern(&self) -> NodeId {
        self.pattern
    }
    pub(crate) fn root(&self) -> NodeId {
        self.root
    }
    pub(crate) fn owner(&self) -> NodeId {
        self.owner
    }
    pub(crate) fn arm(&self) -> Option<NodeId> {
        self.arm
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn edition(&self) -> Option<u16> {
        self.edition
    }
    pub(crate) fn uncertainty(&self) -> BindingUncertainty {
        self.uncertainty
    }
    pub(crate) fn canonical_candidate(&self) -> Option<NodeId> {
        self.canonical
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn name_range(&self) -> SourceRange {
        self.name_range
    }
    pub(crate) fn context(&self) -> BindingContext {
        self.context
    }
    pub(crate) fn conflicts(&self) -> BindingConflicts {
        self.conflicts
    }
    pub(crate) fn mode(&self) -> WrittenBindingMode {
        self.mode
    }
    pub(crate) fn local_source(&self) -> PatternLocalOccurrence {
        self.local
    }
    fn encode(&self, e: &mut StableEncoder) {
        e.u32(self.pattern.0);
        e.u32(self.root.0);
        e.u32(self.owner.0);
        e.option(&self.arm, |e, arm| e.u32(arm.0));
        e.option(&self.canonical, |e, id| e.u32(id.0));
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.u32(self.name_range.start);
        e.u32(self.name_range.end);
        e.string(&self.name);
        match self.context {
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
        e.bool(self.conflicts.conjunction_duplicate);
        e.bool(self.conflicts.alternative_missing);
        e.bool(self.conflicts.written_mode);
        e.bool(self.conflicts.context_unresolved);
        e.bool(self.mode.by_reference);
        e.bool(self.mode.mutable);
        e.option(&self.edition, |e, edition| e.u16(*edition));
        e.u8(match self.uncertainty {
            BindingUncertainty::ExplicitBinding => 0,
            BindingUncertainty::BindingOrConstant => 1,
        });
        match self.local {
            PatternLocalOccurrence::Joined {
                identity,
                scope,
                visible_from,
            } => {
                e.u8(0);
                e.digest(identity);
                e.u32(scope);
                e.u64(visible_from);
            }
            PatternLocalOccurrence::Missing => e.u8(1),
            PatternLocalOccurrence::Conflicting => e.u8(2),
        }
    }
}

pub(crate) struct PreparedPatternBindingOccurrences {
    source: Arc<PreparedBodyContext>,
    rows: Vec<PatternBindingOccurrence>,
    local_patterns: BTreeMap<Digest, NodeId>,
    bytes: usize,
    identity: Digest,
}
impl std::fmt::Debug for PreparedPatternBindingOccurrences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedPatternBindingOccurrences")
            .field("identity", &self.identity)
            .field("rows", &self.rows)
            .finish()
    }
}
fn owning_pattern(
    index: &BodyIndex,
    pattern: NodeId,
    work: &mut usize,
) -> Result<Option<(NodeId, NodeId, Option<NodeId>)>, Diagnostic> {
    let mut node = index
        .nodes()
        .get(pattern.0 as usize)
        .ok_or_else(|| fault("retained occurrence node absent"))?;
    let mut root = pattern;
    let mut arm = None;
    for _ in 0usize.. {
        *work = work.checked_sub(1).ok_or_else(limit)?;
        let Some(parent) = node.parent else {
            return Ok(None);
        };
        node = index
            .nodes()
            .get(parent.0 as usize)
            .ok_or_else(|| fault("retained occurrence parent absent"))?;
        match node.role {
            NodeRole::Pattern if arm.is_none() => root = node.id,
            NodeRole::MatchArm if arm.is_none() => arm = Some(node.id),
            // A pattern function's parameter list owns its parameter patterns
            // (E-R5-010), bound at the function's entry.
            NodeRole::Syntax if node.pattern_statement && arm.is_none() => {
                return Ok(Some((root, node.id, None)));
            }
            NodeRole::Syntax => {}
            NodeRole::Statement if arm.is_none() => return Ok(Some((root, node.id, None))),
            NodeRole::Expression(ExpressionKind::Match) if arm.is_some() => {
                return Ok(Some((root, node.id, arm)));
            }
            // A closure parameter: bound at the closure frame's entry.
            NodeRole::Expression(ExpressionKind::Closure) if arm.is_none() => {
                return Ok(Some((root, node.id, None)));
            }
            // A `for` pattern: bound by each iteration's `Some(PAT)` test.
            NodeRole::Expression(ExpressionKind::For) if arm.is_none() => {
                return Ok(Some((root, node.id, None)));
            }
            NodeRole::Expression(ExpressionKind::Other)
                if arm.is_none() && node.clause_scope.is_some() =>
            {
                return Ok(Some((root, node.id, None)));
            }
            _ => return Ok(None),
        }
    }
    Err(fault("retained occurrence ancestry budget"))
}
impl PreparedPatternBindingOccurrences {
    /// Cold preparation from an opaque, genuinely admitted source body. The
    /// shared body facts/index are retained by Arc; no syntax walk is repeated.
    pub(crate) fn prepare(
        db: &mut Database,
        source: Arc<PreparedBodyContext>,
        byte_limit: usize,
    ) -> QueryPoll<Self> {
        match source.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        QueryPoll::Ready(Self::capture(source, byte_limit))
    }
    pub(in crate::frontend) fn capture(
        source: Arc<PreparedBodyContext>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        let admitted = source.source();
        let handle = admitted.body();
        let index = admitted.index();
        let mut bytes = add(0, std::mem::size_of::<Self>(), byte_limit)?;
        let mut locals: BTreeMap<(u32, u32), Option<&LocalBindingRecipe>> = BTreeMap::new();
        for binding in &source.facts().bindings {
            if !matches!(
                binding.form,
                BindingForm::Match
                    | BindingForm::Condition
                    | BindingForm::Let
                    | BindingForm::For
                    | BindingForm::Parameter
            ) {
                continue;
            }
            bytes = add(
                bytes,
                storage::<((u32, u32), Option<&LocalBindingRecipe>)>(1)?
                    .checked_add(128)
                    .ok_or_else(limit)?,
                byte_limit,
            )?;
            let key = (binding.name_range.start, binding.name_range.end);
            match locals.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(Some(binding));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    entry.insert(None);
                }
            }
        }
        let mut rows = Vec::new();
        let mut work = WORK;
        for node in index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Pattern)
        {
            let Some(group) = index.pattern_bindings().group_for(node.id) else {
                continue;
            };
            let Some(occurrence) = group.occurrence(node.id) else {
                continue;
            };
            let Some((root, owner, arm)) = owning_pattern(index, node.id, &mut work)? else {
                continue;
            };
            let let_owner = index.nodes()[owner.0 as usize].role == NodeRole::Statement;
            let closure_owner = index.nodes()[owner.0 as usize].role
                == NodeRole::Expression(ExpressionKind::Closure);
            let for_owner = index.nodes()[owner.0 as usize].role
                == NodeRole::Expression(ExpressionKind::For);
            let parameter_owner = index.nodes()[owner.0 as usize].role == NodeRole::Syntax;
            if let_owner {
                use ra_ap_syntax::{AstNode, ast};
                // Statement role alone is not a let-owner certificate.
                let syntax = admitted
                    .parsed()
                    .entered_node_syntax(handle, index, owner)?;
                let Some(statement) = syntax.and_then(ast::LetStmt::cast) else {
                    continue;
                };
                let Some(pattern) = statement.pat() else {
                    continue;
                };
                let root_node = &index.nodes()[root.0 as usize];
                let actual = crate::syntax::ast_adapter::range(pattern.syntax().text_range());
                if actual.start
                    != handle
                        .range
                        .start
                        .checked_add(root_node.range.start)
                        .ok_or_else(limit)?
                    || actual.end
                        != handle
                            .range
                            .start
                            .checked_add(root_node.range.end)
                            .ok_or_else(limit)?
                {
                    return Err(fault(
                        "retained let occurrence root differs from actual statement",
                    ));
                }
            }
            let range = SourceRange::checked(
                handle
                    .range
                    .start
                    .checked_add(node.range.start)
                    .ok_or_else(limit)?,
                handle
                    .range
                    .start
                    .checked_add(node.range.end)
                    .ok_or_else(limit)?,
            )?;
            if !handle.range.contains(range) || !range.contains(occurrence.name_range) {
                return Err(fault("retained occurrence escapes admitted body/pattern"));
            }
            let local = match locals.get(&(occurrence.name_range.start, occurrence.name_range.end))
            {
                None => PatternLocalOccurrence::Missing,
                Some(Some(binding))
                    if binding.name == group.name()
                        && (matches!(
                            (binding.form, arm, let_owner),
                            (BindingForm::Match, Some(_), false)
                                | (BindingForm::Condition, None, false)
                                | (BindingForm::Let, None, true)
                        ) || (closure_owner
                            && binding.form == BindingForm::Let
                            && arm.is_none())
                            || (for_owner && binding.form == BindingForm::For && arm.is_none())
                            || (parameter_owner
                                && binding.form == BindingForm::Parameter
                                && arm.is_none()))
                        && matches!(
                            (binding.classification, occurrence.uncertainty),
                            (
                                PatternNameKind::ExplicitBinding,
                                BindingUncertainty::ExplicitBinding
                            ) | (
                                PatternNameKind::BindingOrConstant,
                                BindingUncertainty::BindingOrConstant
                            )
                        )
                        && binding.visible_from > u64::from(range.start)
                        && binding.pattern_range.contains(occurrence.name_range) =>
                {
                    PatternLocalOccurrence::Joined {
                        identity: binding.identity,
                        scope: binding.scope,
                        visible_from: binding.visible_from,
                    }
                }
                _ => PatternLocalOccurrence::Conflicting,
            };
            // Reserve before growing. Final metadata records actual capacities;
            // transient local-map storage is bounded here, then released.
            bytes = add(bytes, storage::<PatternBindingOccurrence>(4)?, byte_limit)?;
            bytes = add(
                bytes,
                group.name().len().checked_mul(4).ok_or_else(limit)?,
                byte_limit,
            )?;
            rows.try_reserve(1).map_err(|_| limit())?;
            rows.push(PatternBindingOccurrence {
                pattern: node.id,
                root,
                owner,
                arm,
                canonical: group.canonical_candidate(),
                range,
                name_range: occurrence.name_range,
                name: group.name().to_owned(),
                context: group.context(),
                conflicts: group.conflicts(),
                mode: occurrence.mode,
                edition: occurrence.edition,
                uncertainty: occurrence.uncertainty,
                local,
            });
        }
        rows.sort_unstable_by_key(|row| row.pattern);
        if rows
            .windows(2)
            .any(|rows| rows[0].pattern == rows[1].pattern)
        {
            return Err(fault("retained pattern occurrence duplicated"));
        }
        let mut retained = add(
            std::mem::size_of::<Self>(),
            storage::<PatternBindingOccurrence>(rows.capacity())?,
            byte_limit,
        )?;
        for row in &rows {
            retained = add(retained, row.name.capacity(), byte_limit)?;
        }
        let mut local_patterns = BTreeMap::new();
        for row in &rows {
            if let PatternLocalOccurrence::Joined { identity, .. } = row.local {
                let row_bytes = 16 * std::mem::size_of::<(Digest, NodeId)>() + 256;
                bytes = add(bytes, row_bytes, byte_limit)?;
                retained = add(retained, row_bytes, byte_limit)?;
                if local_patterns.insert(identity, row.pattern).is_some() {
                    return Err(fault(
                        "retained local binding identity has multiple occurrences",
                    ));
                }
            }
        }
        let mut plan = Self {
            source,
            rows,
            local_patterns,
            bytes: retained,
            identity: Digest::of_bytes(b"unpublished occurrence plan"),
        };
        plan.identity = plan.fingerprint();
        Ok(plan)
    }
    pub(crate) fn source(&self) -> &Arc<PreparedBodyContext> {
        &self.source
    }
    pub(crate) fn rows(&self) -> &[PatternBindingOccurrence] {
        &self.rows
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn canonical_local_source(
        &self,
        occurrence: NodeId,
    ) -> Option<PatternLocalOccurrence> {
        let row = self.rows.get(
            self.rows
                .binary_search_by_key(&occurrence, |row| row.pattern)
                .ok()?,
        )?;
        let canonical = row.canonical?;
        self.rows
            .get(
                self.rows
                    .binary_search_by_key(&canonical, |row| row.pattern)
                    .ok()?,
            )
            .map(|row| row.local)
    }
    pub(crate) fn for_local(&self, identity: Digest) -> Option<&PatternBindingOccurrence> {
        let pattern = self.local_patterns.get(&identity)?;
        let row = self.rows.get(
            self.rows
                .binary_search_by_key(pattern, |row| row.pattern)
                .ok()?,
        )?;
        matches!(row.local, PatternLocalOccurrence::Joined { identity: actual, .. } if actual == identity).then_some(row)
    }
    /// Source selection only. Returning a candidate never permits StageBinding.
    pub(crate) fn select(
        &self,
        index: &BodyIndex,
        pc: &PcKey,
        occurrence: NodeId,
    ) -> Result<&PatternBindingOccurrence, Diagnostic> {
        let admitted = self.source.source();
        if pc.body != admitted.body().key()
            || index.semantic_identity() != admitted.index().semantic_identity()
            || index.body() != admitted.index().body()
            || index.profile() != admitted.index().profile()
            || !matches!(Phase::from_code(pc.phase), Phase::Entry | Phase::Commit)
        {
            return Err(fault("retained occurrence source/body/index/phase differs"));
        }
        let position = self
            .rows
            .binary_search_by_key(&occurrence, |row| row.pattern)
            .map_err(|_| fault("retained pattern occurrence absent"))?;
        let row = &self.rows[position];
        if pc.node != occurrence.0
            && !(row.arm.is_none() && row.root == occurrence && pc.node == row.owner.0)
        {
            return Err(fault("retained occurrence instruction owner differs"));
        }
        Ok(row)
    }
}
impl StableEncode for PreparedPatternBindingOccurrences {
    const DOMAIN: &'static str = "lrr/v2/retained-pattern-binding-source-occurrences";
    fn encode_fields(&self, e: &mut StableEncoder) {
        let admitted = self.source.source();
        e.record(&admitted.parsed().key());
        e.record(&admitted.body().key());
        e.u32(admitted.body().range.start);
        e.u32(admitted.body().range.end);
        e.digest(admitted.index().semantic_identity());
        e.record(admitted.scope());
        e.sequence(&self.rows, |e, row| row.encode(e));
        e.u64(self.local_patterns.len() as u64);
        for (identity, pattern) in &self.local_patterns {
            e.digest(*identity);
            e.u32(pattern.0);
        }
    }
}

/// A module answer is only one component of name admission. Neither Absent nor
/// a positive result below is accepted as local/prelude/hygienic binding proof.
#[derive(Debug)]
struct ModuleObservation {
    dependency: DependencyStamp,
    answer: NameLookup,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PatternModuleSlot {
    Observed(usize),
    /// Scope/hygiene origin receipt is not established by an original bucket.
    PendingHygienicScope,
    PendingSourceOccurrence,
    PendingScopeCertificate,
}
/// Explicit missing authority, rather than a negative guard mislabeled as a
/// valid binding contract. Only a future common query/local/prelude join can
/// produce the separate final binding proof consumed by the ordinary verifier.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PatternBindingAuthorityGaps {
    pub(crate) current_relevant_scope: bool,
    pub(crate) local_prefix: bool,
    pub(crate) compiler_prelude: bool,
    pub(crate) hygienic_scope: bool,
    pub(crate) source_occurrence: bool,
}
#[derive(Debug)]
pub(crate) struct PreparedPatternModuleObservations {
    occurrences: Arc<PreparedPatternBindingOccurrences>,
    slots: Vec<PatternModuleSlot>,
    observations: Vec<ModuleObservation>,
    prelude: Option<RetainedValueBindingPrelude>,
    gaps: PatternBindingAuthorityGaps,
    bytes: usize,
    identity: Digest,
}
impl PreparedPatternModuleObservations {
    /// Genuine query results only: callers cannot supply a NameLookup, seal or
    /// absence Boolean. This does not mint a final binding occurrence proof.
    pub(crate) fn prepare(
        db: &mut Database,
        occurrences: Arc<PreparedPatternBindingOccurrences>,
        byte_limit: usize,
    ) -> QueryPoll<Self> {
        match occurrences.source.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        Self::capture(db, occurrences, byte_limit)
    }
    fn capture(
        db: &mut Database,
        occurrences: Arc<PreparedPatternBindingOccurrences>,
        byte_limit: usize,
    ) -> QueryPoll<Self> {
        let admitted = occurrences.source.source();
        let mut bytes = match add(
            std::mem::size_of::<Self>(),
            storage::<PatternModuleSlot>(occurrences.rows.len()).unwrap_or(usize::MAX),
            byte_limit,
        ) {
            Ok(bytes) => bytes,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut slots = Vec::new();
        if slots.try_reserve_exact(occurrences.rows.len()).is_err() {
            return QueryPoll::Ready(Err(limit()));
        }
        let mut observations: Vec<ModuleObservation> = Vec::new();
        let mut names: BTreeMap<&str, usize> = BTreeMap::new();
        let mut certificate = None;
        let mut selected_certificate = false;
        let mut prelude = None;
        let mut selected_prelude = false;
        let source = admitted.original_source().filter(|source| {
            source.selection().outline.parse == admitted.parsed().key()
                && admitted.scope().owner.is_none()
        });
        for row in &occurrences.rows {
            if !matches!(row.local, PatternLocalOccurrence::Joined { .. }) {
                slots.push(PatternModuleSlot::PendingSourceOccurrence);
                continue;
            }
            let Some(source) = source.filter(|_| row.context == BindingContext::Original) else {
                slots.push(PatternModuleSlot::PendingHygienicScope);
                continue;
            };
            let position = if let Some(position) = names.get(row.name()) {
                *position
            } else {
                if !selected_certificate {
                    certificate = match db
                        .current_lexical_scope_certificate(source.selection(), admitted.scope())
                    {
                        Ok(certificate) => certificate,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    };
                    selected_certificate = true;
                }
                let Some(certificate) = certificate.as_deref() else {
                    slots.push(PatternModuleSlot::PendingScopeCertificate);
                    continue;
                };
                let crate::db::dependency::InputValue::DiscoverySeal(input) = certificate else {
                    return QueryPoll::Ready(Err(fault(
                        "pattern module certificate variant differs",
                    )));
                };
                if input.seal.unresolved_frontiers() != 0
                    || !matches!(input.seal.domain(), SealDomain::Scope { id, namespace } if *id == admitted.scope().fingerprint() && *namespace == Namespace::Value)
                {
                    return QueryPoll::Ready(Err(fault("pattern module scope is not complete")));
                }
                // One genuine published prelude receipt per source/scope. This
                // retains its already-minted policy, not per-name parse replay.
                if !selected_prelude {
                    selected_prelude = true;
                    let mut selected = input
                        .value_binding_preludes
                        .iter()
                        .filter(|proof| proof.source() == source.selection());
                    if let Some(proof) = selected.next() {
                        if selected.next().is_some() {
                            return QueryPoll::Ready(Err(fault(
                                "pattern published Value prelude is ambiguous",
                            )));
                        }
                        let reserve = match proof.metadata_bytes().and_then(|bytes| {
                            add(
                                bytes,
                                std::mem::size_of::<RetainedValueBindingPrelude>(),
                                byte_limit,
                            )
                        }) {
                            Ok(bytes) => bytes,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                        bytes = match add(bytes, reserve, byte_limit) {
                            Ok(bytes) => bytes,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                        prelude = match proof.retain(source, admitted.scope()) {
                            Ok(proof) => Some(proof),
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                    }
                }
                let preparation = admitted
                    .scope()
                    .heap_bytes()
                    .checked_add(row.name.len())
                    .and_then(|n| n.checked_add(std::mem::size_of::<SealedNameKey>()))
                    .and_then(|n| n.checked_mul(4))
                    .and_then(|n| n.checked_add(128));
                bytes = match preparation
                    .and_then(|n| bytes.checked_add(n))
                    .filter(|n| *n <= byte_limit)
                {
                    Some(bytes) => bytes,
                    None => return QueryPoll::Ready(Err(limit())),
                };
                let key = SealedNameKey {
                    seal: input.seal.key(),
                    bucket: BucketKey {
                        scope: admitted.scope().clone(),
                        namespace: Namespace::Value,
                        name: row.name.clone(),
                    },
                };
                let query = QueryKey::SealedName(Box::new(key.clone()));
                let outcome = match db.fetch(&SyntaxProvider, query) {
                    QueryPoll::Ready(Ok(outcome)) => outcome,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let QueryValue::SealedName {
                    key: actual,
                    lookup,
                } = outcome.value.as_ref()
                else {
                    return QueryPoll::Ready(Err(fault(
                        "pattern module query answer variant differs",
                    )));
                };
                if actual.as_ref() != &key || !outcome.cacheable {
                    return QueryPoll::Ready(Err(fault(
                        "pattern module query answer is not retained",
                    )));
                }
                let extra = match lookup.metadata_bytes() {
                    Ok(extra) => extra,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                bytes = match add(
                    bytes,
                    extra
                        .checked_add(storage::<ModuleObservation>(4).unwrap_or(usize::MAX))
                        .unwrap_or(usize::MAX),
                    byte_limit,
                ) {
                    Ok(bytes) => bytes,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                if observations.try_reserve(1).is_err() {
                    return QueryPoll::Ready(Err(limit()));
                }
                let position = observations.len();
                observations.push(ModuleObservation {
                    dependency: DependencyStamp::Query {
                        key: QueryKey::SealedName(Box::new(key)),
                        fingerprint: outcome.fingerprint,
                    },
                    answer: lookup.clone(),
                });
                names.insert(row.name.as_str(), position);
                position
            };
            slots.push(PatternModuleSlot::Observed(position));
        }
        let mut retained = match storage::<PatternModuleSlot>(slots.capacity())
            .and_then(|n| add(std::mem::size_of::<Self>(), n, byte_limit))
            .and_then(|n| {
                add(
                    n,
                    storage::<ModuleObservation>(observations.capacity())?,
                    byte_limit,
                )
            }) {
            Ok(bytes) => bytes,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        for observation in &observations {
            let heap = observation.dependency.heap_bytes().checked_add(
                observation
                    .answer
                    .metadata_bytes()
                    .ok()
                    .and_then(|size| size.checked_sub(std::mem::size_of::<NameLookup>()))
                    .unwrap_or(usize::MAX),
            );
            retained = match heap
                .and_then(|n| retained.checked_add(n))
                .filter(|n| *n <= byte_limit)
            {
                Some(bytes) => bytes,
                None => return QueryPoll::Ready(Err(limit())),
            };
        }
        if let Some(prelude) = &prelude {
            retained = match prelude
                .metadata_bytes()
                .checked_sub(std::mem::size_of::<RetainedValueBindingPrelude>())
                .and_then(|heap| retained.checked_add(heap))
                .filter(|n| *n <= byte_limit)
            {
                Some(bytes) => bytes,
                None => return QueryPoll::Ready(Err(limit())),
            };
        }
        // Drop temporary maps/borrows before moving the retained source root.
        drop(names);
        let gaps = PatternBindingAuthorityGaps {
            current_relevant_scope: true,
            local_prefix: true,
            compiler_prelude: prelude.is_none(),
            hygienic_scope: slots.contains(&PatternModuleSlot::PendingHygienicScope),
            source_occurrence: slots.contains(&PatternModuleSlot::PendingSourceOccurrence),
        };
        let mut plan = Self {
            occurrences,
            slots,
            observations,
            prelude,
            gaps,
            bytes: retained,
            identity: Digest::of_bytes(b"unpublished module observations"),
        };
        plan.identity = plan.fingerprint();
        QueryPoll::Ready(Ok(plan))
    }
    pub(crate) fn remaining_authority(&self) -> PatternBindingAuthorityGaps {
        self.gaps
    }
    pub(crate) fn occurrences(&self) -> &Arc<PreparedPatternBindingOccurrences> {
        &self.occurrences
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn slots(&self) -> &[PatternModuleSlot] {
        &self.slots
    }
    pub(crate) fn answer(&self, index: usize) -> Option<&NameLookup> {
        self.observations.get(index).map(|row| &row.answer)
    }
    pub(crate) fn prelude(&self) -> Option<&RetainedValueBindingPrelude> {
        self.prelude.as_ref()
    }
    pub(crate) fn prelude_answer(&self, occurrence: NodeId) -> Option<RetainedPreludeName> {
        let position = self
            .occurrences
            .rows
            .binary_search_by_key(&occurrence, |row| row.pattern)
            .ok()?;
        if !matches!(
            self.slots.get(position),
            Some(PatternModuleSlot::Observed(_))
        ) {
            return None;
        }
        Some(
            self.prelude
                .as_ref()?
                .lookup(self.occurrences.rows[position].name()),
        )
    }
    pub(crate) fn dependencies(&self) -> impl Iterator<Item = &DependencyStamp> {
        self.observations.iter().map(|row| &row.dependency)
    }
}
impl StableEncode for PreparedPatternModuleObservations {
    const DOMAIN: &'static str = "lrr/v1/retained-pattern-module-name-observations";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.occurrences.identity());
        e.option(&self.prelude, |e, prelude| e.record(prelude));
        e.bool(self.gaps.current_relevant_scope);
        e.bool(self.gaps.local_prefix);
        e.bool(self.gaps.compiler_prelude);
        e.bool(self.gaps.hygienic_scope);
        e.bool(self.gaps.source_occurrence);
        e.sequence(&self.slots, |e, slot| match slot {
            PatternModuleSlot::Observed(index) => {
                e.u8(0);
                e.u64(*index as u64);
            }
            PatternModuleSlot::PendingHygienicScope => e.u8(1),
            PatternModuleSlot::PendingSourceOccurrence => e.u8(2),
            PatternModuleSlot::PendingScopeCertificate => e.u8(3),
        });
        e.sequence(&self.observations, |e, row| {
            e.record(&row.dependency);
            e.record(&row.answer);
        });
    }
}
