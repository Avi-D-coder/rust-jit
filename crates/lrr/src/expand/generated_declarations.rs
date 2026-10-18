//! Exact-slot owned expansion syntax and unsealed nested declaration inventory.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        identity::{ExpansionKey, GrammarSlot},
        record::ExpansionRecord,
    },
    source::snapshot::CfgSet,
    syntax::{
        declaration_scan::{DeclarationScan, DeclarationScanLimits},
        parse::ParsedFile,
        spans::TokenOrigin,
        token_wire::TokenForest,
    },
};
use lrr_expand_adapter::declaration_output::{self, DeclarationGrammar};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(crate) fn declaration_grammar(grammar: GrammarSlot) -> DeclarationGrammar {
    match grammar {
        GrammarSlot::Items => DeclarationGrammar::Items,
        GrammarSlot::Expression => DeclarationGrammar::Expression,
        GrammarSlot::Statements => DeclarationGrammar::Statements,
        GrammarSlot::Type => DeclarationGrammar::Type,
        GrammarSlot::Pattern => DeclarationGrammar::Pattern,
        GrammarSlot::TraitItems => DeclarationGrammar::TraitItems,
        GrammarSlot::ImplItems => DeclarationGrammar::ImplItems,
    }
}
pub struct GeneratedDeclarations {
    record: Arc<ExpansionRecord>,
    parent: Digest,
    parent_invocation: Digest,
    epoch: u64,
    tokens: TokenForest,
    text: String,
    origins: Vec<TokenOrigin>,
    parsed: ParsedFile,
    scan: Arc<DeclarationScan>,
    headers: Vec<crate::expand::generated_items::GeneratedHeader>,
    definitions: BTreeMap<Digest, declaration_output::DefinitionCapture>,
    calls: BTreeMap<Digest, declaration_output::CallCapture>,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn binding(
    parent: Digest,
    parent_invocation: Digest,
    record: &ExpansionRecord,
    epoch: u64,
) -> Digest {
    let mut e = StableEncoder::new(GeneratedDeclarations::DOMAIN);
    e.digest(parent);
    e.digest(parent_invocation);
    e.record(&record.descriptor());
    e.u64(epoch);
    e.finish()
}
impl GeneratedDeclarations {
    /// Borrow the actual already-admitted immutable output tree. This grants no
    /// declaration-namespace completeness: consumers must join the real source,
    /// grammar, producer closure, scope, epoch and output record themselves.
    pub(crate) fn admitted_parsed(&self) -> &ParsedFile {
        &self.parsed
    }
    pub fn admit(
        expected: &ExpansionKey,
        record: Arc<ExpansionRecord>,
        epoch: u64,
        parent: &DeclarationScan,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
    ) -> Result<Self, Diagnostic> {
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.identity == expected.invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "declaration expansion parent producer absent",
                )
            })?;
        Self::admit_attached(expected, record, epoch, parent, cfg, limits, producer)
    }
    pub(crate) fn admit_item_output(
        record: Arc<ExpansionRecord>,
        epoch: u64,
        source: &crate::source::changes::PreparedSourceView,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
        site: &crate::syntax::token_wire::SpanRecord,
    ) -> Result<Self, Diagnostic> {
        let expected = record.key().clone();
        let call = source
            .outline()
            .item_invocations
            .iter()
            .cloned()
            .chain(
                source
                    .outline()
                    .items
                    .iter()
                    .filter_map(crate::syntax::outline::potential_attribute_producer),
            )
            .find(|call| call.item_input == expected.invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "original item invocation attachment absent",
                )
            })?;
        let parent = source.declaration_inventory(limits)?;
        let attribute = matches!(
            &expected.macro_identity,
            crate::expand::identity::MacroIdentity::Procedural {
                kind: crate::expand::identity::ProceduralKind::Attribute,
                ..
            }
        );
        let producer = parent
            .producers()
            .iter()
            .find(|producer| {
                producer.grammar == GrammarSlot::Items
                    && if attribute {
                        producer.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                            && call.range.start <= producer.range.start
                            && producer.range.end <= call.range.end
                            && producer.range.start <= site.start
                            && site.end <= producer.range.end
                    } else {
                        producer.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                            && producer.range == call.range
                    }
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "original item declaration producer attachment absent",
                )
            })?;
        Self::admit_attached(&expected, record, epoch, &parent, cfg, limits, producer)
    }
    pub(crate) fn admit_source_derive_output(
        selection: &crate::service::source_derive_requests::SourceDeriveSelection,
        record: Arc<ExpansionRecord>,
        epoch: u64,
        source: &crate::source::changes::PreparedSourceView,
        cfg: &CfgSet,
    ) -> Result<Self, Diagnostic> {
        let key = record.key().clone();
        let span = selection.declaration_site_span()?;
        if selection.source() != source.selection()
            || selection.input().identity() != key.invocation
            || !key.macro_identity.is_derive()
            || span.source.unit.0 != source.selection().outline.crate_key.0
            || span.source.content.0 != source.selection().outline.parse.content.0
            || span.source.path != source.selection().path.as_str()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "source derive declaration origin differs",
            ));
        }
        let parent = source.declaration_inventory(Default::default())?;
        let producer = parent
            .producers()
            .iter()
            .find(|p| {
                p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                    && p.range.start <= span.start
                    && span.end <= p.range.end
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "source derive declaration producer absent",
                )
            })?;
        Self::admit_attached(
            &key,
            record,
            epoch,
            &parent,
            cfg,
            Default::default(),
            producer,
        )
    }
    pub(crate) fn item_impl_headers(
        &self,
        items: &super::generated_items::GeneratedItems,
        cfg: &CfgSet,
        plan: &crate::frontend::generated_headers::GeneratedHeaderPlan,
    ) -> Result<Vec<(Digest, Digest)>, Diagnostic> {
        self.item_impl_headers_from_projection(items, cfg, plan.impl_projection())
    }
    /// Join the same retained active Outline facts after plan publication.
    /// Completion of the actual declaration job remains a separate authority.
    pub(crate) fn item_impl_headers_from_projection(
        &self,
        items: &super::generated_items::GeneratedItems,
        cfg: &CfgSet,
        projection: &crate::frontend::generated_headers::ActiveImplProjection,
    ) -> Result<Vec<(Digest, Digest)>, Diagnostic> {
        let active = projection.active_impl_ranges(items, cfg, self.scan.scope(), self.epoch)?;
        self.item_associated_headers(
            items,
            cfg,
            active,
            super::generated_items::GeneratedHeaderKind::Impl,
            |item, _, header, _| (item, header),
        )
    }
    pub(crate) fn item_trait_headers(
        &self,
        items: &super::generated_items::GeneratedItems,
        cfg: &CfgSet,
        plan: &crate::frontend::generated_headers::GeneratedHeaderPlan,
    ) -> Result<Vec<(Digest, Digest, Digest, crate::syntax::spans::SourceRange)>, Diagnostic> {
        let active = plan.active_trait_ranges(items, cfg, self.scan.scope(), self.epoch)?;
        self.item_associated_headers(
            items,
            cfg,
            active,
            super::generated_items::GeneratedHeaderKind::Trait,
            |item, identity, header, range| (item, identity, header, range),
        )
    }
    fn item_associated_headers<T>(
        &self,
        items: &super::generated_items::GeneratedItems,
        cfg: &CfgSet,
        active: &[crate::syntax::spans::SourceRange],
        kind: super::generated_items::GeneratedHeaderKind,
        row: impl Fn(Digest, Digest, Digest, crate::syntax::spans::SourceRange) -> T,
    ) -> Result<Vec<T>, Diagnostic> {
        if self.record.descriptor().fingerprint() != items.record().descriptor().fingerprint()
            || self.epoch != items.epoch()
            || self.scan.cfg() != cfg.fingerprint()
            || self.record.key().grammar != GrammarSlot::Items
            || self.text.as_str() != items.text()
            || self.origins.as_slice() != items.origins()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "impl item declaration owner differs",
            ));
        }
        // Both admitted facets use the same actual MacroItems source arena.
        if active
            .windows(2)
            .any(|pair| (pair[0].start, pair[0].end) >= (pair[1].start, pair[1].end))
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "active impl projection is duplicated or unordered",
            ));
        }
        active
            .len()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "impl item attachment budget"))?;
        let mut pairs = Vec::new();
        pairs
            .try_reserve_exact(active.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "impl item attachment allocation"))?;
        for range in active {
            let mut items = items.items().iter().filter(|item| {
                item.kind == kind && item.start == range.start && item.end == range.end
            });
            let item = items.next().ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "active impl raw syntax attachment absent",
                )
            })?;
            if items.next().is_some() {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "active impl raw attachment is ambiguous",
                ));
            }
            fn unique(
                mut rows: impl Iterator<Item = (Digest, Digest)>,
            ) -> Result<(Digest, Digest), Diagnostic> {
                let row = rows.next().ok_or_else(|| {
                    fault(
                        "LRR-E-EXPANSION-INPUT",
                        "owned associated declaration attachment absent",
                    )
                })?;
                if rows.next().is_some() {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "owned associated declaration attachment ambiguous",
                    ));
                }
                Ok(row)
            }
            let (identity, header) = match kind {
                super::generated_items::GeneratedHeaderKind::Impl => unique(
                    self.scan
                        .impls()
                        .iter()
                        .filter(|row| row.range == *range)
                        .map(|row| (row.identity, row.header_identity)),
                )?,
                super::generated_items::GeneratedHeaderKind::Trait => unique(
                    self.scan
                        .traits()
                        .iter()
                        .filter(|row| row.range == *range)
                        .map(|row| (row.identity, row.header_identity)),
                )?,
                _ => {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "associated declaration kind differs",
                    ));
                }
            };
            // Attribute/unsupported-context producer retirement remains the
            // completed declaration job's authority, not this range join.
            pairs.push(row(item.key.fingerprint(), identity, header, *range));
        }
        Ok(pairs)
    }
    pub(crate) fn attribute_item_range(
        &self,
        invocation: Digest,
    ) -> Result<crate::syntax::spans::SourceRange, Diagnostic> {
        self.optional_attribute_item_range(invocation)?
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "replacing attribute owning item absent",
                )
            })
    }
    pub(crate) fn optional_attribute_item_range(
        &self,
        invocation: Digest,
    ) -> Result<Option<crate::syntax::spans::SourceRange>, Diagnostic> {
        let producer = self
            .scan
            .producers()
            .iter()
            .find(|p| {
                p.identity == invocation
                    && p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "attribute declaration attachment absent",
                )
            })?;
        self.parsed.attribute_item_range_at(producer.range)
    }

    pub(crate) fn admit_completed_item_call(
        record: Arc<ExpansionRecord>,
        epoch: u64,
        parent: &DeclarationScan,
        invocation: Digest,
        cfg: &CfgSet,
    ) -> Result<Self, Diagnostic> {
        let producer = parent
            .producers()
            .iter()
            .find(|p| {
                p.identity == invocation
                    && (p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                        || (p.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                            && matches!(
                                &record.key().macro_identity,
                                crate::expand::identity::MacroIdentity::Procedural {
                                    kind: crate::expand::identity::ProceduralKind::Attribute
                                        | crate::expand::identity::ProceduralKind::Derive,
                                    ..
                                } | crate::expand::identity::MacroIdentity::BuiltinDerive { .. }
                            )))
                    && p.grammar == GrammarSlot::Items
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "completed item call attachment absent",
                )
            })?;
        let key = record.key().clone();
        Self::admit_attached(
            &key,
            record,
            epoch,
            parent,
            cfg,
            DeclarationScanLimits::default(),
            producer,
        )
    }
    fn admit_attached(
        expected: &ExpansionKey,
        record: Arc<ExpansionRecord>,
        epoch: u64,
        parent: &DeclarationScan,
        cfg: &CfgSet,
        limits: DeclarationScanLimits,
        producer: &crate::syntax::declaration_scan::DeclarationProducer,
    ) -> Result<Self, Diagnostic> {
        expected.validate()?;
        if epoch == 0
            || expected
                .procedural_epoch
                .is_some_and(|value| value != epoch)
        {
            return Err(fault(
                "LRR-E-EXPANSION-EPOCH",
                "declaration expansion epoch differs",
            ));
        }
        if record.key() != expected
            || producer.grammar != expected.grammar
            || cfg.fingerprint() != parent.cfg()
            // Generated calls retain their actual token-context edition, which
            // can differ from the enclosing fragment's parse edition. Their
            // captured request/owned-call bridge validates that exact context.
            || (parent.generated_origin().is_none()
                && expected.call_edition != parent.parse().profile.edition)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "declaration expansion key/slot/source/cfg differs",
            ));
        }
        let grammar = declaration_grammar(expected.grammar);
        let tokens = record.tokens()?;
        let adapter_tokens = lrr_expand_adapter::token_wire::TokenForest::decode(record.wire())
            .map_err(|e| fault(e.code, &e.message))?;
        let (output, raw_headers) =
            declaration_output::materialize_with_headers(&adapter_tokens, grammar)
                .map_err(|e| fault(e.code, &e.message))?;
        let headers = crate::expand::generated_items::retain_generated_headers(
            expected,
            &output.text,
            &output.origins,
            raw_headers,
        )?;
        tokens
            .spans()
            .len()
            .checked_mul(128)
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "declaration span index budget"))?;
        let spans = tokens
            .spans()
            .iter()
            .map(|s| s.key().0)
            .collect::<BTreeSet<_>>();
        if output.origins.iter().any(|o| !spans.contains(&o.span.0)) {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "declaration syntax origin is absent from output tokens",
            ));
        }
        let root_kind = if expected.grammar == GrammarSlot::Items {
            Some(
                parent
                    .lexical_scopes()
                    .iter()
                    .find(|scope| scope.identity == producer.lexical_scope)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "generated root scope attachment absent",
                        )
                    })?
                    .kind,
            )
        } else {
            None
        };
        let path = parent.path().clone();
        let parent = parent.fingerprint();
        let identity = binding(parent, producer.identity, &record, epoch);
        let parsed = ParsedFile::from_generated_declarations(
            &output.syntax,
            output.text.len(),
            identity,
            expected.call_edition,
            expected.grammar,
        )?
        .with_generated_token_editions(&adapter_tokens, &output.origins)?;
        // Items inherit their retained insertion scope. Other grammar slots
        // keep the scanner's explicit grammar-derived root kind.
        let scan = Arc::new(if let Some(kind) = root_kind {
            parsed.scan_declarations_at_root(path, producer.scope.clone(), cfg, limits, kind)?
        } else {
            parsed.scan_declarations(path, producer.scope.clone(), cfg, limits)?
        });
        let mut definition_rows = BTreeMap::new();
        for definition in output.definitions {
            if !spans.contains(&definition.name_span.0)
                || definition_rows
                    .insert((definition.start, definition.end), definition)
                    .is_some()
            {
                return Err(fault(
                    "LRR-E-MACRO-OUTPUT",
                    "generated definition capture attachment differs",
                ));
            }
        }
        let mut call_rows = BTreeMap::new();
        for call in output.calls {
            if !spans.contains(&call.name_span.0) {
                return Err(fault(
                    "LRR-E-MACRO-OUTPUT",
                    "generated call name span absent from output tokens",
                ));
            }
            if call_rows.insert((call.start, call.end), call).is_some() {
                return Err(fault(
                    "LRR-E-MACRO-OUTPUT",
                    "duplicate generated call capture attachment",
                ));
            }
        }
        let mut definitions = BTreeMap::new();
        for header in scan.macro_definitions() {
            let definition = definition_rows
                .remove(&(header.range.start, header.range.end))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-MACRO-OUTPUT",
                        "active generated definition capture absent",
                    )
                })?;
            definitions.insert(header.identity, definition);
        }
        let mut calls = BTreeMap::new();
        for producer in scan
            .producers()
            .iter()
            .filter(|p| p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
        {
            let call = call_rows
                .remove(&(producer.range.start, producer.range.end))
                .ok_or_else(|| {
                    fault("LRR-E-MACRO-OUTPUT", "active generated call capture absent")
                })?;
            calls.insert(producer.identity, call);
        }
        let value = Self {
            record,
            parent,
            parent_invocation: producer.identity,
            epoch,
            tokens,
            text: output.text,
            origins: crate::expand::generated_items::retain_token_origins(output.origins)?,
            parsed,
            scan,
            headers,
            definitions,
            calls,
        };
        Ok(value)
    }
    pub fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    /// Actual pinned-AST header captures from this admitted record/grammar.
    /// Facts alone grant neither enclosing owner nor member/call authority.
    pub(crate) fn headers(&self) -> &[crate::expand::generated_items::GeneratedHeader] {
        &self.headers
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn origins(&self) -> &[TokenOrigin] {
        &self.origins
    }
    pub fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
    pub(crate) fn hygiene_facts(
        &self,
    ) -> Result<&crate::frontend::hygiene::HygieneFacts, Diagnostic> {
        self.parsed.hygiene_facts().map(Arc::as_ref).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "generated declaration parser hygiene facts absent",
            )
        })
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn record(&self) -> &ExpansionRecord {
        &self.record
    }
    pub(crate) fn record_handle(&self) -> Arc<ExpansionRecord> {
        self.record.clone()
    }
    pub fn parent_invocation(&self) -> Digest {
        self.parent_invocation
    }
    pub fn parent_inventory(&self) -> Digest {
        self.parent
    }
    /// Original name tokens for one retained generated module row. This is
    /// physical token transport, not namespace publication or name authority.
    pub fn module_name_tokens(
        &self,
        identity: Digest,
    ) -> Result<lrr_expand_adapter::token_wire::TokenForest, Diagnostic> {
        let row = self
            .scan
            .modules()
            .iter()
            .find(|row| row.identity == identity)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "active generated module capture absent",
                )
            })?;
        let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(self.record.wire())
            .map_err(|e| fault(e.code, &e.message))?;
        lrr_expand_adapter::generated_attribute::module_binding_forest_for_grammar(
            &tokens,
            row.range.start,
            row.range.end,
            declaration_grammar(self.record.key().grammar),
        )
        .map_err(|e| fault(e.code, &e.message))
    }
    /// Batched physical transport: decode and replay the pinned syntax once.
    /// This remains metadata capture and grants no authority or completion.
    pub(crate) fn module_name_token_forests(
        &self,
    ) -> Result<Vec<lrr_expand_adapter::token_wire::TokenForest>, Diagnostic> {
        if self.scan.modules().is_empty() {
            return Ok(Vec::new());
        }
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(self.scan.modules().len())
            .map_err(|_| fault("LRR-E-EXPANSION-INPUT", "module capture allocation limit"))?;
        ranges.extend(
            self.scan
                .modules()
                .iter()
                .map(|row| (row.range.start, row.range.end)),
        );
        let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(self.record.wire())
            .map_err(|e| fault(e.code, &e.message))?;
        lrr_expand_adapter::generated_attribute::module_binding_forests_for_grammar(
            &tokens,
            &ranges,
            declaration_grammar(self.record.key().grammar),
        )
        .map_err(|e| fault(e.code, &e.message))
    }
    pub fn macro_definition(
        &self,
        identity: Digest,
    ) -> Result<&declaration_output::DefinitionCapture, Diagnostic> {
        self.definitions.get(&identity).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "active generated macro definition capture absent",
            )
        })
    }
    pub fn macro_call(
        &self,
        identity: Digest,
    ) -> Result<&declaration_output::CallCapture, Diagnostic> {
        self.calls.get(&identity).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "active generated macro call capture absent",
            )
        })
    }
    pub(crate) fn attribute_facts(
        &self,
        identity: Digest,
        cfg: &CfgSet,
    ) -> Result<(crate::syntax::attributes::ItemAttributeInputs, usize), Diagnostic> {
        if cfg.fingerprint() != self.scan.cfg() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated attribute cfg differs",
            ));
        }
        let row = self
            .scan
            .producers()
            .iter()
            .find(|p| {
                p.identity == identity
                    && p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated attribute producer absent",
                )
            })?;
        let range = self
            .parsed
            .attribute_item_range_at(row.range)?
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated attribute item absent"))?;
        let facts = self.parsed.attribute_inputs_at(range, cfg)?;
        if !facts.enabled {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated attribute inactive or excessive",
            ));
        }
        let at = facts
            .attributes
            .iter()
            .position(|a| a.range == row.range)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated effective attribute absent",
                )
            })?;
        Ok((facts, at))
    }
    /// Check the exact captured attachment after compiler Test facet selection.
    /// Signature and body preparation remain deferred to their own queries.
    pub(crate) fn validate_compiler_test_target(
        &self,
        identity: Digest,
        cfg: &CfgSet,
    ) -> Result<(), Diagnostic> {
        let (facts, at) = self.attribute_facts(identity, cfg)?;
        let target = self
            .scan
            .producers()
            .iter()
            .find(|row| row.identity == identity)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated Test target producer absent",
                )
            })?;
        let insertion = self
            .scan
            .lexical_scopes()
            .iter()
            .find(|scope| scope.identity == target.lexical_scope)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated Test target lexical attachment absent",
                )
            })?;
        // MACRO_ITEMS is shared by module and associated output parsing. Its
        // direct Fn syntax cannot grant free-function eligibility: the exact
        // captured insertion scope remains authoritative. Nested local items
        // keep their own lexical scope rather than inheriting this refusal.
        if insertion.kind != crate::syntax::declaration_scan::DeclarationScopeKind::AssociatedItems
            && self
                .parsed
                .attribute_targets_free_function(facts.attributes[at].range)?
        {
            return Ok(());
        }
        Err(fault(
            "LRR-E-TEST-DISCOVERY",
            "compiler Test attribute requires a free function",
        ))
    }
    pub(crate) fn attribute_lookup_path(
        &self,
        identity: Digest,
    ) -> Result<(Vec<crate::syntax::imports::ImportSegment>, bool), Diagnostic> {
        let row = self
            .scan
            .producers()
            .iter()
            .find(|row| {
                row.identity == identity
                    && row.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated attribute lookup attachment absent",
                )
            })?;
        self.parsed.attribute_lookup_path_at(row.range)
    }
    pub(crate) fn inert_attribute_definition(
        &self,
        identity: Digest,
        cfg: &CfgSet,
    ) -> Result<Option<Digest>, Diagnostic> {
        let root_kind = self
            .scan
            .lexical_scopes()
            .first()
            .filter(|scope| scope.parent.is_none())
            .ok_or_else(|| fault("LRR-E-INTERNAL-SNAPSHOT", "generated lexical root absent"))?
            .kind;
        let actual = self.parsed.scan_declarations_at_root(
            self.scan.path().clone(),
            self.scan.scope().clone(),
            cfg,
            Default::default(),
            root_kind,
        )?;
        if actual.fingerprint() != self.scan.fingerprint() {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "generated inert source scan differs",
            ));
        }
        super::nested_inert::selected_definition(&actual, identity)
    }
    pub(crate) fn macro_syntax(
        &self,
        identity: Digest,
    ) -> Result<crate::syntax::macro_calls::SourceMacroCall, Diagnostic> {
        let producer = self
            .scan
            .producers()
            .iter()
            .find(|producer| producer.identity == identity)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated invocation producer absent",
                )
            })?;
        // Generated output may name `$crate`; its meaning comes from the token's
        // definition-site hygiene, resolved by `dollar_crate_unit`.
        self.parsed.generated_macro_call_at(producer.range)
    }
    /// The defining crate of a generated call path's leading `$crate` token.
    pub(crate) fn dollar_crate_unit(&self, identity: Digest) -> Result<Digest, Diagnostic> {
        use lrr_expand_adapter::token_wire::{Delimiter, TokenTree};
        let call = self.macro_call(identity)?;
        // The captured path may sit inside invisible (macro-fragment) groups.
        let mut first = call.path.roots().first();
        while let Some(TokenTree::Group {
            delimiter: Delimiter::Invisible,
            children,
            ..
        }) = first
        {
            first = children.first();
        }
        let span = match first {
            Some(TokenTree::Ident { text, span, .. }) if text == "$crate" => *span,
            _ => {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated dollar-crate path token absent",
                ));
            }
        };
        let context = call
            .path
            .spans()
            .iter()
            .find(|record| record.key() == span)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated dollar-crate token span absent",
                )
            })?
            .context;
        Ok(Digest(
            self.hygiene_facts()?
                .dollar_crate(crate::syntax::token_wire::Key(context.0))?
                .0,
        ))
    }
    pub(crate) fn intrinsic_arguments(
        &self,
        identity: Digest,
        definition: crate::expand::scopes::MacroDefinition,
        cfg: &CfgSet,
    ) -> Result<Vec<Arc<super::argument_declarations::ArgumentDeclarations>>, Diagnostic> {
        self.runtime_argument_tokens(identity)?
            .into_iter()
            .enumerate()
            .map(|(ordinal, tokens)| {
                super::argument_declarations::ArgumentDeclarations::from_generated(
                    &self.scan,
                    identity,
                    definition.clone(),
                    ordinal,
                    tokens,
                    cfg,
                )
                .map(Arc::new)
            })
            .collect()
    }
    pub(crate) fn intrinsic_argument_scans(
        &self,
        identity: Digest,
        cfg: &CfgSet,
        kind: crate::facade::macros::RuntimeMacroKind,
    ) -> Result<Vec<Arc<DeclarationScan>>, Diagnostic> {
        if cfg.fingerprint() != self.scan.cfg() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated argument cfg differs",
            ));
        }
        let producer = self
            .scan
            .producers()
            .iter()
            .find(|p| p.identity == identity)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated argument producer absent",
                )
            })?;
        // Original-source vec uses exact list/repeat AST capture; generated token origins
        // require their own splitter before any declaration/no-item admission.
        if kind == crate::facade::macros::RuntimeMacroKind::Vec {
            return Err(fault(
                "LRR-E-UNSUPPORTED",
                "generated vec declaration capture requires token-aware list/repeat splitting",
            ));
        }
        let tokens = self.runtime_argument_tokens(identity)?;
        let minimum = match kind {
            crate::facade::macros::RuntimeMacroKind::Panic
            | crate::facade::macros::RuntimeMacroKind::Println
            | crate::facade::macros::RuntimeMacroKind::Vec
            | crate::facade::macros::RuntimeMacroKind::Unreachable
            | crate::facade::macros::RuntimeMacroKind::Unimplemented => 0,
            crate::facade::macros::RuntimeMacroKind::Assert => 1,
            crate::facade::macros::RuntimeMacroKind::AssertEq
            | crate::facade::macros::RuntimeMacroKind::AssertNe => 2,
        };
        if tokens.len() < minimum {
            return Err(fault(
                "LRR-E-RUNTIME-MACRO-ARGUMENTS",
                "generated assertion lacks required operands",
            ));
        }
        let mut scans = Vec::new();
        scans
            .try_reserve_exact(tokens.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "generated argument scan allocation"))?;
        let mut bytes = 0usize;
        for (ordinal, tokens) in tokens.iter().enumerate() {
            let mut e = StableEncoder::new("lrr/v2/generated-intrinsic-argument-syntax");
            e.digest(self.fingerprint());
            e.digest(identity);
            e.u64(
                u64::try_from(ordinal)
                    .map_err(|_| fault("LRR-E-METADATA-LIMIT", "argument ordinal overflow"))?,
            );
            e.digest(Digest::of_bytes(
                &tokens.encode().map_err(|e| fault(e.code, &e.message))?,
            ));
            let syntax = declaration_output::materialize(tokens, DeclarationGrammar::Expression)
                .map_err(|e| fault(e.code, &e.message))?;
            let parsed = ParsedFile::from_generated_declarations(
                &syntax.syntax,
                syntax.text.len(),
                e.finish(),
                self.scan.parse().profile.edition,
                GrammarSlot::Expression,
            )?
            .with_generated_token_editions(tokens, &syntax.origins)?;
            let scan = Arc::new(parsed.scan_declarations(
                self.scan.path().clone(),
                producer.scope.clone(),
                cfg,
                DeclarationScanLimits::default(),
            )?);
            bytes = bytes
                .checked_add(scan.retained_bytes())
                .ok_or_else(|| {
                    fault("LRR-E-METADATA-LIMIT", "generated argument inventory limit")
                })?;
            scans.push(scan);
        }
        Ok(scans)
    }
    pub fn runtime_argument_tokens(
        &self,
        identity: Digest,
    ) -> Result<Vec<lrr_expand_adapter::token_wire::TokenForest>, Diagnostic> {
        let call = self.macro_call(identity)?;
        lrr_expand_adapter::runtime_arguments::expressions(&call.input)
            .map_err(|e| fault(e.code, &e.message))
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let token_bytes = self
            .tokens
            .allocated_bytes()
            .map_err(|e| fault(e.code, &e.message))?;
        let mut captures = self
            .headers
            .capacity()
            .checked_mul(std::mem::size_of::<
                crate::expand::generated_items::GeneratedHeader,
            >())
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "generated header metadata overflow"))?;
        for header in &self.headers {
            captures = captures
                .checked_add(
                    header
                        .key
                        .structural_path
                        .capacity()
                        .checked_mul(4)
                        .ok_or_else(|| {
                            fault("LRR-E-METADATA-LIMIT", "generated header path overflow")
                        })?,
                )
                .and_then(|n| {
                    n.checked_add(header.name.as_ref().map_or(0, |name| name.text.capacity()))
                })
                .ok_or_else(|| {
                    fault("LRR-E-METADATA-LIMIT", "generated header metadata overflow")
                })?;
        }
        for definition in self.definitions.values() {
            let bytes = definition
                .body
                .allocated_bytes()
                .map_err(|e| fault(e.code, &e.message))?;
            captures = captures
                .checked_add(bytes)
                .and_then(|n| n.checked_add(256))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "definition capture metadata overflow",
                    )
                })?;
        }
        for call in self.calls.values() {
            let path = call
                .path
                .allocated_bytes()
                .map_err(|e| fault(e.code, &e.message))?;
            let input = call
                .input
                .allocated_bytes()
                .map_err(|e| fault(e.code, &e.message))?;
            captures = captures
                .checked_add(path)
                .and_then(|n| n.checked_add(input))
                .and_then(|n| n.checked_add(288))
                .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "call capture metadata overflow"))?;
        }
        std::mem::size_of::<Self>()
            .checked_add(self.record.descriptor().wire_bytes)
            .and_then(|n| n.checked_add(token_bytes))
            .and_then(|n| n.checked_add(self.text.capacity()))
            .and_then(|n| {
                n.checked_add(
                    self.origins
                        .capacity()
                        .checked_mul(std::mem::size_of::<TokenOrigin>())?,
                )
            })
            .and_then(|n| n.checked_add(self.parsed.estimated_bytes()))
            .and_then(|n| n.checked_add(self.scan.retained_bytes()))
            .and_then(|n| n.checked_add(captures))
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "declaration syntax metadata overflow",
                )
            })
    }
}
impl StableEncode for GeneratedDeclarations {
    const DOMAIN: &'static str = "lrr/v2/generated-declaration-input";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.parent);
        e.digest(self.parent_invocation);
        e.record(&self.record.descriptor());
        e.u64(self.epoch);
    }
}

#[cfg(test)]
#[path = "generated_declarations_tests.rs"]
mod scope_replay_tests;
