//! Cold exact token occurrences for the existing unqualified import callback.
//! This joins retained AST locations to source facts; it resolves no namespace.
use super::{AdmittedBodyContext, fault};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::MacroName,
    syntax::{
        ast_adapter::range,
        entered_macros::{BodyImport, EnteredMacroFacts},
        imports::ImportSegment,
        spans::SourceRange,
        token_wire::Key,
    },
};
use ra_ap_syntax::{AstNode, ast};

type ImportKey = (u32, u64, Digest, u32, u32, u32);
fn key(row: &BodyImport) -> ImportKey {
    (
        row.scope,
        row.at,
        row.source.identity,
        row.source.leaf_ordinal,
        row.source.range.start,
        row.source.range.end,
    )
}
pub(crate) struct MacroImportOccurrence {
    key: ImportKey,
    owner: Digest,
    name: String,
    range: SourceRange,
    context: Option<Key>,
    edition: u16,
}
impl MacroImportOccurrence {
    pub(crate) fn context(
        &self,
        source: &AdmittedBodyContext,
        original: Key,
    ) -> Result<Key, Diagnostic> {
        if source.identity() != self.owner || !source.body().range.contains(self.range) {
            return Err(fault("macro import occurrence owning source differs"));
        }
        match self.context {
            Some(context) => Ok(context),
            None if source
                .original_source()
                .is_some_and(|view| view.selection().outline.parse == source.parsed().key()) =>
            {
                Ok(original)
            }
            None => Err(fault(
                "macro import occurrence lacks its actual token context",
            )),
        }
    }
    pub(crate) fn edition(&self) -> u16 {
        self.edition
    }
}
pub(super) struct PreparedMacroImportOccurrences {
    rows: Vec<MacroImportOccurrence>,
    extra_bytes: usize,
}
impl PreparedMacroImportOccurrences {
    pub(super) fn prepare(
        source: &AdmittedBodyContext,
        facts: &EnteredMacroFacts,
    ) -> Result<Self, Diagnostic> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(facts.imports.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for row in &facts.imports {
            let mut target = row.source.path.as_slice();
            if target.len() > 1 && target.last() == Some(&ImportSegment::SelfModule) {
                target = &target[..target.len() - 1];
            }
            // This is exactly the existing body_imports textual callback case.
            // Other paths still use its single ordinary module/import algorithm.
            let [ImportSegment::Name(name)] = target else {
                continue;
            };
            if row.source.absolute {
                continue;
            }
            if !source.body().range.contains(row.source.range) {
                return Err(fault("macro import occurrence escapes admitted body"));
            }
            let syntax = source
                .parsed()
                .syntax_at(row.source.range, |node| ast::UseTree::can_cast(node.kind()))?
                .ok_or_else(|| fault("macro import leaf syntax boundary absent"))?;
            let leaf = ast::UseTree::cast(syntax)
                .ok_or_else(|| fault("macro import leaf syntax differs"))?;
            let mut selected = None;
            let mut depth = 0usize;
            // Use-tree ancestors retain the actual prefix occurrence for
            // `use foo::{self as alias}`. Rename tokens never supply context.
            for node in leaf.syntax().ancestors() {
                depth = depth
                    .checked_add(1)                    .ok_or_else(crate::runtime::arena::resource)?;
                if ast::Use::can_cast(node.kind()) {
                    break;
                }
                let Some(tree) = ast::UseTree::cast(node) else {
                    continue;
                };
                let Some(mut path) = tree.path() else {
                    continue;
                };
                loop {
                    depth = depth
                        .checked_add(1)                        .ok_or_else(crate::runtime::arena::resource)?;
                    let segment = path
                        .segment()
                        .ok_or_else(|| fault("macro import path segment absent"))?;
                    if let Some(ast::PathSegmentKind::Name(reference)) = segment.kind() {
                        let actual = range(reference.syntax().text_range());
                        let edition = source.parsed().token_edition(actual)?;
                        if selected.is_some()
                            || MacroName::parse(reference.text(), edition)?
                                != MacroName::parse(name, edition)?
                        {
                            return Err(fault(
                                "macro import target occurrence is ambiguous or differs",
                            ));
                        }
                        selected = Some((actual, source.identifier_context(actual)?, edition));
                    }
                    match path.qualifier() {
                        Some(parent) => path = parent,
                        None => break,
                    }
                }
            }
            let (range, context, edition) =
                selected.ok_or_else(|| fault("macro import target occurrence absent"))?;
            if context.is_none()
                && !source
                    .original_source()
                    .is_some_and(|view| view.selection().outline.parse == source.parsed().key())
            {
                return Err(fault(
                    "macro import target has no admitted original or generated context",
                ));
            }
            rows.push(MacroImportOccurrence {
                key: key(row),
                owner: source.identity(),
                name: name.clone(),
                range,
                context,
                edition,
            });
        }
        rows.sort_unstable_by_key(|row| row.key);
        if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(fault("macro import occurrence identity is ambiguous"));
        }
        let extra_bytes = rows.iter().try_fold(
            rows.capacity()
                .checked_mul(std::mem::size_of::<MacroImportOccurrence>())
                .ok_or_else(crate::runtime::arena::resource)?,
            |bytes, row| {
                bytes
                    .checked_add(row.name.capacity())
                    .ok_or_else(crate::runtime::arena::resource)
            },
        )?;
        Ok(Self { rows, extra_bytes })
    }
    pub(super) fn extra_metadata_bytes(&self) -> usize {
        self.extra_bytes
    }
    pub(super) fn select(
        &self,
        row: &BodyImport,
        name: &str,
    ) -> Result<&MacroImportOccurrence, Diagnostic> {
        let selected = self
            .rows
            .binary_search_by_key(&key(row), |row| row.key)
            .ok()
            .map(|index| &self.rows[index])
            .ok_or_else(|| fault("actual macro import occurrence proof absent"))?;
        if selected.name != name {
            return Err(fault("actual macro import occurrence target differs"));
        }
        Ok(selected)
    }
}
