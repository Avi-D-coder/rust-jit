//! Owned import syntax from the pinned AST. No namespace resolution or effects.
use super::{
    ast_adapter::range,
    outline::{ScopeKey, Visibility},
    spans::SourceRange,
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
#[derive(Debug, Clone)]
pub enum MacroUseRecipe {
    All,
    Only(Vec<String>),
}
impl StableEncode for MacroUseRecipe {
    const DOMAIN: &'static str = "lrr/v2/extern-macro-use";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::All => e.u32(0),
            Self::Only(names) => {
                e.u32(1);
                e.sequence(names, |e, name| e.string(name));
            }
        }
    }
}
impl MacroUseRecipe {
    pub(crate) fn bytes(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::All => Ok(0),
            Self::Only(names) => names
                .iter()
                .try_fold(
                    names
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())
                        .ok_or_else(|| error("macro-use selector capacity overflow"))?,
                    |n, name| n.checked_add(name.capacity()),
                )
                .ok_or_else(|| error("macro-use selector metadata overflow")),
        }
    }
}
pub(crate) fn macro_use_recipe(
    attributes: &[super::outline::AttributeRecipe],
) -> Result<Option<MacroUseRecipe>, Diagnostic> {
    let mut result = None;
    for attribute in attributes
        .iter()
        .filter(|a| a.class == super::outline::AttributeClass::MacroUse)
    {
        if result.is_some() {
            return Err(error("duplicate extern macro-use attribute"));
        }
        let tokens = &attribute.tokens;
        if tokens.len() == 1 {
            result = Some(MacroUseRecipe::All);
            continue;
        }
        if tokens.len() < 3 || tokens[1].text != "(" || tokens.last().is_none_or(|t| t.text != ")")
        {
            return Err(error("invalid extern macro-use selector list"));
        }
        let mut names = Vec::new();
        let mut expect_name = true;
        for token in &tokens[2..tokens.len() - 1] {
            if expect_name {
                if token.kind != ra_ap_syntax::SyntaxKind::IDENT as u16 {
                    return Err(error("macro-use selector must be an identifier"));
                }
                names
                    .try_reserve(1)
                    .map_err(|_| error("macro-use selector allocation failed"))?;
                names.push(token.text.clone());
            } else if token.text != "," {
                return Err(error("macro-use selector separator is invalid"));
            }
            expect_name = !expect_name;
        }
        result = Some(MacroUseRecipe::Only(names));
    }
    Ok(result)
}
/// Captured explicit dependency declaration; resolution uses preparation rows only.
#[derive(Debug, Clone)]
pub struct SourceExternCrate {
    pub identity: Digest,
    pub scope: ScopeKey,
    pub dependency: String,
    pub alias: Option<String>,
    pub visibility: Visibility,
    pub macro_use: Option<MacroUseRecipe>,
    pub macro_use_bytes: usize,
    pub order: Vec<u32>,
}
impl StableEncode for SourceExternCrate {
    const DOMAIN: &'static str = "lrr/v2/source-extern-crate";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.scope);
        e.string(&self.dependency);
        e.option(&self.alias, |e, name| e.string(name));
        e.record(&self.visibility);
        e.option(&self.macro_use, |e, recipe| e.record(recipe));
        e.sequence(&self.order, |e, position| e.u32(*position));
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ImportSegment {
    Name(String),
    Crate,
    SelfModule,
    Super,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ImportKind {
    Named { alias: Option<String> },
    Anonymous,
    Glob,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceImport {
    pub identity: Digest,
    pub leaf_ordinal: u32,
    pub scope: ScopeKey,
    pub range: SourceRange,
    pub path: Vec<ImportSegment>,
    pub absolute: bool,
    pub kind: ImportKind,
    pub visibility: Visibility,
}
impl StableEncode for SourceImport {
    const DOMAIN: &'static str = "lrr/v2/source-import";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.u32(self.leaf_ordinal);
        e.record(&self.scope);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.bool(self.absolute);
        e.sequence(&self.path, |e, part| match part {
            ImportSegment::Name(name) => {
                e.u32(0);
                e.string(name);
            }
            ImportSegment::Crate => e.u32(1),
            ImportSegment::SelfModule => e.u32(2),
            ImportSegment::Super => e.u32(3),
        });
        match &self.kind {
            ImportKind::Named { alias } => {
                e.u32(0);
                e.option(alias, |e, n| e.string(n));
            }
            ImportKind::Anonymous => e.u32(1),
            ImportKind::Glob => e.u32(2),
        }
        e.record(&self.visibility);
    }
}
#[derive(Default)]
pub(crate) struct ImportBudget {
    bytes: usize,
    leaves: usize,
}
impl ImportBudget {
    pub(crate) fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| error("source import metadata budget exceeded"))?;
        Ok(())
    }
}
fn path_bytes(path: &[ImportSegment]) -> usize {
    path.iter()
        .map(|p| match p {
            ImportSegment::Name(n) => n.len(),
            _ => 0,
        })
        .sum()
}
pub(crate) fn visibility_bytes(v: &Visibility) -> usize {
    match v {
        Visibility::Restricted(tokens) => {
            tokens.len() * std::mem::size_of::<super::ast_adapter::TokenRecipe>()
                + tokens.iter().map(|t| t.text.len()).sum::<usize>()
        }
        _ => 0,
    }
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
pub(crate) fn collect(
    item: &ast::Use,
    scope: &ScopeKey,
    identity: Digest,
    visibility: &Visibility,
    budget: &mut ImportBudget,
) -> Result<Vec<SourceImport>, Diagnostic> {
    let tree = item
        .use_tree()
        .ok_or_else(|| error("source import tree is missing"))?;
    let mut stack = vec![(tree, Vec::new(), false, 0usize)];
    let mut output = Vec::new();
    let mut visited = 0usize;
    while let Some((tree, mut prefix, mut absolute, depth)) = stack.pop() {
        visited = visited
            .checked_add(1)
            .ok_or_else(|| error("source import tree limit exceeded"))?;
        if let Some(mut path) = tree.path() {
            let mut suffix = Vec::new();
            loop {
                let segment = path
                    .segment()
                    .and_then(|s| s.kind())
                    .ok_or_else(|| error("source import segment is missing"))?;
                budget.charge(std::mem::size_of::<ImportSegment>())?;
                let owned = match segment {
                    ast::PathSegmentKind::Name(name) => {
                        budget.charge(usize::from(name.syntax().text_range().len()))?;
                        ImportSegment::Name(name.syntax().text().to_string())
                    }
                    ast::PathSegmentKind::CrateKw => ImportSegment::Crate,
                    ast::PathSegmentKind::SelfKw => ImportSegment::SelfModule,
                    ast::PathSegmentKind::SuperKw => ImportSegment::Super,
                    _ => return Err(error("source import path has a non-module segment")),
                };
                suffix
                    .try_reserve(1)
                    .map_err(|_| error("import path allocation failed"))?;
                suffix.push(owned);
                if let Some(parent) = path.qualifier() {
                    path = parent;
                } else {
                    absolute |= path
                        .segment()
                        .is_some_and(|segment| segment.coloncolon_token().is_some());
                    break;
                }
            }
            prefix
                .len()
                .checked_add(suffix.len())                .ok_or_else(|| error("source import combined path limit exceeded"))?;
            prefix
                .try_reserve(suffix.len())
                .map_err(|_| error("combined import path allocation failed"))?;
            prefix.extend(suffix.into_iter().rev());
        }
        if let Some(list) = tree.use_tree_list() {
            let mut children = Vec::new();
            for child in list.use_trees() {
                children
                    .try_reserve(1)
                    .map_err(|_| error("import child allocation failed"))?;
                children.push(child);
            }
            stack
                .len()
                .checked_add(children.len())
                .ok_or_else(|| error("source import frontier limit exceeded"))?;
            stack
                .try_reserve(children.len())
                .map_err(|_| error("import frontier allocation failed"))?;
            budget.charge(
                children
                    .len()
                    .checked_mul(
                        path_bytes(&prefix) + prefix.len() * std::mem::size_of::<ImportSegment>(),
                    )
                    .ok_or_else(|| error("source import size overflow"))?,
            )?;
            for child in children.into_iter().rev() {
                stack.push((child, prefix.clone(), absolute, depth + 1));
            }
            continue;
        }
        budget.charge(usize::from(tree.syntax().text_range().len()))?;
        let kind = if tree.star_token().is_some() {
            ImportKind::Glob
        } else if let Some(rename) = tree.rename() {
            if rename.underscore_token().is_some() {
                ImportKind::Anonymous
            } else {
                ImportKind::Named {
                    alias: Some(
                        rename
                            .name()
                            .ok_or_else(|| error("import rename is missing"))?
                            .syntax()
                            .text()
                            .to_string(),
                    ),
                }
            }
        } else {
            ImportKind::Named { alias: None }
        };
        if prefix.is_empty() {
            return Err(error("source import leaf has no path"));
        }
        budget.leaves = budget
            .leaves
            .checked_add(1)
            .ok_or_else(|| error("source import leaf limit exceeded"))?;
        budget.charge(
            std::mem::size_of::<SourceImport>() + scope.heap_bytes() + visibility_bytes(visibility),
        )?;
        let leaf_ordinal =
            u32::try_from(output.len()).map_err(|_| error("source import ordinal overflow"))?;
        output
            .try_reserve(1)
            .map_err(|_| error("source import leaf allocation failed"))?;
        output.push(SourceImport {
            identity,
            leaf_ordinal,
            scope: scope.clone(),
            range: range(tree.syntax().text_range()),
            path: prefix,
            absolute,
            kind,
            visibility: visibility.clone(),
        });
    }
    Ok(output)
}

/// Syntactic name demand only. These temporary rows are never published as
/// declaration identities or used to resolve the imported target.
pub(crate) fn may_bind_name(
    item: &ast::Use,
    wanted: &crate::expand::scopes::MacroName,
    edition: u16,
    budget: &mut ImportBudget,
) -> Result<bool, Diagnostic> {
    let rows = collect(
        item,
        &ScopeKey {
            crate_key: Digest([0; 32]),
            module_path: vec![],
            owner: None,
        },
        Digest([0; 32]),
        &Visibility::Private,
        budget,
    )?;
    for row in rows {
        let name = match row.kind {
            ImportKind::Glob => return Ok(true),
            ImportKind::Anonymous => continue,
            ImportKind::Named { alias: Some(alias) } => alias,
            ImportKind::Named { alias: None } => match row.path.last() {
                Some(ImportSegment::Name(name)) => name.clone(),
                Some(ImportSegment::SelfModule) => {
                    let Some(ImportSegment::Name(name)) = row.path.iter().rev().nth(1) else {
                        // An unresolved root binding cannot prove absence.
                        return Ok(true);
                    };
                    name.clone()
                }
                _ => return Ok(true),
            },
        };
        if &crate::expand::scopes::MacroName::parse(&name, edition)? == wanted {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Checked retained storage for an owned use-tree leaf, including capacities.
pub(crate) fn retained_bytes(row: &SourceImport) -> Result<usize, Diagnostic> {
    let mut bytes = std::mem::size_of::<SourceImport>()
        .checked_add(row.scope.heap_bytes())
        .and_then(|n| {
            n.checked_add(
                row.path
                    .capacity()
                    .checked_mul(std::mem::size_of::<ImportSegment>())?,
            )
        })
        .ok_or_else(|| error("import retained storage overflow"))?;
    for part in &row.path {
        if let ImportSegment::Name(name) = part {
            bytes = bytes
                .checked_add(name.capacity())
                .ok_or_else(|| error("import path storage overflow"))?;
        }
    }
    if let ImportKind::Named { alias: Some(alias) } = &row.kind {
        bytes = bytes
            .checked_add(alias.capacity())
            .ok_or_else(|| error("import alias storage overflow"))?;
    }
    if let Visibility::Restricted(tokens) = &row.visibility {
        bytes = bytes
            .checked_add(
                tokens
                    .capacity()
                    .checked_mul(std::mem::size_of::<super::ast_adapter::TokenRecipe>())
                    .ok_or_else(|| error("import visibility storage overflow"))?,
            )
            .ok_or_else(|| error("import visibility storage overflow"))?;
        for token in tokens {
            bytes = bytes
                .checked_add(token.text.capacity())
                .ok_or_else(|| error("import visibility text overflow"))?;
        }
    }
    Ok(bytes)
}
