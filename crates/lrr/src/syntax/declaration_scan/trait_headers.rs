//! Common AST capture of trait/associated declaration facts. No member lookup,
//! symbolic lowering, canonical DefKey, applicability or executable body grant.
use super::*;
use crate::syntax::token_wire::Key;
use ra_ap_syntax::ast::{HasGenericParams, HasTypeBounds};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationNameContext {
    /// The original source's root hygiene must still be joined by its owner.
    Original,
    /// Raw physical token context, not a resolved item-namespace identity.
    Captured(Key),
    /// Owned generated syntax without an admitted token-context attachment.
    /// This fact cannot grant name equivalence or namespace absence.
    UnattachedGenerated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationNameOccurrence {
    pub spelling: String,
    pub range: SourceRange,
    pub edition: u16,
    pub context: DeclarationNameContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssociatedDeclarationKind {
    Function,
    Const,
    TypeAlias,
    MacroProducer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssociatedDeclarationOwner {
    Trait(Digest),
    Impl(Digest),
    /// An admitted fragment's actual parent still belongs to GeneratedDeclarations
    /// and its producer/scan chain, never an owner inferred from this grammar.
    Fragment {
        grammar: GrammarSlot,
        lexical_scope: MacroScope,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedDeclarationHeader {
    /// Exact scan-relative source entry, not a canonical declaration/call key.
    pub identity: Digest,
    pub kind: AssociatedDeclarationKind,
    pub owner: AssociatedDeclarationOwner,
    pub name: Option<DeclarationNameOccurrence>,
    pub range: SourceRange,
    pub header: Vec<TokenRecipe>,
    pub generic_params: Option<SourceRange>,
    pub where_clause: Option<SourceRange>,
    /// Function body or const initializer attachment only; never executable prep.
    pub body: Option<SourceRange>,
    pub lexical_scope: MacroScope,
    pub attribute_inputs: Vec<Digest>,
    /// Unknown macro output remains a producer, not an associated function.
    pub producer: Option<Digest>,
}

#[derive(Clone, Debug)]
pub struct ScannedTrait {
    /// Actual source entry; canonical trait identity requires a later source join.
    pub identity: Digest,
    /// Raw header semantics exclude ordinary method bodies and source offsets.
    pub header_identity: Digest,
    pub name: Option<DeclarationNameOccurrence>,
    pub range: SourceRange,
    pub header: Vec<TokenRecipe>,
    pub generic_params: Option<SourceRange>,
    pub where_clause: Option<SourceRange>,
    pub supertraits: Option<SourceRange>,
    pub associated: Vec<AssociatedDeclarationHeader>,
    pub scope: ScopeKey,
    pub containers: Vec<SourceRange>,
    pub lexical_scope: MacroScope,
    pub attribute_inputs: Vec<Digest>,
}

fn annotations(
    node: &SyntaxNode,
    scan: &DeclarationScan,
    cfg: &CfgSet,
    budget: &mut Budget,
) -> Result<Vec<Digest>, Diagnostic> {
    let (_, metas) = enabled(node, cfg)?;
    budget.charge(
        metas
            .len()
            .checked_mul(std::mem::size_of::<Digest>())
            .ok_or_else(limit)?,
    )?;
    let mut inputs = Vec::new();
    inputs.try_reserve_exact(metas.len()).map_err(|_| limit())?;
    budget.charge(
        inputs
            .capacity()
            .checked_sub(metas.len())
            .ok_or_else(limit)?
            .checked_mul(std::mem::size_of::<Digest>())
            .ok_or_else(limit)?,
    )?;
    inputs.extend(
        metas
            .iter()
            .map(|meta| source_identity(scan, range(meta.syntax().text_range()), 1)),
    );
    Ok(inputs)
}

fn name(
    parsed: &ParsedFile,
    name: Option<ast::Name>,
    budget: &mut Budget,
) -> Result<Option<DeclarationNameOccurrence>, Diagnostic> {
    let Some(name) = name else { return Ok(None) };
    let text = name.text();
    budget.charge(text.len())?;
    let mut spelling = String::new();
    spelling
        .try_reserve_exact(text.len())
        .map_err(|_| limit())?;
    spelling.push_str(&text);
    budget.charge(
        spelling
            .capacity()
            .checked_sub(text.len())
            .ok_or_else(limit)?,
    )?;
    let selected = range(name.syntax().text_range());
    let edition = parsed.token_edition(selected)?;
    // Capture the pinned AST's raw occurrence, including anonymous `_` where
    // the grammar permits it. Normalization/name admission belongs to the
    // later canonical declaration join, never a synthetic parsing wrapper.
    let context = if parsed.generated_origin.is_some() && parsed.hygiene_facts().is_none() {
        DeclarationNameContext::UnattachedGenerated
    } else {
        match parsed.identifier_context(selected)? {
            Some(context) => DeclarationNameContext::Captured(context),
            None => DeclarationNameContext::Original,
        }
    };
    Ok(Some(DeclarationNameOccurrence {
        spelling,
        range: selected,
        edition,
        context,
    }))
}

pub(super) fn associated(
    parsed: &ParsedFile,
    scan: &DeclarationScan,
    cfg: &CfgSet,
    node: &SyntaxNode,
    owner: AssociatedDeclarationOwner,
    lexical_scope: MacroScope,
    budget: &mut Budget,
) -> Result<AssociatedDeclarationHeader, Diagnostic> {
    budget.charge(std::mem::size_of::<AssociatedDeclarationHeader>() + 32)?;
    let item = ast::AssocItem::cast(node.clone()).ok_or_else(|| {
        Diagnostic::new(
            "LRR-E-SOURCE-PARSE",
            "associated declaration node differs".into(),
        )
    })?;
    let (kind, written_name, generic_params, where_clause, body) = match item {
        ast::AssocItem::Fn(item) => (
            AssociatedDeclarationKind::Function,
            item.name(),
            item.generic_param_list()
                .map(|n| range(n.syntax().text_range())),
            item.where_clause().map(|n| range(n.syntax().text_range())),
            item.body().map(|n| range(n.syntax().text_range())),
        ),
        ast::AssocItem::Const(item) => (
            AssociatedDeclarationKind::Const,
            item.name(),
            None,
            None,
            item.body().map(|n| range(n.syntax().text_range())),
        ),
        ast::AssocItem::TypeAlias(item) => (
            AssociatedDeclarationKind::TypeAlias,
            item.name(),
            item.generic_param_list()
                .map(|n| range(n.syntax().text_range())),
            item.where_clause().map(|n| range(n.syntax().text_range())),
            None,
        ),
        ast::AssocItem::MacroCall(_) => (
            AssociatedDeclarationKind::MacroProducer,
            None,
            None,
            None,
            None,
        ),
    };
    let selected = range(node.text_range());
    Ok(AssociatedDeclarationHeader {
        identity: source_identity(scan, selected, 7),
        kind,
        owner,
        name: name(parsed, written_name, budget)?,
        range: selected,
        header: budget.tokens(node, body_exclusion(node))?,
        generic_params,
        where_clause,
        body,
        lexical_scope,
        attribute_inputs: annotations(node, scan, cfg, budget)?,
        producer: (kind == AssociatedDeclarationKind::MacroProducer)
            .then(|| source_identity(scan, selected, 2)),
    })
}

pub(super) fn associated_list(
    parsed: &ParsedFile,
    scan: &DeclarationScan,
    cfg: &CfgSet,
    list: &ast::AssocItemList,
    owner: AssociatedDeclarationOwner,
    parent_scope: MacroScope,
    budget: &mut Budget,
) -> Result<Vec<AssociatedDeclarationHeader>, Diagnostic> {
    let scope = parent_scope.child(u64::from(range(list.syntax().text_range()).start), 0);
    let mut headers = Vec::new();
    for node in list
        .syntax()
        .children()
        .filter(|n| ast::AssocItem::can_cast(n.kind()))
    {
        if !enabled(&node, cfg)?.0 {
            continue;
        }
        if headers.len() >= budget.limits.nodes {
            return Err(limit());
        }
        let header = associated(parsed, scan, cfg, &node, owner.clone(), scope, budget)?;
        reserve_one(&mut headers, budget)?;
        headers.push(header);
    }
    Ok(headers)
}

/// Capture already charges the row's fixed storage. Account any allocator
/// capacity beyond that row as well, before it can enter the finished scan.
pub(super) fn reserve_one<T>(rows: &mut Vec<T>, budget: &mut Budget) -> Result<(), Diagnostic> {
    let before = rows.capacity();
    rows.try_reserve_exact(1).map_err(|_| limit())?;
    let growth = rows.capacity().checked_sub(before).ok_or_else(limit)?;
    budget.charge(
        growth
            .saturating_sub(1)
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(limit)?,
    )
}

pub(super) fn capture_trait(
    parsed: &ParsedFile,
    scan: &DeclarationScan,
    cfg: &CfgSet,
    item: &ast::Trait,
    lexical_scope: MacroScope,
    budget: &mut Budget,
) -> Result<ScannedTrait, Diagnostic> {
    budget.charge(std::mem::size_of::<ScannedTrait>() + 64)?;
    let node = item.syntax();
    let selected = range(node.text_range());
    let identity = source_identity(scan, selected, 6);
    let list = item.assoc_item_list();
    let header = budget.tokens(node, list.as_ref().map(|n| n.syntax().text_range()))?;
    let associated = match list.as_ref() {
        Some(list) => associated_list(
            parsed,
            scan,
            cfg,
            list,
            AssociatedDeclarationOwner::Trait(identity),
            lexical_scope,
            budget,
        )?,
        None => Vec::new(),
    };
    let (scope, containers) = lexical_context(node, &scan.scope, budget)?;
    let mut semantic = StableEncoder::new("lrr/v1/scanned-trait-header");
    semantic.record(&parsed.profile);
    semantic.record(&scope);
    semantic.sequence(&header, |e, token| e.record(token));
    semantic.sequence(&associated, |e, row| {
        e.sequence(&row.header, |e, token| e.record(token));
    });
    Ok(ScannedTrait {
        identity,
        header_identity: semantic.finish(),
        name: name(parsed, item.name(), budget)?,
        range: selected,
        header,
        generic_params: item
            .generic_param_list()
            .map(|n| range(n.syntax().text_range())),
        where_clause: item.where_clause().map(|n| range(n.syntax().text_range())),
        supertraits: item
            .type_bound_list()
            .map(|n| range(n.syntax().text_range())),
        associated,
        scope,
        containers,
        lexical_scope,
        attribute_inputs: annotations(node, scan, cfg, budget)?,
    })
}

pub(super) fn encode_associated(e: &mut StableEncoder, row: &AssociatedDeclarationHeader) {
    e.digest(row.identity);
    e.u8(match row.kind {
        AssociatedDeclarationKind::Function => 0,
        AssociatedDeclarationKind::Const => 1,
        AssociatedDeclarationKind::TypeAlias => 2,
        AssociatedDeclarationKind::MacroProducer => 3,
    });
    match &row.owner {
        AssociatedDeclarationOwner::Trait(id) => {
            e.u8(0);
            e.digest(*id);
        }
        AssociatedDeclarationOwner::Impl(id) => {
            e.u8(1);
            e.digest(*id);
        }
        AssociatedDeclarationOwner::Fragment {
            grammar,
            lexical_scope,
        } => {
            e.u8(2);
            e.record(grammar);
            e.record(lexical_scope);
        }
    }
    e.option(&row.name, encode_name);
    position(e, row.range);
    e.sequence(&row.header, |e, token| e.record(token));
    e.option(&row.generic_params, |e, r| position(e, *r));
    e.option(&row.where_clause, |e, r| position(e, *r));
    e.option(&row.body, |e, r| position(e, *r));
    e.record(&row.lexical_scope);
    e.sequence(&row.attribute_inputs, |e, id| e.digest(*id));
    e.option(&row.producer, |e, id| e.digest(*id));
}

pub(super) fn encode_name(e: &mut StableEncoder, name: &DeclarationNameOccurrence) {
    e.string(&name.spelling);
    position(e, name.range);
    e.u16(name.edition);
    match &name.context {
        DeclarationNameContext::Original => e.u8(0),
        DeclarationNameContext::Captured(context) => {
            e.u8(1);
            e.bytes(&context.0);
        }
        DeclarationNameContext::UnattachedGenerated => e.u8(2),
    }
}
