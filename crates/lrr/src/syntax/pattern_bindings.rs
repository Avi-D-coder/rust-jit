//! Retained syntactic binding groups. These facts neither resolve a constant
//! name nor authorize publishing a local cell. Or-pattern groups preserve actual
//! hygiene classes and record unresolved source conflicts instead of guessing.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::MacroName,
    ir::pc::NodeId,
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
        spans::SourceRange,
        token_wire::NameContextKey,
    },
};
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasName},
};
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum BindingContext {
    Original,
    Normalized(NameContextKey),
    /// This occurrence cannot join another candidate by spelling alone.
    Unresolved(NodeId),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WrittenBindingMode {
    pub by_reference: bool,
    pub mutable: bool,
}
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct BindingConflicts {
    pub conjunction_duplicate: bool,
    pub alternative_missing: bool,
    pub written_mode: bool,
    pub context_unresolved: bool,
}
impl BindingConflicts {
    pub fn is_resolved(self) -> bool {
        self == Self::default()
    }
    fn include(&mut self, other: Self) {
        self.conjunction_duplicate |= other.conjunction_duplicate;
        self.alternative_missing |= other.alternative_missing;
        self.written_mode |= other.written_mode;
        self.context_unresolved |= other.context_unresolved;
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingUncertainty {
    ExplicitBinding,
    BindingOrConstant,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingOccurrence {
    pub pattern: NodeId,
    pub name_range: SourceRange,
    pub edition: Option<u16>,
    pub mode: WrittenBindingMode,
    pub at_binding: bool,
    pub uncertainty: BindingUncertainty,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingGroup {
    name: String,
    context: BindingContext,
    canonical: NodeId,
    occurrences: Vec<BindingOccurrence>,
    conflicts: BindingConflicts,
    requires_namespace: bool,
}
impl BindingGroup {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn context(&self) -> BindingContext {
        self.context
    }
    pub fn occurrences(&self) -> &[BindingOccurrence] {
        &self.occurrences
    }
    pub(crate) fn occurrence(&self, pattern: NodeId) -> Option<&BindingOccurrence> {
        self.occurrences
            .binary_search_by_key(&pattern, |occurrence| occurrence.pattern)
            .ok()
            .map(|index| &self.occurrences[index])
    }
    pub fn conflicts(&self) -> BindingConflicts {
        self.conflicts
    }
    /// A canonical *candidate*, never a namespace/binding authority witness.
    pub fn canonical_candidate(&self) -> Option<NodeId> {
        self.conflicts.is_resolved().then_some(self.canonical)
    }
    pub fn requires_namespace(&self) -> bool {
        self.requires_namespace
    }
    pub fn canonical_name_range(&self) -> SourceRange {
        self.occurrences[0].name_range
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
struct RootGroups {
    root: NodeId,
    first: usize,
    end: usize,
}
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct PatternBindings {
    roots: Vec<RootGroups>,
    groups: Vec<BindingGroup>,
    occurrences: Vec<(NodeId, usize)>,
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "pattern binding source fact budget".into(),
    )
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
}
struct Budget {
    work: usize,
    bytes: usize,
    limit: usize,
}
impl Budget {
    fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(1)
            .filter(|work| *work <= 100_000)
            .ok_or_else(limit)?;
        // Reserve conservatively for allocator Vec growth and transient maps
        // before allocating; final retained accounting uses actual capacities.
        let bytes = bytes.checked_mul(4).ok_or_else(limit)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|bytes| *bytes <= self.limit)
            .ok_or_else(limit)?;
        Ok(())
    }
}
#[derive(Default)]
struct Pending {
    occurrences: Vec<BindingOccurrence>,
    conflicts: BindingConflicts,
}
type Candidates = BTreeMap<(String, BindingContext), Pending>;

impl PatternBindings {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn groups(&self, root: NodeId) -> &[BindingGroup] {
        self.roots
            .binary_search_by_key(&root, |entry| entry.root)
            .ok()
            .map_or(&[], |i| {
                &self.groups[self.roots[i].first..self.roots[i].end]
            })
    }
    pub fn group_for(&self, occurrence: NodeId) -> Option<&BindingGroup> {
        let row = self
            .occurrences
            .binary_search_by_key(&occurrence, |row| row.0)
            .ok()?;
        self.groups.get(self.occurrences[row].1)
    }
    pub fn metadata_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.roots
                    .capacity()
                    .checked_mul(std::mem::size_of::<RootGroups>())?,
            )?
            .checked_add(
                self.groups
                    .capacity()
                    .checked_mul(std::mem::size_of::<BindingGroup>())?,
            )?
            .checked_add(
                self.occurrences
                    .capacity()
                    .checked_mul(std::mem::size_of::<(NodeId, usize)>())?,
            )?;
        for group in &self.groups {
            bytes = bytes.checked_add(group.name.capacity())?.checked_add(
                group
                    .occurrences
                    .capacity()
                    .checked_mul(std::mem::size_of::<BindingOccurrence>())?,
            )?;
        }
        Some(bytes)
    }
    /// Call once after the ordinary index's sorted range rows are finalized.
    /// Incomplete hygiene joins become typed unresolved candidates, not a new
    /// generated-source exclusion and not a same-spelling binding guess.
    pub(crate) fn prepare(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if index.body() != &body.key() || index.profile() != &parsed.profile {
            return Err(invalid("binding fact body/profile differs"));
        }
        if byte_limit == 0 || byte_limit > 32 * 1024 * 1024 {
            return Err(limit());
        }
        let mut budget = Budget {
            work: 0,
            bytes: std::mem::size_of::<Self>(),
            limit: byte_limit,
        };
        let mut result = Self::empty();
        for node in index.pattern_locations() {
            budget.charge(0)?;
            if nested_pattern(index, node.parent, &mut budget)? {
                continue;
            }
            let syntax = parsed
                .entered_node_syntax(body, index, node.id)?
                .and_then(ast::Pat::cast)
                .ok_or_else(|| invalid("indexed binding pattern attachment absent"))?;
            let candidates = collect(parsed, body, index, syntax, 0, &mut budget)?;
            let first = result.groups.len();
            budget.charge(std::mem::size_of::<RootGroups>())?;
            result.roots.try_reserve(1).map_err(|_| limit())?;
            for ((name, context), mut pending) in candidates {
                pending
                    .occurrences
                    .sort_unstable_by_key(|occurrence| occurrence.pattern);
                let canonical = pending.occurrences[0].pattern;
                let group = result.groups.len();
                budget.charge(std::mem::size_of::<BindingGroup>())?;
                result.groups.try_reserve(1).map_err(|_| limit())?;
                for occurrence in &pending.occurrences {
                    budget.charge(std::mem::size_of::<(NodeId, usize)>())?;
                    result.occurrences.try_reserve(1).map_err(|_| limit())?;
                    result.occurrences.push((occurrence.pattern, group));
                }
                let requires_namespace = pending.occurrences.iter().any(|occurrence| {
                    occurrence.uncertainty == BindingUncertainty::BindingOrConstant
                });
                result.groups.push(BindingGroup {
                    name,
                    context,
                    canonical,
                    occurrences: pending.occurrences,
                    conflicts: pending.conflicts,
                    requires_namespace,
                });
            }
            result.roots.push(RootGroups {
                root: node.id,
                first,
                end: result.groups.len(),
            });
        }
        result.occurrences.sort_unstable_by_key(|row| row.0);
        if result
            .occurrences
            .windows(2)
            .any(|rows| rows[0].0 == rows[1].0)
        {
            return Err(invalid("binding occurrence belongs to more than one root"));
        }
        if result
            .metadata_bytes()
            .is_none_or(|bytes| bytes > byte_limit)
        {
            return Err(limit());
        }
        Ok(result)
    }
}

fn nested_pattern(
    index: &BodyIndex,
    mut parent: Option<NodeId>,
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    for _ in 0..256 {
        let Some(id) = parent else {
            return Ok(false);
        };
        budget.charge(0)?;
        let node = index
            .nodes()
            .get(id.0 as usize)
            .ok_or_else(|| invalid("binding parent absent"))?;
        match node.role {
            NodeRole::Pattern => return Ok(true),
            // Record fields and other structural wrappers are not new pattern
            // owners. Stop at actual expression/statement/arm/item boundaries.
            NodeRole::Syntax => parent = node.parent,
            _ => return Ok(false),
        }
    }
    Err(limit())
}
fn identity(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<NodeId, Diagnostic> {
    let range = syntax.text_range();
    let relative = SourceRange {
        start: u32::from(range.start())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
        end: u32::from(range.end())
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
    };
    index
        .nodes_at_range(relative)
        .find(|node| node.role == NodeRole::Pattern)
        .map(|node| node.id)
        .ok_or_else(|| invalid("binding occurrence indexed location absent"))
}
fn children(pattern: &ast::Pat, budget: &mut Budget) -> Result<Vec<ast::Pat>, Diagnostic> {
    let mut pending = Vec::new();
    let mut result = Vec::new();
    for child in pattern.syntax().children() {
        budget.charge(std::mem::size_of::<SyntaxNode>())?;
        pending.try_reserve(1).map_err(|_| limit())?;
        pending.push(child);
    }
    while let Some(node) = pending.pop() {
        budget.charge(0)?;
        if let Some(pattern) = ast::Pat::cast(node.clone()) {
            result.try_reserve(1).map_err(|_| limit())?;
            result.push(pattern);
        } else if !ast::Expr::can_cast(node.kind())
            && !ast::Item::can_cast(node.kind())
            && !ast::TokenTree::can_cast(node.kind())
        {
            for child in node.children() {
                budget.charge(std::mem::size_of::<SyntaxNode>())?;
                pending.try_reserve(1).map_err(|_| limit())?;
                pending.push(child);
            }
        }
    }
    result.sort_unstable_by_key(|pattern| pattern.syntax().text_range().start());
    Ok(result)
}
fn merge(
    mut left: Candidates,
    right: Candidates,
    alternative: bool,
    budget: &mut Budget,
) -> Result<Candidates, Diagnostic> {
    if alternative {
        for (key, group) in &mut left {
            budget.charge(0)?;
            if !right.contains_key(key) {
                group.conflicts.alternative_missing = true;
            }
        }
    }
    for (key, mut next) in right {
        budget.charge(0)?;
        match left.entry(key) {
            Entry::Vacant(entry) => {
                if alternative {
                    next.conflicts.alternative_missing = true;
                }
                entry.insert(next);
            }
            Entry::Occupied(mut entry) => {
                let current = entry.get_mut();
                current.conflicts.include(next.conflicts);
                current.conflicts.conjunction_duplicate |= !alternative;
                current.conflicts.written_mode |=
                    current.occurrences[0].mode != next.occurrences[0].mode;
                budget.charge(
                    next.occurrences
                        .len()
                        .checked_mul(std::mem::size_of::<BindingOccurrence>())
                        .ok_or_else(limit)?,
                )?;
                current
                    .occurrences
                    .try_reserve(next.occurrences.len())
                    .map_err(|_| limit())?;
                current.occurrences.append(&mut next.occurrences);
            }
        }
    }
    Ok(left)
}
fn unresolved_context(error: &Diagnostic) -> bool {
    matches!(
        error.code,
        "LRR-E-MACRO-OUTPUT" | "LRR-E-MACRO-SCOPE" | "LRR-E-UNSUPPORTED"
    )
}
fn collect(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pattern: ast::Pat,
    depth: usize,
    budget: &mut Budget,
) -> Result<Candidates, Diagnostic> {
    budget.charge(0)?;
    if depth >= 256 {
        return Err(limit());
    }
    let mut result = Candidates::new();
    if let ast::Pat::IdentPat(binding) = &pattern {
        let id = identity(index, body, binding.syntax())?;
        let name = binding
            .name()
            .ok_or_else(|| invalid("binding name absent"))?;
        let name_range = crate::syntax::ast_adapter::range(name.syntax().text_range());
        let edition = match parsed.token_edition(name_range) {
            Ok(edition) => Some(edition),
            Err(error) if unresolved_context(&error) => None,
            Err(error) => return Err(error),
        };
        let semantic = if let Some(edition) = edition {
            MacroName::parse(name.text(), edition)?.as_str().to_owned()
        } else {
            crate::syntax::ast_adapter::semantic_name(name.text())
                .ok_or_else(|| invalid("binding name source absent"))?
        };
        budget.charge(256usize.checked_add(semantic.len()).ok_or_else(limit)?)?;
        let context = match parsed.local_name_class(name_range) {
            Ok(None) if edition.is_some() => BindingContext::Original,
            Ok(Some(context)) if edition.is_some() => BindingContext::Normalized(context),
            Ok(_) => BindingContext::Unresolved(id),
            Err(error) if unresolved_context(&error) => BindingContext::Unresolved(id),
            Err(error) => return Err(error),
        };
        let mode = WrittenBindingMode {
            by_reference: binding.ref_token().is_some(),
            mutable: binding.mut_token().is_some(),
        };
        let explicit = mode.by_reference
            || mode.mutable
            || binding.at_token().is_some()
            || binding
                .syntax()
                .parent()
                .and_then(ast::RecordPatField::cast)
                .is_some_and(|field| field.colon_token().is_none());
        result.insert(
            (semantic, context),
            Pending {
                occurrences: vec![BindingOccurrence {
                    pattern: id,
                    name_range,
                    edition,
                    mode,
                    at_binding: binding.at_token().is_some(),
                    uncertainty: if explicit {
                        BindingUncertainty::ExplicitBinding
                    } else {
                        BindingUncertainty::BindingOrConstant
                    },
                }],
                conflicts: BindingConflicts {
                    context_unresolved: matches!(context, BindingContext::Unresolved(_)),
                    ..Default::default()
                },
            },
        );
    }
    let alternatives = matches!(pattern, ast::Pat::OrPat(_));
    let mut first = true;
    for child in children(&pattern, budget)? {
        let next = collect(parsed, body, index, child, depth + 1, budget)?;
        if alternatives && first {
            result = next;
        } else {
            result = merge(result, next, alternatives, budget)?;
        }
        first = false;
    }
    Ok(result)
}

impl StableEncode for PatternBindings {
    const DOMAIN: &'static str = "lrr/v1/syntactic-pattern-binding-groups";
    fn encode_fields(&self, encoder: &mut StableEncoder) {
        encoder.sequence(&self.roots, |e, root| {
            e.u32(root.root.0);
            e.u64(root.first as u64);
            e.u64(root.end as u64);
        });
        encoder.sequence(&self.groups, |e, group| {
            e.string(&group.name);
            e.u32(group.canonical.0);
            e.bool(group.requires_namespace);
            match group.context {
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
            e.bool(group.conflicts.conjunction_duplicate);
            e.bool(group.conflicts.alternative_missing);
            e.bool(group.conflicts.written_mode);
            e.bool(group.conflicts.context_unresolved);
            e.sequence(&group.occurrences, |e, occurrence| {
                e.u32(occurrence.pattern.0);
                e.u32(occurrence.name_range.start);
                e.u32(occurrence.name_range.end);
                e.option(&occurrence.edition, |e, edition| e.u16(*edition));
                e.bool(occurrence.mode.by_reference);
                e.bool(occurrence.mode.mutable);
                e.bool(occurrence.at_binding);
                e.u8(match occurrence.uncertainty {
                    BindingUncertainty::ExplicitBinding => 0,
                    BindingUncertainty::BindingOrConstant => 1,
                });
            });
        });
        encoder.sequence(&self.occurrences, |e, (occurrence, group)| {
            e.u32(occurrence.0);
            e.u64(*group as u64);
        });
    }
}

#[cfg(test)]
#[path = "pattern_bindings_tests.rs"]
mod tests;
