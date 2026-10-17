//! Original intrinsic argument tokens transported through the pinned Expr bridge.
//! An argument syntax identity is not an expansion record or producer completion.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::snapshot::CfgSet,
    syntax::{
        declaration_scan::{DeclarationScan, DeclarationScanLimits},
        parse::ParsedFile,
        spans::SourceRange,
    },
};
use lrr_expand_adapter::{
    declaration_output::{self, DeclarationGrammar},
    token_wire::TokenForest,
};
use std::sync::Arc;

pub struct ArgumentDeclarations {
    identity: Digest,
    parent: Digest,
    invocation: Digest,
    definition: crate::expand::scopes::MacroDefinition,
    range: SourceRange,
    tokens: TokenForest,
    scan: Arc<DeclarationScan>,
    parsed: ParsedFile,
    retained_bytes: usize,
    captures: Digest,
    definitions: std::collections::BTreeMap<Digest, declaration_output::DefinitionCapture>,
    calls: std::collections::BTreeMap<Digest, declaration_output::CallCapture>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTRINSIC-DECLARATIONS", message.into())
}
impl ArgumentDeclarations {
    fn capture_fingerprint(
        definitions: &std::collections::BTreeMap<Digest, declaration_output::DefinitionCapture>,
        calls: &std::collections::BTreeMap<Digest, declaration_output::CallCapture>,
    ) -> Result<Digest, Diagnostic> {
        let mut encoded = StableEncoder::new("lrr/v2/original-argument-macro-captures");
        encoded.u64(
            u64::try_from(definitions.len())
                .map_err(|_| fault("argument definition count overflow"))?,
        );
        for (id, capture) in definitions {
            encoded.digest(*id);
            encoded.u32(capture.start);
            encoded.u32(capture.end);
            encoded.digest(Digest(capture.name_span.0));
            encoded.u16(capture.edition);
            encoded.digest(Digest::of_bytes(
                &capture.body.encode().map_err(|e| fault(&e.message))?,
            ));
        }
        encoded.u64(u64::try_from(calls.len()).map_err(|_| fault("argument call count overflow"))?);
        for (id, capture) in calls {
            encoded.digest(*id);
            encoded.u32(capture.start);
            encoded.u32(capture.end);
            encoded.digest(Digest(capture.site.0));
            encoded.u16(capture.edition);
            encoded.digest(Digest::of_bytes(
                &capture.path.encode().map_err(|e| fault(&e.message))?,
            ));
            encoded.digest(Digest::of_bytes(
                &capture.input.encode().map_err(|e| fault(&e.message))?,
            ));
        }
        Ok(encoded.finish())
    }
    pub(crate) fn source_tokens(
        source: &crate::source::changes::SourceSelection,
        blob: &crate::source::snapshot::SourceBlob,
    ) -> Result<TokenForest, Diagnostic> {
        let wire_source = lrr_expand_adapter::token_wire::SourceIdentity {
            unit: lrr_expand_adapter::token_wire::Key(source.outline.crate_key.0),
            content: lrr_expand_adapter::token_wire::Key(source.outline.parse.content.0),
            path: source.path.as_str().into(),
        };
        let context = lrr_expand_adapter::token_wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
            defining_unit: wire_source.unit,
            edition: super::source_declarative::edition(source.outline.parse.profile.edition)?,
        };
        let forest = lrr_expand_adapter::token_wire::parse_source(
            blob.text()?,
            wire_source,
            context,
            lrr_expand_adapter::token_wire::Key(
                crate::db::key::QueryKey::Outline(source.outline.clone())
                    .fingerprint()
                    .0,
            ),
        )
        .map_err(|e| fault(&e.message))?;
        forest
            .encoded_len()
            .map_err(|e| fault(&e.message))?;
        Ok(forest)
    }
    pub(crate) fn new(
        parent: &DeclarationScan,
        invocation: Digest,
        definition: crate::expand::scopes::MacroDefinition,
        range: SourceRange,
        tokens: TokenForest,
        cfg: &CfgSet,
    ) -> Result<Self, Diagnostic> {
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("intrinsic argument producer absent"))?;
        if range.start < producer.range.start
            || range.end > producer.range.end
            || range.start >= range.end
            || cfg.fingerprint() != parent.cfg()
        {
            return Err(fault("intrinsic argument source range/cfg differs"));
        }
        definition.validate()?;
        let wire = tokens.encode().map_err(|e| fault(&e.message))?;
        let mut encoder = StableEncoder::new("lrr/v2/intrinsic-original-argument-syntax");
        encoder.digest(parent.fingerprint());
        encoder.digest(invocation);
        encoder.digest(definition.identity);
        encoder.u32(range.start);
        encoder.u32(range.end);
        encoder.digest(Digest::of_bytes(&wire));
        let identity = encoder.finish();
        Self::from_tokens(parent, invocation, definition, range, tokens, cfg, identity)
    }
    pub(crate) fn from_generated(
        parent: &DeclarationScan,
        invocation: Digest,
        definition: crate::expand::scopes::MacroDefinition,
        ordinal: usize,
        tokens: TokenForest,
        cfg: &CfgSet,
    ) -> Result<Self, Diagnostic> {
        definition.validate()?;
        if cfg.fingerprint() != parent.cfg() {
            return Err(fault("generated argument cfg differs"));
        }
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("generated argument invocation absent"))?;
        let mut e = StableEncoder::new("lrr/v2/generated-intrinsic-argument-syntax");
        e.digest(parent.fingerprint());
        e.digest(invocation);
        e.digest(definition.identity);
        e.u64(u64::try_from(ordinal).map_err(|_| fault("argument ordinal overflow"))?);
        e.digest(Digest::of_bytes(
            &tokens.encode().map_err(|e| fault(&e.message))?,
        ));
        Self::from_tokens(
            parent,
            invocation,
            definition,
            producer.range,
            tokens,
            cfg,
            e.finish(),
        )
    }
    fn from_tokens(
        parent: &DeclarationScan,
        invocation: Digest,
        definition: crate::expand::scopes::MacroDefinition,
        range: SourceRange,
        tokens: TokenForest,
        cfg: &CfgSet,
        identity: Digest,
    ) -> Result<Self, Diagnostic> {
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("argument producer absent"))?;
        let output = declaration_output::materialize(&tokens, DeclarationGrammar::Expression)
            .map_err(|e| fault(&e.message))?;
        let parsed = ParsedFile::from_generated_declarations(
            &output.syntax,
            output.text.len(),
            identity,
            parent.parse().profile.edition,
            crate::expand::identity::GrammarSlot::Expression,
        )?;
        let scan = Arc::new(parsed.scan_declarations(
            parent.path().clone(),
            producer.scope.clone(),
            cfg,
            DeclarationScanLimits::default(),
        )?);
        let mut definition_rows = std::collections::BTreeMap::new();
        for capture in output.definitions {
            if definition_rows
                .insert((capture.start, capture.end), capture)
                .is_some()
            {
                return Err(fault("duplicate original argument definition capture"));
            }
        }
        let mut call_rows = std::collections::BTreeMap::new();
        for capture in output.calls {
            if call_rows
                .insert((capture.start, capture.end), capture)
                .is_some()
            {
                return Err(fault("duplicate original argument call capture"));
            }
        }
        let mut definitions = std::collections::BTreeMap::new();
        for header in scan.macro_definitions() {
            let capture = definition_rows
                .remove(&(header.range.start, header.range.end))
                .ok_or_else(|| fault("active original argument definition capture absent"))?;
            definitions.insert(header.identity, capture);
        }
        let mut calls = std::collections::BTreeMap::new();
        for producer in scan
            .producers()
            .iter()
            .filter(|p| p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
        {
            let capture = call_rows
                .remove(&(producer.range.start, producer.range.end))
                .ok_or_else(|| fault("active original argument call capture absent"))?;
            calls.insert(producer.identity, capture);
        }
        let mut capture_bytes = 0usize;
        for definition in definitions.values() {
            capture_bytes = capture_bytes
                .checked_add(
                    definition
                        .body
                        .allocated_bytes()
                        .map_err(|e| fault(&e.message))?,
                )
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<declaration_output::DefinitionCapture>() + 128,
                    )
                })
                .ok_or_else(|| fault("original argument definition metadata overflow"))?;
        }
        for call in calls.values() {
            capture_bytes = capture_bytes
                .checked_add(
                    call.input
                        .allocated_bytes()
                        .map_err(|e| fault(&e.message))?,
                )
                .and_then(|n| n.checked_add(call.path.allocated_bytes().ok()?))
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<declaration_output::CallCapture>() + 128)
                })
                .ok_or_else(|| fault("original argument call metadata overflow"))?;
        }
        let captures = Self::capture_fingerprint(&definitions, &calls)?;
        let retained_bytes = tokens
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(capture_bytes)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(scan.retained_bytes()))
            .and_then(|n| n.checked_add(parsed.estimated_bytes()))
            .and_then(|n| n.checked_add(output.text.capacity()))
            .ok_or_else(|| fault("intrinsic argument metadata limit"))?;
        Ok(Self {
            identity,
            parent: parent.fingerprint(),
            invocation,
            definition,
            range,
            tokens,
            scan,
            parsed,
            retained_bytes,
            captures,
            definitions,
            calls,
        })
    }
    pub(crate) fn macro_syntax(
        &self,
        identity: Digest,
    ) -> Result<crate::syntax::macro_calls::SourceMacroCall, Diagnostic> {
        let producer = self
            .scan
            .producers()
            .iter()
            .find(|p| p.identity == identity)
            .ok_or_else(|| fault("argument macro syntax absent"))?;
        self.parsed.macro_call_at(producer.range)
    }
    pub fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub(crate) fn token_context(
        &self,
        span: lrr_expand_adapter::token_wire::Key,
    ) -> Result<crate::syntax::token_wire::Key, Diagnostic> {
        self.tokens
            .spans()
            .iter()
            .find(|s| s.key() == span)
            .map(|s| crate::syntax::token_wire::Key(s.context.0))
            .ok_or_else(|| fault("original argument token context absent"))
    }
    pub fn macro_call(
        &self,
        identity: Digest,
    ) -> Result<&declaration_output::CallCapture, Diagnostic> {
        self.calls
            .get(&identity)
            .ok_or_else(|| fault("original argument call identity absent"))
    }
    pub(crate) fn definition_name_origin(
        &self,
        identity: Digest,
    ) -> Result<crate::syntax::origin::SourceOrigin, Diagnostic> {
        let capture = self.macro_definition(identity)?;
        let span = self
            .tokens
            .spans()
            .iter()
            .find(|span| span.key() == capture.name_span)
            .ok_or_else(|| fault("argument definition name source absent"))?;
        Ok(crate::syntax::origin::SourceOrigin {
            source: crate::syntax::token_wire::SourceIdentity {
                unit: crate::syntax::token_wire::Key(span.source.unit.0),
                content: crate::syntax::token_wire::Key(span.source.content.0),
                path: span.source.path.clone(),
            },
            range: SourceRange::checked(span.start, span.end)?,
        })
    }
    pub fn macro_definition(
        &self,
        identity: Digest,
    ) -> Result<&declaration_output::DefinitionCapture, Diagnostic> {
        self.definitions
            .get(&identity)
            .ok_or_else(|| fault("original argument definition identity absent"))
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn source_range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn parent_inventory(&self) -> Digest {
        self.parent
    }
    pub(crate) fn invocation(&self) -> Digest {
        self.invocation
    }
    pub(crate) fn definition(&self) -> &crate::expand::scopes::MacroDefinition {
        &self.definition
    }
    pub(crate) fn validate_source(
        &self,
        parent: &DeclarationScan,
        cfg: &CfgSet,
        forest: &TokenForest,
    ) -> Result<(), Diagnostic> {
        if Self::capture_fingerprint(&self.definitions, &self.calls)? != self.captures {
            return Err(fault(
                "intrinsic argument captures differ from retained identity",
            ));
        }
        let tokens = super::source_declarative::tokens_for_source_range(forest, self.range)?;
        if tokens.encode().map_err(|e| fault(&e.message))?
            != self.tokens.encode().map_err(|e| fault(&e.message))?
        {
            return Err(fault(
                "intrinsic argument tokens differ from captured source",
            ));
        }
        let rebuilt = Self::new(
            parent,
            self.invocation,
            self.definition.clone(),
            self.range,
            tokens,
            cfg,
        )?;
        if rebuilt.fingerprint() != self.fingerprint() {
            return Err(fault("intrinsic argument inventory does not revalidate"));
        }
        Ok(())
    }
}
impl std::fmt::Debug for ArgumentDeclarations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArgumentDeclarations")
            .field("identity", &self.identity)
            .field("range", &self.range)
            .finish()
    }
}
impl StableEncode for ArgumentDeclarations {
    const DOMAIN: &'static str = "lrr/v2/intrinsic-original-argument-inventory";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.digest(self.parent);
        e.digest(self.invocation);
        e.digest(self.definition.identity);
        e.digest(self.scan.fingerprint());
        e.digest(self.captures);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_argument_captures_preserve_real_call_tokens_and_local_definition() {
        let text = "macro_rules! outer{($v:expr)=>{{$v}}}fn cold(){assert!({macro_rules! local{()=>{true}}local!();outer!(7);true});}";
        let profile = crate::core::profile::ProfileIdentity::current();
        let blob = crate::source::snapshot::SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let unit = Digest([61; 32]);
        let cfg = CfgSet::guest_default();
        let source = crate::source::changes::SourceSelection {
            path: crate::source::snapshot::SourcePath::new("main.rs").unwrap(),
            outline: crate::db::key::OutlineKey {
                parse: crate::syntax::parse::ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
        let parent = parsed
            .scan_declarations(
                source.path.clone(),
                crate::syntax::outline::ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &cfg,
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.path[0].text == "assert")
            .unwrap();
        let start = text.find("{macro_rules!").unwrap() as u32;
        let end = text.find("});}").unwrap() as u32 + 1;
        let range = SourceRange { start, end };
        let forest = ArgumentDeclarations::source_tokens(&source, &blob).unwrap();
        let tokens =
            super::super::source_declarative::tokens_for_source_range(&forest, range).unwrap();
        let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::new(
            crate::facade::unit(&profile, "core"),
            profile.edition,
        )
        .unwrap();
        let definition = catalog
            .definitions()
            .find(|d| {
                catalog.select(d).unwrap() == Some(crate::facade::macros::RuntimeMacroKind::Assert)
            })
            .unwrap()
            .clone();
        let argument =
            ArgumentDeclarations::new(&parent, producer.identity, definition, range, tokens, &cfg)
                .unwrap();
        assert_eq!(argument.scan().macro_definitions().len(), 1);
        assert_eq!(argument.scan().producers().len(), 2);
        let local = &argument.scan().macro_definitions()[0];
        let local_identity = local.identity;
        assert!(
            !argument
                .macro_definition(local.identity)
                .unwrap()
                .body
                .roots()
                .is_empty()
        );
        for call in argument.scan().producers() {
            let capture = argument.macro_call(call.identity).unwrap();
            assert_eq!(
                (capture.start, capture.end),
                (call.range.start, call.range.end)
            );
            assert!(
                capture
                    .input
                    .spans()
                    .iter()
                    .any(|s| s.key() == capture.site && s.source.content.0 == blob.content().0)
            );
        }
        argument.validate_source(&parent, &cfg, &forest).unwrap();
        let mut substituted = ArgumentDeclarations::new(
            &parent,
            producer.identity,
            argument.definition().clone(),
            range,
            argument.tokens.clone(),
            &cfg,
        )
        .unwrap();
        let original = substituted.fingerprint();
        let call = substituted.calls.values_mut().next().unwrap();
        call.site = lrr_expand_adapter::token_wire::Key([99; 32]);
        // A retained capture mutation is detected independently of the parse inventory.
        assert_eq!(substituted.fingerprint(), original);
        assert!(substituted.validate_source(&parent, &cfg, &forest).is_err());
        let parent = Arc::new(
            crate::frontend::impl_index::DeclarationMacroIndex::new(Arc::new(parent)).unwrap(),
        );
        let argument = Arc::new(argument);
        let mut index = crate::frontend::impl_index::ArgumentDeclarationMacroIndex::new(
            argument.clone(),
            parent.clone(),
        )
        .unwrap();
        let local_call = argument
            .scan()
            .producers()
            .iter()
            .find(|p| p.path[0].text == "local")
            .unwrap();
        assert!(
            matches!(index.lookup(local_call.identity).unwrap(),crate::frontend::macro_scopes::HygienicLookup::Selection(crate::frontend::macro_origins::MacroSelection::Defined(d)) if d.identity==local_identity)
        );
        let inputs = crate::source::changes::PreparedSourceInputs::refresh(
            None,
            vec![crate::source::changes::SourceUnitInput {
                path: source.path.clone(),
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg: cfg.clone(),
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let crate::source::changes::SourceSelectionOutcome::Selected(view) =
            inputs.select(&source).unwrap()
        else {
            panic!("source not selected")
        };
        let provider = crate::expand::source_declarative::SourceDeclarative::new(view).unwrap();
        let crate::expand::source_declarative::SourceDeclarativeDemand::Ready(request) = provider
            .argument_declaration_demand(
                &index,
                local_call.identity,
                Digest([62; 32]),
                Digest([63; 32]),
            )
            .unwrap()
        else {
            panic!("local argument demand absent")
        };
        let record = Arc::new(request.expand().unwrap());
        let output = Arc::new(
            crate::expand::generated_declarations::GeneratedDeclarations::admit(
                request.key(),
                record,
                1,
                argument.scan(),
                &cfg,
                DeclarationScanLimits::default(),
            )
            .unwrap(),
        );
        let child = crate::frontend::impl_index::GeneratedDeclarationMacroIndex::new(
            output,
            crate::frontend::impl_index::DeclarationMacroParent::Argument(Arc::new(index.clone())),
        )
        .unwrap();
        assert!(child.is_complete());
        index.adopt_completed_child(request.key(), &child).unwrap();
        let outer = argument
            .scan()
            .producers()
            .iter()
            .find(|p| p.path[0].text == "outer")
            .unwrap();
        assert!(matches!(
            provider
                .argument_declaration_demand(
                    &index,
                    outer.identity,
                    Digest([62; 32]),
                    Digest([63; 32])
                )
                .unwrap(),
            crate::expand::source_declarative::SourceDeclarativeDemand::Ready(_)
        ));
        assert!(!index.is_complete());
    }
    #[test]
    fn rustc_assertion_arguments_preserve_every_written_operand_and_format_tail() {
        let text = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/assert-eq-macro-success.rs"
        );
        let blob = crate::source::snapshot::SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let profile = crate::core::profile::ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let unit = Digest([64; 32]);
        let source = crate::source::changes::SourceSelection {
            path: crate::source::snapshot::SourcePath::new("assert-eq-macro-success.rs").unwrap(),
            outline: crate::db::key::OutlineKey {
                parse: crate::syntax::parse::ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
        let parent = parsed
            .scan_declarations(
                source.path.clone(),
                crate::syntax::outline::ScopeKey {
                    crate_key: unit,
                    module_path: vec![],
                    owner: None,
                },
                &cfg,
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::new(
            crate::facade::unit(&profile, "core"),
            profile.edition,
        )
        .unwrap();
        let definition = catalog
            .definitions()
            .find(|d| {
                catalog.select(d).unwrap()
                    == Some(crate::facade::macros::RuntimeMacroKind::AssertEq)
            })
            .unwrap()
            .clone();
        let forest = ArgumentDeclarations::source_tokens(&source, &blob).unwrap();
        let mut counts = Vec::new();
        for producer in parent
            .producers()
            .iter()
            .filter(|p| p.path[0].text == "assert_eq")
        {
            let call = parsed.macro_call_at(producer.range).unwrap();
            let args = parsed
                .runtime_macro_arguments(&call, crate::facade::macros::RuntimeMacroKind::AssertEq)
                .unwrap();
            counts.push(args.declaration_arguments().len());
            for range in args.declaration_arguments() {
                let tokens =
                    super::super::source_declarative::tokens_for_source_range(&forest, *range)
                        .unwrap();
                let argument = ArgumentDeclarations::new(
                    &parent,
                    producer.identity,
                    definition.clone(),
                    *range,
                    tokens,
                    &cfg,
                )
                .unwrap();
                assert!(argument.scan().impls().is_empty());
                assert!(argument.scan().producers().is_empty());
                argument.validate_source(&parent, &cfg, &forest).unwrap();
            }
        }
        assert_eq!(counts, [2, 2, 2, 2, 3, 4, 6]);
        assert!(
            parent
                .producers()
                .iter()
                .any(|p| p.kind
                    == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute)
        );
    }
}
