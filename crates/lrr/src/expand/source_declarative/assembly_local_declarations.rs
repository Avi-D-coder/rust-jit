//! Original local headers in the real initial structural tree, before any edits.
use super::*;
use crate::{
    frontend::impl_index::CompletedOriginalAnnotation,
    syntax::{body_items::BodyItem, outline::ItemOutline, parse::ParsedFile},
};

pub(crate) struct InitialLocalDeclaration {
    source: SourceSelection,
    initial_syntax: Digest,
    original: ItemOutline,
    header: SourceRange,
}
impl InitialLocalDeclaration {
    pub(crate) fn matches(&self, prepared: &PreparedSourceSyntax, row: &BodyItem)
        -> Result<bool, Diagnostic> {
        Ok(prepared.source() == &self.source
            && prepared.occurrence(row.item.header_span)? == SourceOccurrence::Original {
                initial_syntax: self.initial_syntax, range: self.header,
            }
            && row.item.header.same_semantics(&self.original.header)
            && row.item.def_key.item_kind == self.original.def_key.item_kind
            && row.item.def_key.declared_name == self.original.def_key.declared_name)
    }
}
pub(super) fn capture(
    source: &PreparedSourceView,
    parsed: &ParsedFile,
    normalized: &crate::syntax::outline::Outline,
    initial_syntax: Digest,
    completion: &CompletedOriginalAnnotation,
) -> Result<InitialLocalDeclaration, Diagnostic> {
    let target = completion.original_target(source)?;
    let target_plan = target.local_plan().ok_or_else(|| fault("annotation has no original local plan"))?;
    let original = target.item();
    let cfg = source.captured_macro_cfg()?;
    // Select the nearest canonical outline owner, then follow only the actual
    // lexical body ancestry to the requested declaration. No callee is lowered.
    let ancestor = source.outline().items.iter().filter(|item| item.body.as_ref()
        .is_some_and(|body| body.range.contains(target_plan.body().range)))
        .min_by_key(|item| item.body.as_ref().unwrap().range.len())
        .ok_or_else(|| fault("original local annotation outline ancestor absent"))?;
    let initial = normalized.items.iter().find(|item|
        item.def_key.fingerprint() == ancestor.def_key.fingerprint())
        .ok_or_else(|| fault("normalized original local ancestor absent"))?;
    let original_parsed = source.retained_declaration_parser()?;
    let mut owners = (ancestor.clone(), initial.clone());
    loop {
        let (Some(original_body), Some(initial_body)) = (&owners.0.body, &owners.1.body) else {
            return Err(fault("original local annotation ancestor body absent"));
        };
        let original_plan = original_parsed.declaration_scope_plan(original_body, cfg, &owners.0.scope)?;
        let initial_plan = parsed.declaration_scope_plan(initial_body, cfg, &owners.1.scope)?;
        if original_body.def_key == target_plan.body().def_key {
            let row = initial_plan.declarations().declarations().find(|row|
                row.item.def_key.fingerprint() == original.def_key.fingerprint())
                .ok_or_else(|| fault("normalized initial local declaration absent"))?;
            if !row.item.header.same_semantics(&original.header) || row.item.scope != original.scope {
                return Err(fault("normalized initial local declaration differs"));
            }
            return Ok(InitialLocalDeclaration { source: source.selection().clone(), initial_syntax,
                original: original.clone(), header: row.item.header_span });
        }
        let parent = original_plan.declarations().declarations().filter(|row| row.item.body.as_ref()
            .is_some_and(|body| body.range.contains(target_plan.body().range)))
            .min_by_key(|row| row.item.body.as_ref().unwrap().range.len())
            .ok_or_else(|| fault("original local annotation body ancestry absent"))?;
        let initial_parent = initial_plan.declarations().declarations().find(|row|
            row.item.def_key.fingerprint() == parent.item.def_key.fingerprint())
            .ok_or_else(|| fault("normalized local annotation body ancestry absent"))?;
        if !parent.item.header.same_semantics(&initial_parent.item.header) {
            return Err(fault("normalized local annotation parent header differs"));
        }
        owners = (parent.item.clone(), initial_parent.item.clone());
    }
}
