//! Retained original-source set; no public source-completeness assertion.
use super::*;
use crate::frontend::names::NameNamespaces;

pub(super) struct RetainedConstantSources {
    views: Vec<PreparedSourceView>,
    bytes: usize,
}
impl Clone for RetainedConstantSources {
    fn clone(&self) -> Self {
        Self {
            views: self.views.iter().map(PreparedSourceView::share).collect(),
            bytes: self.bytes,
        }
    }
}
impl std::fmt::Debug for RetainedConstantSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.views.iter().map(PreparedSourceView::selection))
            .finish()
    }
}
impl RetainedConstantSources {
    pub(super) fn capture(
        db: &Database,
        target: &PreparedSourceView,
        views: &[&PreparedSourceView],
        stamps: &BTreeMap<InputKey, Digest>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if views.is_empty() || views.len() > 256 {
            return Err(limit());
        }
        let mut bytes = Some(std::mem::size_of::<Self>())
            .and_then(|n| {
                n.checked_add(
                    views
                        .len()
                        .checked_mul(std::mem::size_of::<PreparedSourceView>())?,
                )
            })
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        let mut locations = BTreeSet::new();
        let mut target_seen = false;
        for (index, view) in views.iter().enumerate() {
            if !views[..index]
                .iter()
                .any(|earlier| earlier.same_publication(view))
            {
                bytes = bytes
                    .checked_add(view.prepared_metadata_bytes()?)
                    .filter(|n| *n <= byte_limit)
                    .ok_or_else(limit)?;
                for (key, value) in view.prepared_inputs() {
                    if db
                        .published_input(key)
                        .is_none_or(|(_, actual)| actual != value.fingerprint())
                    {
                        return Err(fault(
                            "constant retained original source/compiler publication changed",
                        ));
                    }
                }
            }
            if view.selection().canonical()? != *view.selection() {
                return Err(fault("constant source selection is not canonical"));
            }
            view.require_original_hygiene()?;
            view.require_declaration_syntax()?;
            if !locations.insert(view.selection().location_identity()) {
                return Err(fault(
                    "constant retained source location is duplicate/ambiguous",
                ));
            }
            target_seen |= view.selection() == target.selection() && view.same_publication(target);
            bytes = bytes
                .checked_add(view.selection().metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        if !target_seen {
            return Err(fault("constant target is not in retained source set"));
        }
        // Conservative completeness comes from the actual published source
        // universe, not an arbitrary subset or a caller completeness flag.
        let mut captured_count = 0usize;
        for key in stamps.keys() {
            if let InputKey::CapturedSource(location) = key {
                captured_count = captured_count
                    .checked_add(1)
                    .filter(|n| *n <= 256)
                    .ok_or_else(limit)?;
                let Some((InputValue::CapturedSource(selection), _)) = db.published_input(key)
                else {
                    return Err(fault("constant captured source input variant differs"));
                };
                if selection.location_identity() != *location
                    || !views.iter().any(|view| view.selection() == selection)
                {
                    return Err(fault(
                        "constant complete current original source set is not retained",
                    ));
                }
            }
        }
        if captured_count != views.len() {
            return Err(fault(
                "constant retained set differs from actual current source membership",
            ));
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(views.len())
            .map_err(|_| limit())?;
        retained.extend(views.iter().map(|view| view.share()));
        retained.sort_by_key(|view| view.selection().location_identity());
        Ok(Self {
            views: retained,
            bytes,
        })
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn views(&self) -> Vec<&PreparedSourceView> {
        // Already admitted count <= 256, no guest data or provider selection.
        self.views.iter().collect()
    }
    pub(super) fn parser_reservation(&self) -> Result<usize, Diagnostic> {
        self.views.iter().try_fold(0usize, |bytes, view| {
            bytes
                .checked_add(
                    view.captured_macro_blob()?
                        .bytes()
                        .len()
                        .checked_mul(128)
                        .ok_or_else(limit)?,
                )
                .ok_or_else(limit)
        })
    }
    pub(super) fn matches_seal_sources(&self, sources: &[SourceSelection]) -> bool {
        sources.len() == self.views.len()
            && self.views.iter().all(|view| {
                sources
                    .iter()
                    .filter(|source| *source == view.selection())
                    .count()
                    == 1
            })
    }
    pub(super) fn encode(&self, e: &mut StableEncoder) {
        e.u64(self.views.len() as u64);
        for view in &self.views {
            e.string(view.selection().path.as_str());
            e.record(&view.selection().outline);
        }
    }
    pub(super) fn validate_names(
        &self,
        target: &PreparedSourceView,
        value: &DiscoveryInput,
        types: &DiscoveryInput,
    ) -> Result<(), Diagnostic> {
        // Preserve the established exact singleton producer, including its
        // privately published compiler-facet import resolution. A raw source
        // graph cannot reconstruct that separate facet authority. The new
        // multisource lane requires explicit original graph replay below.
        if self.views.len() == 1 {
            return Ok(());
        }
        if value.seal.key().unit_graph != types.seal.key().unit_graph
            || value.seal.key().epoch != types.seal.key().epoch
            || value.source_dependencies != types.source_dependencies
            || value.compiler_inputs != types.compiler_inputs
        {
            return Err(fault(
                "constant Type/Value world/compiler/source graph differs",
            ));
        }
        let mut graph = NameNamespaces::from_outlines(
            &self
                .views
                .iter()
                .map(PreparedSourceView::outline)
                .collect::<Vec<_>>(),
        )?;
        for view in &self.views {
            graph.require_source(view.selection(), view.outline())?;
        }
        let dependencies = target.source_dependencies()?;
        let mut units = BTreeSet::new();
        for view in &self.views {
            if units.insert(view.selection().outline.crate_key) {
                for (alias, provider) in dependencies
                    .aliases_for(
                        view.selection().outline.crate_key,
                        view.selection().outline.parse.profile.edition,
                    )?
                    .iter()
                {
                    graph.dependency(
                        view.selection().outline.crate_key,
                        alias.as_str(),
                        *provider,
                        view.selection().outline.parse.profile.edition,
                    )?;
                }
            }
        }
        let _ = graph.resolve_imports()?; // bounded pure fixed point; no arbitrary absence fallback.
        for input in [value, types] {
            let certified = input
                .scope
                .as_ref()
                .ok_or_else(|| fault("constant retained scope absent"))?;
            let replay =
                graph.certified_scope(&certified.scope, certified.namespace, certified.edition)?;
            if replay.fingerprint() != certified.fingerprint() {
                return Err(fault(
                    "constant original namespace replay differs from published seal",
                ));
            }
        }
        Ok(())
    }
}
