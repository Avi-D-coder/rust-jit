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
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "original source-prefix attribute budget".into(),
    )
}
pub(crate) fn capture(
    source: &PreparedSourceView,
    scan: &DeclarationScan,
    byte_limit: usize,
) -> Result<BTreeSet<Digest>, Diagnostic> {
    if !(128..=32 * 1024 * 1024).contains(&byte_limit) {
        return Err(limit());
    }
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
    let mut metas = Vec::new();
    for attribute in tree.attrs() {
        if metas.len() >= 100_000 {
            return Err(limit());
        }
        metas.try_reserve_exact(1).map_err(|_| limit())?;
        metas.push(
            attribute
                .meta()
                .ok_or_else(|| fault("original root attribute meta absent"))?,
        );
    }
    let (enabled, metas) =
        crate::syntax::attributes::effective_metas(metas, source.captured_macro_cfg()?)?;
    if !enabled {
        return Ok(BTreeSet::new());
    }
    let mut bytes = metas
        .capacity()
        .checked_mul(std::mem::size_of::<ra_ap_syntax::ast::Meta>())
        .and_then(|bytes| bytes.checked_add(128))
        .filter(|bytes| *bytes <= byte_limit)
        .ok_or_else(limit)?;
    // Associate actual root Meta ranges once. Replaying each prefix will use
    // the retained set and independently checked source scan instead.
    let mut selected = BTreeMap::new();
    let row_bytes = std::mem::size_of::<(
        (u32, u32),
        Option<&crate::syntax::declaration_scan::DeclarationProducer>,
    )>()
    .checked_add(128)
    .ok_or_else(limit)?;
    for meta in metas {
        let range: SourceRange = crate::syntax::ast_adapter::range(meta.syntax().text_range());
        bytes = bytes
            .checked_add(row_bytes)
            .filter(|bytes| *bytes <= byte_limit)
            .ok_or_else(limit)?;
        if selected.insert((range.start, range.end), None).is_some() {
            return Err(fault("original root meta range is duplicated"));
        }
    }
    for row in scan.producers() {
        if row.kind != DeclarationProducerKind::Attribute {
            continue;
        }
        if let Some(selected) = selected.get_mut(&(row.range.start, row.range.end))
            && selected.replace(row).is_some()
        {
            return Err(fault("exact original root meta producer is ambiguous"));
        }
    }
    let mut result = BTreeSet::new();
    for row in selected.into_values() {
        let row = row.ok_or_else(|| fault("exact original root meta producer absent"))?;
        if row.lexical_scope != root.identity
            || row.scope != *scan.scope()
            || row.scope.owner.is_some()
            || !row.containers.is_empty()
        {
            return Err(fault("original root meta producer ownership differs"));
        }
        bytes = bytes
            .checked_add(std::mem::size_of::<Digest>() + 128)
            .filter(|bytes| *bytes <= byte_limit)
            .ok_or_else(limit)?;
        if !result.insert(row.identity) {
            return Err(fault("original root meta producer duplicated"));
        }
    }
    Ok(result)
}
