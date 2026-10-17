//! Private upstream span slots mapped to semantic owned records. No query engine is exposed.
use crate::{
    AdapterError,
    declarative::DeclarativeSite,
    hygiene::{HygieneStore, MarkRequest},
    token_wire::{ContextRecord, Key, SpanRecord, TokenForest, TokenTree, pinned},
};
use span::{EditionedFileId, FileId, ROOT_ERASED_FILE_AST_ID, Span, SpanAnchor, TextRange};
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[path = "span_store/proc_callbacks.rs"]
mod proc_callbacks;
#[derive(Clone, Default)]
pub(crate) struct SpanStore {
    hygiene: HygieneStore,
    contexts: BTreeMap<Key, ContextRecord>,
    records: BTreeMap<Key, SpanRecord>,
    slots: BTreeMap<Key, Span>,
    reverse: HashMap<Span, Key>,
    anchors: HashMap<SpanAnchor, Key>,
    next_slot: u32,
    proc_contexts: Option<BTreeMap<Key, span::SyntaxContext>>,
}
fn error(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-TOKEN-WIRE",
        message: message.into(),
    }
}
impl SpanStore {
    /// Checked owned-table estimate, including reserved hash capacity and path
    /// allocations. The shared pinned Salsa arena is explicitly excluded by
    /// HygieneStore::metadata_bytes; no unstable cancelling introspection runs.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, AdapterError> {
        let overflow = || error("attachment span store metadata overflow");
        let table =
            |len: usize, size: usize| len.checked_mul(size.checked_mul(16)?.checked_add(256)?);
        let mut bytes = std::mem::size_of::<Self>();
        for size in [
            table(
                self.contexts.len(),
                std::mem::size_of::<(Key, ContextRecord)>(),
            ),
            table(self.records.len(), std::mem::size_of::<(Key, SpanRecord)>()),
            table(self.slots.len(), std::mem::size_of::<(Key, Span)>()),
            self.reverse
                .capacity()
                .checked_mul(std::mem::size_of::<(Span, Key)>() + 16),
            self.anchors
                .capacity()
                .checked_mul(std::mem::size_of::<(SpanAnchor, Key)>() + 16),
            self.proc_contexts.as_ref().map_or(Some(0), |rows| {
                table(
                    rows.len(),
                    std::mem::size_of::<(Key, span::SyntaxContext)>(),
                )
            }),
            Some(self.hygiene.metadata_bytes()?),
        ] {
            bytes = bytes
                .checked_add(size.ok_or_else(overflow)?)
                .ok_or_else(overflow)?;
        }
        for record in self.records.values() {
            bytes = bytes
                .checked_add(record.source.path.capacity())
                .ok_or_else(overflow)?;
        }
        Ok(bytes)
    }
    pub(crate) fn owned_span_key(&self, span: Span) -> Result<Key, AdapterError> {
        self.reverse
            .get(&span)
            .copied()
            .ok_or_else(|| error("unregistered source span"))
    }
    pub(crate) fn import_proc(
        &mut self,
        forest: &TokenForest,
    ) -> Result<tt::TopSubtree, AdapterError> {
        forest.validate()?;
        self.hygiene.import(forest.contexts())?;
        let labels = self.proc_contexts.get_or_insert_with(BTreeMap::new);
        for record in forest.contexts() {
            let key = record.key();
            if let std::collections::btree_map::Entry::Vacant(entry) = labels.entry(key) {
                entry.insert(self.hygiene.procedural_transport_context(key)?);
            }
        }
        self.import(forest)
    }
    pub(crate) fn transport_context(&self, key: Key) -> Result<span::SyntaxContext, AdapterError> {
        match &self.proc_contexts {
            Some(labels) => labels
                .get(&key)
                .copied()
                .ok_or_else(|| error("unregistered transport context")),
            None => self.hygiene.registered_context(key),
        }
    }
    pub(crate) fn import(&mut self, forest: &TokenForest) -> Result<tt::TopSubtree, AdapterError> {
        forest.validate()?;
        self.hygiene.import(forest.contexts())?;
        self.sync_contexts();
        for record in forest.spans() {
            let key = record.key();
            if self.slots.contains_key(&key) {
                continue;
            }
            let context = self
                .contexts
                .get(&record.context)
                .ok_or_else(|| error("unknown owned context"))?;
            let slot = self.next_slot;
            self.next_slot = self
                .next_slot
                .checked_add(1)
                .ok_or_else(|| error("span slot overflow"))?;
            if slot >= 2_000_000 {
                return Err(AdapterError {
                    code: "LRR-E-MACRO-OUTPUT",
                    message: "private span table limit".into(),
                });
            }
            let s = Span {
                range: TextRange::new(record.start.into(), record.end.into()),
                anchor: SpanAnchor {
                    file_id: EditionedFileId::new(
                        FileId::from_raw(slot),
                        pinned::edition(record.edition),
                    ),
                    ast_id: ROOT_ERASED_FILE_AST_ID,
                },
                ctx: self.transport_context(context.key())?,
            };
            self.records.insert(key, record.clone());
            self.anchors.insert(s.anchor, key);
            self.slots.insert(key, s);
            if self.reverse.insert(s, key).is_some() {
                return Err(error("ambiguous private span slot"));
            }
        }
        let [
            TokenTree::Group {
                delimiter,
                open,
                close,
                children,
            },
        ] = forest.roots()
        else {
            return Err(error("declarative input requires one explicit token group"));
        };
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: pinned::delimiter(*delimiter),
            open: self.span(*open)?,
            close: self.span(*close)?,
        });
        pinned::emit(children, &mut builder, &self.slots)?;
        Ok(builder.build())
    }
    pub(crate) fn span(&self, key: Key) -> Result<Span, AdapterError> {
        self.slots
            .get(&key)
            .copied()
            .ok_or_else(|| error("unregistered stable span"))
    }
    pub(crate) fn database(&self) -> &dyn salsa::Database {
        self.hygiene.database()
    }
    fn sync_contexts(&mut self) {
        self.contexts = self
            .hygiene
            .records()
            .into_iter()
            .map(|record| (record.key(), record))
            .collect();
    }
    pub(crate) fn context_parent(&self, key: [u8; 32]) -> Result<mbe::ContextParent, AdapterError> {
        let record = self
            .contexts
            .get(&Key(key))
            .ok_or_else(|| error("unregistered preflight context ancestor"))?;
        match record.parent {
            None => Ok(mbe::ContextParent::Root),
            Some(parent) => {
                let record = self
                    .contexts
                    .get(&parent)
                    .ok_or_else(|| error("missing preflight context ancestor"))?;
                Ok(mbe::ContextParent::Parent(mbe::ContextWireCost {
                    context: parent.0,
                    context_bytes: crate::token_wire::context_wire_size(record)?,
                }))
            }
        }
    }
    /// Precompute closed, read-only span substitutions. Neither preflight nor
    /// materialization may allocate a context or change source coordinates.
    pub(crate) fn prepare_marks(
        &mut self,
        definition: &TokenForest,
        call_site: Key,
        site: DeclarativeSite,
        transparency: crate::token_wire::Transparency,
    ) -> Result<HashMap<Span, Span>, AdapterError> {
        let call_context = self
            .records
            .get(&call_site)
            .ok_or_else(|| error("unregistered call-site record"))?
            .context;
        let mut marked_contexts = BTreeMap::new();
        for record in definition.spans() {
            if marked_contexts.contains_key(&record.context) {
                continue;
            }
            let context = self.hygiene.apply(MarkRequest {
                parent: record.context,
                expansion: site.expansion,
                defining_unit: site.defining_unit,
                definition_edition: site.definition_edition,
                call_site: call_context,
                transparency,
            })?;
            marked_contexts.insert(record.context, context);
        }
        let mut staged = Vec::new();
        staged
            .try_reserve(definition.spans().len())
            .map_err(|_| error("marked span allocation failed"))?;
        for record in definition.spans() {
            let old = self.span(record.key())?;
            let mut record = record.clone();
            record.context = *marked_contexts
                .get(&record.context)
                .ok_or_else(|| error("missing marked template context"))?;
            let marked = Span {
                ctx: self.hygiene.registered_context(record.context)?,
                ..old
            };
            staged.push((record.key(), record, old, marked));
        }
        let additions = staged
            .iter()
            .filter(|(key, ..)| !self.records.contains_key(key))
            .count();
        if self
            .records
            .len()
            .checked_add(additions)
            .is_none_or(|n| n > 2_000_000)
        {
            return Err(error("marked span table limit exceeded"));
        }
        let mut substitutions = HashMap::new();
        substitutions
            .try_reserve(staged.len())
            .map_err(|_| error("marked span preview allocation failed"))?;
        self.reverse
            .try_reserve(additions)
            .map_err(|_| error("marked span index allocation failed"))?;
        self.sync_contexts();
        for (key, record, old, marked) in staged {
            substitutions.insert(old, marked);
            self.records.insert(key, record);
            self.slots.insert(key, marked);
            self.reverse.insert(marked, key);
        }
        Ok(substitutions)
    }
    pub(crate) fn key(&self, span: Span) -> Result<Key, AdapterError> {
        self.reverse
            .get(&span)
            .copied()
            .ok_or_else(|| error("unregistered upstream span"))
    }
    pub(crate) fn wire_cost(&self, span: Span) -> Result<mbe::SpanWireCost, AdapterError> {
        let key = self.key(span)?;
        let record = self
            .records
            .get(&key)
            .ok_or_else(|| error("unregistered preflight span record"))?;
        let context = self
            .contexts
            .get(&record.context)
            .ok_or_else(|| error("unregistered preflight context"))?;
        let (context_bytes, span_bytes) = crate::token_wire::record_wire_sizes(context, record)?;
        Ok(mbe::SpanWireCost {
            context: record.context.0,
            context_bytes,
            span_bytes,
        })
    }
    pub(crate) fn export(&self, tree: &tt::TopSubtree) -> Result<TokenForest, AdapterError> {
        let mut used = BTreeSet::new();
        let root = pinned::group(
            tree.top_subtree().delimiter,
            tree.iter(),
            0,
            &mut 0,
            &mut |span| {
                let key = self.key(span)?;
                used.insert(key);
                Ok(key)
            },
        )?;
        self.export_records(vec![root], used)
    }

    /// Export a closed origin table independently of a token serialization.
    /// Attached syntax cannot be round-tripped through tokens without losing
    /// fragment boundaries. Its source map still needs these exact records.
    pub(crate) fn export_records(
        &self,
        roots: Vec<TokenTree>,
        used: BTreeSet<Key>,
    ) -> Result<TokenForest, AdapterError> {
        let mut contexts = BTreeSet::new();
        let mut records = Vec::new();
        for key in used {
            let record = self
                .records
                .get(&key)
                .ok_or_else(|| error("unknown exported span"))?;
            contexts.insert(record.context);
            records.push(record.clone());
        }
        let mut pending: Vec<_> = contexts.iter().copied().collect();
        while let Some(key) = pending.pop() {
            let record = self
                .contexts
                .get(&key)
                .ok_or_else(|| error("missing exported context ancestor"))?;
            if let Some(parent) = record.parent
                && contexts.insert(parent)
            {
                pending.push(parent);
            }
        }
        let contexts = contexts
            .into_iter()
            .map(|key| {
                self.contexts
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| error("unknown exported context"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let forest = TokenForest::new(roots, contexts, records)?;
        forest.encoded_len()?; // Enforce the complete output-record budget without allocating wire.
        Ok(forest)
    }

    /// The matched server can return actual ranged/resolved/located variants.
    /// Preserve those coordinates, remapping only registered anchors/contexts.
    /// Never treat an unknown file or hygiene slot as a caller span.
    pub(crate) fn export_proc(
        &mut self,
        tree: &tt::TopSubtree,
        origin: Key,
    ) -> Result<TokenForest, AdapterError> {
        let mut contexts: HashMap<span::SyntaxContext, Option<Key>> = HashMap::new();
        for key in self.contexts.keys() {
            let context = self.transport_context(*key)?;
            contexts
                .entry(context)
                .and_modify(|existing| {
                    if *existing != Some(*key) {
                        *existing = None;
                    }
                })
                .or_insert(Some(*key));
        }
        let mut text_bytes = 0usize;
        let mut register = |span: Span| -> Result<(), AdapterError> {
            if self.reverse.contains_key(&span) {
                return Ok(());
            }
            if self.records.len() >= 2_000_000 {
                return Err(error("procedural span table limit"));
            }
            let location = self
                .anchors
                .get(&span.anchor)
                .and_then(|key| self.records.get(key))
                .ok_or_else(|| error("procedural output has an unregistered source anchor"))?;
            let original = self
                .slots
                .get(&location.key())
                .ok_or_else(|| error("missing original source span slot"))?;
            let context = if original.ctx == span.ctx {
                location.context
            } else {
                contexts.get(&span.ctx).copied().flatten().ok_or_else(|| {
                    error("procedural output has an unregistered or ambiguous hygiene context")
                })?
            };
            text_bytes = text_bytes
                .checked_add(location.source.path.len())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("procedural span source metadata limit"))?;
            let record = SpanRecord {
                source: location.source.clone(),
                start: span.range.start().into(),
                end: span.range.end().into(),
                context,
                edition: location.edition,
                origin,
            };
            let key = record.key();
            self.records.insert(key, record);
            self.reverse.insert(span, key);
            Ok(())
        };
        for token in tree.view().as_token_trees().iter_flat_tokens() {
            match token {
                tt::TokenTree::Leaf(leaf) => register(*leaf.span())?,
                tt::TokenTree::Subtree(group) => {
                    register(group.delimiter.open)?;
                    register(group.delimiter.close)?;
                }
            }
        }
        self.export(tree)
    }
}
