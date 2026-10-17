//! One immutable initial assembly per actual captured source. Forking shares
//! pinned syntax and copies only bounded mutable attachment bookkeeping.
use super::*;
use lrr_expand_adapter::syntax_attachment::SyntaxAttachmentSeed;
const ARC: usize = 2 * std::mem::size_of::<usize>();

pub(crate) struct SourceAssemblySeed {
    source: PreparedSourceView,
    epoch: u64,
    tree: SyntaxAttachmentSeed,
    calls: Arc<[CallBinding]>,
    originals: Arc<[OriginalDeclarationAnchor]>,
    macros: Arc<[MacroDefinitionAnchor]>,
    scan: Arc<DeclarationScan>,
    retained_bytes: usize,
    bytes: usize,
    identity: Digest,
}
impl SourceAssemblySeed {
    pub(crate) fn prepare(
        provider: &SourceDeclarative,
        index: &DeclarationMacroIndex,
        epoch: u64,
    ) -> Result<Arc<Self>, Diagnostic> {
        Self::from_scan(provider, index.scan_handle(), epoch)
    }
    pub(crate) fn from_scan(
        provider: &SourceDeclarative,
        scan: &Arc<DeclarationScan>,
        epoch: u64,
    ) -> Result<Arc<Self>, Diagnostic> {
        Self::from_forest(&provider.owned_forest_handle(), scan, epoch)
    }
    pub(crate) fn from_forest(
        forest: &crate::expand::source_declarative::OwnedForestHandle,
        scan: &Arc<DeclarationScan>,
        epoch: u64,
    ) -> Result<Arc<Self>, Diagnostic> {
        // Exactly the existing checked original forest/scan/capture route.
        let initial =
            SourceSyntaxAssembly::from_source(forest.source(), &*forest.get()?, scan, epoch)?;
        let SourceSyntaxAssembly {
            source,
            epoch,
            tree,
            calls,
            originals,
            macro_definitions,
            occurrences,
            outputs,
            expansion_definitions,
            retained_bytes,
        } = initial;
        if !outputs.is_empty() || !expansion_definitions.is_empty() {
            return Err(fault("source assembly seed is not initial"));
        }
        let tree = tree.into_seed().map_err(adapter)?;
        occurrences.validate(tree.text_len())?;
        let mut bytes = charge(std::mem::size_of::<Self>(), ARC * 4)?;
        bytes = charge(bytes, retained_bytes)?;
        bytes = charge(bytes, tree.metadata_bytes())?;
        bytes = charge_calls(bytes, calls.capacity())?;
        bytes = charge(bytes, macro_anchors::metadata(&macro_definitions)?)?;
        // Bound Vec->Arc slice simultaneous ownership; no hidden body copy.
        let peak = charge(bytes, anchors::metadata(&originals)?)?;
        let peak = charge_calls(peak, calls.capacity())?;
        charge(peak, macro_anchors::metadata(&macro_definitions)?)?;
        let mut identity = StableEncoder::new("lrr/v1/initial-source-assembly-seed");
        identity.string(source.selection().path.as_str());
        identity.record(&source.selection().outline);
        identity.u64(epoch);
        identity.digest(Digest(tree.identity().0));
        identity.u32(tree.text_len());
        identity.sequence(&calls, |e, call| {
            e.option(&call.invocation, |e, value| e.digest(*value));
            e.option(&call.grammar, |e, grammar| e.record(grammar));
            e.digest(call.parent_identity);
            e.digest(call.anchor);
        });
        identity.sequence(&originals, |e, anchor| anchor.encode(e));
        identity.sequence(&macro_definitions, |e, anchor| anchor.encode(e));
        Ok(Arc::new(Self {
            source,
            epoch,
            tree,
            calls: Arc::from(calls.into_boxed_slice()),
            originals: Arc::from(originals.into_boxed_slice()),
            macros: Arc::from(macro_definitions.into_boxed_slice()),
            scan: Arc::clone(scan),
            retained_bytes,
            bytes,
            identity: identity.finish(),
        }))
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        self.source.selection()
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn fork(&self) -> Result<SourceSyntaxAssembly, Diagnostic> {
        // Check the combined temporary tables before allocating any of them.
        let peak = charge_calls(self.bytes, self.calls.len())?;
        let peak = charge(
            peak,
            self.originals
                .len()
                .checked_mul(std::mem::size_of::<OriginalDeclarationAnchor>())
                .ok_or_else(|| fault("source seed anchor size overflow"))?,
        )?;
        charge(
            peak,
            self.macros
                .len()
                .checked_mul(std::mem::size_of::<MacroDefinitionAnchor>())
                .ok_or_else(|| fault("source seed macro anchor size overflow"))?,
        )?;
        let calls = copy_rows(&self.calls)?;
        let originals = copy_rows(&self.originals)?;
        let macro_definitions = copy_rows(&self.macros)?;
        let bytes = charge_calls(self.bytes, calls.capacity())?;
        let bytes = charge(bytes, anchors::metadata(&originals)?)?;
        charge(bytes, macro_anchors::metadata(&macro_definitions)?)?;
        Ok(SourceSyntaxAssembly {
            source: self.source.share(),
            epoch: self.epoch,
            tree: self.tree.fork().map_err(adapter)?,
            occurrences: SourceOccurrences::new(
                Digest(self.tree.identity().0),
                self.tree.text_len(),
            )?,
            calls,
            outputs: Vec::new(),
            originals,
            macro_definitions,
            expansion_definitions: BTreeMap::new(),
            retained_bytes: self.retained_bytes,
        })
    }
}
fn copy_rows<T: Clone>(rows: &[T]) -> Result<Vec<T>, Diagnostic> {
    charge(
        0,
        rows.len()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| fault("source seed fork row size overflow"))?,
    )?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(rows.len())
        .map_err(|_| fault("source seed fork allocation"))?;
    charge(
        0,
        owned
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| fault("source seed fork capacity overflow"))?,
    )?;
    owned.extend_from_slice(rows);
    Ok(owned)
}

/// The initial assembly of one captured source, built on first demand. Its
/// inputs (provider source, owned forest, declaration scan, epoch) are fixed
/// at discovery completion; sources never entered are never assembled.
pub(crate) struct LazySourceAssemblySeed {
    forest: crate::expand::source_declarative::OwnedForestHandle,
    scan: Arc<DeclarationScan>,
    epoch: u64,
    seed: std::sync::OnceLock<Arc<SourceAssemblySeed>>,
}
impl LazySourceAssemblySeed {
    pub(crate) fn new(provider: &SourceDeclarative, scan: &Arc<DeclarationScan>, epoch: u64) -> Self {
        Self {
            forest: provider.owned_forest_handle(),
            scan: Arc::clone(scan),
            epoch,
            seed: std::sync::OnceLock::new(),
        }
    }
    pub(crate) fn ready(seed: Arc<SourceAssemblySeed>) -> Result<Self, Diagnostic> {
        let lazy = Self {
            forest: crate::expand::source_declarative::OwnedForestHandle::detached(seed.source.share()),
            scan: Arc::clone(&seed.scan),
            epoch: seed.epoch,
            seed: std::sync::OnceLock::new(),
        };
        let _ = lazy.seed.set(seed);
        Ok(lazy)
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        self.forest.source().selection()
    }
    pub(crate) fn source_view(&self) -> &PreparedSourceView {
        self.forest.source()
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub(crate) fn get(&self) -> Result<&Arc<SourceAssemblySeed>, Diagnostic> {
        if let Some(seed) = self.seed.get() {
            return Ok(seed);
        }
        let seed = SourceAssemblySeed::from_forest(&self.forest, &self.scan, self.epoch)?;
        Ok(self.seed.get_or_init(|| seed))
    }
}
