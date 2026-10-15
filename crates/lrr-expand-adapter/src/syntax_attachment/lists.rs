//! Grammar-list attachment. No name lookup, expression evaluation or guest work.
use super::*;
use syntax::{SyntaxKind, SyntaxToken};

#[derive(Clone, Copy, Debug)]
pub enum ListSlot {
    Statements,
    Items,
    TraitItems,
    ImplItems,
}
impl ListSlot {
    fn grammar(self) -> DeclarationGrammar {
        match self {
            Self::Statements => DeclarationGrammar::Statements,
            Self::Items => DeclarationGrammar::Items,
            Self::TraitItems => DeclarationGrammar::TraitItems,
            Self::ImplItems => DeclarationGrammar::ImplItems,
        }
    }
    fn accepts_container(self, container: &SyntaxNode, grammar: DeclarationGrammar) -> bool {
        match self {
            Self::Statements => {
                ast::StmtList::can_cast(container.kind())
                    || (container.kind() == SyntaxKind::MACRO_STMTS
                        && matches!(grammar, DeclarationGrammar::Statements))
            }
            Self::Items => {
                ast::ItemList::can_cast(container.kind())
                    || ast::SourceFile::can_cast(container.kind())
                    || (container.kind() == SyntaxKind::MACRO_ITEMS
                        && matches!(grammar, DeclarationGrammar::Items))
            }
            Self::TraitItems | Self::ImplItems => {
                if ast::AssocItemList::can_cast(container.kind()) {
                    container.parent().is_some_and(|parent| match self {
                        Self::TraitItems => ast::Trait::can_cast(parent.kind()),
                        Self::ImplItems => ast::Impl::can_cast(parent.kind()),
                        _ => false,
                    })
                } else {
                    container.kind() == SyntaxKind::MACRO_ITEMS
                        && matches!(
                            (self, grammar),
                            (Self::TraitItems, DeclarationGrammar::TraitItems)
                                | (Self::ImplItems, DeclarationGrammar::ImplItems)
                        )
                }
            }
        }
    }
}

struct ListTarget {
    node: SyntaxNode,
    container: SyntaxNode,
    // An actual caller token, never a fabricated source span.
    semicolon: Option<SyntaxToken>,
    statement: bool,
}

impl SyntaxAttachment {
    fn select_list(&self, call: SyntaxNode, slot: ListSlot) -> Result<ListTarget, AdapterError> {
        let mut node = call.clone();
        let mut semicolon = None;
        let mut statement = false;
        if matches!(slot, ListSlot::Statements) {
            let expression = call
                .parent()
                .filter(|n| {
                    ast::MacroExpr::can_cast(n.kind()) && n.text_range() == call.text_range()
                })
                .ok_or_else(|| error("statement attachment does not select a statement macro"))?;
            let parent = expression
                .parent()
                .ok_or_else(|| error("statement attachment has no caller list"))?;
            if let Some(stmt) = ast::ExprStmt::cast(parent.clone()) {
                if parent.text_range().start() != call.text_range().start() {
                    return Err(error(
                        "statement attachment cannot discard caller attributes",
                    ));
                }
                semicolon = stmt.semicolon_token();
                statement = true;
                node = parent;
            } else {
                // A parenthesized invocation at the end of an ordinary block
                // has expression grammar. Braced statement invocations and an
                // already selected statement-fragment root retain list grammar.
                let braced = ast::MacroCall::cast(call)
                    .and_then(|call| call.token_tree())
                    .is_some_and(|tt| tt.l_curly_token().is_some());
                if !braced && parent.kind() != SyntaxKind::MACRO_STMTS {
                    return Err(error(
                        "tail expression macro requires expression attachment",
                    ));
                }
                node = expression;
            }
        } else if call.children().any(|n| ast::Attr::can_cast(n.kind())) {
            return Err(error(
                "item attachment requires prior caller attribute admission",
            ));
        }
        let container = node
            .parent()
            .filter(|container| slot.accepts_container(container, self.grammar))
            .ok_or_else(|| error("list attachment grammar differs from the caller container"))?;
        Ok(ListTarget {
            node,
            container,
            semicolon,
            statement,
        })
    }

    /// Replace one invocation with zero or more pinned statements/items in its
    /// existing list. Retain the syntax forest between recursive expansions;
    /// reparsing flattened display text would lose prior fragment boundaries.
    pub fn attach_list(
        &mut self,
        start: u32,
        end: u32,
        output: &TokenForest,
        slot: ListSlot,
    ) -> Result<(), AdapterError> {
        let call = self.select_call(start, end)?;
        let target = self.select_list(call, slot)?;
        let mut prepared = self.prepare(output, slot.grammar())?;
        if matches!(slot, ListSlot::Statements) {
            self.statement_suffix(&mut prepared, &target)?;
        }
        let range = target.node.text_range();
        self.apply(range, prepared, |replacement| {
            let index = target
                .container
                .children_with_tokens()
                .position(|element| element.as_node() == Some(&target.node))
                .ok_or_else(|| error("list attachment target lost its parent position"))?;
            // Flatten only the parser's list container, never an actual block.
            let children = replacement.green().children().map(|child| child.to_owned());
            let green = target
                .container
                .green()
                .splice_children(index..=index, children);
            Ok(SyntaxNode::new_root(target.container.replace_with(green)))
        })
    }

    fn statement_suffix(
        &self,
        prepared: &mut PreparedFragment,
        target: &ListTarget,
    ) -> Result<(), AdapterError> {
        let Some(last) = prepared.root.children().last() else {
            // An empty expansion removes the original invocation, leaving
            // preceding ExprStmt nodes intact rather than inventing a tail.
            return Ok(());
        };
        let is_expression = ast::Expr::can_cast(last.kind());
        let stmt = ast::ExprStmt::cast(last.clone());
        let append = target.semicolon.as_ref().filter(|_| {
            is_expression || stmt.as_ref().is_some_and(|s| s.semicolon_token().is_none())
        });
        if !is_expression && append.is_none() {
            return Ok(());
        }
        if is_expression && !target.statement {
            return Ok(());
        }
        if let Some(semi) = append {
            let old_len = u32::from(prepared.root.text_range().len());
            let final_len = old_len
                .checked_add(u32::from(semi.text_range().len()))
                .filter(|n| *n <= MAX_TEXT)
                .ok_or_else(|| error("statement semicolon text budget"))?;
            let mut origin = SpanMap::empty();
            origin.push(
                semi.text_range().len(),
                self.origins.span_at(semi.text_range().start()),
            );
            prepared.origins = splice_origins(
                &prepared.origins,
                TextRange::empty(last.text_range().end()),
                &origin,
                u32::from(semi.text_range().len()),
                final_len,
            )?;
        }
        let replacement = if is_expression {
            // Reuse the actual caller ExprStmt node kind. The wrapper records
            // statement position and a carried semicolon; it adds no scope.
            let mut children = vec![last.green().to_owned().into()];
            if let Some(semi) = append {
                children.push(semi.green().to_owned().into());
            }
            target.node.green().splice_children(.., children)
        } else {
            let index = last.green().children().count();
            let semi = append.ok_or_else(|| error("statement semicolon absent"))?;
            last.green()
                .insert_child(index, semi.green().to_owned().into())
        };
        prepared.root = replace_node(&last, &SyntaxNode::new_root(replacement))?;
        Ok(())
    }
}
