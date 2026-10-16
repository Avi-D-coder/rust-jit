//! Lexical facts for one explicitly entered body, using the pinned AST only.
use super::{
    ast_adapter::{range, tokens},
    outline::{AttributeClass, AttributeRecipe, BodyHandle, ProducerKind, attributes},
    parse::ParsedFile,
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    source::snapshot::CfgSet,
};
use ra_ap_syntax::{
    AstNode, SyntaxNode, TextRange, TextSize,
    ast::{self, HasAttrs, HasLoopBody, HasName},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub struct BodyMacroScope {
    pub ordinal: u32,
    pub parent: Option<u32>,
    pub range: SourceRange,
}
#[derive(Debug, Clone)]
pub enum BodyMacroEventKind {
    Definition {
        identity: Digest,
        name: String,
        attributes: Vec<AttributeRecipe>,
    },
    Producer {
        identity: Digest,
        kind: ProducerKind,
    },
}
#[derive(Debug, Clone)]
pub struct BodyMacroEvent {
    pub scope: u32,
    pub at: u64,
    pub range: SourceRange,
    pub kind: BodyMacroEventKind,
}
#[derive(Debug, Clone)]
pub struct BodyMacroInvocation {
    pub range: SourceRange,
    pub scope: u32,
    pub attribute_input: Option<Digest>,
}
#[derive(Debug, Clone)]
pub struct BodyImport {
    pub scope: u32,
    /// Start of the retained use declaration, before any of its leaf routes.
    pub at: u64,
    pub source: super::imports::SourceImport,
    pub attribute_input: Option<Digest>,
}
#[derive(Debug, Clone)]
pub struct EnteredMacroFacts {
    pub bindings: Vec<super::local_bindings::LocalBindingRecipe>,
    pub items: Vec<super::body_items::BodyItem>,
    pub imports: Vec<BodyImport>,
    pub invocations: Vec<BodyMacroInvocation>,
    pub scopes: Vec<BodyMacroScope>,
    pub events: Vec<BodyMacroEvent>,
    pub opaque: Vec<SourceRange>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl ParsedFile {
    pub(crate) fn entered_macro_facts(
        &self,
        body: &BodyHandle,
        cfg: &CfgSet,
        containing: &super::outline::ScopeKey,
    ) -> Result<EnteredMacroFacts, Diagnostic> {
        body.require_entry()?;
        let root = self
            .entered_body_root(body)
            .map_err(|failure| error(&failure.message))?;
        self.lexical_facts(&root, body, cfg, containing, &[])
    }
    pub(crate) fn entered_container_facts(
        &self,
        handle: &super::body_items::ContainerHandle,
        cfg: &CfgSet,
        containing: &super::outline::ScopeKey,
        context: &[super::ast_adapter::TokenRecipe],
    ) -> Result<EnteredMacroFacts, Diagnostic> {
        use crate::source::manifest::ItemKind;
        if handle.entry_blocked {
            return Err(error("container entry requires prior attribute expansion"));
        }
        let root = self
            .parsed
            .tree()
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(handle.range.start),
                TextSize::from(handle.range.end),
            ))
            .into_node()
            .ok_or_else(|| error("container syntax boundary missing"))?;
        let valid = match handle.key.kind {
            ItemKind::Module => ast::ItemList::can_cast(root.kind()),
            ItemKind::Trait | ItemKind::Impl => ast::AssocItemList::can_cast(root.kind()),
            ItemKind::ExternBlock => ast::ExternItemList::can_cast(root.kind()),
            _ => false,
        };
        if !valid
            || range(root.text_range()) != handle.range
            || self.body_content_identity(&root)? != handle.key.content
        {
            return Err(error("container kind/range/content mismatch"));
        }
        let owner = BodyHandle {
            def_key: handle.key.definition,
            body_hash: handle.key.content,
            range: handle.range,
            entry_blocked: false,
        };
        self.lexical_facts(&root, &owner, cfg, containing, context)
    }
    fn lexical_facts(
        &self,
        root: &SyntaxNode,
        body: &BodyHandle,
        cfg: &CfgSet,
        containing: &super::outline::ScopeKey,
        header_context: &[super::ast_adapter::TokenRecipe],
    ) -> Result<EnteredMacroFacts, Diagnostic> {
        let mut facts = EnteredMacroFacts {
            bindings: vec![],
            items: vec![],
            imports: vec![],
            invocations: vec![],
            scopes: vec![BodyMacroScope {
                ordinal: 0,
                parent: None,
                range: body.range,
            }],
            events: vec![],
            opaque: vec![],
        };
        let mut binding_budget = super::local_bindings::BindingBudget::default();
        if let Some(function) = root.parent().and_then(ast::Fn::cast)
            && let Some(parameters) = function.param_list()
        {
            if let Some(receiver) = parameters.self_param()
                && super::local_bindings::cfg_enabled(receiver.attrs(), cfg)?
            {
                binding_budget.receiver(&receiver, body, &mut facts.bindings)?;
            }
            for parameter in parameters.params() {
                if !super::local_bindings::cfg_enabled(parameter.attrs(), cfg)? {
                    continue;
                }
                if let Some(pattern) = parameter.pat() {
                    binding_budget.collect(
                        &pattern,
                        body,
                        0,
                        u64::from(body.range.start),
                        super::local_bindings::BindingForm::Parameter,
                        &mut facts.bindings,
                    )?;
                }
            }
        }
        let mut stack =
            if ast::Expr::can_cast(root.kind()) && !ast::BlockExpr::can_cast(root.kind()) {
                // A root MacroExpr must visit its own cfg/attribute boundary before
                // the invocation; its token tree must remain opaque. Containers
                // and ordinary function blocks keep their existing child walk.
                vec![(root.clone(), 0, 0)]
            } else {
                children(root, 0, 0)?
            };
        stack.reverse();
        let mut producers = BTreeMap::<u32, u32>::new();
        let mut item_budget = super::body_items::BodyItemBudget::default();
        let mut import_budget = super::imports::ImportBudget::default();
        let mut import_scope = containing.clone();
        import_scope.owner = Some(body.def_key);
        let mut visited = 0usize;
        let mut condition_bytes = 0usize;
        while let Some((node, mut scope, mut depth)) = stack.pop() {
            visited = visited
                .checked_add(1)
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("entered macro syntax limit exceeded"))?;
            let conditional = if let Some(expression) = ast::IfExpr::cast(node.clone()) {
                expression.condition().zip(expression.then_branch())
            } else if let Some(expression) = ast::WhileExpr::cast(node.clone()) {
                expression.condition().zip(expression.loop_body())
            } else {
                None
            };
            let mut conditional_scope = None;
            if let Some((condition, consequent)) = conditional {
                let clauses = super::conditions::source_clauses(
                    condition.clone(),
                    Default::default(),
                    &mut condition_bytes,
                )?;
                if clauses
                    .iter()
                    .any(|clause| matches!(clause, ast::Expr::LetExpr(_)))
                {
                    let nested_depth = depth
                        .checked_add(1)
                        .filter(|depth| *depth <= 256)
                        .ok_or_else(|| error("body macro scope depth exceeded"))?;
                    let ordinal = u32::try_from(facts.scopes.len())
                        .map_err(|_| error("body macro scope ordinal overflow"))?;
                    let range = SourceRange::checked(
                        u32::from(condition.syntax().text_range().start()),
                        u32::from(consequent.syntax().text_range().end()),
                    )?;
                    facts
                        .scopes
                        .try_reserve(1)
                        .map_err(|_| error("body macro scope allocation failed"))?;
                    facts.scopes.push(BodyMacroScope {
                        ordinal,
                        parent: Some(scope),
                        range,
                    });
                    for clause in clauses {
                        let ast::Expr::LetExpr(clause) = clause else {
                            continue;
                        };
                        let pattern = clause
                            .pat()
                            .ok_or_else(|| error("condition pattern boundary missing"))?;
                        let visible_from = u64::from(u32::from(clause.syntax().text_range().end()));
                        binding_budget.collect(
                            &pattern,
                            body,
                            ordinal,
                            visible_from,
                            super::local_bindings::BindingForm::Condition,
                            &mut facts.bindings,
                        )?;
                        pattern_producers(
                            &mut facts,
                            &mut producers,
                            body,
                            ordinal,
                            &pattern,
                            visible_from,
                        )?;
                    }
                    conditional_scope = Some((range, ordinal, nested_depth));
                }
            }
            if let Some(statement) = ast::LetStmt::cast(node.clone()) {
                if !super::local_bindings::cfg_enabled(statement.attrs(), cfg)? {
                    continue;
                }
                if let Some(pattern) = statement.pat() {
                    binding_budget.collect(
                        &pattern,
                        body,
                        scope,
                        u64::from(u32::from(statement.syntax().text_range().end())),
                        super::local_bindings::BindingForm::Let,
                        &mut facts.bindings,
                    )?;
                }
            }
            if let Some(loop_expression) = ast::ForExpr::cast(node.clone()) {
                if !super::local_bindings::cfg_enabled(loop_expression.attrs(), cfg)? {
                    continue;
                }
                if loop_expression.iterable().is_none() || loop_expression.loop_body().is_none() {
                    return Err(error("for iterator/body boundary missing"));
                }
            }
            if let Some(arm) = ast::MatchArm::cast(node.clone()) {
                if !super::local_bindings::cfg_enabled(arm.attrs(), cfg)? {
                    continue;
                }
                let pattern = arm
                    .pat()
                    .ok_or_else(|| error("match pattern/body boundary missing"))?;
                if arm.expr().is_none() || arm.fat_arrow_token().is_none() {
                    return Err(error("match pattern/body boundary missing"));
                }
                depth = depth
                    .checked_add(1)
                    .filter(|n| *n <= 256)
                    .ok_or_else(|| error("body macro scope depth exceeded"))?;
                let ordinal = u32::try_from(facts.scopes.len())
                    .map_err(|_| error("body macro scope ordinal overflow"))?;
                facts
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| error("body macro scope allocation failed"))?;
                facts.scopes.push(BodyMacroScope {
                    ordinal,
                    parent: Some(scope),
                    range: range(node.text_range()),
                });
                scope = ordinal;
                let visible_from = u64::from(range(pattern.syntax().text_range()).end);
                binding_budget.collect(
                    &pattern,
                    body,
                    scope,
                    visible_from,
                    super::local_bindings::BindingForm::Match,
                    &mut facts.bindings,
                )?;
                pattern_producers(
                    &mut facts,
                    &mut producers,
                    body,
                    scope,
                    &pattern,
                    visible_from,
                )?;
            }
            if ast::ClosureExpr::can_cast(node.kind()) {
                opaque(&mut facts, range(node.text_range()))?;
                continue;
            }
            if let Some(expression) = ast::MacroExpr::cast(node.clone()) {
                let call = expression
                    .macro_call()
                    .ok_or_else(|| error("body macro expression call missing"))?;
                let item = ast::Item::MacroCall(call.clone());
                let (enabled, attrs) = super::outline::attributes_from_meta(
                    &item,
                    cfg,
                    expression
                        .syntax()
                        .children()
                        .filter_map(ast::Attr::cast)
                        .filter_map(|a| a.meta())
                        .collect(),
                )?;
                if !enabled {
                    continue;
                }
                let transform = attrs.iter().any(|a| {
                    matches!(
                        a.class,
                        AttributeClass::TransformUnsupported | AttributeClass::Derive
                    )
                });
                let input = if transform || item_or_statement_macro(call.syntax()) {
                    Some(add_producer(
                        &mut facts,
                        &mut producers,
                        body,
                        scope,
                        &node,
                        if transform {
                            ProducerKind::Attribute
                        } else {
                            ProducerKind::ItemMacro
                        },
                    )?)
                } else {
                    None
                };
                add_invocation(
                    &mut facts,
                    scope,
                    call.syntax(),
                    if transform { input } else { None },
                )?;
                continue;
            }
            if let Some(item) = ast::Item::cast(node.clone()) {
                if !matches!(item, ast::Item::MacroCall(_)) {
                    opaque(&mut facts, range(node.text_range()))?;
                }
                let (enabled, attrs) = attributes(&item, cfg)?;
                if !enabled {
                    continue;
                }
                let transform = attrs.iter().any(|a| {
                    matches!(
                        a.class,
                        AttributeClass::TransformUnsupported | AttributeClass::Derive
                    )
                });
                let mut attribute_input = None;
                if transform
                    || matches!(item, ast::Item::MacroCall(_)) && item_or_statement_macro(&node)
                {
                    let input = add_producer(
                        &mut facts,
                        &mut producers,
                        body,
                        scope,
                        &node,
                        if transform {
                            ProducerKind::Attribute
                        } else {
                            ProducerKind::ItemMacro
                        },
                    )?;
                    if transform {
                        attribute_input = Some(input);
                    }
                }
                if let Some(header) = item_budget.collect(
                    &item,
                    super::body_items::BodyItemContext {
                        parsed: self,
                        parent: body,
                        containing,
                        scope,
                        profile: &self.profile,
                        cfg,
                        header_context,
                    },
                    &attrs,
                    attribute_input,
                )? {
                    if !transform && let ast::Item::MacroRules(definition) = &item {
                        let name = definition
                            .name()
                            .ok_or_else(|| error("entered macro name boundary is missing"))?
                            .syntax()
                            .text()
                            .to_string();
                        super::ast_adapter::macro_name(&name, self.edition)
                            .ok_or_else(|| error("entered macro source name is invalid"))?;
                        facts
                            .events
                            .try_reserve(1)
                            .map_err(|_| error("body macro event allocation failed"))?;
                        facts.events.push(BodyMacroEvent {
                            scope,
                            at: u64::from(range(node.text_range()).start),
                            range: range(node.text_range()),
                            kind: BodyMacroEventKind::Definition {
                                identity: crate::core::digest::StableEncode::fingerprint(
                                    &header.item.def_key,
                                ),
                                name,
                                attributes: attrs,
                            },
                        });
                    }
                    if facts.items.len() >= 100_000 {
                        return Err(error("body item header count exceeded"));
                    }
                    facts
                        .items
                        .try_reserve(1)
                        .map_err(|_| error("body item header allocation failed"))?;
                    facts.items.push(header);
                }
                if let ast::Item::Use(import) = &item {
                    let mut e = StableEncoder::new("lrr/v2/body-source-import");
                    e.digest(body.def_key);
                    e.u32(scope);
                    e.u64(
                        u64::try_from(facts.imports.len())
                            .map_err(|_| error("body import occurrence overflow"))?,
                    );
                    e.sequence(&tokens(&node, None), |e, t| e.record(t));
                    let rows = super::imports::collect(
                        import,
                        &import_scope,
                        e.finish(),
                        &super::outline::visibility(&node),
                        &mut import_budget,
                    )?;
                    facts
                        .imports
                        .len()
                        .checked_add(rows.len())
                        .filter(|n| *n <= 100_000)
                        .ok_or_else(|| error("body import leaf limit exceeded"))?;
                    facts
                        .imports
                        .try_reserve(rows.len())
                        .map_err(|_| error("body import allocation failed"))?;
                    facts
                        .imports
                        .extend(rows.into_iter().map(|source| BodyImport {
                            scope,
                            at: u64::from(range(node.text_range()).start),
                            source,
                            attribute_input,
                        }));
                }
                if matches!(item, ast::Item::MacroCall(_)) {
                    add_invocation(&mut facts, scope, &node, attribute_input)?;
                }
                // Item bodies/token trees have their own explicit entry demand.
                continue;
            }
            if ast::MacroCall::can_cast(node.kind()) {
                if item_or_statement_macro(&node) {
                    add_producer(
                        &mut facts,
                        &mut producers,
                        body,
                        scope,
                        &node,
                        ProducerKind::ItemMacro,
                    )?;
                }
                add_invocation(&mut facts, scope, &node, None)?;
                continue;
            }
            if ast::BlockExpr::can_cast(node.kind()) {
                depth = depth
                    .checked_add(1)
                    .filter(|n| *n <= 256)
                    .ok_or_else(|| error("body macro scope depth exceeded"))?;
                let ordinal = u32::try_from(facts.scopes.len())
                    .map_err(|_| error("body macro scope ordinal overflow"))?;
                facts
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| error("body macro scope allocation failed"))?;
                facts.scopes.push(BodyMacroScope {
                    ordinal,
                    parent: Some(scope),
                    range: range(node.text_range()),
                });
                scope = ordinal;
                if let Some(loop_expression) = node.parent().and_then(ast::ForExpr::cast)
                    && loop_expression
                        .loop_body()
                        .is_some_and(|block| block.syntax() == &node)
                {
                    let pattern = loop_expression
                        .pat()
                        .ok_or_else(|| error("for pattern boundary missing"))?;
                    binding_budget.collect(
                        &pattern,
                        body,
                        scope,
                        u64::from(range(node.text_range()).start),
                        super::local_bindings::BindingForm::For,
                        &mut facts.bindings,
                    )?;
                    // Pattern macros can introduce names in the loop body, while their
                    // original invocation is resolved in the enclosing source scope.
                    pattern_producers(
                        &mut facts,
                        &mut producers,
                        body,
                        scope,
                        &pattern,
                        u64::from(range(node.text_range()).start),
                    )?;
                }
            }
            let mut children = children(&node, scope, depth)?;
            if let Some((condition_range, condition_scope, condition_depth)) = conditional_scope {
                for (child, scope, depth) in &mut children {
                    if condition_range.contains(range(child.text_range())) {
                        *scope = condition_scope;
                        *depth = condition_depth;
                    }
                }
            }
            stack
                .len()
                .checked_add(children.len())
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("body macro traversal frontier limit exceeded"))?;
            stack
                .try_reserve(children.len())
                .map_err(|_| error("body macro traversal allocation failed"))?;
            children.reverse();
            stack.extend(children);
        }
        Ok(facts)
    }
}
fn pattern_producers(
    facts: &mut EnteredMacroFacts,
    producers: &mut BTreeMap<u32, u32>,
    body: &BodyHandle,
    scope: u32,
    pattern: &ast::Pat,
    at: u64,
) -> Result<(), Diagnostic> {
    let mut pattern_nodes = vec![pattern.syntax().clone()];
    let mut visited_patterns = 0usize;
    while let Some(pattern_node) = pattern_nodes.pop() {
        visited_patterns = visited_patterns
            .checked_add(1)
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("binding pattern producer scan limit exceeded"))?;
        if ast::Expr::can_cast(pattern_node.kind())
            || ast::Item::can_cast(pattern_node.kind())
            || ast::TokenTree::can_cast(pattern_node.kind())
        {
            continue;
        }
        if let Some(macro_pattern) = ast::MacroPat::cast(pattern_node.clone()) {
            let call = macro_pattern
                .macro_call()
                .ok_or_else(|| error("binding pattern macro boundary missing"))?;
            add_producer(
                facts,
                producers,
                body,
                scope,
                call.syntax(),
                ProducerKind::ItemMacro,
            )?;
            facts
                .events
                .last_mut()
                .expect("admitted pattern producer")
                .at = at;
            continue;
        }
        let rows = children(&pattern_node, scope, 0)?;
        pattern_nodes
            .len()
            .checked_add(rows.len())
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("binding pattern producer frontier exceeded"))?;
        pattern_nodes
            .try_reserve(rows.len())
            .map_err(|_| error("binding pattern producer allocation failed"))?;
        pattern_nodes.extend(rows.into_iter().rev().map(|(node, _, _)| node));
    }
    Ok(())
}

fn add_producer(
    facts: &mut EnteredMacroFacts,
    ordinals: &mut BTreeMap<u32, u32>,
    body: &BodyHandle,
    scope: u32,
    node: &SyntaxNode,
    kind: ProducerKind,
) -> Result<Digest, Diagnostic> {
    let ordinal = ordinals.entry(scope).or_default();
    let mut e = StableEncoder::new("lrr/v2/body-macro-producer");
    e.digest(body.def_key);
    e.u32(scope);
    e.u32(*ordinal);
    e.sequence(&tokens(node, None), |e, t| e.record(t));
    *ordinal = ordinal
        .checked_add(1)
        .ok_or_else(|| error("body producer ordinal overflow"))?;
    let identity = e.finish();
    facts
        .events
        .try_reserve(1)
        .map_err(|_| error("body macro event allocation failed"))?;
    facts.events.push(BodyMacroEvent {
        scope,
        at: u64::from(range(node.text_range()).start),
        range: range(node.text_range()),
        kind: BodyMacroEventKind::Producer { identity, kind },
    });
    Ok(identity)
}

// Item-container calls and statement calls can introduce later declarations.
// Initializer expression calls do not create this lexical publication frontier.
fn item_or_statement_macro(node: &SyntaxNode) -> bool {
    node.parent().is_some_and(|parent| {
        ast::StmtList::can_cast(parent.kind())
            || ast::ItemList::can_cast(parent.kind())
            || ast::AssocItemList::can_cast(parent.kind())
            || ast::ExternItemList::can_cast(parent.kind())
            || parent
                .parent()
                .is_some_and(|parent| ast::ExprStmt::can_cast(parent.kind()))
    })
}

fn children(
    node: &SyntaxNode,
    scope: u32,
    depth: usize,
) -> Result<Vec<(SyntaxNode, u32, usize)>, Diagnostic> {
    let mut children = Vec::new();
    for node in node.children() {
        if children.len() >= 1_000_000 {
            return Err(error("body macro child limit exceeded"));
        }
        children
            .try_reserve(1)
            .map_err(|_| error("body macro child allocation failed"))?;
        children.push((node, scope, depth));
    }
    Ok(children)
}

fn opaque(facts: &mut EnteredMacroFacts, range: SourceRange) -> Result<(), Diagnostic> {
    facts
        .opaque
        .try_reserve(1)
        .map_err(|_| error("opaque body range allocation failed"))?;
    facts.opaque.push(range);
    Ok(())
}

fn add_invocation(
    facts: &mut EnteredMacroFacts,
    scope: u32,
    node: &SyntaxNode,
    attribute_input: Option<Digest>,
) -> Result<(), Diagnostic> {
    if facts.invocations.len() >= 100_000 {
        return Err(error("entered macro invocation count exceeded"));
    }
    facts
        .invocations
        .try_reserve(1)
        .map_err(|_| error("entered macro invocation allocation failed"))?;
    facts.invocations.push(BodyMacroInvocation {
        scope,
        range: range(node.text_range()),
        attribute_input,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::SourceBlob,
    };
    #[test]
    fn rustc_static_macro_initializer_retains_only_its_actual_outer_invocation() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/nested-macro-expansion.rs"
        );
        let cfg = CfgSet::guest_default();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed.outline(Digest([149; 32]), vec![], &cfg).unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "FOO")
            .unwrap();
        let body = item.body.as_ref().unwrap();
        let facts = parsed.entered_macro_facts(body, &cfg, &item.scope).unwrap();
        assert_eq!(facts.scopes.len(), 1);
        assert_eq!(facts.scopes[0].range, body.range);
        assert_eq!(facts.scopes[0].parent, None);
        assert_eq!(facts.invocations.len(), 1);
        assert_eq!(facts.invocations[0].range, body.range);
        assert_eq!(facts.invocations[0].scope, 0);
        assert!(facts.invocations[0].attribute_input.is_none());
        assert!(facts.events.is_empty());
        assert!(facts.bindings.is_empty());
        assert!(facts.items.is_empty());
        assert!(facts.imports.is_empty());
        // Root expression invocation is retained; the nested concat token tree
        // remains opaque until the explicit compiler macro argument demand.
        // No concat output, static value, main body or upstream run-pass ran.
    }
}

impl EnteredMacroFacts {
    /// Owned lexical plan storage, including allocator capacity slack. Shared
    /// parser/index handles are accounted separately by their retaining owner.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        fn add(bytes: &mut usize, value: usize) -> Result<(), Diagnostic> {
            *bytes = bytes
                .checked_add(value)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("entered lexical plan metadata limit"))?;
            Ok(())
        }
        fn slots<T>(values: &Vec<T>) -> Result<usize, Diagnostic> {
            values
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| error("entered lexical plan capacity overflow"))
        }
        fn tokens(
            bytes: &mut usize,
            values: &Vec<super::ast_adapter::TokenRecipe>,
        ) -> Result<(), Diagnostic> {
            add(bytes, slots(values)?)?;
            for token in values {
                add(bytes, token.text.capacity())?;
            }
            Ok(())
        }
        fn visibility(
            bytes: &mut usize,
            value: &super::outline::Visibility,
        ) -> Result<(), Diagnostic> {
            if let super::outline::Visibility::Restricted(values) = value {
                tokens(bytes, values)?;
            }
            Ok(())
        }
        fn attributes(bytes: &mut usize, values: &Vec<AttributeRecipe>) -> Result<(), Diagnostic> {
            add(bytes, slots(values)?)?;
            for value in values {
                add(bytes, value.name.capacity())?;
                tokens(bytes, &value.tokens)?;
            }
            Ok(())
        }
        let mut bytes = std::mem::size_of::<Self>();
        add(&mut bytes, slots(&self.bindings)?)?;
        add(&mut bytes, slots(&self.items)?)?;
        add(&mut bytes, slots(&self.imports)?)?;
        add(&mut bytes, slots(&self.invocations)?)?;
        add(&mut bytes, slots(&self.scopes)?)?;
        add(&mut bytes, slots(&self.events)?)?;
        add(&mut bytes, slots(&self.opaque)?)?;
        for binding in &self.bindings {
            add(&mut bytes, binding.name.capacity())?;
            add(&mut bytes, binding.raw_name.capacity())?;
        }
        for row in &self.items {
            add(&mut bytes, slots(&row.namespaces)?)?;
            let item = &row.item;
            add(&mut bytes, item.scope.heap_bytes())?;
            add(&mut bytes, item.header.profile.semantics.capacity())?;
            tokens(&mut bytes, &item.header.tokens)?;
            tokens(&mut bytes, &item.header.context)?;
            attributes(&mut bytes, &item.header.attributes)?;
            visibility(&mut bytes, &item.header.visibility)?;
            add(&mut bytes, item.def_key.declared_name.capacity())?;
            add(&mut bytes, slots(&item.def_key.module_path)?)?;
            for part in &item.def_key.module_path {
                add(&mut bytes, part.capacity())?;
            }
            add(&mut bytes, slots(&item.constructor_fields)?)?;
            for field in &item.constructor_fields {
                visibility(&mut bytes, field)?;
            }
        }
        for import in &self.imports {
            add(&mut bytes, super::imports::retained_bytes(&import.source)?)?;
        }
        for event in &self.events {
            if let BodyMacroEventKind::Definition {
                name,
                attributes: values,
                ..
            } = &event.kind
            {
                add(&mut bytes, name.capacity())?;
                attributes(&mut bytes, values)?;
            }
        }
        Ok(bytes)
    }
}
