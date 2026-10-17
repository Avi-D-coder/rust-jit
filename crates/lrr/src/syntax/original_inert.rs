//! Original source-prefix AST adapter. Only checked producer identities leave
//! the syntax boundary; source/scan/edition/cfg and ownership remain exact.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    source::changes::PreparedSourceView,
    syntax::{
        declaration_scan::{DeclarationProducerKind, DeclarationScan, DeclarationScopeKind},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast::HasAttrs};
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
pub(crate) fn capture(
    source: &PreparedSourceView,
    scan: &DeclarationScan,
    byte_limit: usize,
) -> Result<BTreeSet<Digest>, Diagnostic> {
    let _ = byte_limit; // E-R5-006: no metadata byte budget.
    source.require_original_hygiene()?;
    source.require_declaration_syntax()?;
    let parsed = source.retained_declaration_parser()?;
    if scan.parse() != &source.selection().outline.parse
        || scan.path() != &source.selection().path
        || scan.cfg() != source.selection().outline.cfg
        || scan.scope() != &source.outline().root_scope
        || scan.generated_origin() != parsed.generated_origin
    {
        return Err(fault(
            "original prefix source/scan/profile/cfg/owning scope differs",
        ));
    }
    let root = scan
        .lexical_scopes()
        .first()
        .ok_or_else(|| fault("original prefix lexical root absent"))?;
    if root.parent.is_some() || root.kind != DeclarationScopeKind::Module {
        return Err(fault(
            "original prefix is not an original module source root",
        ));
    }
    if !scan
        .producers()
        .iter()
        .any(|row| row.kind == DeclarationProducerKind::Attribute)
    {
        return Ok(BTreeSet::new());
    }
    let tree = parsed.parsed.tree();
    let metas = tree
        .attrs()
        .map(|attribute| {
            attribute
                .meta()
                .ok_or_else(|| fault("original root attribute meta absent"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (enabled, metas) =
        crate::syntax::attributes::effective_metas(metas, source.captured_macro_cfg()?)?;
    if !enabled {
        return Ok(BTreeSet::new());
    }
    // Associate actual root Meta ranges once. Replaying each prefix will use
    // the retained set and independently checked source scan instead. Builtin
    // inert and tool attributes never name a macro, so the scan records no
    // producer for them.
    let mut selected = BTreeMap::new();
    for meta in metas {
        let range: SourceRange = crate::syntax::ast_adapter::range(meta.syntax().text_range());
        let inert = meta.path().is_some_and(|path| {
            let text: String = path
                .syntax()
                .text()
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            crate::syntax::outline::is_builtin_inert_attribute(&text)
                || crate::syntax::outline::is_tool_attribute(&text)
        });
        if selected.insert((range.start, range.end), (inert, None)).is_some() {
            return Err(fault("original root meta range is duplicated"));
        }
    }
    for row in scan.producers() {
        if row.kind != DeclarationProducerKind::Attribute {
            continue;
        }
        if let Some((_, selected)) = selected.get_mut(&(row.range.start, row.range.end))
            && selected.replace(row).is_some()
        {
            return Err(fault("exact original root meta producer is ambiguous"));
        }
    }
    let mut result = BTreeSet::new();
    for (inert, row) in selected.into_values() {
        let Some(row) = row else {
            if inert {
                continue;
            }
            return Err(fault("exact original root meta producer absent"));
        };
        if row.lexical_scope != root.identity
            || row.scope != *scan.scope()
            || row.scope.owner.is_some()
            || !row.containers.is_empty()
        {
            return Err(fault("original root meta producer ownership differs"));
        }
        if !result.insert(row.identity) {
            return Err(fault("original root meta producer duplicated"));
        }
    }
    Ok(result)
}
