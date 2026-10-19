//! Names projected from a real completed fragment, using ordinary header capture.
use super::*;
use crate::{
    core::digest::StableEncoder,
    frontend::generated_headers::{GeneratedHeaderPlan, declared_names::DeclaredItemName},
    syntax::{outline::ScopeKey, token_wire::ContextRecord},
};
use std::collections::BTreeSet;

pub(crate) struct CompletedDeclarationNameEffects {
    pub(crate) names: Vec<Arc<DeclaredItemName>>,
    pub(crate) pending: Vec<(ScopeKey, Digest)>,
    fingerprint: Digest,
    outputs: std::collections::BTreeMap<(Digest, Digest), ScopeKey>,
}
/// A direct index of the actual completed fragment, minted only below. An
/// arbitrary admitted output or an empty pending set is not this receipt.
pub(crate) struct CompletedDeclarationNameSource<'a>(&'a GeneratedDeclarations);
impl CompletedDeclarationNameSource<'_> {
    pub(crate) fn output(&self) -> &GeneratedDeclarations { self.0 }
}
impl CompletedDeclarationNameEffects {
    pub(crate) fn fingerprint(&self) -> Digest { self.fingerprint }
    pub(crate) fn outputs(&self) -> impl Iterator<Item = (&(Digest, Digest), &ScopeKey)> { self.outputs.iter() }
}
impl CompletedDeclarationFragment {
    pub(crate) fn declaration_name_effects(
        &self,
        root: &ContextRecord,
        cfg: &CfgSet,
    ) -> Result<Arc<CompletedDeclarationNameEffects>, Diagnostic> {
        if let Some((owner, configuration, effects)) = self.name_effects.get() {
            if *owner != root.key() || *configuration != cfg.fingerprint() { return Err(limit()); }
            return Ok(Arc::clone(effects));
        }
        let mut names = Vec::new();
        let mut outputs = std::collections::BTreeMap::new();
        let mut unresolved = BTreeSet::new();
        let mut fingerprint = StableEncoder::new("lrr/v2/completed-declaration-name-effects/active-output-declarations");
        fingerprint.digest(Digest(root.key().0)); fingerprint.digest(cfg.fingerprint());
        let mut fragments = vec![self];
        while let Some(fragment) = fragments.pop() {
            let masks = ModuleReplacementRanges::new(&fragment.generated_replacement_masks()?)?;
            for index in std::iter::once(&fragment.index).chain(&fragment.completed_indexes) {
                if !index.is_complete() { return Err(limit()); }
                let output = index.output();
                if output.scan().cfg() != cfg.fingerprint() { return Err(limit()); }
                let items = crate::expand::generated_items::GeneratedItems::admit(
                    output.record().key(), output.record_handle(), output.epoch())?;
                let completed = CompletedDeclarationNameSource(output);
                let plan = GeneratedHeaderPlan::prepare_completed_declarations(&items, &completed, root, cfg)?;
                fingerprint.digest(output.fingerprint());
                let descriptor = output.record().descriptor().fingerprint();
                let active = plan.declared_names().map(|name| name.range())
                    .chain(plan.active_impl_ranges(&items, cfg, output.scan().scope(), output.epoch())?.iter().copied())
                    .chain(plan.active_trait_ranges(&items, cfg, output.scan().scope(), output.epoch())?.iter().copied())
                    .map(|range| (range.start, range.end)).collect::<BTreeSet<_>>();
                for header in items.items() {
                    let range = crate::syntax::spans::SourceRange::checked(header.start, header.end)?;
                    if !active.contains(&(range.start, range.end)) || masks.covers(output.scan().fingerprint(), range) { continue; }
                    if outputs.insert((descriptor, header.key.fingerprint()), output.scan().scope().clone())
                        .is_some_and(|scope| scope != *output.scan().scope()) { return Err(limit()); }
                }
                for declaration in plan.declared_names() {
                    if !masks.covers(output.scan().fingerprint(), declaration.range()) {
                        fingerprint.digest(declaration.fingerprint());
                        names.push(Arc::clone(declaration));
                    }
                }
                // Impl/trait inventory and macro definitions have independent
                // namespaces. Their actual completed fragment is not an open
                // Type/Value-name producer. Imports/module loads/unknown scope
                // effects still need their ordinary namespace preparation.
                let macro_headers = items.items().iter().filter(|header|
                    header.kind == crate::expand::generated_items::GeneratedHeaderKind::MacroRules)
                    .map(|header| header.key.fingerprint()).collect::<BTreeSet<_>>();
                for pending in plan.blocked() {
                    if Some(*pending) != plan.impl_frontier()
                        && Some(*pending) != plan.trait_frontier()
                        && !macro_headers.contains(pending)
                    {
                        unresolved.insert((output.scan().scope().clone(), *pending));
                    }
                }
            }
            for argument in &fragment.completed_arguments {
                fragments.extend(argument.assembly.completed.iter().map(Arc::as_ref));
            }
        }
        let pending = unresolved.into_iter().collect::<Vec<_>>();
        for (scope, id) in &pending { fingerprint.record(scope); fingerprint.digest(*id); }
        fingerprint.map(&outputs, |encoder, (record, declaration), scope| {
            encoder.digest(*record); encoder.digest(*declaration); encoder.record(scope);
        });
        let effects = Arc::new(CompletedDeclarationNameEffects { names, pending, outputs, fingerprint: fingerprint.finish() });
        let _ = self.name_effects.set((root.key(), cfg.fingerprint(), effects));
        let (owner, configuration, effects) = self.name_effects.get().ok_or_else(limit)?;
        if *owner != root.key() || *configuration != cfg.fingerprint() { return Err(limit()); }
        Ok(Arc::clone(effects))
    }
}
