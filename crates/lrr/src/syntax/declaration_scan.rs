//! Declaration skeleton traversal; no executable lowering, resolution or effects.
use super::{
    ast_adapter::{TokenRecipe, range},
    outline::ScopeKey,
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{identity::GrammarSlot, scopes::ScopeKey as MacroScope},
    source::snapshot::{CfgSet, SourcePath},
};
use ra_ap_syntax::{
    AstNode, SyntaxNode, TextRange,
    ast::{self, HasName},
};

#[path = "declaration_scan/trait_headers.rs"]
mod trait_headers;
pub use trait_headers::{
    AssociatedDeclarationHeader, AssociatedDeclarationKind, AssociatedDeclarationOwner,
    DeclarationNameContext, DeclarationNameOccurrence, ScannedTrait,
};

#[cfg(test)]
#[path = "declaration_scan/trait_header_tests.rs"]
mod trait_header_tests;

#[derive(Clone, Copy, Debug)]
pub struct DeclarationScanLimits {
    pub nodes: usize,
    pub impls: usize,
    pub producers: usize,
    pub depth: usize,
    pub metadata_bytes: usize,
}
impl Default for DeclarationScanLimits {
    fn default() -> Self {
        Self {
            nodes: usize::MAX,
            impls: usize::MAX,
            producers: usize::MAX,
            depth: usize::MAX,
            metadata_bytes: usize::MAX,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct DeclarationScanCost {
    pub visited_nodes: usize,
    pub inspected_tokens: usize,
    pub disabled_subtrees: usize,
    pub max_depth: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationProducerKind {
    Macro,
    Attribute,
}
#[derive(Clone, Debug)]
pub struct DeclarationProducer {
    pub identity: Digest,
    pub kind: DeclarationProducerKind,
    pub range: SourceRange,
    pub path: Vec<TokenRecipe>,
    pub unqualified_attribute: Option<String>,
    pub scope: ScopeKey,
    pub containers: Vec<SourceRange>,
    pub lexical_scope: MacroScope,
    pub grammar: GrammarSlot,
}
/// Pure retained lexical attachment facts; an item output is not by itself a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationScopeKind {
    Module,
    Block,
    Closure,
    AssociatedItems,
    Unattached,
}
impl DeclarationScopeKind {
    fn tag(self) -> u8 {
        match self {
            Self::Module => 0,
            Self::Block => 1,
            Self::Closure => 2,
            Self::AssociatedItems => 3,
            Self::Unattached => 4,
        }
    }
}
impl StableEncode for DeclarationScopeKind {
    const DOMAIN: &'static str = "lrr/v2/declaration-scope-kind";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u8(self.tag());
    }
}
#[derive(Clone, Debug)]
pub struct DeclarationLexicalScope {
    pub identity: MacroScope,
    pub kind: DeclarationScopeKind,
    pub parent: Option<MacroScope>,
    pub range: SourceRange,
}
#[derive(Clone, Debug)]
pub struct ScannedMacroDefinition {
    pub identity: Digest,
    pub name: String,
    pub range: SourceRange,
    pub body: SourceRange,
    pub lexical_scope: MacroScope,
}
/// Retained module declaration header and exact lexical ownership. Capturing a
/// header does not publish a module route, load an out-of-line file or expand.
#[derive(Clone, Debug)]
pub struct ScannedModule {
    pub identity: Digest,
    pub name: String,
    pub name_range: SourceRange,
    /// Validated token-context edition; original sources use their recorded edition.
    pub name_edition: u16,
    pub range: SourceRange,
    pub header: Vec<TokenRecipe>,
    pub body: Option<SourceRange>,
    pub scope: ScopeKey,
    pub containers: Vec<SourceRange>,
    pub lexical_scope: MacroScope,
    pub visibility: super::visibility::VisibilityScope,
    pub attribute_inputs: Vec<Digest>,
}
/// Owned use-tree leaves attached to the declaration-only lexical skeleton.
#[derive(Clone, Debug)]
pub struct ScannedImport {
    pub source: super::imports::SourceImport,
    pub lexical_scope: MacroScope,
    /// Annotation decisions belong to the coordinator, never the capture pass.
    pub attribute_inputs: Vec<Digest>,
}
#[derive(Clone, Debug)]
pub struct ScannedImpl {
    /// Exact source attachment; distinct repeated headers remain distinct entries.
    pub identity: Digest,
    /// Header semantics exclude method bodies; exact source dependencies do not.
    pub header_identity: Digest,
    pub range: SourceRange,
    pub header: Vec<TokenRecipe>,
    pub associated: Vec<AssociatedDeclarationHeader>,
    pub scope: ScopeKey,
    pub containers: Vec<SourceRange>,
}
pub struct DeclarationScan {
    path: SourcePath,
    parse: ParseKey,
    scope: ScopeKey,
    cfg: Digest,
    generated_origin: Option<Digest>,
    impls: Vec<ScannedImpl>,
    traits: Vec<ScannedTrait>,
    associated_fragments: Vec<AssociatedDeclarationHeader>,
    producers: Vec<DeclarationProducer>,
    lexical_scopes: Vec<DeclarationLexicalScope>,
    macro_definitions: Vec<ScannedMacroDefinition>,
    modules: Vec<ScannedModule>,
    imports: Vec<ScannedImport>,
    cost: DeclarationScanCost,
    retained_bytes: usize,
    /// The immutable completed scan's stable fingerprint, computed once.
    fingerprint_memo: std::sync::OnceLock<Digest>,
}
impl std::fmt::Debug for DeclarationScan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclarationScan")
            .field("fingerprint", &self.fingerprint())
            .field("impl_count", &self.impls.len())
            .field("trait_count", &self.traits.len())
            .field("producer_count", &self.producers.len())
            .finish()
    }
}
impl DeclarationScan {
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    pub fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn cfg(&self) -> Digest {
        self.cfg
    }
    pub fn generated_origin(&self) -> Option<Digest> {
        self.generated_origin
    }
    pub fn impls(&self) -> &[ScannedImpl] {
        &self.impls
    }
    pub fn traits(&self) -> &[ScannedTrait] {
        &self.traits
    }
    /// Direct associated-slot output facts. Actual parent declaration attachment
    /// remains in GeneratedDeclarations; this is never a guessed trait/impl owner.
    pub fn associated_fragments(&self) -> &[AssociatedDeclarationHeader] {
        &self.associated_fragments
    }
    pub fn producers(&self) -> &[DeclarationProducer] {
        &self.producers
    }
    pub fn lexical_scopes(&self) -> &[DeclarationLexicalScope] {
        &self.lexical_scopes
    }
    pub fn macro_definitions(&self) -> &[ScannedMacroDefinition] {
        &self.macro_definitions
    }
    pub fn modules(&self) -> &[ScannedModule] {
        &self.modules
    }
    pub fn imports(&self) -> &[ScannedImport] {
        &self.imports
    }
    pub fn cost(&self) -> &DeclarationScanCost {
        &self.cost
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
fn position(e: &mut StableEncoder, r: SourceRange) {
    e.u32(r.start);
    e.u32(r.end);
}
impl StableEncode for DeclarationScan {
    const DOMAIN: &'static str =
        "lrr/v3/declaration-skeleton-with-trait-and-associated-source-facts";
    // A scan is immutable once `scan_declarations_in_scope` returns it; the
    // fingerprint is never requested while the scan is being built.
    fn fingerprint(&self) -> Digest {
        *self.fingerprint_memo.get_or_init(|| {
            let mut encoder = StableEncoder::new(Self::DOMAIN);
            self.encode_fields(&mut encoder);
            encoder.finish()
        })
    }
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.path.as_str());
        e.record(&self.parse);
        e.record(&self.scope);
        e.digest(self.cfg);
        e.option(&self.generated_origin, |e, origin| e.digest(*origin));
        e.sequence(&self.impls, |e, h| {
            e.digest(h.identity);
            e.digest(h.header_identity);
            position(e, h.range);
            e.record(&h.scope);
            e.sequence(&h.containers, |e, r| position(e, *r));
            e.sequence(&h.associated, trait_headers::encode_associated);
        });
        e.sequence(&self.traits, |e, row| {
            e.digest(row.identity);
            e.digest(row.header_identity);
            e.option(&row.name, trait_headers::encode_name);
            position(e, row.range);
            e.sequence(&row.header, |e, token| e.record(token));
            e.option(&row.generic_params, |e, r| position(e, *r));
            e.option(&row.where_clause, |e, r| position(e, *r));
            e.option(&row.supertraits, |e, r| position(e, *r));
            e.sequence(&row.associated, trait_headers::encode_associated);
            e.record(&row.scope);
            e.sequence(&row.containers, |e, r| position(e, *r));
            e.record(&row.lexical_scope);
            e.sequence(&row.attribute_inputs, |e, id| e.digest(*id));
        });
        e.sequence(&self.associated_fragments, trait_headers::encode_associated);
        e.sequence(&self.producers, |e, p| {
            e.digest(p.identity);
            e.u8(match p.kind {
                DeclarationProducerKind::Macro => 0,
                DeclarationProducerKind::Attribute => 1,
            });
            position(e, p.range);
            e.sequence(&p.path, |e, t| e.record(t));
            e.option(&p.unqualified_attribute, |e, name| e.string(name));
            e.record(&p.scope);
            e.sequence(&p.containers, |e, r| position(e, *r));
            e.record(&p.lexical_scope);
            e.record(&p.grammar);
        });
        e.sequence(&self.lexical_scopes, |e, s| {
            e.record(&s.identity);
            e.record(&s.kind);
            e.option(&s.parent, |e, p| e.record(p));
            position(e, s.range);
        });
        e.sequence(&self.imports, |e, row| {
            e.record(&row.source);
            e.record(&row.lexical_scope);
            e.sequence(&row.attribute_inputs, |e, input| e.digest(*input));
        });
        e.sequence(&self.modules, |e, m| {
            e.digest(m.identity);
            e.string(&m.name);
            e.u16(m.name_edition);
            position(e, m.name_range);
            position(e, m.range);
            e.sequence(&m.header, |e, token| e.record(token));
            e.option(&m.body, |e, body| position(e, *body));
            e.record(&m.scope);
            e.sequence(&m.containers, |e, r| position(e, *r));
            e.record(&m.lexical_scope);
            e.record(&m.visibility);
            e.sequence(&m.attribute_inputs, |e, input| e.digest(*input));
        });
        e.sequence(&self.macro_definitions, |e, d| {
            e.digest(d.identity);
            e.string(&d.name);
            position(e, d.range);
            position(e, d.body);
            e.record(&d.lexical_scope);
        });
    }
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-DECLARATION-SCAN-LIMIT",
        "declaration scan count/depth/metadata limit".into(),
    )
}
struct Budget {
    limits: DeclarationScanLimits,
    bytes: usize,
    cost: DeclarationScanCost,
    owners: std::collections::BTreeMap<(u32, u32, Digest), Digest>,
}
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(n)
            .filter(|n| *n <= self.limits.metadata_bytes)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn tokens(
        &mut self,
        node: &SyntaxNode,
        excluded: Option<TextRange>,
    ) -> Result<Vec<TokenRecipe>, Diagnostic> {
        let mut result = Vec::new();
        let mut walk = node.preorder_with_tokens();
        while let Some(event) = walk.next() {
            let ra_ap_syntax::WalkEvent::Enter(element) = event else {
                continue;
            };
            if let ra_ap_syntax::NodeOrToken::Node(current) = &element {
                let function_body = ast::BlockExpr::can_cast(current.kind())
                    && current
                        .parent()
                        .is_some_and(|parent| ast::Fn::can_cast(parent.kind()));
                if function_body || excluded.is_some_and(|r| r.contains_range(current.text_range()))
                {
                    walk.skip_subtree();
                }
                continue;
            }
            let Some(token) = element.into_token() else {
                continue;
            };
            if token.kind().is_trivia()
                || excluded.is_some_and(|r| r.contains_range(token.text_range()))
            {
                continue;
            }
            self.cost.inspected_tokens = self
                .cost
                .inspected_tokens
                .checked_add(1)
                .ok_or_else(limit)?;
            self.charge(std::mem::size_of::<TokenRecipe>() + token.text().len() + 32)?;
            result.try_reserve_exact(1).map_err(|_| limit())?;
            result.push(TokenRecipe {
                kind: token.kind() as u16,
                text: token.text().to_owned(),
            });
        }
        Ok(result)
    }
}
fn enabled(node: &SyntaxNode, cfg: &CfgSet) -> Result<(bool, Vec<ast::Meta>), Diagnostic> {
    // The pinned tree can attach statement attributes to nodes omitted by
    // AnyHasAttrs. Use its direct Attr children, the same ownership rule used
    // by HasAttrs, so cfg still prunes a macro statement before its call.
    super::attributes::effective_metas(
        node.children()
            .filter_map(ast::Attr::cast)
            .filter_map(|a| a.meta())
            .collect(),
        cfg,
    )
}
fn body_exclusion(node: &SyntaxNode) -> Option<TextRange> {
    // Initializers and method bodies remain opaque in their header recipes,
    // but the main structural traversal still discovers nested declarations.
    if let Some(f) = ast::Fn::cast(node.clone()) {
        return f.body().map(|b| b.syntax().text_range());
    }
    if let Some(c) = ast::Const::cast(node.clone()) {
        return c.body().map(|b| b.syntax().text_range());
    }
    None
}
fn lexical_context(
    node: &SyntaxNode,
    root: &ScopeKey,
    budget: &mut Budget,
) -> Result<(ScopeKey, Vec<SourceRange>), Diagnostic> {
    let mut ancestors = Vec::new();
    for ancestor in node.ancestors().skip(1) {
        if ancestors.len() >= budget.limits.depth {
            return Err(limit());
        }
        budget.charge(64)?;
        ancestors.try_reserve_exact(1).map_err(|_| limit())?;
        ancestors.push(ancestor);
    }
    for part in &root.module_path {
        budget.charge(part.capacity().checked_add(32).ok_or_else(limit)?)?;
    }
    let mut scope = root.clone();
    for ancestor in ancestors.iter().rev() {
        if let Some(module) = ast::Module::cast(ancestor.clone())
            && let Some(name) = module.name()
        {
            let text = name.text();
            budget.charge(text.len().checked_add(64).ok_or_else(limit)?)?;
            let name = super::ast_adapter::semantic_name(text).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-SOURCE-NAME",
                    "declaration module name is invalid".into(),
                )
            })?;
            scope
                .module_path
                .try_reserve_exact(1)
                .map_err(|_| limit())?;
            scope.module_path.push(name);
        }
    }
    for ancestor in ancestors.iter().rev() {
        if ast::Fn::can_cast(ancestor.kind()) {
            let selected = range(ancestor.text_range());
            let key = (selected.start, selected.end, scope.fingerprint());
            if let Some(owner) = budget.owners.get(&key) {
                scope.owner = Some(*owner);
                continue;
            }
            let header = budget.tokens(ancestor, body_exclusion(ancestor))?;
            let mut e = StableEncoder::new("lrr/v2/declaration-function-owner");
            e.record(&scope);
            e.sequence(&header, |e, t| e.record(t));
            let owner = e.finish();
            budget.charge(160)?;
            budget.owners.insert(key, owner);
            scope.owner = Some(owner);
        }
    }
    let mut containers = Vec::new();
    for ancestor in ancestors.iter().rev() {
        if ast::Item::can_cast(ancestor.kind())
            || ast::BlockExpr::can_cast(ancestor.kind())
            || ast::ClosureExpr::can_cast(ancestor.kind())
        {
            budget.charge(32)?;
            containers.try_reserve_exact(1).map_err(|_| limit())?;
            containers.push(range(ancestor.text_range()));
        }
    }
    Ok((scope, containers))
}
fn source_identity(scan: &DeclarationScan, selected: SourceRange, tag: u8) -> Digest {
    let mut e = StableEncoder::new("lrr/v2/declaration-source-entry");
    e.record(&scan.parse);
    e.string(scan.path.as_str());
    e.record(&scan.scope);
    e.digest(scan.cfg);
    e.option(&scan.generated_origin, |e, origin| e.digest(*origin));
    e.u8(tag);
    position(&mut e, selected);
    e.finish()
}
fn macro_grammar(node: &SyntaxNode, root_grammar: Option<GrammarSlot>) -> GrammarSlot {
    let Some(parent) = node.parent() else {
        return GrammarSlot::Items;
    };
    if parent.kind() == ra_ap_syntax::SyntaxKind::MACRO_ITEMS {
        return root_grammar.unwrap_or(GrammarSlot::Items);
    }
    if parent.kind() == ra_ap_syntax::SyntaxKind::MACRO_STMTS {
        return GrammarSlot::Statements;
    }
    if ast::MacroExpr::can_cast(parent.kind()) {
        if parent.parent().is_some_and(|n| {
            ast::ExprStmt::can_cast(n.kind()) || n.kind() == ra_ap_syntax::SyntaxKind::MACRO_STMTS
        }) {
            return GrammarSlot::Statements;
        }
        return GrammarSlot::Expression;
    }
    if ast::MacroType::can_cast(parent.kind()) {
        return GrammarSlot::Type;
    }
    if ast::MacroPat::can_cast(parent.kind()) {
        return GrammarSlot::Pattern;
    }
    if ast::AssocItemList::can_cast(parent.kind()) {
        return if parent
            .parent()
            .is_some_and(|n| ast::Trait::can_cast(n.kind()))
        {
            GrammarSlot::TraitItems
        } else {
            GrammarSlot::ImplItems
        };
    }
    if ast::StmtList::can_cast(parent.kind()) {
        return GrammarSlot::Statements;
    }
    GrammarSlot::Items
}
impl ParsedFile {
    pub fn scan_declarations(
        &self,
        path: SourcePath,
        scope: ScopeKey,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
    ) -> Result<DeclarationScan, Diagnostic> {
        self.scan_declarations_in_scope(path, scope, cfg, limits, None)
    }
    /// Pure capture with an insertion-scope fact retained by the coordinator.
    /// Item grammar alone cannot grant a generated root module attachment.
    pub fn scan_declarations_at_root(
        &self,
        path: SourcePath,
        scope: ScopeKey,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
        root_kind: DeclarationScopeKind,
    ) -> Result<DeclarationScan, Diagnostic> {
        self.scan_declarations_in_scope(path, scope, cfg, limits, Some(root_kind))
    }
    fn scan_declarations_in_scope(
        &self,
        path: SourcePath,
        scope: ScopeKey,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
        requested_root: Option<DeclarationScopeKind>,
    ) -> Result<DeclarationScan, Diagnostic> {
        if requested_root == Some(DeclarationScopeKind::Module)
            && self
                .declaration_grammar
                .is_some_and(|grammar| grammar != GrammarSlot::Items)
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "module declaration root does not match generated grammar".into(),
            ));
        }
        let root_kind = requested_root.unwrap_or_else(|| {
            if self.generated_origin.is_none() && scope.owner.is_none() {
                DeclarationScopeKind::Module
            } else {
                match self.declaration_grammar {
                    Some(GrammarSlot::Statements) => DeclarationScopeKind::Block,
                    Some(GrammarSlot::TraitItems | GrammarSlot::ImplItems) => {
                        DeclarationScopeKind::AssociatedItems
                    }
                    _ => DeclarationScopeKind::Unattached,
                }
            }
        });
        if !self.errors.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-PARSE",
                "declaration skeleton contains syntax errors".into(),
            ));
        }
        let ceiling = DeclarationScanLimits::default();
        if limits.nodes > ceiling.nodes
            || limits.impls > ceiling.impls
            || limits.producers > ceiling.producers
            || limits.depth > ceiling.depth
            || limits.metadata_bytes > ceiling.metadata_bytes
        {
            return Err(limit());
        }
        let mut budget = Budget {
            limits,
            bytes: 0,
            cost: DeclarationScanCost::default(),
            owners: std::collections::BTreeMap::new(),
        };
        budget.charge(std::mem::size_of::<DeclarationScan>())?;
        budget.charge(path.as_str().len())?;
        for part in &scope.module_path {
            budget.charge(
                part.capacity()
                    .checked_add(std::mem::size_of::<String>())
                    .ok_or_else(limit)?,
            )?;
        }
        let mut scan = DeclarationScan {
            path,
            parse: self.key(),
            scope,
            cfg: cfg.fingerprint(),
            generated_origin: self.generated_origin,
            impls: Vec::new(),
            traits: Vec::new(),
            associated_fragments: Vec::new(),
            producers: Vec::new(),
            lexical_scopes: Vec::new(),
            macro_definitions: Vec::new(),
            modules: Vec::new(),
            imports: Vec::new(),
            cost: DeclarationScanCost::default(),
            retained_bytes: 0,
            fingerprint_memo: std::sync::OnceLock::new(),
        };
        let root = self.parsed.tree();
        let root_range = range(root.syntax().text_range());
        let mut root_identity = StableEncoder::new("lrr/v2/declaration-lexical-root-kind");
        root_identity.digest(source_identity(&scan, root_range, 3));
        root_identity.record(&root_kind);
        let root_scope = MacroScope::root(root_identity.finish());
        budget.charge(std::mem::size_of::<DeclarationLexicalScope>() + 64)?;
        scan.lexical_scopes.push(DeclarationLexicalScope {
            identity: root_scope,
            kind: root_kind,
            parent: None,
            range: root_range,
        });
        let mut stack = vec![(root.syntax().clone(), 0usize, root_scope, root_kind)];
        while let Some((node, depth, inherited_scope, inherited_kind)) = stack.pop() {
            budget.cost.visited_nodes = budget
                .cost
                .visited_nodes
                .checked_add(1)
                .filter(|n| *n <= limits.nodes)
                .ok_or_else(limit)?;
            if depth > limits.depth {
                return Err(limit());
            }
            budget.cost.max_depth = budget.cost.max_depth.max(depth);
            let (active, metas) = enabled(&node, cfg)?;
            if !active {
                budget.cost.disabled_subtrees = budget
                    .cost
                    .disabled_subtrees
                    .checked_add(1)
                    .ok_or_else(limit)?;
                continue;
            }
            let scope_kind = if ast::BlockExpr::can_cast(node.kind()) {
                Some(DeclarationScopeKind::Block)
            } else if ast::ClosureExpr::can_cast(node.kind()) {
                Some(DeclarationScopeKind::Closure)
            } else if ast::ItemList::can_cast(node.kind()) {
                Some(DeclarationScopeKind::Module)
            } else if ast::AssocItemList::can_cast(node.kind()) {
                Some(DeclarationScopeKind::AssociatedItems)
            } else {
                None
            };
            let lexical_scope = if let Some(kind) = scope_kind {
                if scan.lexical_scopes.len() >= limits.nodes {
                    return Err(limit());
                }
                budget.charge(std::mem::size_of::<DeclarationLexicalScope>() + 64)?;
                let selected = range(node.text_range());
                let identity = inherited_scope.child(u64::from(selected.start), 0);
                scan.lexical_scopes
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                scan.lexical_scopes.push(DeclarationLexicalScope {
                    identity,
                    kind,
                    parent: Some(inherited_scope),
                    range: selected,
                });
                identity
            } else {
                inherited_scope
            };
            let lexical_kind = scope_kind.unwrap_or(inherited_kind);
            let mut import_annotations =
                if ast::Use::can_cast(node.kind()) || ast::Module::can_cast(node.kind()) {
                    let mut inputs = Vec::new();
                    inputs.try_reserve_exact(metas.len()).map_err(|_| limit())?;
                    for meta in &metas {
                        inputs.push(source_identity(&scan, range(meta.syntax().text_range()), 1));
                    }
                    inputs
                } else {
                    Vec::new()
                };
            for meta in metas {
                if scan.producers.len() >= limits.producers {
                    return Err(limit());
                }
                budget.charge(std::mem::size_of::<DeclarationProducer>() + 64)?;
                let selected = range(meta.syntax().text_range());
                let path = budget.tokens(meta.syntax(), None)?;
                // The pinned parser represents cfg as its own AST node, with
                // no Path. Retain the actual keyword as a syntax fact so that
                // compiler Attribute selection can still prove it inert.
                let unqualified_attribute = match &meta {
                    ast::Meta::CfgMeta(cfg) => cfg
                        .cfg_token()
                        .and_then(|token| super::ast_adapter::semantic_name(token.text())),
                    _ => meta
                        .path()
                        .filter(|path| {
                            path.qualifier().is_none() && path.coloncolon_token().is_none()
                        })
                        .and_then(|path| path.segment())
                        .filter(|segment| segment.coloncolon_token().is_none())
                        .and_then(|segment| segment.name_ref())
                        .and_then(|name| super::ast_adapter::semantic_name(name.text())),
                };
                if let Some(name) = &unqualified_attribute {
                    budget.charge(name.capacity().checked_add(32).ok_or_else(limit)?)?;
                }
                // Builtin inert and tool attributes are never macro calls.
                let path_text: String = path.iter().map(|token| token.text.as_str()).collect();
                if unqualified_attribute
                    .as_deref()
                    .is_some_and(super::outline::is_builtin_inert_attribute)
                    || super::outline::is_tool_attribute(&path_text)
                {
                    continue;
                }
                let (scope, containers) = lexical_context(&node, &scan.scope, &mut budget)?;
                scan.producers.try_reserve_exact(1).map_err(|_| limit())?;
                scan.producers.push(DeclarationProducer {
                    identity: source_identity(&scan, selected, 1),
                    kind: DeclarationProducerKind::Attribute,
                    unqualified_attribute,
                    range: selected,
                    path,
                    scope,
                    containers,
                    lexical_scope,
                    grammar: macro_grammar(&node, self.declaration_grammar),
                });
            }
            if let Some(item) = ast::Module::cast(node.clone()) {
                if scan.modules.len() >= limits.producers {
                    return Err(limit());
                }
                let name = item.name().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-PARSE",
                        "module declaration has no name".into(),
                    )
                })?;
                budget.charge(std::mem::size_of::<ScannedModule>() + 128)?;
                budget.charge(name.text().len())?;
                let spelling = name.text().to_string();
                let name_edition = self.token_edition(range(name.syntax().text_range()))?;
                super::ast_adapter::macro_name(&spelling, name_edition).ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-NAME",
                        "module declaration name is invalid".into(),
                    )
                })?;
                let body = item.item_list().map(|list| list.syntax().text_range());
                let header = budget.tokens(&node, body)?;
                let (mut scope, containers) = lexical_context(&node, &scan.scope, &mut budget)?;
                // Namespace ownership stops at a retained module boundary;
                // source containers still retain enclosing cold function ranges.
                if lexical_kind == DeclarationScopeKind::Module {
                    scope.owner = None;
                }
                budget.charge(scope.heap_bytes().checked_add(128).ok_or_else(limit)?)?;
                let visibility = super::visibility::VisibilityScope::for_declaration(
                    &super::outline::visibility(&node),
                    &scope,
                    scan.parse.profile.edition,
                )?;
                budget.charge(
                    import_annotations
                        .len()
                        .checked_mul(std::mem::size_of::<Digest>())
                        .ok_or_else(limit)?,
                )?;
                let selected = range(node.text_range());
                scan.modules.try_reserve_exact(1).map_err(|_| limit())?;
                scan.modules.push(ScannedModule {
                    identity: source_identity(&scan, selected, 5),
                    name: spelling,
                    name_range: range(name.syntax().text_range()),
                    name_edition,
                    range: selected,
                    header,
                    body: body.map(range),
                    scope,
                    containers,
                    lexical_scope,
                    visibility,
                    attribute_inputs: std::mem::take(&mut import_annotations),
                });
            }
            if let Some(item) = ast::Use::cast(node.clone()) {
                let selected = range(node.text_range());
                let identity = source_identity(&scan, selected, 4);
                let (scope, _) = lexical_context(&node, &scan.scope, &mut budget)?;
                let mut import_budget = super::imports::ImportBudget::default();
                let leaves = super::imports::collect(
                    &item,
                    &scope,
                    identity,
                    &super::outline::Visibility::Private,
                    &mut import_budget,
                )?;
                scan.imports
                    .len()
                    .checked_add(leaves.len())
                    .filter(|n| *n <= limits.producers)
                    .ok_or_else(limit)?;
                scan.imports
                    .try_reserve_exact(leaves.len())
                    .map_err(|_| limit())?;
                for source in leaves {
                    let bytes = super::imports::retained_bytes(&source)?
                        .checked_add(std::mem::size_of::<ScannedImport>() + 64)
                        .and_then(|n| {
                            n.checked_add(
                                import_annotations
                                    .len()
                                    .checked_mul(std::mem::size_of::<Digest>())?,
                            )
                        })
                        .ok_or_else(limit)?;
                    budget.charge(bytes)?;
                    let mut attribute_inputs = Vec::new();
                    attribute_inputs
                        .try_reserve_exact(import_annotations.len())
                        .map_err(|_| limit())?;
                    attribute_inputs.extend_from_slice(&import_annotations);
                    scan.imports.push(ScannedImport {
                        source,
                        lexical_scope,
                        attribute_inputs,
                    });
                }
            }
            if let Some(definition) = ast::MacroRules::cast(node.clone()) {
                if scan.macro_definitions.len() >= limits.producers {
                    return Err(limit());
                }
                let name = definition.name().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-PARSE",
                        "local macro definition has no name".into(),
                    )
                })?;
                // Generated literal and captured names retain their own token
                // edition, independently of the enclosing invocation profile.
                // Preserve raw spelling until that edition-aware validator.
                let name_edition = self.token_edition(range(name.syntax().text_range()))?;
                let name = name.text().to_string();
                super::ast_adapter::macro_name(&name, name_edition).ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-NAME",
                        "local macro definition name is invalid".into(),
                    )
                })?;
                let body = definition.token_tree().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-PARSE",
                        "local macro definition has no token tree".into(),
                    )
                })?;
                budget.charge(std::mem::size_of::<ScannedMacroDefinition>() + name.len() + 64)?;
                let selected = range(node.text_range());
                scan.macro_definitions
                    .try_reserve_exact(1)
                    .map_err(|_| limit())?;
                scan.macro_definitions.push(ScannedMacroDefinition {
                    identity: source_identity(&scan, selected, 4),
                    name,
                    range: selected,
                    body: range(body.syntax().text_range()),
                    lexical_scope,
                });
                continue;
            }
            if let Some(item) = ast::Trait::cast(node.clone()) {
                if scan
                    .traits
                    .len()
                    .checked_add(scan.impls.len())
                    .ok_or_else(limit)?
                    >= limits.impls
                {
                    return Err(limit());
                }
                let row = trait_headers::capture_trait(
                    self,
                    &scan,
                    cfg,
                    &item,
                    lexical_scope,
                    &mut budget,
                )?;
                trait_headers::reserve_one(&mut scan.traits, &mut budget)?;
                scan.traits.push(row);
            }
            if let Some(item) = ast::Impl::cast(node.clone()) {
                if scan
                    .traits
                    .len()
                    .checked_add(scan.impls.len())
                    .ok_or_else(limit)?
                    >= limits.impls
                {
                    return Err(limit());
                }
                budget.charge(std::mem::size_of::<ScannedImpl>() + 64)?;
                let list = item.assoc_item_list();
                let header =
                    budget.tokens(&node, list.as_ref().map(|l| l.syntax().text_range()))?;
                let selected = range(node.text_range());
                let identity = source_identity(&scan, selected, 0);
                let associated = match list.as_ref() {
                    Some(list) => trait_headers::associated_list(
                        self,
                        &scan,
                        cfg,
                        list,
                        AssociatedDeclarationOwner::Impl(identity),
                        lexical_scope,
                        &mut budget,
                    )?,
                    None => Vec::new(),
                };
                let mut e = StableEncoder::new("lrr/v2/scanned-impl-header");
                let (scope, containers) = lexical_context(&node, &scan.scope, &mut budget)?;
                e.record(&self.profile);
                e.record(&scope);
                e.sequence(&header, |e, t| e.record(t));
                e.sequence(&associated, |e, h| {
                    e.sequence(&h.header, |e, t| e.record(t))
                });
                scan.impls.try_reserve_exact(1).map_err(|_| limit())?;
                scan.impls.push(ScannedImpl {
                    identity,
                    header_identity: e.finish(),
                    range: selected,
                    header,
                    associated,
                    scope,
                    containers,
                });
            }
            if root_kind == DeclarationScopeKind::AssociatedItems
                && node.parent().is_some_and(|parent| {
                    matches!(
                        parent.kind(),
                        ra_ap_syntax::SyntaxKind::SOURCE_FILE
                            | ra_ap_syntax::SyntaxKind::MACRO_ITEMS
                    )
                })
                && ast::AssocItem::can_cast(node.kind())
            {
                if scan.associated_fragments.len() >= limits.nodes {
                    return Err(limit());
                }
                let row = trait_headers::associated(
                    self,
                    &scan,
                    cfg,
                    &node,
                    AssociatedDeclarationOwner::Fragment {
                        grammar: self.declaration_grammar.unwrap_or(GrammarSlot::Items),
                        lexical_scope: root_scope,
                    },
                    lexical_scope,
                    &mut budget,
                )?;
                trait_headers::reserve_one(&mut scan.associated_fragments, &mut budget)?;
                scan.associated_fragments.push(row);
            }
            if let Some(call) = ast::MacroCall::cast(node.clone()) {
                if scan.producers.len() >= limits.producers {
                    return Err(limit());
                }
                budget.charge(std::mem::size_of::<DeclarationProducer>() + 64)?;
                let selected = range(node.text_range());
                let path = call.path().ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-PARSE",
                        "declaration producer has no path".into(),
                    )
                })?;
                let path = budget.tokens(path.syntax(), None)?;
                let (scope, containers) = lexical_context(&node, &scan.scope, &mut budget)?;
                scan.producers.try_reserve_exact(1).map_err(|_| limit())?;
                scan.producers.push(DeclarationProducer {
                    identity: source_identity(&scan, selected, 2),
                    kind: DeclarationProducerKind::Macro,
                    unqualified_attribute: None,
                    range: selected,
                    path,
                    scope,
                    containers,
                    lexical_scope,
                    grammar: macro_grammar(&node, self.declaration_grammar),
                });
                continue;
            }
            if ast::TokenTree::can_cast(node.kind()) || ast::Attr::can_cast(node.kind()) {
                continue;
            }
            // Reverse insertion preserves the pinned syntax tree's source order.
            let mut children = Vec::new();
            for child in node.children() {
                let count = stack
                    .len()
                    .checked_add(children.len())
                    .and_then(|n| n.checked_add(1))
                    .filter(|n| *n <= limits.nodes)
                    .ok_or_else(limit)?;
                let pending = count
                    .checked_mul(128)
                    .and_then(|n| n.checked_add(budget.bytes))
                    .filter(|n| *n <= limits.metadata_bytes)
                    .ok_or_else(limit)?;
                let _ = pending;
                children.try_reserve_exact(1).map_err(|_| limit())?;
                children.push(child);
            }
            stack
                .try_reserve_exact(children.len())
                .map_err(|_| limit())?;
            for child in children.into_iter().rev() {
                stack.push((
                    child,
                    depth.checked_add(1).ok_or_else(limit)?,
                    lexical_scope,
                    lexical_kind,
                ));
            }
        }
        scan.cost = budget.cost;
        scan.retained_bytes = budget.bytes;
        Ok(scan)
    }
}

/// Pure test-only owned syntax fixture construction. Parser/adapter syntax
/// representations remain inside their authorized syntax owner.
#[cfg(test)]
// Pure owned-syntax reconstruction for attachment tests; no runtime body or macro execution.
pub(crate) fn generated_declaration_fixture(source: &str, grammar: GrammarSlot) -> ParsedFile {
    use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
    use lrr_expand_adapter::item_output::SyntaxEvent;
    use ra_ap_syntax::{NodeOrToken, WalkEvent};
    assert!(matches!(
        grammar,
        GrammarSlot::Items | GrammarSlot::Statements
    ));
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let mut syntax = Vec::new();
    for event in parsed.parsed.tree().syntax().preorder_with_tokens() {
        match event {
            WalkEvent::Enter(NodeOrToken::Node(node)) => {
                syntax.push(SyntaxEvent::Start(node.kind() as u16))
            }
            WalkEvent::Enter(NodeOrToken::Token(token)) => syntax.push(SyntaxEvent::Token {
                kind: token.kind() as u16,
                text: token.text().into(),
            }),
            WalkEvent::Leave(NodeOrToken::Node(_)) => syntax.push(SyntaxEvent::Finish),
            WalkEvent::Leave(NodeOrToken::Token(_)) => {}
        }
    }
    let root = if grammar == GrammarSlot::Statements {
        ra_ap_syntax::SyntaxKind::MACRO_STMTS
    } else {
        ra_ap_syntax::SyntaxKind::MACRO_ITEMS
    };
    syntax[0] = SyntaxEvent::Start(root as u16);
    ParsedFile::from_generated_declarations(
        &syntax,
        source.len(),
        Digest::of_bytes(source.as_bytes()),
        2024,
        grammar,
    )
    .unwrap()
}

#[cfg(test)]
mod scope_kind_tests {
    use super::generated_declaration_fixture as generated;
    use super::*;
    use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
    fn capture(parsed: &ParsedFile, kind: Option<DeclarationScopeKind>) -> DeclarationScan {
        let scope = ScopeKey {
            crate_key: Digest([226; 32]),
            module_path: vec![],
            owner: None,
        };
        match kind {
            None => parsed
                .scan_declarations(
                    SourcePath::new("scope-kind.rs").unwrap(),
                    scope,
                    &CfgSet::default(),
                    DeclarationScanLimits::default(),
                )
                .unwrap(),
            Some(kind) => parsed
                .scan_declarations_at_root(
                    SourcePath::new("scope-kind.rs").unwrap(),
                    scope,
                    &CfgSet::default(),
                    DeclarationScanLimits::default(),
                    kind,
                )
                .unwrap(),
        }
    }
    #[test]
    fn attribute_producers_keep_the_exact_owned_declaration_grammar() {
        let source = "#[identity] fn free(){} impl A { #[identity] fn method(){} } trait T { #[identity] fn method(); } fn cold(){ #[identity] fn local(){} } #[cfg(any())] impl Hidden { #[identity] fn absent(){} }";
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let scan = capture(&parsed, None);
        let attributes = scan
            .producers()
            .iter()
            .filter(|row| {
                row.kind == DeclarationProducerKind::Attribute
                    && row.unqualified_attribute.as_deref() == Some("identity")
            })
            .collect::<Vec<_>>();
        assert_eq!(attributes.len(), 4);
        assert_eq!(
            attributes.iter().map(|row| row.grammar).collect::<Vec<_>>(),
            [
                GrammarSlot::Items,
                GrammarSlot::ImplItems,
                GrammarSlot::TraitItems,
                GrammarSlot::Statements
            ]
        );
        for row in &attributes[1..3] {
            assert_eq!(
                scan.lexical_scopes()
                    .iter()
                    .find(|scope| scope.identity == row.lexical_scope)
                    .unwrap()
                    .kind,
                DeclarationScopeKind::AssociatedItems
            );
        }
    }
    #[test]
    fn module_block_closure_and_associated_scopes_are_structural_facts() {
        let source = "mod m{use crate::d::*; fn cold(){use crate::d::*; let c=||{use crate::d::*;};}} const X:u8={use crate::d::*;0}; trait T{fn cold(){}} impl S{fn cold(){}}";
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let scan = capture(&parsed, None);
        assert_eq!(scan.lexical_scopes()[0].kind, DeclarationScopeKind::Module);
        for required in [
            DeclarationScopeKind::Module,
            DeclarationScopeKind::Block,
            DeclarationScopeKind::Closure,
            DeclarationScopeKind::AssociatedItems,
        ] {
            assert!(
                scan.lexical_scopes()
                    .iter()
                    .any(|scope| scope.kind == required)
            );
        }
        let kinds = scan
            .imports()
            .iter()
            .map(|import| {
                scan.lexical_scopes()
                    .iter()
                    .find(|scope| scope.identity == import.lexical_scope)
                    .unwrap()
                    .kind
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                DeclarationScopeKind::Module,
                DeclarationScopeKind::Block,
                DeclarationScopeKind::Block,
                DeclarationScopeKind::Block
            ]
        );
        assert!(scan.imports().last().unwrap().source.scope.owner.is_none());
    }
    #[test]
    fn generated_item_grammar_does_not_invent_a_module_attachment() {
        let parsed = generated("use dep::*; #[m] fn cold(){}", GrammarSlot::Items);
        let unknown = capture(&parsed, None);
        let module = capture(&parsed, Some(DeclarationScopeKind::Module));
        let block = capture(&parsed, Some(DeclarationScopeKind::Block));
        assert_eq!(
            unknown.lexical_scopes()[0].kind,
            DeclarationScopeKind::Unattached
        );
        assert_eq!(
            module.lexical_scopes()[0].kind,
            DeclarationScopeKind::Module
        );
        assert_eq!(block.lexical_scopes()[0].kind, DeclarationScopeKind::Block);
        assert_ne!(unknown.fingerprint(), module.fingerprint());
        assert_ne!(module.fingerprint(), block.fingerprint());
        assert_ne!(
            module.lexical_scopes()[0].identity,
            block.lexical_scopes()[0].identity
        );
        // One physical source point retains its identity; attachment facts and scope identity differ.
        assert_eq!(
            module.producers()[0].identity,
            block.producers()[0].identity
        );
        assert_eq!(module.parse(), block.parse());
    }
    #[test]
    fn statement_grammar_is_block_and_cannot_be_promoted_to_a_module() {
        let parsed = generated("use dep::*; #[m] fn cold(){}", GrammarSlot::Statements);
        assert_eq!(
            capture(&parsed, None).lexical_scopes()[0].kind,
            DeclarationScopeKind::Block
        );
        let result = parsed.scan_declarations_at_root(
            SourcePath::new("statements.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([226; 32]),
                module_path: vec![],
                owner: None,
            },
            &CfgSet::default(),
            DeclarationScanLimits::default(),
            DeclarationScopeKind::Module,
        );
        let failure = result.unwrap_err();
        assert_eq!(failure.code, "LRR-E-MACRO-OUTPUT");
        assert_eq!(
            failure.message,
            "module declaration root does not match generated grammar"
        );
    }
    #[test]
    fn stable_scope_kind_encoding_separates_each_semantic_attachment() {
        let kinds = [
            DeclarationScopeKind::Module,
            DeclarationScopeKind::Block,
            DeclarationScopeKind::Closure,
            DeclarationScopeKind::AssociatedItems,
            DeclarationScopeKind::Unattached,
        ];
        let digests = kinds
            .map(|kind| kind.fingerprint())
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(digests.len(), kinds.len());
        assert_eq!(
            DeclarationScan::DOMAIN,
            "lrr/v3/declaration-skeleton-with-trait-and-associated-source-facts"
        );
        let parsed = generated("#[m] fn cold(){}", GrammarSlot::Items);
        let scans = kinds.map(|kind| capture(&parsed, Some(kind)));
        let identities = scans
            .iter()
            .map(DeclarationScan::fingerprint)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(identities.len(), kinds.len());
    }
}

#[cfg(test)]
mod module_capture_tests {
    use super::*;
    use crate::{core::profile::ProfileIdentity, source::snapshot::SourceBlob};
    fn parsed(source: &str) -> ParsedFile {
        ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
    }
    fn scope() -> ScopeKey {
        ScopeKey {
            crate_key: Digest([255; 32]),
            module_path: vec![],
            owner: None,
        }
    }
    #[test]
    fn module_headers_keep_exact_names_owners_annotations_and_opaque_bodies() {
        let source = "#[cfg(any())]mod off{mod hidden{}} #[producer]mod root{fn cold(){mod local{fn bad(){let _:u8=\"bad\";}}}mod nested;} dummy!{mod opaque{}}";
        let scan = parsed(source)
            .scan_declarations(
                SourcePath::new("modules.rs").unwrap(),
                scope(),
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        assert_eq!(
            scan.modules()
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["root", "local", "nested"]
        );
        for row in scan.modules() {
            assert_eq!(
                &source[row.name_range.start as usize..row.name_range.end as usize],
                row.name
            );
            assert!(
                !row.header
                    .iter()
                    .any(|token| token.text == "let" || token.text == "bad")
            );
        }
        let root = &scan.modules()[0];
        assert_eq!(root.attribute_inputs.len(), 1);
        assert!(
            scan.producers()
                .iter()
                .any(|row| row.identity == root.attribute_inputs[0])
        );
        assert_eq!(root.lexical_scope, scan.lexical_scopes()[0].identity);
        let local = &scan.modules()[1];
        assert!(local.scope.owner.is_some());
        assert_eq!(local.scope.module_path, ["root"]);
        assert_eq!(
            scan.lexical_scopes()
                .iter()
                .find(|row| row.identity == local.lexical_scope)
                .unwrap()
                .kind,
            DeclarationScopeKind::Block
        );
        let nested = &scan.modules()[2];
        assert_eq!(nested.scope.owner, None);
        assert_eq!(nested.scope.module_path, ["root"]);
        assert_ne!(local.identity, nested.identity);
        assert!(local.body.is_some());
        assert!(nested.body.is_none());
    }
    #[test]
    fn module_header_count_limit_is_typed_and_cfg_pruning_precedes_capture() {
        let limits = DeclarationScanLimits {
            producers: 1,
            ..DeclarationScanLimits::default()
        };
        let result = parsed("mod a{} mod b{}").scan_declarations(
            SourcePath::new("limited.rs").unwrap(),
            scope(),
            &CfgSet::default(),
            limits,
        );
        assert_eq!(result.unwrap_err().code, "LRR-E-DECLARATION-SCAN-LIMIT");
        let scan = parsed("#[cfg(any())]mod a{} mod b{}")
            .scan_declarations(
                SourcePath::new("limited.rs").unwrap(),
                scope(),
                &CfgSet::default(),
                limits,
            )
            .unwrap();
        assert_eq!(scan.modules().len(), 1);
        assert_eq!(scan.modules()[0].name, "b");
    }
    #[test]
    fn module_physical_identity_is_distinct_from_attachment_kind() {
        let generated = generated_declaration_fixture("mod local{}", GrammarSlot::Items);
        let capture = |kind| {
            generated
                .scan_declarations_at_root(
                    SourcePath::new("generated.rs").unwrap(),
                    scope(),
                    &CfgSet::default(),
                    DeclarationScanLimits::default(),
                    kind,
                )
                .unwrap()
        };
        let module = capture(DeclarationScopeKind::Module);
        let block = capture(DeclarationScopeKind::Block);
        assert_eq!(module.modules()[0].identity, block.modules()[0].identity);
        assert_ne!(
            module.modules()[0].lexical_scope,
            block.modules()[0].lexical_scope
        );
        assert_ne!(module.fingerprint(), block.fingerprint());
    }
}
