//! Source-owned function-like procedural requests; assembly executes no helper.
use super::{
    expansion::DemandContext,
    item_macro_requests::{PreparedItemRequest, ResolvedItemTarget},
    macro_session::MacroSession,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::{
        identity::{ExpansionKey, MacroIdentity, ProceduralKind},
        source_declarative::SourceDeclarative,
    },
    frontend::macro_names::{MacroNamespaces, PathLookup},
    source::changes::PreparedSourceView,
    syntax::{outline::ScopeKey, token_wire::TokenForest},
};
use lrr_expand_adapter::{proc_client::ProcInvocation, token_wire::Key};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
pub(crate) enum CapturedDeclarationParent {
    Generated(std::sync::Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>),
    Argument(std::sync::Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>),
}
impl CapturedDeclarationParent {
    pub(crate) fn metadata_bytes(&self) -> usize {
        match self {
            Self::Generated(index) => index.retained_metadata_bytes(),
            Self::Argument(index) => index.metadata_bytes(),
        }
    }
    pub(crate) fn cfg(&self) -> Digest {
        match self {
            Self::Generated(index) => index.output().scan().cfg(),
            Self::Argument(index) => index.argument().scan().cfg(),
        }
    }
    pub(crate) fn origin(&self) -> Option<Key> {
        match self {
            Self::Generated(index) => Some(Key(index.output().record().key().fingerprint().0)),
            Self::Argument(index) => index.emitting_origin().map(|key| Key(key.0)),
        }
    }
}
struct EmittedInvocation {
    parent: CapturedDeclarationParent,
    sources: Vec<lrr_expand_adapter::proc_client::CapturedSpanSource>,
}
pub struct FunctionInvocationInput {
    source: PreparedSourceView,
    provider: Option<SourceDeclarative>,
    invocation: Digest,
    scope: ScopeKey,
    arguments: TokenForest,
    call_site: Key,
    grammar: crate::expand::identity::GrammarSlot,
    declaration_range: Option<crate::syntax::spans::SourceRange>,
    emitted: Option<EmittedInvocation>,
}
impl FunctionInvocationInput {
    pub fn from_source(
        source: &PreparedSourceView,
        invocation: Digest,
    ) -> Result<Self, Diagnostic> {
        let call = source
            .outline()
            .item_invocations
            .iter()
            .find(|call| call.item_input == invocation && call.attribute_input.is_none())
            .ok_or_else(|| fault("active original function-like invocation absent"))?;
        let provider = SourceDeclarative::new(source.share())?;
        let range = source.macro_token_group(call.range, false)?;
        let delimited = crate::expand::source_declarative::group(&provider.owned_forest()?, range)?;
        let delimited = TokenForest::decode(&delimited.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))?;
        let [crate::syntax::token_wire::TokenTree::Group { open, .. }] = delimited.roots() else {
            return Err(fault("source invocation argument group absent"));
        };
        let call_site = Key(open.0);
        let arguments = crate::expand::attributes::macro_argument_contents(&delimited)?;
        Ok(Self {
            source: source.share(),
            provider: Some(provider),
            invocation,
            scope: call.scope.clone(),
            arguments,
            call_site,
            grammar: crate::expand::identity::GrammarSlot::Items,
            declaration_range: None,
            emitted: None,
        })
    }
    /// Cold declaration syntax capture. This neither selects nor executes a macro.
    pub fn from_declaration_source(
        source: &PreparedSourceView,
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        invocation: Digest,
    ) -> Result<Self, Diagnostic> {
        let actual = source.declaration_inventory(Default::default())?;
        if scan.fingerprint() != actual.fingerprint() {
            return Err(fault("cold procedural source scan differs"));
        }
        let producer = actual
            .producers()
            .iter()
            .find(|p| {
                p.identity == invocation
                    && p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            })
            .ok_or_else(|| fault("cold procedural source invocation absent"))?;
        let provider = SourceDeclarative::new(source.share())?;
        let range = source.macro_token_group(producer.range, false)?;
        let delimited = crate::expand::source_declarative::group(&provider.owned_forest()?, range)?;
        let delimited = TokenForest::decode(&delimited.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))?;
        let [crate::syntax::token_wire::TokenTree::Group { open, .. }] = delimited.roots() else {
            return Err(fault("cold source invocation argument group absent"));
        };
        let call_site = Key(open.0);
        let arguments = crate::expand::attributes::macro_argument_contents(&delimited)?;
        Ok(Self {
            source: source.share(),
            provider: Some(provider),
            invocation,
            scope: producer.scope.clone(),
            arguments,
            call_site,
            grammar: producer.grammar,
            declaration_range: Some(producer.range),
            emitted: None,
        })
    }
    pub(crate) fn from_generated_declaration(
        source: &PreparedSourceView,
        parent: std::sync::Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>,
        invocation: Digest,
        providers: &[SourceDeclarative],
    ) -> Result<Self, Diagnostic> {
        let original = parent.source_index()?.scan();
        let selected = source.selection();
        if original.parse() != &selected.outline.parse
            || original.path() != &selected.path
            || original.cfg() != selected.outline.cfg
        {
            return Err(fault("generated procedural source attachment differs"));
        }
        let output = parent.output();
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| {
                p.identity == invocation
                    && p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            })
            .ok_or_else(|| fault("generated procedural producer absent"))?;
        let call = output.macro_call(invocation)?;
        let delimited = TokenForest::decode(&call.input.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))?;
        let arguments = crate::expand::attributes::macro_argument_contents(&delimited)?;
        let sources =
            crate::expand::source_declarative::captured_procedural_sources(&arguments, providers)?;
        let scope = producer.scope.clone();
        let grammar = producer.grammar;
        let call_site = call.site;
        Ok(Self {
            source: source.share(),
            provider: None,
            invocation,
            scope,
            arguments,
            call_site,
            grammar,
            declaration_range: None,
            emitted: Some(EmittedInvocation {
                parent: CapturedDeclarationParent::Generated(parent),
                sources,
            }),
        })
    }
    pub(crate) fn invocation_identity(&self) -> Digest {
        self.invocation
    }
    pub(crate) fn captured_parent(&self) -> Option<&CapturedDeclarationParent> {
        self.emitted.as_ref().map(|row| &row.parent)
    }
    pub(crate) fn from_argument_declaration(
        source: &PreparedSourceView,
        parent: std::sync::Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        invocation: Digest,
        providers: &[SourceDeclarative],
    ) -> Result<Self, Diagnostic> {
        let original = parent.source_index().scan();
        let selected = source.selection();
        if original.parse() != &selected.outline.parse
            || original.path() != &selected.path
            || original.cfg() != selected.outline.cfg
        {
            return Err(fault("argument procedural source differs"));
        }
        let argument = parent.argument();
        let producer = argument
            .scan()
            .producers()
            .iter()
            .find(|p| {
                p.identity == invocation
                    && p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            })
            .ok_or_else(|| fault("argument procedural producer absent"))?;
        let call = argument.macro_call(invocation)?;
        let delimited = TokenForest::decode(&call.input.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))?;
        let arguments = crate::expand::attributes::macro_argument_contents(&delimited)?;
        let sources =
            crate::expand::source_declarative::captured_procedural_sources(&arguments, providers)?;
        let scope = producer.scope.clone();
        let grammar = producer.grammar;
        let call_site = call.site;
        Ok(Self {
            source: source.share(),
            provider: None,
            invocation,
            scope,
            arguments,
            call_site,
            grammar,
            declaration_range: None,
            emitted: Some(EmittedInvocation {
                parent: CapturedDeclarationParent::Argument(parent),
                sources,
            }),
        })
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .arguments
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(std::mem::size_of::<Self>())
            .ok_or_else(|| fault("procedural input metadata overflow"))?;
        if let Some(provider) = &self.provider {
            bytes = bytes
                .checked_add(provider.module_index_bytes()?)
                .ok_or_else(|| fault("procedural source index metadata overflow"))?;
        }
        if let Some(emitted) = &self.emitted {
            bytes = bytes
                .checked_add(
                    emitted
                        .sources
                        .capacity()
                        .checked_mul(std::mem::size_of::<
                            lrr_expand_adapter::proc_client::CapturedSpanSource,
                        >())
                        .ok_or_else(|| fault("procedural source vector metadata overflow"))?,
                )
                .ok_or_else(|| fault("procedural source vector metadata overflow"))?;
            bytes = bytes
                .checked_add(emitted.parent.metadata_bytes())
                .ok_or_else(|| fault("procedural parent metadata overflow"))?;
            for source in &emitted.sources {
                bytes = bytes
                    .checked_add(source.heap_bytes().map_err(|e| fault(&e.message))?)
                    .ok_or_else(|| fault("procedural source metadata overflow"))?;
            }
        }
        if bytes > 512 * 1024 * 1024 {
            return Err(fault("procedural input metadata limit"));
        }
        Ok(bytes)
    }
    pub fn grammar(&self) -> crate::expand::identity::GrammarSlot {
        self.grammar
    }
    pub(crate) fn source_selection(&self) -> &crate::source::changes::SourceSelection {
        self.source.selection()
    }
    pub fn arguments(&self) -> &TokenForest {
        &self.arguments
    }
    pub fn call_site(&self) -> Key {
        self.call_site
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn parent_origin(&self) -> Key {
        if let Some(emitted) = &self.emitted
            && let Some(origin) = emitted.parent.origin()
        {
            return origin;
        }
        Key(
            crate::db::key::QueryKey::Outline(self.source.selection().outline.clone())
                .fingerprint()
                .0,
        )
    }
    /// Validate a real source lookup against retained SDK inventory, never a name guess.
    pub fn resolve(
        &self,
        session: &MacroSession,
        context: &DemandContext,
        namespaces: &MacroNamespaces,
        target: ResolvedItemTarget<'_>,
    ) -> Result<PreparedFunctionRequest, Diagnostic> {
        if self.emitted.is_some() {
            return Err(fault(
                "emitted procedural input requires its captured declaration selection",
            ));
        }
        let lookup = if let Some(range) = self.declaration_range {
            let syntax = self.source.macro_call_at(range)?;
            let mut scope = self.scope.clone();
            scope.owner = None;
            let index = crate::frontend::impl_index::DeclarationMacroIndex::new(
                self.source.declaration_inventory(Default::default())?,
            )?;
            match index.lookup(self.invocation)? {
                crate::frontend::macro_scopes::HygienicLookup::Selection(
                    crate::frontend::macro_origins::MacroSelection::Defined(definition),
                ) => PathLookup::Defined(definition),
                crate::frontend::macro_scopes::HygienicLookup::Selection(
                    crate::frontend::macro_origins::MacroSelection::Ambiguous(definitions),
                ) => PathLookup::Ambiguous(definitions),
                crate::frontend::macro_scopes::HygienicLookup::Selection(
                    crate::frontend::macro_origins::MacroSelection::Absent,
                ) => namespaces.lookup_invocation_segments(
                    syntax.segments(),
                    syntax.absolute(),
                    &scope,
                    syntax.key().profile.edition,
                )?,
                crate::frontend::macro_scopes::HygienicLookup::NeedInput(input)
                    if input == self.invocation && syntax.segments().len() > 1 =>
                {
                    namespaces.lookup_invocation_segments(
                        syntax.segments(),
                        syntax.absolute(),
                        &scope,
                        syntax.key().profile.edition,
                    )?
                }
                _ => PathLookup::NeedInput(self.invocation),
            }
        } else {
            self.provider
                .as_ref()
                .ok_or_else(|| fault("original procedural source provider absent"))?
                .resolved_function_call(self.invocation, namespaces)?
        };
        let definition = match lookup {
            PathLookup::Defined(definition) => definition,
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "source procedural function lookup is not uniquely complete".into(),
                ));
            }
        };
        self.resolve_definition(session, context, &definition, target)
    }
    pub(crate) fn resolve_definition(
        &self,
        session: &MacroSession,
        context: &DemandContext,
        definition: &crate::expand::scopes::MacroDefinition,
        mut target: ResolvedItemTarget<'_>,
    ) -> Result<PreparedFunctionRequest, Diagnostic> {
        if context.unit != self.scope.crate_key
            || context.epoch == 0
            || target.scope != Key(self.scope.fingerprint().0)
            || target.call_site != self.call_site
            || target.parent_origin != self.parent_origin()
        {
            return Err(fault("source procedural consumer/scope/call site differs"));
        }
        let catalog = session.export_catalog(context)?;
        let selected = catalog
            .select(definition, ProceduralKind::Function)?
            .ok_or_else(|| fault("source candidate is not a prepared procedural function"))?;
        if !matches!(selected.identity(), MacroIdentity::Procedural { artifact, export, .. }
            if *artifact == Digest(target.artifact.0) && export == &target.export)
        {
            return Err(fault(
                "source procedural export differs from supplied target",
            ));
        }
        let native = lrr_expand_adapter::token_wire::TokenForest::decode(
            &self.arguments.encode().map_err(|e| fault(&e.message))?,
        )
        .map_err(|e| fault(&e.message))?;
        let site = native
            .spans()
            .iter()
            .find(|span| span.key() == self.call_site)
            .ok_or_else(|| fault("owned source call site absent"))?;
        if target
            .sources
            .iter()
            .any(|source| source.source() == &site.source)
        {
            return Err(fault(
                "source caller capture must be supplied by the owned selection",
            ));
        }
        if let Some(emitted) = &self.emitted {
            if let CapturedDeclarationParent::Generated(parent) = &emitted.parent
                && parent.output().epoch() != context.epoch
            {
                return Err(fault("generated procedural emission epoch differs"));
            }
            if target
                .sources
                .len()
                .checked_add(emitted.sources.len())
                .is_none_or(|n| n > 4096)
                || target.sources.iter().any(|row| {
                    emitted
                        .sources
                        .iter()
                        .any(|owned| owned.source() == row.source())
                })
            {
                return Err(fault("generated procedural capture count/conflict"));
            }
            target
                .sources
                .try_reserve_exact(emitted.sources.len())
                .map_err(|_| fault("generated procedural capture allocation"))?;
            target.sources.extend(emitted.sources.iter().cloned());
        } else {
            let captured = lrr_expand_adapter::proc_client::CapturedSpanSource::new(
                site.source.clone(),
                self.source.captured_macro_blob()?.text()?.into(),
                lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes,
                None,
            )
            .map_err(|e| fault(&e.message))?;
            if target.sources.len() >= 4096 {
                return Err(fault("source procedural capture count limit"));
            }
            target
                .sources
                .try_reserve(1)
                .map_err(|_| fault("source capture allocation"))?;
            target.sources.push(captured);
        }
        let prepared = PreparedItemRequest::function_like(
            session,
            context,
            self.invocation,
            &self.arguments,
            self.grammar,
            target,
        )?;
        if &prepared.key().macro_identity != selected.identity() {
            return Err(fault(
                "source procedural identity differs from frozen session",
            ));
        }
        Ok(PreparedFunctionRequest {
            source: self.source.selection().clone(),
            prepared,
            source_derive_proof: None,
            source_attribute_proof: None,
        })
    }
}
pub struct PreparedSourceRequest {
    source: crate::source::changes::SourceSelection,
    prepared: PreparedItemRequest,
    source_derive_proof: Option<Digest>,
    source_attribute_proof: Option<Digest>,
}
/// Compatibility name for the original function-like source request API.
pub type PreparedFunctionRequest = PreparedSourceRequest;
impl PreparedSourceRequest {
    pub(crate) fn from_source_derive(
        source: crate::source::changes::SourceSelection,
        prepared: PreparedItemRequest,
        proof: Digest,
    ) -> Self {
        Self {
            source,
            prepared,
            source_derive_proof: Some(proof),
            source_attribute_proof: None,
        }
    }
    pub(crate) fn source_derive_proof(&self) -> Option<Digest> {
        self.source_derive_proof
    }
    pub(crate) fn from_source_attribute(
        source: crate::source::changes::SourceSelection,
        prepared: PreparedItemRequest,
        proof: Digest,
    ) -> Self {
        Self {
            source,
            prepared,
            source_derive_proof: None,
            source_attribute_proof: Some(proof),
        }
    }
    pub(crate) fn source_attribute_proof(&self) -> Option<Digest> {
        self.source_attribute_proof
    }
    pub fn source(&self) -> &crate::source::changes::SourceSelection {
        &self.source
    }
    pub fn request(&self) -> &ProcInvocation {
        self.prepared.request()
    }
    pub fn key(&self) -> &ExpansionKey {
        self.prepared.key()
    }
}
