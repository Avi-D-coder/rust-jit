//! Exact-key, epoch-checked generated item syntax. No macro execution or publication.
use crate::syntax::spans::TokenOrigin;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        identity::{ExpansionKey, GeneratedItemKey, GrammarSlot},
        record::ExpansionRecord,
    },
    syntax::token_wire::TokenForest,
};
pub use lrr_expand_adapter::item_output::ItemKind as GeneratedHeaderKind;
use lrr_expand_adapter::item_output::{self, ItemKind, Name};
use std::sync::Arc;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedHeader {
    pub key: GeneratedItemKey,
    pub kind: ItemKind,
    pub name: Option<Name>,
    pub start: u32,
    pub end: u32,
    pub header_start: u32,
    pub header_end: u32,
}
#[derive(Debug, Clone)]
pub struct GeneratedCall {
    pub key: GeneratedItemKey,
    pub path: TokenForest,
    pub input: TokenForest,
    pub site: crate::syntax::token_wire::Key,
    pub edition: u16,
}
#[derive(Debug, Clone)]
pub struct GeneratedItems {
    record: Arc<ExpansionRecord>,
    tokens: TokenForest,
    text: String,
    origins: Vec<TokenOrigin>,
    items: Vec<GeneratedHeader>,
    epoch: u64,
    calls: Vec<GeneratedCall>,
    macro_bodies: std::collections::BTreeMap<Digest, (u16, TokenForest)>,
    hygiene: Arc<crate::frontend::hygiene::HygieneFacts>,
    syntax: Vec<item_output::SyntaxEvent>,
    item_classes: std::collections::BTreeMap<
        crate::syntax::token_wire::Key,
        crate::syntax::token_wire::NameContextKey,
    >,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub(crate) fn retain_token_origins(
    rows: Vec<item_output::Origin>,
) -> Result<Vec<TokenOrigin>, Diagnostic> {
    rows.len()
        .checked_mul(std::mem::size_of::<TokenOrigin>())
        .filter(|bytes| *bytes <= 256 * 1024 * 1024)
        .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "generated origin retention budget"))?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(rows.len())
        .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated origin allocation"))?;
    for row in rows {
        owned.push(TokenOrigin {
            start: row.start,
            end: row.end,
            span: crate::syntax::token_wire::Key(row.span.0),
        });
    }
    Ok(owned)
}
impl GeneratedItems {
    pub fn admit(
        expected: &ExpansionKey,
        record: Arc<ExpansionRecord>,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        expected.validate()?;
        if epoch == 0 || expected.procedural_epoch.is_some_and(|e| e != epoch) {
            return Err(fault(
                "LRR-E-EXPANSION-EPOCH",
                "generated item record epoch differs from current discovery",
            ));
        }
        if record.key() != expected || expected.grammar != GrammarSlot::Items {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated item record does not match exact demanded item key",
            ));
        }
        if record.descriptor().wire_bytes > 32 * 1024 * 1024 {
            return Err(fault(
                "LRR-E-DISCOVERY-LIMIT",
                "generated item wire size limit",
            ));
        }
        let tokens = record.tokens()?;
        let adapter_tokens = lrr_expand_adapter::token_wire::TokenForest::decode(record.wire())
            .map_err(|e| fault(e.code, &e.message))?;
        let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
        hygiene
            .import(adapter_tokens.contexts())
            .map_err(|e| fault(e.code, &e.message))?;
        let demanded = adapter_tokens
            .contexts()
            .iter()
            .map(|c| c.key())
            .collect::<Vec<_>>();
        let rows = hygiene
            .resolution_contexts(&demanded)
            .map_err(|e| fault(e.code, &e.message))?;
        let names = hygiene
            .name_contexts(&demanded)
            .map_err(|e| fault(e.code, &e.message))?;
        let item_classes = names
            .iter()
            .map(|row| {
                (
                    crate::syntax::token_wire::Key(row.original.0),
                    crate::syntax::token_wire::NameContextKey(crate::syntax::token_wire::Key(
                        row.item.0.0,
                    )),
                )
            })
            .collect();
        let contexts_wire =
            lrr_expand_adapter::token_wire::TokenForest::new(vec![], hygiene.records(), vec![])
                .and_then(|f| f.encode())
                .map_err(|e| fault(e.code, &e.message))?;
        let contexts =
            TokenForest::decode(&contexts_wire).map_err(|e| fault(e.code, &e.message))?;
        let hygiene = Arc::new(
            crate::frontend::hygiene::HygieneFacts::new(
                contexts.contexts().to_vec(),
                rows.into_iter()
                    .map(|r| crate::syntax::token_wire::ResolutionContext {
                        original: crate::syntax::token_wire::Key(r.original.0),
                        item: crate::syntax::token_wire::Key(r.item.0),
                        local: crate::syntax::token_wire::Key(r.local.0),
                    })
                    .collect(),
            )?
            .with_name_contexts(
                names
                    .into_iter()
                    .map(|r| crate::syntax::token_wire::NameResolutionContext {
                        original: crate::syntax::token_wire::Key(r.original.0),
                        item: crate::syntax::token_wire::NameContextKey(
                            crate::syntax::token_wire::Key(r.item.0.0),
                        ),
                        local: crate::syntax::token_wire::NameContextKey(
                            crate::syntax::token_wire::Key(r.local.0.0),
                        ),
                    })
                    .collect(),
            )?,
        );
        let parsed =
            item_output::materialize(&adapter_tokens).map_err(|e| fault(e.code, &e.message))?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(parsed.items.len())
            .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated header allocation"))?;
        for item in parsed.items {
            let mut e = StableEncoder::new("lrr/v2/generated-header");
            e.u32(item.kind.tag());
            e.string(&parsed.text[item.header_start as usize..item.header_end as usize]);
            let first = parsed
                .origins
                .partition_point(|o| o.end <= item.header_start);
            let last = parsed
                .origins
                .partition_point(|o| o.start < item.header_end);
            e.u64(
                u64::try_from(last - first)
                    .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated header origin count"))?,
            );
            for origin in &parsed.origins[first..last] {
                e.u32(origin.start.max(item.header_start) - item.header_start);
                e.u32(origin.end.min(item.header_end) - item.header_start);
                e.digest(Digest(origin.span.0));
            }
            let key = GeneratedItemKey {
                expansion: expected.fingerprint(),
                structural_path: item.structural_path,
                header: e.finish(),
            };
            key.validate()?;
            items.push(GeneratedHeader {
                key,
                kind: item.kind,
                name: item.name,
                start: item.start,
                end: item.end,
                header_start: item.header_start,
                header_end: item.header_end,
            });
        }
        let mut macro_bodies = std::collections::BTreeMap::new();
        for body in parsed.macros {
            let header = items
                .get(body.ordinal as usize)
                .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated macro header absent"))?;
            let wire = body
                .tokens
                .encode()
                .map_err(|e| fault(e.code, &e.message))?;
            let tokens = TokenForest::decode(&wire).map_err(|e| fault(e.code, &e.message))?;
            macro_bodies.insert(header.key.fingerprint(), (body.edition, tokens));
        }
        let mut calls = Vec::new();
        calls
            .try_reserve_exact(parsed.calls.len())
            .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated call allocation"))?;
        for call in parsed.calls {
            let header = items
                .get(call.ordinal as usize)
                .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated call header absent"))?;
            let path =
                TokenForest::decode(&call.path.encode().map_err(|e| fault(e.code, &e.message))?)
                    .map_err(|e| fault(e.code, &e.message))?;
            let input =
                TokenForest::decode(&call.input.encode().map_err(|e| fault(e.code, &e.message))?)
                    .map_err(|e| fault(e.code, &e.message))?;
            calls.push(GeneratedCall {
                key: header.key.clone(),
                path,
                input,
                site: crate::syntax::token_wire::Key(call.site.0),
                edition: call.edition,
            });
        }
        Ok(Self {
            record,
            tokens,
            text: parsed.text,
            origins: retain_token_origins(parsed.origins)?,
            items,
            epoch,
            syntax: parsed.syntax,
            item_classes,
            macro_bodies,
            calls,
            hygiene,
        })
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let fail = || fault("LRR-E-METADATA-LIMIT", "generated syntax metadata overflow");
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.record.estimated_bytes()?)
            .and_then(|n| n.checked_add(self.text.capacity()))
            .ok_or_else(fail)?;
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes.checked_add(n).ok_or_else(fail)?;
            Ok(())
        };
        add(self
            .tokens
            .allocated_bytes()
            .map_err(|e| fault("LRR-E-METADATA-LIMIT", &e.to_string()))?)?;
        add(self
            .origins
            .capacity()
            .checked_mul(std::mem::size_of::<TokenOrigin>())
            .ok_or_else(fail)?)?;
        add(self
            .items
            .capacity()
            .checked_mul(std::mem::size_of::<GeneratedHeader>())
            .ok_or_else(fail)?)?;
        for header in &self.items {
            add(header
                .key
                .structural_path
                .capacity()
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(fail)?)?;
            if let Some(name) = &header.name {
                add(name.text.capacity())?;
            }
        }
        add(self
            .calls
            .capacity()
            .checked_mul(std::mem::size_of::<GeneratedCall>())
            .ok_or_else(fail)?)?;
        for call in &self.calls {
            add(call
                .path
                .allocated_bytes()
                .map_err(|e| fault("LRR-E-METADATA-LIMIT", &e.to_string()))?)?;
            add(call
                .input
                .allocated_bytes()
                .map_err(|e| fault("LRR-E-METADATA-LIMIT", &e.to_string()))?)?;
            add(call
                .key
                .structural_path
                .capacity()
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(fail)?)?;
        }
        for (_, tokens) in self.macro_bodies.values() {
            add(128)?;
            add(tokens
                .allocated_bytes()
                .map_err(|e| fault("LRR-E-METADATA-LIMIT", &e.to_string()))?)?;
        }
        add(self.item_classes.len().checked_mul(160).ok_or_else(fail)?)?;
        add(self.hygiene.metadata_bytes()?)?;
        add(self
            .syntax
            .capacity()
            .checked_mul(std::mem::size_of::<item_output::SyntaxEvent>())
            .ok_or_else(fail)?)?;
        for event in &self.syntax {
            if let item_output::SyntaxEvent::Token { text, .. } = event {
                add(text.capacity())?;
            }
        }
        Ok(bytes)
    }
    pub(crate) fn hygiene_facts(&self) -> Arc<crate::frontend::hygiene::HygieneFacts> {
        self.hygiene.clone()
    }
    pub(crate) fn macro_edition(&self, id: Digest) -> Result<u16, Diagnostic> {
        self.macro_bodies
            .get(&id)
            .map(|b| b.0)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated macro body absent"))
    }
    pub(crate) fn expand_macro_body(
        &self,
        id: Digest,
        owner: Digest,
        key: &ExpansionKey,
        input: &TokenForest,
        site: crate::syntax::token_wire::Key,
    ) -> Result<ExpansionRecord, Diagnostic> {
        key.validate()?;
        let (edition, body) = self
            .macro_bodies
            .get(&id)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated macro body absent"))?;
        let call_site = input
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated macro call site absent"))?;
        let call_edition = match call_site.edition {
            crate::syntax::token_wire::Edition::E2015 => 2015,
            crate::syntax::token_wire::Edition::E2018 => 2018,
            crate::syntax::token_wire::Edition::E2021 => 2021,
            crate::syntax::token_wire::Edition::E2024 => 2024,
        };
        if key.parent_context != Digest(call_site.context.0) || key.call_edition != call_edition {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated macro call site provenance differs",
            ));
        }
        let wire = input.encode().map_err(|e| fault(e.code, &e.message))?;
        if key.macro_identity
            != (crate::expand::identity::MacroIdentity::Declaration { definition: id })
            || key.definition_edition != *edition
            || key.input_tokens != Digest::of_bytes(&wire)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated macro demand differs",
            ));
        }
        let body_wire = body.encode().map_err(|e| fault(e.code, &e.message))?;
        let body = lrr_expand_adapter::token_wire::TokenForest::decode(&body_wire)
            .map_err(|e| fault(e.code, &e.message))?;
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
            .map_err(|e| fault(e.code, &e.message))?;
        let definition = lrr_expand_adapter::declarative::DeclarativeMacro::new(body)
            .map_err(|e| fault(e.code, &e.message))?;
        let edition = match *edition {
            2015 => lrr_expand_adapter::token_wire::Edition::E2015,
            2018 => lrr_expand_adapter::token_wire::Edition::E2018,
            2021 => lrr_expand_adapter::token_wire::Edition::E2021,
            2024 => lrr_expand_adapter::token_wire::Edition::E2024,
            _ => return Err(fault("LRR-E-EXPANSION-INPUT", "generated macro edition")),
        };
        let (result, _) = definition
            .expand_hygienic_with_limits(
                &input,
                lrr_expand_adapter::token_wire::Key(site.0),
                lrr_expand_adapter::declarative::DeclarativeSite {
                    expansion: lrr_expand_adapter::token_wire::Key(key.fingerprint().0),
                    defining_unit: lrr_expand_adapter::token_wire::Key(owner.0),
                    definition_edition: edition,
                },
                2_000_000,
                32 * 1024 * 1024,
            )
            .map_err(|e| fault(e.code, &e.message))?;
        let output = result.map_err(|e| fault("LRR-E-MACRO-EXPANSION", &e.to_string()))?;
        ExpansionRecord::from_wire(
            key.clone(),
            output
                .tokens
                .encode()
                .map_err(|e| fault(e.code, &e.message))?,
        )
    }
    pub(crate) fn item_class(
        &self,
        context: crate::syntax::token_wire::Key,
    ) -> Result<crate::syntax::token_wire::NameContextKey, Diagnostic> {
        self.item_classes.get(&context).copied().ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "generated item comparison class absent",
            )
        })
    }
    pub(crate) fn syntax_events(&self) -> &[item_output::SyntaxEvent] {
        &self.syntax
    }
    /// Raw top-level syntax facts; cfg and macro selection belong to discovery.
    pub fn calls(&self) -> &[GeneratedCall] {
        &self.calls
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn origins(&self) -> &[TokenOrigin] {
        &self.origins
    }
    /// Project exact generated-arena offsets to retained token spans. A range
    /// may select several source origins; callers must not invent a combined
    /// source range or equate token context ancestry with producer ancestry.
    /// Empty output means no mapped token (for example inserted whitespace).
    pub fn project_source_spans(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Vec<&crate::syntax::token_wire::SpanRecord>, Diagnostic> {
        crate::syntax::spans::project_origin_spans(
            &self.text,
            &self.origins,
            range,
            |origin| (origin.start, origin.end),
            |origin| {
                let spans = self.tokens.spans();
                spans
                    .binary_search_by_key(&origin.span, |span| span.key())
                    .map(|index| &spans[index])
                    .map_err(|_| {
                        fault(
                            "LRR-E-INTERNAL-SOURCE-MAP",
                            "diagnostic token span is absent",
                        )
                    })
            },
        )
    }

    /// Attach a reusable body-relative span only through the current generated
    /// declaration and exact body bytes. No parsing or body query is performed.
    pub fn project_body_source_spans(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        span: &crate::syntax::spans::BodySpan,
    ) -> Result<Vec<&crate::syntax::token_wire::SpanRecord>, Diagnostic> {
        use crate::syntax::spans::SourceRange;
        let invalid = || {
            fault(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "generated body origin identity or extent differs",
            )
        };
        if body.key() != span.body_key
            || body.entry_blocked
            || !self.items.iter().any(|item| {
                item.key.fingerprint() == body.def_key
                    && item.start <= body.range.start
                    && body.range.end <= item.end
            })
        {
            return Err(invalid());
        }
        SourceRange::checked(body.range.start, body.range.end)?;
        let bytes = self
            .text
            .get(body.range.start as usize..body.range.end as usize)
            .ok_or_else(invalid)?;
        if Digest::of_bytes(bytes.as_bytes()) != body.body_hash
            || span.relative_start > span.relative_end
            || span.relative_end > body.range.len()
        {
            return Err(invalid());
        }
        self.project_source_spans(SourceRange {
            start: body
                .range
                .start
                .checked_add(span.relative_start)
                .ok_or_else(invalid)?,
            end: body
                .range
                .start
                .checked_add(span.relative_end)
                .ok_or_else(invalid)?,
        })
    }
    /// Declaration-only view of the same immutable owned item output. This
    /// neither retires nested producers nor prepares any executable body.
    pub fn declaration_facet(
        &self,
        request: &super::source_declarative::SourceDeclarativeRequest,
        source: &crate::source::changes::PreparedSourceView,
        cfg: &crate::source::snapshot::CfgSet,
        limits: crate::syntax::declaration_scan::DeclarationScanLimits,
    ) -> Result<super::generated_declarations::GeneratedDeclarations, Diagnostic> {
        let base = request.source_base()?;
        let selected = source.selection();
        if request.key() != self.record.key()
            || base.source.unit.0 != selected.outline.crate_key.0
            || base.source.content.0 != selected.outline.parse.content.0
            || base.source.path != selected.path.as_str()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "item declaration source/request attachment differs",
            ));
        }
        super::generated_declarations::GeneratedDeclarations::admit_item_output(
            self.record.clone(),
            self.epoch,
            source,
            cfg,
            limits,
            &base,
        )
    }
    pub fn items(&self) -> &[GeneratedHeader] {
        &self.items
    }
    pub fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
    pub(crate) fn frontier_identity(&self, syntax: Digest) -> Digest {
        let mut e = crate::core::digest::StableEncoder::new("lrr/v2/generated-syntax-frontier");
        e.digest(self.record.key().fingerprint());
        e.digest(syntax);
        e.finish()
    }
    pub(crate) fn record_input(&self) -> Arc<ExpansionRecord> {
        self.record.clone()
    }
    pub(crate) fn record_root(&self) -> &Arc<ExpansionRecord> {
        &self.record
    }
    pub fn record(&self) -> &ExpansionRecord {
        &self.record
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        db::key::OutlineKey,
        expand::source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
        source::{
            changes::{
                PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
                SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    fn source(text: &str) -> PreparedSourceView {
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let path = SourcePath::new("main.rs").unwrap();
        let unit = Digest([241; 32]);
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("source absent")
        };
        view
    }

    #[test]
    fn rustc_generated_function_body_projection_requires_current_definition_and_bytes() {
        use crate::expand::generated_items::GeneratedItems;
        use crate::syntax::{parse::ParsedFile, spans::BodySpan};
        let stock = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
        );
        let mut prior = None;
        for prefix in ["", "\n\n"] {
            let text = format!("{prefix}{stock}");
            let view = source(&text);
            let invocation = view
                .outline()
                .item_invocations
                .iter()
                .find(|call| call.scope.owner.is_none())
                .unwrap()
                .item_input;
            let provider = SourceDeclarative::new(view).unwrap();
            let SourceDeclarativeDemand::Ready(demand) = provider
                .demand(invocation, Digest([243; 32]), Digest([244; 32]))
                .unwrap()
            else {
                panic!("root stock function expansion unresolved");
            };
            let key = demand.key().clone();
            let generated =
                GeneratedItems::admit(&key, std::sync::Arc::new(demand.expand().unwrap()), 1)
                    .unwrap();
            let parsed = ParsedFile::from_generated_items(&generated).unwrap();
            let outline = parsed
                .outline(Digest([241; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let mut body = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "bar")
                .unwrap()
                .body
                .clone()
                .unwrap();
            // GeneratedSyntaxInput publishes this exact generated member identity,
            // distinct from the reusable outline declaration key.
            body.def_key = generated
                .items()
                .iter()
                .find(|item| item.name.as_ref().is_some_and(|name| name.text == "bar"))
                .unwrap()
                .key
                .fingerprint();
            let offset = u32::try_from(generated.text().find("\"quux\"").unwrap()).unwrap();
            let span = BodySpan {
                body_key: body.key(),
                relative_start: offset.checked_sub(body.range.start).unwrap(),
                relative_end: offset.checked_sub(body.range.start).unwrap() + 6,
            };
            let projected = generated.project_body_source_spans(&body, &span).unwrap();
            assert_eq!(projected.len(), 1);
            let actual = projected[0];
            let expected = text.find("\"quux\"").unwrap();
            assert_eq!(
                (actual.start as usize, actual.end as usize),
                (expected, expected + 6)
            );
            assert_eq!(actual.source.content.0, Digest::of_bytes(text.as_bytes()).0);
            if let Some((old_body, old_span)) = prior.take() {
                assert_eq!(
                    generated
                        .project_body_source_spans(&old_body, &old_span)
                        .unwrap_err()
                        .code,
                    "LRR-E-INTERNAL-SOURCE-MAP"
                );
            }
            let mut foreign = body.clone();
            foreign.def_key = Digest([0; 32]);
            assert_eq!(
                generated
                    .project_body_source_spans(&foreign, &span)
                    .unwrap_err()
                    .code,
                "LRR-E-INTERNAL-SOURCE-MAP"
            );
            let mut damaged = body.clone();
            damaged.body_hash = Digest([0; 32]);
            let damaged_span = BodySpan {
                body_key: damaged.key(),
                ..span.clone()
            };
            assert_eq!(
                generated
                    .project_body_source_spans(&damaged, &damaged_span)
                    .unwrap_err()
                    .code,
                "LRR-E-INTERNAL-SOURCE-MAP"
            );
            let past_end = BodySpan {
                relative_end: u32::MAX,
                ..span.clone()
            };
            assert_eq!(
                generated
                    .project_body_source_spans(&body, &past_end)
                    .unwrap_err()
                    .code,
                "LRR-E-INTERNAL-SOURCE-MAP"
            );
            prior = Some((body, span));
        }
    }
}
