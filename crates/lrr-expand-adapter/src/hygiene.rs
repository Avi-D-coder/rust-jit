//! Owned hygiene boundary over the pinned mark/normalization algorithm.
//! Private call/context IDs never escape this database or enter HIR providers.
use crate::{
    AdapterError,
    token_wire::{
        ContextRecord, Edition, Key, NameContextKey, NameResolutionContext, ResolutionContext,
        TokenForest, Transparency, pinned,
    },
};
use salsa::plumbing::AsId;
use span::SyntaxContext;
use std::collections::{BTreeMap, HashMap};

#[salsa::db]
#[derive(Default, Clone)]
struct HygieneDb {
    storage: salsa::Storage<Self>,
}
#[salsa::db]
impl salsa::Database for HygieneDb {}
// Input IDs provide safe database-owned call identities without reconstructing
// raw Salsa IDs or allocating a HIR MacroCallLoc with synthetic source facts.
#[salsa::input]
struct MarkIdentity {
    key: Key,
}

#[derive(Debug, Clone, Copy)]
pub enum ResolutionMode {
    Item,
    Local,
}
#[derive(Debug, Clone, Copy)]
pub struct MarkRequest {
    pub parent: Key,
    pub expansion: Key,
    pub defining_unit: Key,
    pub definition_edition: Edition,
    pub call_site: Key,
    pub transparency: Transparency,
}
#[derive(Default, Clone)]
pub struct HygieneStore {
    db: HygieneDb,
    records: BTreeMap<Key, ContextRecord>,
    contexts: BTreeMap<Key, SyntaxContext>,
    marks: BTreeMap<Key, (MarkIdentity, Key)>,
    reverse_marks: HashMap<span::MacroCallId, Key>,
}
const MAX_CONTEXTS: usize = 2_000_000;
const MAX_DEPTH: usize = 256;
fn error(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-TOKEN-WIRE",
        message: message.into(),
    }
}
fn transparency(value: Transparency) -> span::Transparency {
    match value {
        Transparency::Opaque => span::Transparency::Opaque,
        Transparency::SemiTransparent => span::Transparency::SemiOpaque,
        Transparency::Transparent => span::Transparency::Transparent,
    }
}
fn owned_transparency(value: span::Transparency) -> Transparency {
    match value {
        span::Transparency::Opaque => Transparency::Opaque,
        span::Transparency::SemiOpaque => Transparency::SemiTransparent,
        span::Transparency::Transparent => Transparency::Transparent,
    }
}
fn owned_edition(value: span::Edition) -> Edition {
    match value {
        span::Edition::Edition2015 => Edition::E2015,
        span::Edition::Edition2018 => Edition::E2018,
        span::Edition::Edition2021 => Edition::E2021,
        span::Edition::Edition2024 => Edition::E2024,
    }
}
impl HygieneStore {
    /// Normalize only demanded contexts through the actual pinned machinery.
    /// The caller transports these rows together with `records()` afterward,
    /// which includes any normalized ancestor records created here.
    pub fn resolution_contexts(
        &mut self,
        demanded: &[Key],
    ) -> Result<Vec<ResolutionContext>, AdapterError> {
        if demanded.len() > MAX_CONTEXTS {
            return Err(error("resolution context table limit exceeded"));
        }
        for key in demanded {
            self.context(*key)?;
        }
        let mut rows = Vec::new();
        rows.try_reserve(demanded.len())
            .map_err(|_| error("resolution context allocation failed"))?;
        for key in demanded {
            rows.push(ResolutionContext {
                original: *key,
                item: self.normalize(*key, ResolutionMode::Item)?,
                local: self.normalize(*key, ResolutionMode::Local)?,
            });
        }
        rows.sort_by_key(|row| row.original);
        rows.dedup_by_key(|row| row.original);
        Ok(rows)
    }
    /// Comparison classes are stable remappings of actual normalized upstream
    /// contexts. Their canonical root owner is erased only in this encoding;
    /// no comparison-only context is published into the source/span tables.
    pub fn name_contexts(
        &self,
        demanded: &[Key],
    ) -> Result<Vec<NameResolutionContext>, AdapterError> {
        if demanded.len() > MAX_CONTEXTS {
            return Err(error("name comparison table limit exceeded"));
        }
        for key in demanded {
            self.context(*key)?;
        }
        let mut rows = Vec::new();
        rows.try_reserve(demanded.len())
            .map_err(|_| error("name comparison allocation failed"))?;
        for original in demanded {
            rows.push(NameResolutionContext {
                original: *original,
                item: self.name_class(*original, ResolutionMode::Item)?,
                local: self.name_class(*original, ResolutionMode::Local)?,
            });
        }
        rows.sort_by_key(|row| row.original);
        rows.dedup_by_key(|row| row.original);
        Ok(rows)
    }
    fn name_class(&self, key: Key, mode: ResolutionMode) -> Result<NameContextKey, AdapterError> {
        let mut raw = self.normalized(key, mode)?;
        raw.remove_root_edition();
        // Reuse the stable owned remapping, not frontend IDs or a second
        // normalization algorithm. Raw context metadata remains unchanged.
        let rows = self.describe(raw, Key([0; 32]))?;
        Ok(NameContextKey::from_canonical_context(
            rows.last().expect("root record").0.key(),
        ))
    }
    pub(crate) fn database(&self) -> &dyn salsa::Database {
        &self.db
    }
    pub(crate) fn registered_context(&self, key: Key) -> Result<SyntaxContext, AdapterError> {
        self.context(key)
    }
    pub fn records(&self) -> Vec<ContextRecord> {
        self.records.values().cloned().collect()
    }
    fn context(&self, key: Key) -> Result<SyntaxContext, AdapterError> {
        self.contexts
            .get(&key)
            .copied()
            .ok_or_else(|| error("unregistered hygiene context"))
    }
    fn root_record(&self, mut key: Key) -> Result<&ContextRecord, AdapterError> {
        for _ in 0..=MAX_DEPTH {
            let record = self
                .records
                .get(&key)
                .ok_or_else(|| error("unregistered context ancestor"))?;
            match record.parent {
                Some(parent) => key = parent,
                None => return Ok(record),
            }
        }
        Err(error("hygiene ancestor depth exceeded"))
    }
    pub fn root(&mut self, unit: Key, edition: Edition) -> Result<Key, AdapterError> {
        let record = ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: unit,
            edition,
        };
        let key = record.key();
        if !self.records.contains_key(&key) && self.records.len() >= MAX_CONTEXTS {
            return Err(error("hygiene context table limit exceeded"));
        }
        self.records.insert(key, record);
        self.contexts
            .insert(key, SyntaxContext::root(pinned::edition(edition)));
        Ok(key)
    }
    /// Private RPC label, never an owned semantic context or normalization input.
    /// Safe pinned mark allocation keeps distinct owned roots distinguishable
    /// when the server independently copies a span's location and context.
    pub(crate) fn procedural_transport_context(
        &mut self,
        key: Key,
    ) -> Result<SyntaxContext, AdapterError> {
        let context = self.context(key)?;
        let record = self
            .records
            .get(&key)
            .ok_or_else(|| error("unknown transport context"))?
            .clone();
        let mut hash = blake3::Hasher::new();
        hash.update(b"lrr/procedural-transport-context/v1\0");
        hash.update(&key.0);
        let call = self.mark_id(Key(*hash.finalize().as_bytes()), record.defining_unit)?;
        Ok(hir_expand::hygiene::apply_mark_with_call_site(
            &self.db,
            context,
            call,
            span::Transparency::Opaque,
            pinned::edition(record.edition),
            context,
        ))
    }
    fn mark_id(&mut self, expansion: Key, unit: Key) -> Result<span::MacroCallId, AdapterError> {
        if let Some((identity, existing_unit)) = self.marks.get(&expansion) {
            if *existing_unit != unit {
                return Err(error("expansion identity has conflicting defining units"));
            }
            return Ok(span::MacroCallId(identity.as_id()));
        }
        if self.marks.len() >= MAX_CONTEXTS {
            return Err(error("hygiene mark table limit exceeded"));
        }
        self.reverse_marks
            .try_reserve(1)
            .map_err(|_| error("hygiene mark allocation failed"))?;
        let identity = MarkIdentity::new(&self.db, expansion);
        let call = span::MacroCallId(identity.as_id());
        self.marks.insert(expansion, (identity, unit));
        self.reverse_marks.insert(call, expansion);
        Ok(call)
    }
    pub fn apply(&mut self, request: MarkRequest) -> Result<Key, AdapterError> {
        let parent = self.context(request.parent)?;
        let call_site = self.context(request.call_site)?;
        let normalized_call = match request.transparency {
            Transparency::SemiTransparent => call_site.normalize_to_macros_2_0(&self.db),
            Transparency::Transparent => call_site.normalize_to_macro_rules(&self.db),
            Transparency::Opaque => call_site,
        };
        // Select only the owned root identity of the hierarchy actually used by
        // the upstream algorithm. Mark application itself remains upstream.
        let base = if request.transparency != Transparency::Opaque && !normalized_call.is_root() {
            request.call_site
        } else {
            request.parent
        };
        let root_unit = self.root_record(base)?.defining_unit;
        let call = self.mark_id(request.expansion, request.defining_unit)?;
        let marked = hir_expand::hygiene::apply_mark_with_call_site(
            &self.db,
            parent,
            call,
            transparency(request.transparency),
            pinned::edition(request.definition_edition),
            call_site,
        );
        self.adopt(marked, root_unit)
    }
    fn normalized(&self, key: Key, mode: ResolutionMode) -> Result<SyntaxContext, AdapterError> {
        let context = self.context(key)?;
        Ok(match mode {
            ResolutionMode::Item => context.normalize_to_macros_2_0(&self.db),
            ResolutionMode::Local => context.normalize_to_macro_rules(&self.db),
        })
    }
    /// Context equivalence only; source unit/member/lexical scope identity must
    /// also agree before a name is considered resolved.
    pub fn equivalent(&self, a: Key, b: Key, mode: ResolutionMode) -> Result<bool, AdapterError> {
        Ok(self.normalized(a, mode)? == self.normalized(b, mode)?)
    }
    /// Name comparison follows the pinned Name/HygieneId root-edition erasure.
    /// Source owner routing must still use the original owned context record.
    pub fn name_equivalent(
        &self,
        a: Key,
        b: Key,
        mode: ResolutionMode,
    ) -> Result<bool, AdapterError> {
        let mut a = self.normalized(a, mode)?;
        let mut b = self.normalized(b, mode)?;
        a.remove_root_edition();
        b.remove_root_edition();
        Ok(a == b)
    }
    pub fn normalize(&mut self, key: Key, mode: ResolutionMode) -> Result<Key, AdapterError> {
        let unit = self.root_record(key)?.defining_unit;
        self.adopt(self.normalized(key, mode)?, unit)
    }
    fn describe(
        &self,
        mut raw: SyntaxContext,
        root_unit: Key,
    ) -> Result<Vec<(ContextRecord, SyntaxContext)>, AdapterError> {
        let mut chain = Vec::new();
        while !raw.is_root() {
            if chain.len() >= MAX_DEPTH {
                return Err(error("hygiene context depth exceeded"));
            }
            chain
                .try_reserve(1)
                .map_err(|_| error("hygiene ancestor allocation failed"))?;
            let call = raw
                .outer_expn(&self.db)
                .ok_or_else(|| error("marked context lacks expansion"))?;
            let expansion = *self
                .reverse_marks
                .get(&call)
                .ok_or_else(|| error("unregistered private expansion identity"))?;
            let unit = self
                .marks
                .get(&expansion)
                .ok_or_else(|| error("unregistered expansion metadata"))?
                .1;
            chain.push((
                raw,
                expansion,
                unit,
                owned_edition(raw.edition(&self.db)),
                owned_transparency(raw.outer_transparency(&self.db)),
            ));
            raw = raw.parent(&self.db);
        }
        let root = ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: root_unit,
            edition: owned_edition(raw.edition(&self.db)),
        };
        let mut parent = root.key();
        let mut rows = Vec::new();
        rows.try_reserve(
            chain
                .len()
                .checked_add(1)
                .ok_or_else(|| error("hygiene remapping count overflow"))?,
        )
        .map_err(|_| error("hygiene remapping allocation failed"))?;
        rows.push((root, raw));
        for (raw, expansion, unit, edition, transparency) in chain.into_iter().rev() {
            let record = ContextRecord {
                parent: Some(parent),
                expansion: Some(expansion),
                transparency,
                defining_unit: unit,
                edition,
            };
            parent = record.key();
            rows.push((record, raw));
        }
        Ok(rows)
    }
    fn adopt(&mut self, raw: SyntaxContext, root_unit: Key) -> Result<Key, AdapterError> {
        let described = self.describe(raw, root_unit)?;
        let parent = described.last().expect("root record").0.key();
        let staged: BTreeMap<_, _> = described
            .into_iter()
            .map(|(record, raw)| (record.key(), (record, raw)))
            .collect();
        let additions = staged
            .keys()
            .filter(|k| !self.records.contains_key(*k))
            .count();
        if self
            .records
            .len()
            .checked_add(additions)
            .is_none_or(|n| n > MAX_CONTEXTS)
        {
            return Err(error("hygiene context table limit exceeded"));
        }
        for (key, (record, raw)) in staged {
            self.records.insert(key, record);
            self.contexts.insert(key, raw);
        }
        Ok(parent)
    }
    /// Import a complete owned ancestor table. The chain already embodies any
    /// call-site inheritance; root call-site reconstruction directly interns
    /// those recorded marks through the original upstream algorithm.
    pub fn import(&mut self, records: &[ContextRecord]) -> Result<(), AdapterError> {
        if records.len() > MAX_CONTEXTS {
            return Err(error("imported hygiene table limit exceeded"));
        }
        let mut owned = Vec::new();
        owned
            .try_reserve(records.len())
            .map_err(|_| error("hygiene import allocation failed"))?;
        owned.extend_from_slice(records);
        let forest = TokenForest::new(Vec::new(), owned, Vec::new())?;
        forest.encoded_len()?;
        let mut incoming = BTreeMap::new();
        let mut units = BTreeMap::new();
        for record in forest.contexts() {
            if let Some(expansion) = record.expansion {
                if self
                    .marks
                    .get(&expansion)
                    .is_some_and(|(_, unit)| *unit != record.defining_unit)
                {
                    return Err(error("imported expansion has conflicting defining units"));
                }
                if units
                    .insert(expansion, record.defining_unit)
                    .is_some_and(|unit| unit != record.defining_unit)
                {
                    return Err(error("imported expansion has conflicting defining units"));
                }
            } else if record.transparency != Transparency::Opaque {
                return Err(error("root hygiene context must be opaque"));
            }
            incoming.insert(record.key(), record);
        }
        let new_contexts = incoming
            .keys()
            .filter(|key| !self.records.contains_key(*key))
            .count();
        let new_marks = units
            .keys()
            .filter(|key| !self.marks.contains_key(*key))
            .count();
        if self
            .records
            .len()
            .checked_add(new_contexts)
            .is_none_or(|n| n > MAX_CONTEXTS)
            || self
                .marks
                .len()
                .checked_add(new_marks)
                .is_none_or(|n| n > MAX_CONTEXTS)
        {
            return Err(error("imported hygiene table limit exceeded"));
        }
        // Reserve the complete iterative walk before publishing any input.
        // Its initial frontier can grow by at most one entry per ancestor.
        let capacity = incoming
            .len()
            .checked_add(MAX_DEPTH + 1)
            .ok_or_else(|| error("hygiene worklist size overflow"))?;
        let mut pending = Vec::new();
        pending
            .try_reserve(capacity)
            .map_err(|_| error("hygiene worklist allocation failed"))?;
        pending.extend(incoming.keys().copied());
        self.reverse_marks
            .try_reserve(new_marks)
            .map_err(|_| error("hygiene mark allocation failed"))?;
        for (expansion, unit) in units {
            self.mark_id(expansion, unit)?;
        }
        while let Some(key) = pending.pop() {
            if self.contexts.contains_key(&key) {
                continue;
            }
            let record = incoming
                .get(&key)
                .ok_or_else(|| error("missing imported context"))?;
            let raw = if let Some(parent) = record.parent {
                let Some(parent_raw) = self.contexts.get(&parent).copied() else {
                    pending.push(key);
                    pending.push(parent);
                    continue;
                };
                let expansion = record
                    .expansion
                    .ok_or_else(|| error("missing imported expansion"))?;
                let call = self.mark_id(expansion, record.defining_unit)?;
                hir_expand::hygiene::apply_mark_with_call_site(
                    &self.db,
                    parent_raw,
                    call,
                    transparency(record.transparency),
                    pinned::edition(record.edition),
                    SyntaxContext::root(pinned::edition(record.edition)),
                )
            } else {
                SyntaxContext::root(pinned::edition(record.edition))
            };
            self.contexts.insert(key, raw);
            self.records.insert(key, (*record).clone());
        }
        Ok(())
    }
}
