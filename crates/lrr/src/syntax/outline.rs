//! Declaration completeness and body handles, without executable semantics.
use super::{
    ast_adapter::{TokenRecipe, range, tokens},
    parse::ParsedFile,
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    source::{
        manifest::{BodyKey, DefKey, ItemKind, Namespace},
        snapshot::CfgSet,
    },
};
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasAttrs, HasName},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ScopeKey {
    pub crate_key: Digest,
    pub module_path: Vec<String>,
    pub owner: Option<Digest>,
}
impl StableEncode for ScopeKey {
    const DOMAIN: &'static str = "lrr/v2/syntax-scope";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.crate_key);
        e.sequence(&self.module_path, |e, p| e.string(p));
        e.option(&self.owner, |e, d| e.digest(*d));
    }
}
impl ScopeKey {
    pub fn heap_bytes(&self) -> usize {
        self.module_path.capacity() * std::mem::size_of::<String>()
            + self.module_path.iter().map(String::capacity).sum::<usize>()
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AttributeClass {
    Harmless,
    Conditional,
    Test,
    Derive,
    Representation,
    ModulePath,
    ToolLint,
    TransformUnsupported,
    MacroExport,
    MacroUse,
}
impl AttributeClass {
    fn tag(&self) -> u32 {
        match self {
            Self::Harmless => 1,
            Self::Conditional => 2,
            Self::Test => 3,
            Self::Derive => 4,
            Self::Representation => 5,
            Self::ModulePath => 6,
            Self::ToolLint => 7,
            Self::TransformUnsupported => 8,
            Self::MacroExport => 9,
            Self::MacroUse => 10,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeRecipe {
    pub name: String,
    pub class: AttributeClass,
    pub tokens: Vec<TokenRecipe>,
    /// Source attachment, deliberately excluded from semantic header identity.
    #[serde(default)]
    pub source_meta: Option<SourceRange>,
}
impl StableEncode for AttributeRecipe {
    const DOMAIN: &'static str = "lrr/v2/attribute";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(&self.name);
        e.u32(self.class.tag());
        e.sequence(&self.tokens, |e, t| e.record(t));
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
    Crate,
    Super,
    Restricted(Vec<TokenRecipe>),
}
impl StableEncode for Visibility {
    const DOMAIN: &'static str = "lrr/v2/visibility";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Private => e.u32(0),
            Self::Public => e.u32(1),
            Self::Crate => e.u32(2),
            Self::Super => e.u32(3),
            Self::Restricted(tokens) => {
                e.u32(4);
                e.sequence(tokens, |e, t| e.record(t));
            }
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HeaderRecipe {
    pub profile: ProfileIdentity,
    pub tokens: Vec<TokenRecipe>,
    pub context: Vec<TokenRecipe>,
    pub attributes: Vec<AttributeRecipe>,
    pub visibility: Visibility,
}
impl StableEncode for HeaderRecipe {
    const DOMAIN: &'static str = "lrr/v2/declaration-header";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.profile);
        e.sequence(&self.tokens, |e, t| e.record(t));
        e.sequence(&self.context, |e, t| e.record(t));
        e.sequence(&self.attributes, |e, a| e.record(a));
        e.record(&self.visibility);
    }
}
impl HeaderRecipe {
    /// Semantic equality across an explicitly admitted source-coordinate
    /// transformation. Attribute source locations remain separate attachments;
    /// ordinary equality still checks them for exact current-source consumers.
    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.profile == other.profile
            && self.tokens == other.tokens
            && self.context == other.context
            && self.visibility == other.visibility
            && self.attributes.len() == other.attributes.len()
            && self
                .attributes
                .iter()
                .zip(&other.attributes)
                .all(|(left, right)| {
                    left.name == right.name
                        && left.class == right.class
                        && left.tokens == right.tokens
                })
    }
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + (self.tokens.capacity() + self.context.capacity())
                * std::mem::size_of::<TokenRecipe>()
            + self
                .tokens
                .iter()
                .chain(&self.context)
                .map(|t| t.text.capacity())
                .sum::<usize>()
            + self.attributes.capacity() * std::mem::size_of::<AttributeRecipe>()
            + self
                .attributes
                .iter()
                .map(|a| {
                    a.name.capacity()
                        + a.tokens.capacity() * std::mem::size_of::<TokenRecipe>()
                        + a.tokens.iter().map(|t| t.text.capacity()).sum::<usize>()
                })
                .sum::<usize>()
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeclarationContext {
    pub scope: ScopeKey,
    pub range: SourceRange,
}
impl StableEncode for DeclarationContext {
    const DOMAIN: &'static str = "lrr/v2/declaration-context";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.scope);
        e.u32(self.range.start);
        e.u32(self.range.end);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyHandle {
    pub def_key: Digest,
    /// Exact UTF-8 bytes for text sources; tagged structural representation
    /// (including owning editions) for retained syntax. Never display text alone.
    pub body_hash: Digest,
    pub range: SourceRange,
    pub entry_blocked: bool,
}
impl BodyHandle {
    pub fn key(&self) -> BodyKey {
        BodyKey {
            definition: self.def_key,
            exact_body_bytes: self.body_hash,
        }
    }
    pub fn require_entry(&self) -> Result<(), Diagnostic> {
        if self.entry_blocked {
            Err(Diagnostic::new(
                "LRR-E-PARSE-BLOCKS-ENTRY",
                "entered body does not have reliable syntax boundaries".into(),
            ))
        } else {
            Ok(())
        }
    }
}
impl StableEncode for BodyHandle {
    const DOMAIN: &'static str = "lrr/v2/body-handle";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key());
        e.bool(self.entry_blocked);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemOutline {
    pub def_key: DefKey,
    pub scope: ScopeKey,
    pub header: HeaderRecipe,
    pub constructor_fields: Vec<Visibility>,
    pub body: Option<BodyHandle>,
    pub declaration_span: SourceRange,
    pub header_span: SourceRange,
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeMembers {
    pub candidates: BTreeMap<(Namespace, String), Vec<Digest>>,
    pub blockers: Vec<ScopeBlocker>,
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ScopeBlocker {
    pub kind: BlockerKind,
    pub syntax: Digest,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum BlockerKind {
    Attribute,
    ItemMacro,
    ParseBoundary,
}
impl StableEncode for ScopeBlocker {
    const DOMAIN: &'static str = "lrr/v2/scope-blocker";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(match self.kind {
            BlockerKind::Attribute => 1,
            BlockerKind::ItemMacro => 2,
            BlockerKind::ParseBoundary => 3,
        });
        e.digest(self.syntax);
    }
}
impl StableEncode for ScopeMembers {
    const DOMAIN: &'static str = "lrr/v2/scope-members";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.map(&self.candidates, |e, (namespace, name), candidates| {
            e.u32(namespace.tag());
            e.string(name);
            e.sequence(candidates, |e, d| e.digest(*d));
        });
        e.sequence(&self.blockers, |e, b| {
            e.u32(match b.kind {
                BlockerKind::Attribute => 1,
                BlockerKind::ItemMacro => 2,
                BlockerKind::ParseBoundary => 3,
            });
            e.digest(b.syntax);
        });
    }
}
impl ScopeMembers {
    pub fn normalize(&mut self) {
        for candidates in self.candidates.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        self.blockers.sort();
        self.blockers.dedup();
    }
    pub fn merge(&mut self, other: &Self) {
        for (key, candidates) in &other.candidates {
            self.candidates
                .entry(key.clone())
                .or_default()
                .extend(candidates);
        }
        self.blockers.extend(other.blockers.iter().cloned());
        self.normalize();
    }
}
/// Snapshot-relative textual positions are attachments, not persistent item IDs.
#[derive(Debug, Clone)]
pub struct TextualEvent {
    pub scope: ScopeKey,
    pub at: u64,
    pub kind: TextualEventKind,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProducerKind {
    ItemMacro,
    Attribute,
    ModuleSource,
}
#[derive(Debug, Clone)]
pub enum TextualEventKind {
    /// Module-scoped macro2 membership; never a textual macro_rules binding.
    ModuleDefinition {
        identity: Digest,
        source_name: String,
    },
    Definition {
        identity: Digest,
        source_name: String,
    },
    Producer {
        identity: Digest,
        kind: ProducerKind,
    },
    OpenScope {
        child: ScopeKey,
        ordinal: u32,
    },
}
/// Exact enabled original item calls; source ranges are parse attachments.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemInvocation {
    pub scope: ScopeKey,
    pub range: SourceRange,
    pub item_input: Digest,
    pub frontier: Digest,
    pub attribute_input: Option<Digest>,
}
#[derive(Debug, Clone)]
pub struct Outline {
    pub cfg: Digest,
    pub item_invocations: Vec<ItemInvocation>,
    pub items: Vec<ItemOutline>,
    pub textual_events: Vec<TextualEvent>,
    pub imports: Vec<super::imports::SourceImport>,
    pub extern_crates: Vec<super::imports::SourceExternCrate>,
    pub root_scope: ScopeKey,
    pub scopes: BTreeMap<ScopeKey, ScopeMembers>,
    pub content: Digest,
    pub profile: ProfileIdentity,
}
impl ItemOutline {
    pub fn constructor_field_bytes(&self) -> Result<usize, Diagnostic> {
        self.constructor_fields
            .iter()
            .try_fold(0usize, |n, v| {
                n.checked_add(std::mem::size_of::<Visibility>())
                    .and_then(|n| n.checked_add(super::imports::visibility_bytes(v)))
                    .filter(|n| *n <= 32 * 1024 * 1024)
            })
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "constructor metadata size limit"))
    }
}
impl StableEncode for Outline {
    const DOMAIN: &'static str = "lrr/v3/outline";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.cfg);
        e.record(&self.profile);
        e.sequence(&self.items, |e, item| {
            e.record(&item.def_key);
            e.record(&item.header);
            e.sequence(&item.constructor_fields, |e, v| e.record(v));
            e.option(&item.body, |e, b| e.record(b));
        });
        e.sequence(&self.extern_crates, |e, row| e.record(row));
        e.map(&self.scopes, |e, key, members| {
            e.record(key);
            e.record(members);
        });
    }
}
impl Outline {
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.item_invocations.capacity() * std::mem::size_of::<ItemInvocation>()
            + self
                .item_invocations
                .iter()
                .map(|call| call.scope.heap_bytes())
                .sum::<usize>()
            + self.textual_events.capacity() * std::mem::size_of::<TextualEvent>()
            + self
                .textual_events
                .iter()
                .map(|event| {
                    event.scope.heap_bytes()
                        + match &event.kind {
                            TextualEventKind::OpenScope { child, .. } => child.heap_bytes(),
                            TextualEventKind::Definition { source_name, .. }
                            | TextualEventKind::ModuleDefinition { source_name, .. } => {
                                source_name.capacity()
                            }
                            _ => 0,
                        }
                })
                .sum::<usize>()
            + self.imports.capacity() * std::mem::size_of::<super::imports::SourceImport>()
            + self
                .imports
                .iter()
                .map(|import| {
                    import.scope.heap_bytes()
                        + super::imports::visibility_bytes(&import.visibility)
                        + import.path.capacity()
                            * std::mem::size_of::<super::imports::ImportSegment>()
                        + import
                            .path
                            .iter()
                            .map(|part| match part {
                                super::imports::ImportSegment::Name(name) => name.capacity(),
                                _ => 0,
                            })
                            .sum::<usize>()
                        + match &import.kind {
                            super::imports::ImportKind::Named { alias: Some(alias) } => {
                                alias.capacity()
                            }
                            _ => 0,
                        }
                })
                .sum::<usize>()
            + self.extern_crates.capacity()
                * std::mem::size_of::<super::imports::SourceExternCrate>()
            + self
                .extern_crates
                .iter()
                .map(|row| {
                    row.scope.heap_bytes()
                        + row.dependency.capacity()
                        + row.order.capacity() * std::mem::size_of::<u32>()
                        + row.macro_use_bytes
                        + row.alias.as_ref().map_or(0, String::capacity)
                        + super::imports::visibility_bytes(&row.visibility)
                })
                .sum::<usize>()
            + self.root_scope.heap_bytes()
            + self.items.capacity() * std::mem::size_of::<ItemOutline>()
            + self
                .items
                .iter()
                .map(|i| {
                    i.header.estimated_bytes()
                        + i.constructor_fields.len() * std::mem::size_of::<Visibility>()
                        + i.constructor_fields
                            .iter()
                            .map(super::imports::visibility_bytes)
                            .sum::<usize>()
                        + i.def_key.declared_name.capacity()
                        + i.def_key
                            .module_path
                            .iter()
                            .map(String::capacity)
                            .sum::<usize>()
                })
                .sum::<usize>()
            + self
                .scopes
                .values()
                .map(|s| {
                    s.candidates
                        .iter()
                        .map(|((_, name), values)| name.capacity() + values.capacity() * 32 + 64)
                        .sum::<usize>()
                        + s.blockers.capacity() * std::mem::size_of::<ScopeBlocker>()
                })
                .sum::<usize>()
    }
}
fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub(crate) fn constructor_fields(
    item: &ast::Item,
    cfg: &CfgSet,
) -> Result<Vec<Visibility>, Diagnostic> {
    let ast::Item::Struct(item) = item else {
        return Ok(vec![]);
    };
    let Some(fields) = item.syntax().children().find_map(ast::TupleFieldList::cast) else {
        return Ok(vec![]);
    };
    let mut result = Vec::new();
    let mut bytes = 0usize;
    for field in fields.fields() {
        let (enabled, _) = super::attributes::effective_metas(
            field.attrs().filter_map(|a| a.meta()).collect(),
            cfg,
        )?;
        if !enabled {
            continue;
        }
        let vis = visibility(field.syntax());
        bytes = bytes
            .checked_add(std::mem::size_of::<Visibility>())
            .and_then(|n| n.checked_add(super::imports::visibility_bytes(&vis)))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-SOURCE-LIMIT",
                    "constructor visibility metadata limit",
                )
            })?;
        if result.len() >= 1_000_000 {
            return Err(fault("LRR-E-SOURCE-LIMIT", "constructor field count limit"));
        }
        result
            .try_reserve(1)
            .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "constructor field allocation failed"))?;
        result.push(vis);
    }
    Ok(result)
}
pub(crate) fn visibility(node: &SyntaxNode) -> Visibility {
    let Some(vis) = node.children().find_map(ast::Visibility::cast) else {
        return Visibility::Private;
    };
    let ts = tokens(vis.syntax(), None);
    let spelling: String = ts.iter().map(|t| t.text.as_str()).collect();
    match spelling.as_str() {
        "pub" => Visibility::Public,
        "pub(crate)" => Visibility::Crate,
        "pub(super)" => Visibility::Super,
        _ => Visibility::Restricted(ts),
    }
}
pub(crate) fn attributes(
    item: &ast::Item,
    cfg: &CfgSet,
) -> Result<(bool, Vec<AttributeRecipe>), Diagnostic> {
    attributes_from_meta(item, cfg, item.attrs().filter_map(|a| a.meta()).collect())
}
pub(crate) fn attributes_from_meta(
    item: &ast::Item,
    cfg: &CfgSet,
    pending: Vec<ast::Meta>,
) -> Result<(bool, Vec<AttributeRecipe>), Diagnostic> {
    let (enabled, metas) = super::attributes::effective_metas(pending, cfg)?;
    let mut result = Vec::new();
    for meta in metas {
        let name = meta
            .path()
            .map(|p| {
                tokens(p.syntax(), None)
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<String>()
            })
            .or_else(|| meta.simple_name().map(|n| n.to_string()))
            .unwrap_or_default();
        let class = match name.as_str() {
            "doc" | "allow" | "warn" | "deny" | "forbid" | "inline" | "cold" | "must_use"
            | "deprecated" => AttributeClass::Harmless,
            "macro_export" if matches!(item, ast::Item::MacroRules(_)) => {
                AttributeClass::MacroExport
            }
            "no_implicit_prelude" if matches!(item, ast::Item::Module(_)) => {
                AttributeClass::Harmless
            }
            "macro_use"
                if matches!(item, ast::Item::Module(_))
                    && tokens(meta.syntax(), None).len() == 1 =>
            {
                AttributeClass::Harmless
            }
            "macro_use" if matches!(item, ast::Item::ExternCrate(_)) => AttributeClass::MacroUse,
            "cfg" => AttributeClass::Conditional,
            "test" | "ignore" | "should_panic" => AttributeClass::Test,
            "derive" => AttributeClass::Derive,
            "repr" => AttributeClass::Representation,
            "path" if matches!(item, ast::Item::Module(_)) => AttributeClass::ModulePath,
            "rustfmt::skip" => AttributeClass::ToolLint,
            n if n.starts_with("clippy::") => AttributeClass::ToolLint,
            _ => AttributeClass::TransformUnsupported,
        };
        result.push(AttributeRecipe {
            name,
            class,
            tokens: tokens(meta.syntax(), None),
            source_meta: Some(range(meta.syntax().text_range())),
        });
    }
    Ok((enabled, result))
}

/// Pinned parser recovery can retain a malformed declaration as ERROR rather
/// than an ast::Item. Such a declaration container cannot certify completeness.
/// Inspect direct children only: executable bodies and macro tokens stay opaque.
fn retain_declaration_parse_boundary(
    container: &SyntaxNode,
    scope: &ScopeKey,
    source: Digest,
    outline: &mut Outline,
) -> Result<(), Diagnostic> {
    let mut work = 0usize;
    for child in container.children() {
        work = work
            .checked_add(1)
            .filter(|count| *count <= 100_000)
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "declaration recovery work limit"))?;
        if child.kind() != ra_ap_syntax::SyntaxKind::ERROR {
            continue;
        }
        let error_range = range(child.text_range());
        let mut identity = StableEncoder::new("lrr/v2/declaration-parse-boundary");
        identity.digest(source);
        identity.record(scope);
        identity.u32(error_range.start);
        identity.u32(error_range.end);
        let members = outline.scopes.get_mut(scope).ok_or_else(|| {
            fault(
                "LRR-E-PARSE-BLOCKS-ENTRY",
                "declaration recovery scope absent",
            )
        })?;
        members
            .blockers
            .len()
            .checked_add(1)
            .filter(|count| *count <= 100_000)
            .and_then(|count| count.checked_mul(std::mem::size_of::<ScopeBlocker>()))
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("LRR-E-SOURCE-LIMIT", "declaration recovery metadata limit"))?;
        members.blockers.try_reserve(1).map_err(|_| {
            fault(
                "LRR-E-SOURCE-LIMIT",
                "declaration recovery allocation failed",
            )
        })?;
        members.blockers.push(ScopeBlocker {
            kind: BlockerKind::ParseBoundary,
            syntax: identity.finish(),
        });
        // One exact observed parse boundary is sufficient to prevent a seal.
        return Ok(());
    }
    Ok(())
}

impl ParsedFile {
    /// Pure header traversal. Function bodies and macro token trees are opaque.
    pub fn outline(
        &self,
        crate_key: Digest,
        module_path: Vec<String>,
        cfg: &CfgSet,
    ) -> Result<Outline, Diagnostic> {
        let initial = ScopeKey {
            crate_key,
            module_path: super::ast_adapter::semantic_module_path(&module_path).ok_or_else(
                || {
                    fault(
                        "LRR-E-SOURCE-LIMIT",
                        "invalid or oversized semantic module path",
                    )
                },
            )?,
            owner: None,
        };
        let mut outline = Outline {
            cfg: cfg.fingerprint(),
            item_invocations: Vec::new(),
            items: Vec::new(),
            textual_events: Vec::new(),
            imports: Vec::new(),
            extern_crates: Vec::new(),
            root_scope: initial.clone(),
            scopes: BTreeMap::from([(initial.clone(), ScopeMembers::default())]),
            content: self.content,
            profile: self.profile.clone(),
        };
        let root = self.parsed.tree();
        if !super::attributes::source_root_enabled(&root, cfg)? {
            return Ok(outline);
        }
        retain_declaration_parse_boundary(root.syntax(), &initial, self.content, &mut outline)?;
        let mut stack: Vec<_> = root
            .syntax()
            .children()
            .filter_map(ast::Item::cast)
            .map(|i| (i, initial.clone(), Vec::<TokenRecipe>::new()))
            .collect();
        stack.reverse();
        let mut import_budget = super::imports::ImportBudget::default();
        let mut ordinals: BTreeMap<(ScopeKey, Namespace, String), u32> = BTreeMap::new();
        let mut count = 0;
        while let Some((item, scope, context)) = stack.pop() {
            count += 1;
            if count > 100_000 {
                return Err(fault("LRR-E-SOURCE-LIMIT", "outline item limit"));
            }
            let (enabled, attributes) = attributes(&item, cfg)?;
            if !enabled {
                continue;
            }
            let node = item.syntax();
            let mut container: Option<SyntaxNode> = None;
            let mut executable: Option<SyntaxNode> = None;
            let mut blocked = false;
            let mut add_value_constructor = false;
            let (kind, namespace, name, index_name) = match &item {
                ast::Item::Fn(i) => {
                    executable = node
                        .children()
                        .find_map(ast::BlockExpr::cast)
                        .map(|b| b.syntax().clone());
                    if let Some(body) = &executable {
                        let ends: Vec<_> = body
                            .descendants_with_tokens()
                            .filter_map(|e| e.into_token())
                            .filter(|t| !t.kind().is_trivia())
                            .collect();
                        blocked = !ends.first().is_some_and(|t| t.text() == "{")
                            || !ends.last().is_some_and(|t| t.text() == "}");
                    }
                    (
                        ItemKind::Function,
                        Namespace::Value,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Const(i) => {
                    executable = node
                        .children()
                        .find_map(ast::Expr::cast)
                        .map(|e| e.syntax().clone());
                    (
                        ItemKind::Constant,
                        Namespace::Value,
                        i.name().map(|n| n.text().to_string()),
                        // `const _` is an actual unnamed item, not a missing
                        // identifier or a member callable as `_`. Retain its
                        // header/body/ordinal without publishing a name.
                        i.underscore_token().is_none(),
                    )
                }
                ast::Item::Static(i) => {
                    executable = node
                        .children()
                        .find_map(ast::Expr::cast)
                        .map(|e| e.syntax().clone());
                    (
                        ItemKind::Static,
                        Namespace::Value,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Struct(i) => {
                    add_value_constructor = node
                        .children()
                        .any(|n| ast::TupleFieldList::can_cast(n.kind()))
                        || i.semicolon_token().is_some()
                            && !node
                                .children()
                                .any(|n| ast::RecordFieldList::can_cast(n.kind()));
                    (
                        ItemKind::Struct,
                        Namespace::Type,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Union(i) => (
                    ItemKind::Union,
                    Namespace::Type,
                    i.name().map(|n| n.text().to_string()),
                    true,
                ),
                ast::Item::Enum(i) => (
                    ItemKind::Enum,
                    Namespace::Type,
                    i.name().map(|n| n.text().to_string()),
                    true,
                ),
                ast::Item::TypeAlias(i) => (
                    ItemKind::TypeAlias,
                    Namespace::Type,
                    i.name().map(|n| n.text().to_string()),
                    true,
                ),
                ast::Item::Module(i) => {
                    container = i.item_list().map(|n| n.syntax().clone());
                    (
                        ItemKind::Module,
                        Namespace::Type,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Trait(i) => {
                    container = i.assoc_item_list().map(|n| n.syntax().clone());
                    (
                        ItemKind::Trait,
                        Namespace::Type,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Impl(i) => {
                    container = i.assoc_item_list().map(|n| n.syntax().clone());
                    (ItemKind::Impl, Namespace::Type, None, false)
                }
                ast::Item::MacroRules(i) => {
                    container = i.token_tree().map(|n| n.syntax().clone());
                    (
                        ItemKind::Macro,
                        Namespace::Macro,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::MacroDef(i) => {
                    if self.generated_origin.is_some() || scope.owner.is_some() {
                        return Err(fault(
                            "LRR-E-MACRO2-SOURCE",
                            "non-original-module macro2 declaration dialect admission is not established",
                        ));
                    }
                    container = i
                        .syntax()
                        .children()
                        .filter_map(ast::TokenTree::cast)
                        .last()
                        .map(|n| n.syntax().clone());
                    (
                        ItemKind::Macro,
                        Namespace::Macro,
                        i.name().map(|n| n.text().to_string()),
                        true,
                    )
                }
                ast::Item::Use(_) => (ItemKind::Import, Namespace::Type, None, false),
                ast::Item::ExternCrate(i) => {
                    let anonymous = i
                        .rename()
                        .is_some_and(|rename| rename.underscore_token().is_some());
                    (
                        ItemKind::Module,
                        Namespace::Type,
                        if anonymous {
                            None
                        } else {
                            i.rename()
                                .and_then(|r| r.name())
                                .map(|n| n.text().to_string())
                                .or_else(|| i.name_ref().map(|n| n.text().to_string()))
                        },
                        !anonymous,
                    )
                }
                ast::Item::ExternBlock(i) => {
                    container = i.extern_item_list().map(|n| n.syntax().clone());
                    (ItemKind::ExternBlock, Namespace::Type, None, false)
                }
                ast::Item::MacroCall(_) | ast::Item::AsmExpr(_) => {
                    // Associated invocation slots belong to the retained
                    // declaration scan, never the module-item publisher.
                    if scope.owner.is_some() {
                        continue;
                    }
                    let ts = tokens(node, None);
                    let mut e = StableEncoder::new("lrr/v2/item-scope-blocker");
                    e.sequence(&ts, |e, t| e.record(t));
                    let syntax = e.finish();
                    let mut invocation = StableEncoder::new("lrr/v2/source-item-producer");
                    invocation.record(&scope);
                    invocation.digest(syntax);
                    // The local occurrence ordinal disambiguates repeated identical
                    // invocations without making a byte offset their identity.
                    invocation.u64(
                        u64::try_from(outline.textual_events.len()).map_err(|_| {
                            fault("LRR-E-SOURCE-LIMIT", "producer ordinal overflow")
                        })?,
                    );
                    let identity = invocation.finish();
                    let transforming = attributes.iter().any(|attribute| {
                        matches!(
                            attribute.class,
                            AttributeClass::TransformUnsupported | AttributeClass::Derive
                        )
                    });
                    let attribute_input = if transforming {
                        let mut e = StableEncoder::new("lrr/v2/source-item-attribute-producer");
                        e.record(&scope);
                        e.digest(identity);
                        Some(e.finish())
                    } else {
                        None
                    };
                    if matches!(item, ast::Item::MacroCall(_)) {
                        outline.item_invocations.try_reserve(1).map_err(|_| {
                            fault("LRR-E-SOURCE-LIMIT", "item invocation allocation failed")
                        })?;
                        outline.item_invocations.push(ItemInvocation {
                            scope: scope.clone(),
                            range: range(node.text_range()),
                            item_input: identity,
                            frontier: syntax,
                            attribute_input,
                        });
                    }
                    outline.textual_events.push(TextualEvent {
                        scope: scope.clone(),
                        at: u64::from(range(node.text_range()).start),
                        kind: TextualEventKind::Producer {
                            identity: attribute_input.unwrap_or(identity),
                            kind: if attribute_input.is_some() {
                                ProducerKind::Attribute
                            } else {
                                ProducerKind::ItemMacro
                            },
                        },
                    });
                    outline
                        .scopes
                        .entry(scope)
                        .or_default()
                        .blockers
                        .push(ScopeBlocker {
                            kind: if attribute_input.is_some() {
                                BlockerKind::Attribute
                            } else {
                                BlockerKind::ItemMacro
                            },
                            syntax,
                        });
                    continue;
                }
            };
            let name = match name {
                Some(n) => super::ast_adapter::semantic_name(&n).ok_or_else(|| {
                    fault(
                        "LRR-E-SOURCE-LIMIT",
                        "semantic declaration name limit exceeded",
                    )
                })?,
                None if index_name => {
                    outline
                        .scopes
                        .entry(scope)
                        .or_default()
                        .blockers
                        .push(ScopeBlocker {
                            kind: BlockerKind::ParseBoundary,
                            syntax: self.content,
                        });
                    continue;
                }
                None => String::new(),
            };
            let excluded = executable
                .as_ref()
                .or(container.as_ref())
                .map(|n| n.text_range());
            let header = HeaderRecipe {
                profile: self.profile.clone(),
                tokens: tokens(node, excluded),
                context,
                attributes,
                visibility: visibility(node),
            };
            let impl_identity = if kind == ItemKind::Impl {
                Some(header.fingerprint())
            } else {
                scope.owner
            };
            let ordinal = ordinals
                .entry((scope.clone(), namespace, name.clone()))
                .or_default();
            let duplicate_ordinal = *ordinal;
            *ordinal = ordinal.checked_add(1).ok_or_else(|| {
                fault(
                    "LRR-E-SOURCE-LIMIT",
                    "duplicate declaration ordinal overflow",
                )
            })?;
            let def_key = DefKey {
                crate_key,
                module_path: scope.module_path.clone(),
                namespace,
                item_kind: kind,
                declared_name: name.clone(),
                enclosing_impl: impl_identity,
                duplicate_ordinal,
            };
            let definition = def_key.fingerprint();
            if let ast::Item::Use(import) = &item {
                let facts = super::imports::collect(
                    import,
                    &scope,
                    definition,
                    &header.visibility,
                    &mut import_budget,
                )?;
                outline
                    .imports
                    .try_reserve(facts.len())
                    .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "import index allocation failed"))?;
                outline.imports.extend(facts);
            }
            if let ast::Item::ExternCrate(external) = &item {
                import_budget.charge(
                    scope
                        .heap_bytes()
                        .checked_add(usize::from(external.syntax().text_range().len()))
                        .and_then(|n| n.checked_add(name.len()))
                        .and_then(|n| {
                            n.checked_add(super::imports::visibility_bytes(&header.visibility))
                        })
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(|| {
                            fault("LRR-E-SOURCE-LIMIT", "extern crate metadata overflow")
                        })?,
                )?;
                let dependency = external
                    .name_ref()
                    .ok_or_else(|| fault("LRR-E-PARSE-BLOCKS-ENTRY", "extern crate name absent"))?
                    .text()
                    .to_string();
                if outline.extern_crates.len() >= 100_000 {
                    return Err(fault("LRR-E-SOURCE-LIMIT", "extern crate count exceeded"));
                }
                outline
                    .extern_crates
                    .try_reserve(1)
                    .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "extern crate allocation failed"))?;
                let macro_use = super::imports::macro_use_recipe(&header.attributes)?;
                let macro_use_bytes = macro_use
                    .as_ref()
                    .map(|r| r.bytes())
                    .transpose()?
                    .unwrap_or(0);
                import_budget.charge(macro_use_bytes)?;
                outline
                    .extern_crates
                    .push(super::imports::SourceExternCrate {
                        identity: definition,
                        scope: scope.clone(),
                        dependency,
                        alias: if external
                            .rename()
                            .is_some_and(|rename| rename.underscore_token().is_some())
                        {
                            None
                        } else {
                            Some(name.clone())
                        },
                        visibility: header.visibility.clone(),
                        macro_use,
                        macro_use_bytes,
                        order: vec![u32::from(external.syntax().text_range().start())],
                    });
            }
            let members = outline.scopes.entry(scope.clone()).or_default();
            if index_name {
                members
                    .candidates
                    .entry((namespace, name.clone()))
                    .or_default()
                    .push(definition);
                if add_value_constructor {
                    members
                        .candidates
                        .entry((Namespace::Value, name.clone()))
                        .or_default()
                        .push(definition);
                }
            }
            if scope.owner.is_none()
                && header
                    .attributes
                    .iter()
                    .any(|a| a.class == AttributeClass::TransformUnsupported)
            {
                members.blockers.push(ScopeBlocker {
                    kind: BlockerKind::Attribute,
                    syntax: header.fingerprint(),
                });
            }
            let body = if let Some(body) = executable {
                Some(BodyHandle {
                    def_key: definition,
                    body_hash: self.body_content_identity(&body)?,
                    range: range(body.text_range()),
                    entry_blocked: blocked,
                })
            } else {
                None
            };
            let span = range(node.text_range());
            let transform = scope.owner.is_none()
                && header.attributes.iter().any(|a| {
                    matches!(
                        a.class,
                        AttributeClass::TransformUnsupported | AttributeClass::Derive
                    )
                });
            if transform {
                let mut producer = StableEncoder::new("lrr/v2/source-attribute-producer");
                producer.digest(definition);
                producer.record(&header);
                outline.textual_events.push(TextualEvent {
                    scope: scope.clone(),
                    at: u64::from(span.start),
                    kind: TextualEventKind::Producer {
                        identity: producer.finish(),
                        kind: ProducerKind::Attribute,
                    },
                });
            } else if let ast::Item::MacroRules(macro_rules) = &item {
                outline.textual_events.push(TextualEvent {
                    scope: scope.clone(),
                    at: u64::from(span.start),
                    kind: TextualEventKind::Definition {
                        identity: definition,
                        source_name: macro_rules
                            .name()
                            .ok_or_else(|| {
                                fault("LRR-E-PARSE-BLOCKS-ENTRY", "macro name is missing")
                            })?
                            .syntax()
                            .text()
                            .to_string(),
                    },
                });
            } else if let ast::Item::MacroDef(macro_definition) = &item {
                outline.textual_events.push(TextualEvent {
                    scope: scope.clone(),
                    at: u64::from(span.start),
                    kind: TextualEventKind::ModuleDefinition {
                        identity: definition,
                        source_name: macro_definition
                            .name()
                            .ok_or_else(|| {
                                fault("LRR-E-PARSE-BLOCKS-ENTRY", "module macro name missing")
                            })?
                            .syntax()
                            .text()
                            .to_string(),
                    },
                });
            }

            let header_span = SourceRange {
                start: span.start,
                end: excluded.map(|r| u32::from(r.start())).unwrap_or(span.end),
            };
            if let Some(container) = container {
                let child_scope = match kind {
                    ItemKind::Module => {
                        let mut s = scope.clone();
                        s.module_path.push(name);
                        s.owner = None;
                        s
                    }
                    ItemKind::Impl | ItemKind::Trait => {
                        let mut s = scope.clone();
                        s.owner = Some(if kind == ItemKind::Impl {
                            header.fingerprint()
                        } else {
                            definition
                        });
                        s
                    }
                    _ => scope.clone(),
                };
                if !matches!(kind, ItemKind::Macro) {
                    if child_scope != scope {
                        outline.textual_events.push(TextualEvent {
                            scope: scope.clone(),
                            at: u64::from(span.start),
                            kind: TextualEventKind::OpenScope {
                                child: child_scope.clone(),
                                ordinal: duplicate_ordinal,
                            },
                        });
                    }
                    outline.scopes.entry(child_scope.clone()).or_default();
                    retain_declaration_parse_boundary(
                        &container,
                        &child_scope,
                        self.content,
                        &mut outline,
                    )?;
                    let mut children: Vec<_> = container
                        .children()
                        .filter_map(ast::Item::cast)
                        .map(|i| {
                            (
                                i,
                                child_scope.clone(),
                                if matches!(
                                    kind,
                                    ItemKind::Impl | ItemKind::Trait | ItemKind::ExternBlock
                                ) {
                                    header.tokens.clone()
                                } else {
                                    Vec::new()
                                },
                            )
                        })
                        .collect();
                    children.reverse();
                    stack.extend(children);
                }
            } else if matches!(item, ast::Item::Module(_)) {
                let mut child = scope.clone();
                child.module_path.push(name);
                child.owner = None;
                outline.textual_events.push(TextualEvent {
                    scope: scope.clone(),
                    at: u64::from(span.start),
                    kind: TextualEventKind::OpenScope {
                        child: child.clone(),
                        ordinal: duplicate_ordinal,
                    },
                });
                let mut producer = StableEncoder::new("lrr/v2/source-module-input");
                producer.digest(definition);
                outline.textual_events.push(TextualEvent {
                    scope: child.clone(),
                    at: 0,
                    kind: TextualEventKind::Producer {
                        identity: producer.finish(),
                        kind: ProducerKind::ModuleSource,
                    },
                });
                outline.scopes.entry(child).or_default();
            }
            outline.items.push(ItemOutline {
                def_key,
                scope,
                header,
                constructor_fields: constructor_fields(&item, cfg)?,
                body,
                declaration_span: span,
                header_span,
            });
        }
        for members in outline.scopes.values_mut() {
            members.normalize();
        }
        Ok(outline)
    }
}

/// A test marker can instead resolve to a procedural attribute of the same name.
/// Discovery reserves that candidate only in a configured procedural namespace.
pub(crate) fn potential_attribute_producer(item: &ItemOutline) -> Option<ItemInvocation> {
    if let Some(call) = attribute_producer(item) {
        return Some(call);
    }
    if item.scope.owner.is_some()
        || !item
            .header
            .attributes
            .iter()
            .any(|a| a.name == "test" && a.class == AttributeClass::Test)
    {
        return None;
    }
    attribute_invocation(item)
}
fn attribute_invocation(item: &ItemOutline) -> Option<ItemInvocation> {
    let mut encoder = crate::core::digest::StableEncoder::new("lrr/v2/source-attribute-producer");
    encoder.digest(item.def_key.fingerprint());
    encoder.record(&item.header);
    let identity = encoder.finish();
    Some(ItemInvocation {
        scope: item.scope.clone(),
        range: item.declaration_span,
        item_input: identity,
        frontier: item.header.fingerprint(),
        attribute_input: Some(identity),
    })
}

pub fn attribute_producer(item: &ItemOutline) -> Option<ItemInvocation> {
    if item.scope.owner.is_some() {
        return None;
    }
    if !item.header.attributes.iter().any(|attribute| {
        matches!(
            attribute.class,
            crate::syntax::outline::AttributeClass::TransformUnsupported
                | crate::syntax::outline::AttributeClass::Derive
        )
    }) {
        return None;
    }
    let mut encoder = crate::core::digest::StableEncoder::new("lrr/v2/source-attribute-producer");
    encoder.digest(item.def_key.fingerprint());
    encoder.record(&item.header);
    let identity = encoder.finish();
    Some(ItemInvocation {
        scope: item.scope.clone(),
        range: item.declaration_span,
        item_input: identity,
        frontier: item.header.fingerprint(),
        attribute_input: Some(identity),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snapshot::SourceBlob;
    fn outline(text: &str) -> Outline {
        ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap()
        .outline(Digest::of_bytes(b"crate"), vec![], &CfgSet::guest_default())
        .unwrap()
    }
    #[test]
    fn associated_annotations_and_item_macros_keep_declaration_worklist_ownership() {
        let captured = outline(
            "struct A; impl A { #[decorate] fn new() {} emit!(); } #[decorate] fn free() {}",
        );
        let method = captured
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "new")
            .unwrap();
        assert!(method.scope.owner.is_some());
        assert_eq!(method.def_key.enclosing_impl, method.scope.owner);
        assert!(
            attribute_producer(method).is_none(),
            "associated annotation entered module item worklist"
        );
        assert!(potential_attribute_producer(method).is_none());
        let free = captured
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "free")
            .unwrap();
        assert!(attribute_producer(free).is_some());
        assert!(
            captured
                .item_invocations
                .iter()
                .all(|call| call.scope.owner.is_none()),
            "associated macro entered module item worklist"
        );
        assert!(
            captured
                .textual_events
                .iter()
                .all(|event| event.scope.owner.is_none()
                    || !matches!(event.kind, TextualEventKind::Producer { .. })),
            "associated producer became an ordinary textual frontier"
        );
    }
    #[test]
    fn body_edit_retains_header_and_bucket_fingerprints() {
        let a = outline("fn f(x:i64)->i64 { x+1_i64 } fn cold(){missing();}");
        let b = outline("fn f(x:i64)->i64 { x+9_i64 } fn cold(){missing();}");
        assert_eq!(
            a.items[0].def_key.fingerprint(),
            b.items[0].def_key.fingerprint()
        );
        assert_eq!(
            a.items[0].header.fingerprint(),
            b.items[0].header.fingerprint()
        );
        assert_ne!(
            a.items[0].body.as_ref().unwrap().body_hash,
            b.items[0].body.as_ref().unwrap().body_hash
        );
        assert_eq!(
            a.scopes.values().next().unwrap().fingerprint(),
            b.scopes.values().next().unwrap().fingerprint()
        );
    }
    #[test]
    fn outlining_cold_missing_and_unsupported_bodies_succeeds() {
        let o = outline(
            "fn cold(_: Absent) { absent(); let f=|| async { undefined() }; custom!(); } fn live() {}",
        );
        assert_eq!(o.items.len(), 2);
        assert!(o.scopes.values().all(|s| s.blockers.is_empty()));
        assert!(o.items[0].body.as_ref().is_some_and(|b| !b.entry_blocked));
    }
    #[test]
    fn impl_body_edits_keep_identity_and_header_edits_change_it() {
        let a = outline("struct A; impl A {fn f(){absent();}} ");
        let b = outline("struct A; impl A {fn f(){other();}} ");
        let c = outline("struct A; impl<T> A {fn f(){other();}} ");
        assert_eq!(
            a.items[2].def_key.fingerprint(),
            b.items[2].def_key.fingerprint()
        );
        assert_ne!(
            b.items[2].def_key.fingerprint(),
            c.items[2].def_key.fingerprint()
        );
    }
    #[test]
    fn duplicate_candidates_and_scope_blockers_are_deterministic() {
        let a = outline("fn f(){} fn f(){} item_macro!(); #[transform] fn g(){}");
        let s = a.scopes.values().next().unwrap();
        assert_eq!(s.candidates[&(Namespace::Value, "f".into())].len(), 2);
        assert_ne!(
            a.items[0].def_key.fingerprint(),
            a.items[1].def_key.fingerprint()
        );
        assert_eq!(s.blockers.len(), 2);
        assert_eq!(
            a.fingerprint(),
            outline("fn f(){} fn f(){} item_macro!(); #[transform] fn g(){}").fingerprint()
        );
    }
    #[test]
    fn outlines_cfg_derives_and_attributes_without_expansion() {
        let o = outline(
            "#[cfg(any())] fn absent(){} #[derive(Copy, Unknown)] struct S; #[cfg_attr(unix, test)] fn case(){} macro_rules! m {() => {unknown()}} ",
        );
        assert_eq!(o.items.len(), 3);
        assert!(
            o.items[1]
                .header
                .attributes
                .iter()
                .any(|a| a.class == AttributeClass::Test)
        );
        assert!(o.scopes.values().all(|s| s.blockers.is_empty()));
    }
    #[test]
    fn unmatched_body_delimiter_blocks_only_entry() {
        let o = outline("fn cold() { let x=1_i64;");
        assert!(o.items[0].body.as_ref().unwrap().require_entry().is_err());
    }
    #[test]
    fn unchanged_anonymous_const_headers_are_distinct_unnamed_items_all_editions() {
        const SOURCE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/loop.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse(
                &crate::source::snapshot::SourceBlob::new(SOURCE.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let outline = parsed
                .outline(Digest([225; 32]), Vec::new(), &CfgSet::guest_default())
                .unwrap();
            let anonymous = outline
                .items
                .iter()
                .filter(|item| {
                    item.def_key.item_kind == ItemKind::Constant
                        && item.def_key.declared_name.is_empty()
                })
                .collect::<Vec<_>>();
            assert_eq!(anonymous.len(), 5);
            let definitions = anonymous
                .iter()
                .map(|item| item.def_key.fingerprint())
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(definitions.len(), 5);
            let members = outline.scopes.get(&outline.root_scope).unwrap();
            assert!(members.blockers.is_empty());
            assert!(
                !members
                    .candidates
                    .contains_key(&(Namespace::Value, String::new()))
            );
            assert!(
                !members
                    .candidates
                    .contains_key(&(Namespace::Value, "_".into()))
            );
            for item in anonymous {
                assert!(item.body.as_ref().is_some_and(|body| !body.entry_blocked));
                assert!(
                    SOURCE
                        [item.declaration_span.start as usize..item.declaration_span.end as usize]
                        .starts_with("const _:")
                );
            }
        }
    }
    #[test]
    fn anonymous_body_edits_keep_headers_without_hiding_missing_identifiers() {
        let before = outline("const _:i32=1; const _:i32=2; fn f(){}");
        let after = outline("const _:i32=9; const _:i32=8; fn f(){}");
        for (before, after) in before.items.iter().zip(&after.items) {
            assert_eq!(before.def_key, after.def_key);
            assert_eq!(before.header, after.header);
        }
        assert_ne!(
            before.items[0].body.as_ref().unwrap().key(),
            after.items[0].body.as_ref().unwrap().key()
        );
        let malformed = outline("const :i32=1; fn f(){}");
        assert!(malformed.scopes.values().any(|scope| {
            scope
                .blockers
                .iter()
                .any(|blocker| blocker.kind == BlockerKind::ParseBoundary)
        }));
    }
    #[test]
    fn malformed_const_recovery_cannot_silently_complete_a_declaration_scope() {
        for edition in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(b"const :i32=1; fn f(){}".to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            assert!(!parsed.errors.is_empty());
            let captured = parsed
                .outline(Digest([226; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let root = &captured.scopes[&captured.root_scope];
            assert!(
                root.blockers
                    .iter()
                    .any(|blocker| blocker.kind == BlockerKind::ParseBoundary)
            );
            assert_eq!(captured.items.len(), 1);
            assert_eq!(captured.items[0].def_key.declared_name, "f");
            assert!(
                root.candidates
                    .contains_key(&(Namespace::Value, "f".into()))
            );
            assert!(
                !root
                    .candidates
                    .contains_key(&(Namespace::Value, String::new()))
            );
            assert!(
                !root
                    .candidates
                    .contains_key(&(Namespace::Value, "_".into()))
            );
        }
    }
    #[test]
    fn nested_declaration_recovery_does_not_inspect_cold_bodies_or_macro_tokens() {
        let captured = outline(
            "mod broken {const :i32=1;} mod clean {fn f(){}} fn cold(){let = ;} macro_rules! m {()=>{const :i32=1;}}",
        );
        assert!(captured.scopes[&captured.root_scope].blockers.is_empty());
        let broken = ScopeKey {
            module_path: vec!["broken".into()],
            ..captured.root_scope.clone()
        };
        assert!(
            captured.scopes[&broken]
                .blockers
                .iter()
                .any(|blocker| blocker.kind == BlockerKind::ParseBoundary)
        );
        let clean = ScopeKey {
            module_path: vec!["clean".into()],
            ..captured.root_scope.clone()
        };
        assert!(captured.scopes[&clean].blockers.is_empty());
        let cold = captured
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "cold")
            .unwrap();
        assert!(cold.body.as_ref().unwrap().require_entry().is_ok());
        assert!(
            captured
                .items
                .iter()
                .any(|item| item.def_key.declared_name == "m")
        );
        assert_eq!(captured.items.len(), 5);
    }
}
