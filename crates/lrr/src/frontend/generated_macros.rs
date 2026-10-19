//! Demand-only generated declarative definitions in the existing hygienic resolver.
type CompletedDeclarationAttribute = (
    Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    crate::syntax::spans::SourceRange,
);
type CompletedDeclarationDerives = (
    Digest,
    Vec<Arc<crate::expand::generated_declarations::GeneratedDeclarations>>,
);
use super::nested_compile_inputs::EagerMacroResolver;
use crate::expand::source_declarative::CapturedMacroContext;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        generated_items::GeneratedItems,
        identity::ExpansionKey,
        record::ExpansionRecord,
        scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes},
    },
    frontend::{
        hygiene::HygieneFacts,
        macro_origins::ExpansionAncestry,
        macro_scopes::{HygienicLookup, MacroDeclarations, MacroReference},
    },
    source::{manifest::ItemKind, snapshot::CfgSet},
    syntax::{
        outline::ScopeKey as SourceScope,
        parse::ParsedFile,
        token_wire::{Key, TokenForest},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub(crate) struct OutwardMacro {
    pub scope: SourceScope,
    pub name: MacroName,
    pub definition: MacroDefinition,
    pub context: Key,
    pub origin: Option<Key>,
    at: u32,
}
pub(crate) struct GeneratedRouteSnapshot {
    record: Digest,
    owner: SourceScope,
    epoch: u64,
    origin: Key,
    routes: Arc<super::source_macro_routes::SourceMacroRoutes>,
}
impl GeneratedRouteSnapshot {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.routes
            .metadata_bytes()?
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| n.checked_add(self.owner.heap_bytes()))
            .ok_or_else(|| fault("child route snapshot metadata overflow"))
    }
}
pub enum GeneratedCallDemand {
    Ready(Arc<crate::expand::generated_call_request::GeneratedCallRequest>),
    NeedDeclaration(Arc<super::generated_derive_exports::DeriveDeclarationRequest>),
    Append {
        request: Arc<crate::expand::generated_call_request::GeneratedCallRequest>,
        record: Arc<ExpansionRecord>,
    },
    NeedDeriveExecution {
        occurrence: Digest,
        helpers: Arc<super::generated_derive_helpers::GeneratedDeriveHelpers>,
        request: Option<Arc<crate::expand::generated_call_request::GeneratedCallRequest>>,
        append: bool,
    },
    NeedInput(Digest),
    Complete,
}
struct ModuleParent {
    demand: super::generated_headers::GeneratedModuleSource,
    lexical: ScopeKey,
    before: u64,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum GeneratedCallIndex {
    Function(usize),
    Attribute(usize),
    Derive(usize, usize),
}
struct EmptyDeriveCertificate {
    item: usize,
    annotation: usize,
    definition: MacroDefinition,
    binding: Digest,
    members: Option<Digest>,
}
#[derive(Clone)]
struct InheritedHelpers {
    source: InheritedHelperSource,
    introduced: BTreeSet<Digest>,
}
#[derive(Clone)]
enum InheritedHelperSource {
    Generated(Arc<super::generated_derive_helpers::GeneratedDeriveHelpers>),
    Original(Arc<super::generated_derive_helpers::OriginalDeriveHelpers>),
}
impl InheritedHelperSource {
    fn retained_bytes(&self) -> usize {
        match self {
            Self::Generated(h) => h.retained_bytes(),
            Self::Original(h) => h.retained_bytes(),
        }
    }
    fn fingerprint(&self) -> Digest {
        match self {
            Self::Generated(h) => h.fingerprint(),
            Self::Original(h) => h.fingerprint(),
        }
    }
    fn inherited_match(
        &self,
        name: &str,
        class: crate::syntax::token_wire::NameContextKey,
        introduced: &BTreeSet<Digest>,
    ) -> bool {
        match self {
            Self::Generated(h) => h.inherited_match(name, class, introduced),
            Self::Original(h) => h.inherited_match(name, class),
        }
    }
}
fn inherited_helper_bytes(
    rows: impl IntoIterator<Item = (usize, usize)>,
) -> Result<usize, Diagnostic> {
    let limit = || {
        Diagnostic::new(
            "LRR-E-DISCOVERY-LIMIT",
            "inherited helper environment allocation limit".into(),
        )
    };
    let mut bytes = 0usize;
    let mut count = 0usize;
    for (source, introduced) in rows {
        count = count
            .checked_add(1)
            .ok_or_else(limit)?;
        bytes = introduced
            .checked_mul(96)
            .and_then(|n| n.checked_add(source))
            .and_then(|n| n.checked_add(128 + std::mem::size_of::<InheritedHelpers>()))
            .and_then(|n| n.checked_add(bytes))
            .ok_or_else(limit)?;
    }
    Ok(bytes)
}
pub struct GeneratedMacroCatalog {
    owner: SourceScope,
    root_context: Key,
    caller_root: crate::syntax::token_wire::ContextRecord,
    outward: Vec<OutwardMacro>,

    items: Arc<GeneratedItems>,
    cfg: CfgSet,
    calls: BTreeMap<(u32, Digest), GeneratedCallIndex>,
    call_sites: BTreeMap<Digest, (u32, GeneratedCallIndex)>,
    call_scopes: BTreeMap<Digest, SourceScope>,
    module_scopes: Arc<BTreeMap<SourceScope, ScopeKey>>,
    declaration_scopes: Arc<BTreeMap<Digest, CapturedDeclarationScope>>,
    module_sources: BTreeMap<Digest, ModuleParent>,
    completed_module_sources: BTreeMap<Digest, ModuleParent>,
    source_body_owners: BTreeMap<Digest, Arc<crate::expand::source_declarative::SourceDeclarative>>,
    call_frontiers: std::collections::BTreeSet<Digest>,
    pending: Option<Arc<crate::expand::generated_call_request::GeneratedCallRequest>>,
    helper_receipts:
        BTreeMap<Digest, Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>>,
    helper_receipt_bytes: usize,
    helper_scopes: BTreeMap<Digest, Arc<super::generated_derive_helpers::GeneratedDeriveHelpers>>,
    helper_scope_bytes: usize,
    derive_outputs: BTreeMap<Digest, Arc<ExpansionRecord>>,
    derive_output_bytes: usize,
    appended_derives: std::collections::BTreeSet<Digest>,
    settled_derive_items: std::collections::BTreeSet<Digest>,
    empty_derives: BTreeMap<(Digest, usize), EmptyDeriveCertificate>,
    inherited_helpers: Vec<InheritedHelpers>,
    inherited_items: BTreeMap<Digest, Digest>,
    compiler_test_items: BTreeMap<Digest, Digest>,
    compiler_test_prefixes: BTreeMap<Digest, (BTreeSet<usize>, Digest)>,
    compiler_test_prefix_bytes: usize,
    test_registration_candidates: BTreeMap<Digest, Arc<crate::syntax::outline::ItemOutline>>,
    test_registration_candidate_bytes: usize,
    test_registrations: BTreeMap<Digest, Arc<crate::syntax::outline::ItemOutline>>,
    derive_parent_views: BTreeMap<Digest, Arc<TextualScopes>>,
    derive_parent_view_bytes: usize,
    completed: BTreeMap<Digest, Arc<ExpansionRecord>>,
    inflight: BTreeMap<Digest, ExpansionKey>,
    completed_wire_bytes: usize,
    completed_allocated_bytes: usize,
    completed_headers: usize,

    definitions: BTreeMap<Digest, MacroDefinition>,
    body_owners: BTreeMap<Digest, Arc<GeneratedItems>>,
    route_children: Vec<Arc<GeneratedRouteSnapshot>>,
    selection_children: Vec<Arc<Self>>,
    scopes: Arc<TextualScopes>,
    scope: ScopeKey,
    declarations: Arc<MacroDeclarations>,
    facts: Arc<HygieneFacts>,
    ancestry: Arc<ExpansionAncestry>,
    edition: u16,
}
/// The actual canonical generated declaration and its retained lexical edge.
/// This table does not introduce an impl or trait into module namespaces.
pub(crate) struct CapturedDeclarationScope {
    pub(crate) context: crate::syntax::outline::DeclarationContext,
    pub(crate) lexical: ScopeKey,
}
pub(crate) struct GeneratedCompilerInput<'a> {
    pub resolver: GeneratedEagerResolver<'a>,
    pub caller: super::nested_compile_inputs::NestedLexicalContext,
    pub from: crate::source::include_paths::IncludeLocation,
}
pub(crate) struct GeneratedEagerResolver<'a> {
    catalog: &'a GeneratedMacroCatalog,
    context: CapturedMacroContext<'a>,
    ancestors: &'a [&'a GeneratedMacroCatalog],
    namespaces: super::macro_names::MacroNamespaces,
    scopes: TextualScopes,
    before: u64,
    source_base: &'a crate::syntax::token_wire::SpanRecord,
    input: &'a TokenForest,
}
impl super::nested_compile_inputs::EagerMacroResolver for GeneratedEagerResolver<'_> {
    fn compiler(&self) -> &super::compile_input_catalog::CompileInputCatalog {
        self.context
            .provider
            .compiler
            .as_deref()
            .expect("configured generated compiler")
    }
    fn validate_include_location(
        &self,
        site: &crate::syntax::token_wire::SpanRecord,
        from: &crate::source::include_paths::IncludeLocation,
        included: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Result<(), Diagnostic> {
        if self.input.spans().iter().find(|s| s.key() == site.key()) != Some(site)
            || from.unit.0 != self.source_base.source.unit.0
            || from.path.as_str() != self.source_base.source.path
        {
            return Err(fault("generated include invocation source base differs"));
        }
        if let Some(view) = included
            && (view.location() != from
                || view.blob().content().0 != self.source_base.source.content.0
                || view.caller_context() != self.source_base.context
                || view.origin() != self.source_base.origin)
        {
            return Err(fault("generated included invocation base differs"));
        }
        if included.is_none() && from.origin != self.source_base.origin {
            return Err(fault("generated source invocation origin differs"));
        }
        Ok(())
    }
    fn resolve_returned_expansion(
        &self,
        nested: &crate::expand::include_arguments::NestedStringInvocation,
        caller: &super::nested_compile_inputs::NestedLexicalContext,
        context: &super::nested_compile_inputs::NestedExpansionContext<'_>,
        facts: &HygieneFacts,
        _before: u64,
    ) -> Result<super::nested_compile_inputs::NestedExpansionSelection, Diagnostic> {
        self.resolve_nested(nested, caller, context, Some(facts))
    }
    fn resolve_expansion(
        &self,
        nested: &crate::expand::include_arguments::NestedStringInvocation,
        caller: &super::nested_compile_inputs::NestedLexicalContext,
        context: &super::nested_compile_inputs::NestedExpansionContext<'_>,
    ) -> Result<super::nested_compile_inputs::NestedExpansionSelection, Diagnostic> {
        self.resolve_nested(nested, caller, context, None)
    }
}
impl GeneratedEagerResolver<'_> {
    fn resolve_nested(
        &self,
        nested: &crate::expand::include_arguments::NestedStringInvocation,
        caller: &super::nested_compile_inputs::NestedLexicalContext,
        context: &super::nested_compile_inputs::NestedExpansionContext<'_>,
        returned: Option<&HygieneFacts>,
    ) -> Result<super::nested_compile_inputs::NestedExpansionSelection, Diagnostic> {
        use super::nested_compile_inputs::{NestedExpansionSelection, NestedMacroLookup};
        let span = nested
            .arguments()
            .spans()
            .iter()
            .find(|s| s.key() == nested.reference_span())
            .ok_or_else(|| fault("generated nested reference absent"))?;
        let edition = match span.edition {
            crate::syntax::token_wire::Edition::E2015 => 2015,
            crate::syntax::token_wire::Edition::E2018 => 2018,
            crate::syntax::token_wire::Edition::E2021 => 2021,
            crate::syntax::token_wire::Edition::E2024 => 2024,
        };
        if returned.is_none()
            && (span.source.unit.0 != caller.scope.crate_key.0
                || span.source.content.0 != caller.source.content.0
                || edition != caller.source.profile.edition)
        {
            return Err(fault("generated nested source context differs"));
        }
        let name = MacroName::parse(
            nested
                .segments()
                .last()
                .ok_or_else(|| fault("generated nested member absent"))?,
            edition,
        )?;
        let bare = !nested.absolute() && nested.segments().len() == 1;
        let textual = if bare {
            self.catalog.resolve_reference_with_facts(
                MacroReference {
                    scope: caller.textual_scope,
                    before: self.before,
                    name: &name,
                    context: span.context,
                    origin: caller.origin,
                },
                Some(self.context),
                self.ancestors,
                false,
                &self.scopes,
                returned,
            )?
        } else {
            HygienicLookup::Selection(super::macro_origins::MacroSelection::Absent)
        };
        let path = match textual {
            HygienicLookup::NeedInput(input) => super::macro_names::PathLookup::NeedInput(input),
            HygienicLookup::Selection(super::macro_origins::MacroSelection::Defined(
                definition,
            )) => super::macro_names::PathLookup::Defined(definition),
            HygienicLookup::Selection(super::macro_origins::MacroSelection::Ambiguous(
                definitions,
            )) => super::macro_names::PathLookup::Ambiguous(definitions),
            HygienicLookup::Selection(super::macro_origins::MacroSelection::Absent) => {
                let parts = nested
                    .segments()
                    .iter()
                    .map(|p| match p.as_str() {
                        "crate" => crate::syntax::imports::ImportSegment::Crate,
                        "self" => crate::syntax::imports::ImportSegment::SelfModule,
                        "super" => crate::syntax::imports::ImportSegment::Super,
                        _ => crate::syntax::imports::ImportSegment::Name(p.clone()),
                    })
                    .collect::<Vec<_>>();
                self.namespaces.lookup_invocation_segments(
                    &parts,
                    nested.absolute(),
                    &caller.scope,
                    edition,
                )?
            }
        };
        let lookup = match path {
            super::macro_names::PathLookup::Defined(definition) => {
                match self.compiler().select(&definition)? {
                    Some(descriptor) => NestedMacroLookup::Compiler {
                        definition,
                        descriptor: descriptor.clone(),
                    },
                    None => NestedMacroLookup::Other(definition),
                }
            }
            super::macro_names::PathLookup::NeedInput(input) => NestedMacroLookup::NeedInput(input),
            super::macro_names::PathLookup::Absent => NestedMacroLookup::Absent,
            super::macro_names::PathLookup::Ambiguous(definitions) => {
                NestedMacroLookup::Ambiguous(definitions)
            }
            super::macro_names::PathLookup::Inaccessible(_) => NestedMacroLookup::Inaccessible,
        };
        NestedExpansionSelection::from_lookup(lookup, nested, caller, context)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn contained_scope(scope: &SourceScope, owner: &SourceScope) -> bool {
    (scope.owner == owner.owner
        || scope.owner.is_none() && scope.module_path.len() > owner.module_path.len())
        && scope.crate_key == owner.crate_key
        && scope.module_path.starts_with(&owner.module_path)
}
impl GeneratedMacroCatalog {
    pub(crate) fn retained_items(&self) -> &Arc<GeneratedItems> {
        &self.items
    }
    pub(crate) fn retained_declaration_scopes(
        &self,
    ) -> &Arc<BTreeMap<Digest, CapturedDeclarationScope>> {
        &self.declaration_scopes
    }
    pub(crate) fn retain_body_outer(
        &self,
        owner: &SourceScope,
        before: u64,
    ) -> Result<super::macro_boundary::RetainedMacroScope, Diagnostic> {
        let scope = *self
            .module_scopes
            .get(owner)
            .ok_or_else(|| fault("generated body module macro scope absent"))?;
        super::macro_boundary::RetainedMacroScope::new(
            Arc::clone(&self.scopes),
            Arc::clone(&self.declarations),
            Arc::clone(&self.facts),
            Arc::clone(&self.ancestry),
            scope,
            before,
        )
    }
    pub(crate) fn retain_outer(
        &self,
        outer: super::macro_scopes::OuterMacroScope<'_>,
    ) -> Result<Option<super::macro_boundary::RetainedMacroScope>, Diagnostic> {
        if !std::ptr::eq(outer.declarations, self.declarations.as_ref()) {
            return Ok(None);
        }
        let scopes = if std::ptr::eq(outer.scopes, self.scopes.as_ref()) {
            Some(&self.scopes)
        } else {
            self.derive_parent_views
                .values()
                .find(|view| std::ptr::eq(outer.scopes, view.as_ref()))
        };
        let Some(scopes) = scopes else {
            return Ok(None);
        };
        Ok(Some(super::macro_boundary::RetainedMacroScope::new(
            Arc::clone(scopes),
            Arc::clone(&self.declarations),
            Arc::clone(&self.facts),
            Arc::clone(&self.ancestry),
            outer.scope,
            outer.before,
        )?))
    }
    pub fn new(
        items: &GeneratedItems,
        owner: SourceScope,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare(items, owner, owner_context, cfg, epoch, false)
    }
    pub fn for_discovery(
        items: &GeneratedItems,
        owner: SourceScope,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        Self::prepare(items, owner, owner_context, cfg, epoch, true)
    }
    /// Root item output at its actual admitted insertion scope, including a
    /// block owner. It uses the same catalog and output producer closure.
    pub(crate) fn for_admitted_output(
        items: &GeneratedItems,
        declarations: &crate::expand::generated_declarations::GeneratedDeclarations,
        cfg: &CfgSet,
        epoch: u64,
    ) -> Result<Self, Diagnostic> {
        let context = Key(items.record().key().parent_context.0);
        Self::prepare_owned(items, declarations.scan().scope().clone(), context,
            context, cfg, epoch, true, None, Some(declarations))
    }
    pub(crate) fn require_captured_owner(
        &self, items: &GeneratedItems, scope: &SourceScope, cfg: &CfgSet, epoch: u64,
    ) -> Result<(), Diagnostic> {
        if self.items.record().descriptor().fingerprint() != items.record().descriptor().fingerprint()
            || &self.owner != scope || self.cfg != *cfg
            || self.items.epoch() != epoch || items.epoch() != epoch {
            return Err(fault("generated catalog insertion receipt differs"));
        }
        Ok(())
    }
    pub(crate) fn require_pending_owner(
        &self, items: &GeneratedItems, scope: &SourceScope, cfg: &CfgSet, epoch: u64,
    ) -> Result<(), Diagnostic> {
        if self.pending_scope()? != scope || self.cfg != *cfg
            || self.items.epoch() != epoch || items.epoch() != epoch
            || !self.pending.as_ref().is_some_and(|request| request.key() == items.record().key()) {
            return Err(fault("generated child actual pending insertion differs"));
        }
        Ok(())
    }
    pub(crate) fn inherit_original_helpers(
        &mut self,
        helpers: Arc<super::generated_derive_helpers::OriginalDeriveHelpers>,
    ) -> Result<(), Diagnostic> {
        if !self.inherited_helpers.is_empty()
            || self.items.record().key().invocation != helpers.producer()
            || !matches!(
                self.items.record().key().macro_identity,
                crate::expand::identity::MacroIdentity::Procedural {
                    kind: crate::expand::identity::ProceduralKind::Attribute,
                    ..
                }
            )
        {
            return Err(fault(
                "original helper attribute owner or expansion kind differs",
            ));
        }
        self.metadata_bytes()?
            .checked_add(inherited_helper_bytes([(helpers.retained_bytes(), 0)])?)
            .ok_or_else(|| fault("original inherited helper metadata limit"))?;
        self.inherited_helpers
            .try_reserve_exact(1)
            .map_err(|_| fault("original inherited helper allocation"))?;
        self.inherited_helpers.push(InheritedHelpers {
            source: InheritedHelperSource::Original(helpers),
            introduced: BTreeSet::new(),
        });
        Ok(())
    }
    fn prepare(
        items: &GeneratedItems,
        owner: SourceScope,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
        allow_calls: bool,
    ) -> Result<Self, Diagnostic> {
        Self::prepare_owned(
            items,
            owner,
            owner_context,
            owner_context,
            cfg,
            epoch,
            allow_calls,
            None,
            None,
        )
    }
    pub(crate) fn record_identity(&self) -> Digest {
        self.items.record().key().fingerprint()
    }
    /// Physical inline suffix in this actual retained expansion, never a name guess.
    pub(crate) fn inline_caller_extension(
        &self,
        target: &SourceScope,
    ) -> Result<crate::syntax::modules::InlineModuleLayout, Diagnostic> {
        let suffix = target
            .module_path
            .strip_prefix(self.owner.module_path.as_slice())
            .filter(|_| {
                target.crate_key == self.owner.crate_key
                    && target.owner.is_none()
                    && self.owner.owner.is_none()
            })
            .ok_or_else(|| fault("generated inline caller scope mismatch"))?;
        if suffix.is_empty() {
            return Ok(Default::default());
        }
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        crate::syntax::modules::inline_module_layout(&parsed.parsed.tree(), &self.cfg, suffix)
    }
    pub(crate) fn captured_owner(
        &self,
    ) -> (
        &GeneratedItems,
        &SourceScope,
        &crate::syntax::token_wire::ContextRecord,
    ) {
        (&self.items, &self.owner, &self.caller_root)
    }
    pub(crate) fn preflight_derive_output(
        &self,
        record: &ExpansionRecord,
    ) -> Result<usize, Diagnostic> {
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| fault("derive output has no pending request"))?;
        if !request.is_derive() || request.key() != record.key() || self.derive_outputs.contains_key(&record.key().invocation) {
            return Err(fault("derive output differs from pending occurrence"));
        }
        self.derive_output_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| fault("retained derive output limit"))
    }
    pub(crate) fn retain_derive_output(
        &mut self,
        record: Arc<ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        let bytes = self.preflight_derive_output(&record)?;
        self.derive_outputs.insert(record.key().invocation, record);
        self.derive_output_bytes = bytes;
        self.pending = None;
        Ok(())
    }
    pub(crate) fn helper_inert_count(&self) -> Result<usize, Diagnostic> {
        self.helper_scopes.values().try_fold(0usize, |n, scope| {
            n.checked_add(scope.inert_count())
                .ok_or_else(|| fault("helper attribute count overflow"))
        })
    }
    pub(crate) fn collect_helper_counts(
        &self,
        counts: &mut BTreeMap<Digest, (usize, usize)>,
        depth: usize,
    ) -> Result<(), Diagnostic> {
        let identity = self.record_identity();
        let own = (self.helper_receipt_count(), self.helper_inert_count()?);
        if let Some(old) = counts.get(&identity) {
            if *old != own {
                return Err(fault("helper catalogue statistics owner conflict"));
            }
            return Ok(());
        }
        counts.insert(identity, own);
        for child in &self.selection_children {
            child.collect_helper_counts(
                counts,
                depth
                    .checked_add(1)
                    .ok_or_else(|| fault("helper statistics depth overflow"))?,
            )?;
        }
        Ok(())
    }
    pub(crate) fn helper_receipt_count(&self) -> usize {
        self.helper_receipts.len()
    }
    pub(crate) fn preflight_helper_receipt(
        &self,
        request: &super::generated_derive_exports::DeriveDeclarationRequest,
        receipt: &crate::expand::derive_receipt::PreparedDeriveDeclaration,
    ) -> Result<usize, Diagnostic> {
        receipt.validate_request(request)?;
        if !matches!(&request.origin, crate::expand::derive_origin::DeriveDeclarationOrigin::Generated(parent)
            if parent.fingerprint()==self.items.record().descriptor().fingerprint()) || self.helper_receipts.contains_key(&request.demand()) {
            return Err(fault(
                "helper receipt origin differs or was already admitted",
            ));
        }
        self.helper_receipt_bytes
            .checked_add(receipt.estimated_bytes()?)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| fault("generated helper receipt metadata limit"))
    }
    pub(crate) fn adopt_helper_receipt(
        &mut self,
        request: &super::generated_derive_exports::DeriveDeclarationRequest,
        receipt: Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>,
    ) -> Result<(), Diagnostic> {
        let bytes = self.preflight_helper_receipt(request, &receipt)?;
        self.helper_receipts.insert(request.demand(), receipt);
        self.helper_receipt_bytes = bytes;
        Ok(())
    }
    pub(crate) fn cfg(&self) -> &CfgSet {
        &self.cfg
    }
    pub(crate) fn preflight_park_child(&self, key: &ExpansionKey) -> Result<(), Diagnostic> {
        if self
            .pending
            .as_ref()
            .is_none_or(|request| request.key() != key) || self.inflight.contains_key(&key.invocation) {
            return Err(fault("parked child request differs or is duplicated"));
        }
        Ok(())
    }
    pub(crate) fn park_child(&mut self, key: &ExpansionKey) -> Result<(), Diagnostic> {
        self.preflight_park_child(key)?;
        self.inflight.insert(key.invocation, key.clone());
        self.pending = None;
        Ok(())
    }
    pub(crate) fn pending_position(&self) -> Result<u32, Diagnostic> {
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| fault("generated parent request absent"))?;
        self.call_sites
            .get(&request.key().invocation)
            .map(|(at, _)| *at)
            .ok_or_else(|| fault("generated pending position absent"))
    }
    pub(crate) fn pending_scope(&self) -> Result<&SourceScope, Diagnostic> {
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| fault("generated pending scope request absent"))?;
        self.call_scopes
            .get(&request.key().invocation)
            .ok_or_else(|| fault("generated pending source scope absent"))
    }
    pub(crate) fn child_for_discovery(&self, items: &GeneratedItems) -> Result<Self, Diagnostic> {
        Self::prepare_owned(
            items,
            self.pending_scope()?.clone(),
            Key(items.record().key().parent_context.0),
            self.root_context,
            &self.cfg,
            self.items.epoch(),
            true,
            Some(self),
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_owned(
        items: &GeneratedItems,
        owner: SourceScope,
        call_context: Key,
        owner_context: Key,
        cfg: &CfgSet,
        epoch: u64,
        allow_calls: bool,
        parent: Option<&Self>,
        admitted: Option<&crate::expand::generated_declarations::GeneratedDeclarations>,
    ) -> Result<Self, Diagnostic> {
        if epoch != items.epoch() {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-EPOCH",
                "generated macro catalog epoch differs".into(),
            ));
        }
        if let Some(declarations) = admitted {
            if declarations.record().descriptor().fingerprint() != items.record().descriptor().fingerprint()
                || declarations.scan().scope() != &owner || declarations.scan().cfg() != cfg.fingerprint()
                || declarations.epoch() != epoch {
                return Err(fault("generated macro admitted insertion differs"));
            }
        }
        if owner.owner.is_some() && admitted.is_none() && parent.is_none()
            || items.record().key().parent_context != Digest(call_context.0) {
            return Err(fault("generated macro owner scope/context differs"));
        }
        let context = if let Some(parent) = parent {
            if parent.pending_scope()? != &owner
                || parent.root_context != owner_context
                || !parent
                    .pending
                    .as_ref()
                    .is_some_and(|request| request.key() == items.record().key())
            {
                return Err(fault(
                    "generated child caller scope or exact pending request differs",
                ));
            }
            parent.caller_root.clone()
        } else {
            let context = items
                .tokens()
                .contexts()
                .iter()
                .find(|c| c.key() == owner_context)
                .ok_or_else(|| fault("generated macro owner context absent"))?;
            if context.parent.is_some() || context.defining_unit.0 != owner.crate_key.0 {
                return Err(fault("generated macro owner is not caller source root"));
            }
            let mut cursor = call_context;
            let mut steps = 0usize;
            while cursor != owner_context {
                steps = steps
                    .checked_add(1)
                    .filter(|n| *n <= items.tokens().contexts().len())
                    .ok_or_else(|| fault("generated macro context ancestry cycles"))?;
                cursor = items
                    .tokens()
                    .contexts()
                    .iter()
                    .find(|c| c.key() == cursor)
                    .and_then(|c| c.parent)
                    .ok_or_else(|| {
                        fault("generated macro call does not reach captured caller root")
                    })?;
            }
            context.clone()
        };
        let parsed = ParsedFile::from_generated_items(items)?;
        let outline = if admitted.is_some() || parent.is_some() {
            parsed.outline_item_fragment(&owner, cfg)?
        } else {
            parsed.outline(owner.crate_key, owner.module_path.clone(), cfg)?
        };
        let permitted = outline
            .item_invocations
            .iter()
            .filter(|c| {
                allow_calls && contained_scope(&c.scope, &owner) && c.attribute_input.is_none()
            })
            .map(|c| c.item_input)
            .collect::<std::collections::BTreeSet<_>>();
        let mut frontiers = outline
            .item_invocations
            .iter()
            .filter(|c| permitted.contains(&c.item_input))
            .map(|c| c.frontier)
            .collect::<std::collections::BTreeSet<_>>();
        let attributes = outline
            .items
            .iter()
            .filter_map(|item| {
                outline.item_attribute_producer(item)
                    .map(|call| (call.item_input, call.frontier))
            })
            .collect::<BTreeMap<_, _>>();
        if allow_calls {
            frontiers.extend(attributes.values().copied());
        }
        let module_plan = if let Some(declarations) = admitted {
            super::generated_headers::GeneratedHeaderPlan::prepare_admitted_with_root(
                items, declarations, call_context, &context, cfg, epoch,
            )?
        } else if let Some(parent) = parent {
            super::generated_headers::GeneratedHeaderPlan::prepare_pending_child(
                items, owner.clone(), call_context, &context, cfg, epoch, parent,
            )?
        } else {
            super::generated_headers::GeneratedHeaderPlan::prepare_with_captured_root(
                items, owner.clone(), call_context, &context, cfg, epoch,
            )?
        };
        let mut module_scopes_allowed = module_plan
            .module_sources()
            .iter()
            .map(|d| d.scope().clone())
            .collect::<BTreeSet<_>>();
        for item in &outline.items {
            if item.def_key.item_kind == ItemKind::Module
                && outline.item_attribute_producer(item).is_some()
            {
                let mut child = item.scope.clone();
                child.module_path.push(item.def_key.declared_name.clone());
                module_scopes_allowed.insert(child);
            }
        }
        if !outline.scopes.contains_key(&owner)
            || outline.scopes.iter().any(|(scope, m)| {
                m.blockers.iter().any(|b| {
                    !contained_scope(scope, &owner)
                        || !matches!(
                            b.kind,
                            crate::syntax::outline::BlockerKind::ItemMacro
                                | crate::syntax::outline::BlockerKind::Attribute
                        )
                        || !frontiers.contains(&b.syntax)
                })
            })
            || outline.textual_events.iter().any(|e| match e.kind {
                crate::syntax::outline::TextualEventKind::Producer { identity, kind } => {
                    !contained_scope(&e.scope, &owner)
                        || match kind {
                            crate::syntax::outline::ProducerKind::ItemMacro => {
                                !permitted.contains(&identity)
                            }
                            crate::syntax::outline::ProducerKind::ModuleSource => {
                                !allow_calls || !module_scopes_allowed.contains(&e.scope)
                            }
                            crate::syntax::outline::ProducerKind::Attribute => {
                                !allow_calls || !attributes.contains_key(&identity)
                            }
                        }
                }
                _ => false,
            })
        {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "generated macro fragment has unresolved producers".into(),
            ));
        }
        let scope = ScopeKey::root(owner.fingerprint());
        let mut scopes = TextualScopes::default();
        scopes.add_root(scope)?;
        let mut module_scopes = BTreeMap::from([(owner.clone(), scope)]);
        let mut declaration_lexical_scopes = BTreeMap::from([(owner.clone(), scope)]);
        let mut scope_bytes = owner.heap_bytes();
        for event in &outline.textual_events {
            if let crate::syntax::outline::TextualEventKind::OpenScope { child, ordinal } =
                &event.kind
                && child.crate_key == owner.crate_key
                && child.module_path.starts_with(&owner.module_path)
            {
                scope_bytes = scope_bytes
                    .checked_add(child.heap_bytes())
                    .and_then(|n| n.checked_add(160))
                    .ok_or_else(|| fault("generated module scope metadata limit"))?;
                let parent = *declaration_lexical_scopes
                    .get(&event.scope)
                    .ok_or_else(|| fault("generated module lexical parent absent"))?;
                // Associated declarations inherit the actual opening frontier,
                // like PublishedMacroIndex. Existing module cutoffs stay put.
                let before = if child.owner.is_some() {
                    event.at
                        .checked_add(1)
                        .ok_or_else(|| fault("generated scope position overflow"))?
                } else {
                    event.at
                };
                let key = scopes.add_child(parent, before, *ordinal)?;
                if declaration_lexical_scopes.insert(child.clone(), key).is_some() {
                    return Err(fault("generated module scope duplicated"));
                }
                if contained_scope(child, &owner) {
                    module_scopes.insert(child.clone(), key);
                }
            }
        }
        let mut module_sources = BTreeMap::new();
        for demand in module_plan.module_sources() {
            let header = items
                .items()
                .iter()
                .find(|h| &h.key == demand.header())
                .ok_or_else(|| fault("module canonical parent header absent"))?;
            let item = outline
                .items
                .iter()
                .find(|item| {
                    item.declaration_span.start == header.start
                        && item.declaration_span.end == header.end
                })
                .ok_or_else(|| fault("module parent source scope absent"))?;
            let lexical = *module_scopes
                .get(&item.scope)
                .ok_or_else(|| fault("module parent lexical scope absent"))?;
            let child = *module_scopes
                .get(demand.scope())
                .ok_or_else(|| fault("module child lexical scope absent"))?;
            scopes.block(child, 0, demand.producer())?;
            scope_bytes = scope_bytes
                .checked_add(demand.metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("module parent scope metadata limit"))?;
            if module_sources
                .insert(
                    demand.producer(),
                    ModuleParent {
                        demand: demand.clone(),
                        lexical,
                        before: u64::from(header.start),
                    },
                )
                .is_some()
            {
                return Err(fault("duplicate module parent scope"));
            }
        }
        for item in &outline.items {
            if item.def_key.item_kind != ItemKind::Module
                || !item.header.attributes.iter().any(|attribute| {
                    attribute.name == "macro_use"
                        && attribute.class == crate::syntax::outline::AttributeClass::Harmless
                })
            {
                continue;
            }
            let mut child = item.scope.clone();
            child.module_path.push(item.def_key.declared_name.clone());
            let parent = *module_scopes
                .get(&item.scope)
                .ok_or_else(|| fault("generated macro-use parent scope absent"))?;
            let child = *module_scopes
                .get(&child)
                .ok_or_else(|| fault("generated macro-use child scope absent"))?;
            scopes.macro_use_module(parent, child, u64::from(item.declaration_span.end))?;
        }
        let mut facts = parent
            .map(|p| p.facts.clone())
            .unwrap_or_else(|| items.hygiene_facts());
        Arc::make_mut(&mut facts).extend(&items.hygiene_facts())?;
        let mut declarations = MacroDeclarations::default();
        let mut ancestry = parent
            .map(|p| p.ancestry.as_ref().clone())
            .unwrap_or_default();
        let origin = Key(items.record().key().fingerprint().0);
        ancestry.register(origin, parent.map(Self::origin))?;
        let headers = items
            .items()
            .iter()
            .map(|h| ((h.start, h.end), h))
            .collect::<BTreeMap<_, _>>();
        let mut declaration_scopes = BTreeMap::new();
        for item in &outline.items {
            let Some(header) = headers.get(&(item.declaration_span.start, item.declaration_span.end)) else {
                continue;
            };
            let lexical = *declaration_lexical_scopes
                .get(&item.scope)
                .ok_or_else(|| fault("generated declaration lexical parent absent"))?;
            if declaration_scopes.insert(
                header.key.fingerprint(),
                CapturedDeclarationScope {
                    context: crate::syntax::outline::DeclarationContext {
                        scope: item.scope.clone(),
                        range: item.declaration_span,
                    },
                    lexical,
                },
            ).is_some() {
                return Err(fault("generated canonical declaration scope duplicated"));
            }
        }
        let spans = items
            .tokens()
            .spans()
            .iter()
            .map(|s| (s.key().0, s))
            .collect::<BTreeMap<_, _>>();
        let mut definitions = BTreeMap::new();
        let mut outward = Vec::new();
        for item in &outline.items {
            if !contained_scope(&item.scope, &owner)
                || item.def_key.item_kind != ItemKind::Macro
                || outline.item_attribute_producer(item).is_some()
            {
                continue;
            }
            let header = headers
                .get(&(item.declaration_span.start, item.declaration_span.end))
                .ok_or_else(|| fault("generated macro header absent"))?;
            if header.kind != crate::expand::generated_items::GeneratedHeaderKind::MacroRules {
                continue;
            }
            let name = header
                .name
                .as_ref()
                .ok_or_else(|| fault("generated macro name absent"))?;
            let context = spans
                .get(&name.span.0)
                .ok_or_else(|| fault("generated macro name span absent"))?
                .context;
            let definition = MacroDefinition {
                identity: header.key.fingerprint(),
                defining_unit: owner.crate_key,
                definition_edition: items.macro_edition(header.key.fingerprint())?,
            };
            scopes.define(
                *module_scopes
                    .get(&item.scope)
                    .ok_or_else(|| fault("generated macro lexical module absent"))?,
                u64::from(header.start),
                MacroName::parse(&name.text, definition.definition_edition)?,
                definition.clone(),
            )?;
            declarations.register(definition.clone(), context, Some(origin), &facts, &ancestry)?;
            scope_bytes = scope_bytes
                .checked_add(item.scope.heap_bytes())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("generated outward scope metadata limit"))?;
            outward
                .try_reserve(1)
                .map_err(|_| fault("outward macro declaration allocation"))?;
            outward.push(OutwardMacro {
                scope: item.scope.clone(),
                name: MacroName::parse(&name.text, definition.definition_edition)?,
                definition: definition.clone(),
                context,
                origin: Some(origin),
                at: header.start,
            });
            definitions.insert(definition.identity, definition);
        }
        let indices = items
            .calls()
            .iter()
            .enumerate()
            .map(|(i, c)| (c.key.fingerprint(), i))
            .collect::<BTreeMap<_, _>>();
        let mut calls = BTreeMap::new();
        let mut call_sites = BTreeMap::new();
        let mut call_scopes = BTreeMap::new();
        for call in &outline.item_invocations {
            if !permitted.contains(&call.item_input) {
                continue;
            }
            let header = headers
                .get(&(call.range.start, call.range.end))
                .ok_or_else(|| fault("active generated call header absent"))?;
            let identity = header.key.fingerprint();
            let index = *indices
                .get(&identity)
                .ok_or_else(|| fault("active generated call token facts absent"))?;
            if call_sites
                .insert(
                    identity,
                    (header.start, GeneratedCallIndex::Function(index)),
                )
                .is_some()
            {
                return Err(fault("duplicate generated call-site identity"));
            }
            scope_bytes = scope_bytes
                .checked_add(call.scope.heap_bytes())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("generated call scope metadata limit"))?;
            call_scopes.insert(identity, call.scope.clone());
            scopes.block(
                *module_scopes
                    .get(&call.scope)
                    .ok_or_else(|| fault("generated call lexical module absent"))?,
                u64::from(header.start),
                identity,
            )?;
            calls.insert(
                (header.start, identity),
                GeneratedCallIndex::Function(index),
            );
        }
        let mut test_registration_candidates = BTreeMap::new();
        let mut test_registration_candidate_bytes = 0usize;
        for item in &outline.items {
            if !allow_calls || outline.item_attribute_producer(item).is_none()
            {
                continue;
            }
            let header = headers
                .get(&(item.declaration_span.start, item.declaration_span.end))
                .ok_or_else(|| fault("generated attribute structural header absent"))?;
            let index = items
                .items()
                .partition_point(|candidate| candidate.start < header.start);
            if items
                .items()
                .get(index)
                .is_none_or(|candidate| candidate.key != header.key)
            {
                return Err(fault("generated attribute canonical index differs"));
            }
            let identity = header.key.fingerprint();
            if item.def_key.item_kind == ItemKind::Function
                && item
                    .header
                    .attributes
                    .iter()
                    .any(|attribute| attribute.name == "test")
            {
                let metadata = crate::syntax::outline::ItemOutline {
                    def_key: item.def_key.clone(),
                    scope: item.scope.clone(),
                    header: item.header.clone(),
                    body: None,
                    constructor_fields: Vec::new(),
                    variants: Vec::new(),
                    discriminants: Vec::new(),
                    declaration_span: item.declaration_span,
                    header_span: item.header_span,
                };
                test_registration_candidate_bytes = test_registration_candidate_bytes
                    .checked_add(test_registration_bytes(&metadata)?)
                    .ok_or_else(|| fault("generated test registration candidate byte limit"))?;
                if test_registration_candidates
                        .insert(identity, Arc::new(metadata))
                        .is_some() {
                    return Err(fault(
                        "generated test registration candidate count/conflict",
                    ));
                }
            }
            if call_sites
                    .insert(
                        identity,
                        (header.start, GeneratedCallIndex::Attribute(index)),
                    )
                    .is_some() {
                return Err(fault("generated attribute call index limit/conflict"));
            }
            scope_bytes = scope_bytes
                .checked_add(item.scope.heap_bytes())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("generated attribute scope limit"))?;
            call_scopes.insert(identity, item.scope.clone());
            scopes.block(
                *module_scopes
                    .get(&item.scope)
                    .ok_or_else(|| fault("generated attribute lexical scope absent"))?,
                u64::from(header.start),
                identity,
            )?;
            calls.insert(
                (header.start, identity),
                GeneratedCallIndex::Attribute(index),
            );
        }
        let mut inherited_helpers = Vec::new();
        if let Some(parent) = parent
            && matches!(
                items.record().key().macro_identity,
                crate::expand::identity::MacroIdentity::Procedural {
                    kind: crate::expand::identity::ProceduralKind::Attribute,
                    ..
                }
            )
        {
            let local = parent.helper_scopes.get(&items.record().key().invocation);
            let count = parent
                .inherited_helpers
                .len()
                .checked_add(usize::from(local.is_some()))
                .ok_or_else(|| fault("inherited helper scope count limit"))?;
            inherited_helper_bytes(
                parent
                    .inherited_helpers
                    .iter()
                    .map(|e| (e.source.retained_bytes(), e.introduced.len()))
                    .chain(local.map(|source| {
                        (
                            source.retained_bytes(),
                            source
                                .occurrences()
                                .filter(|id| parent.appended_derives.contains(id))
                                .count(),
                        )
                    })),
            )?;
            inherited_helpers
                .try_reserve_exact(count)
                .map_err(|_| fault("inherited helper allocation"))?;
            inherited_helpers.extend(parent.inherited_helpers.iter().cloned());
            if let Some(source) = local {
                let introduced = source
                    .occurrences()
                    .filter(|id| parent.appended_derives.contains(id))
                    .collect();
                inherited_helpers.push(InheritedHelpers {
                    source: InheritedHelperSource::Generated(source.clone()),
                    introduced,
                });
            }
        }
        Ok(Self {
            owner,
            root_context: owner_context,
            caller_root: context,
            outward,
            items: Arc::new(items.clone()),
            cfg: cfg.clone(),
            calls,
            call_sites,
            call_scopes,
            module_scopes: Arc::new(module_scopes),
            declaration_scopes: Arc::new(declaration_scopes),
            module_sources,
            completed_module_sources: BTreeMap::new(),
            source_body_owners: BTreeMap::new(),
            call_frontiers: frontiers
                .into_iter()
                .map(|id| items.frontier_identity(id))
                .collect(),
            pending: None,
            helper_receipts: BTreeMap::new(),
            helper_receipt_bytes: 0,
            helper_scopes: BTreeMap::new(),
            helper_scope_bytes: 0,
            derive_outputs: BTreeMap::new(),
            derive_output_bytes: 0,
            appended_derives: std::collections::BTreeSet::new(),
            settled_derive_items: std::collections::BTreeSet::new(),
            empty_derives: BTreeMap::new(),
            inherited_helpers,
            inherited_items: BTreeMap::new(),
            compiler_test_items: BTreeMap::new(),
            compiler_test_prefixes: BTreeMap::new(),
            compiler_test_prefix_bytes: 0,
            test_registration_candidates,
            test_registration_candidate_bytes,
            test_registrations: BTreeMap::new(),
            derive_parent_views: BTreeMap::new(),
            derive_parent_view_bytes: 0,
            completed: BTreeMap::new(),
            inflight: BTreeMap::new(),
            completed_wire_bytes: 0,
            completed_allocated_bytes: 0,
            completed_headers: 0,
            definitions,
            body_owners: BTreeMap::new(),
            route_children: Vec::new(),
            selection_children: Vec::new(),
            scopes: Arc::new(scopes),
            scope,
            declarations: Arc::new(declarations),
            facts,
            ancestry: Arc::new(ancestry),
            edition: items.record().key().call_edition,
        })
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self.items.metadata_bytes()?;
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(|| fault("generated catalogue metadata overflow"))?;
            Ok(())
        };
        add(std::mem::size_of::<Self>())?;
        if let Some(request) = &self.pending {
            add(request.procedural_bytes()?)?;
        }
        add(self.owner.heap_bytes())?;
        add(self.scopes.metadata_bytes()?)?;
        add(self.derive_parent_view_bytes)?;
        add(self.test_registration_candidate_bytes)?;
        add(self
            .test_registrations
            .len()
            .checked_mul(128)
            .ok_or_else(|| fault("generated test registration index overflow"))?)?;
        add(inherited_helper_bytes(
            self.inherited_helpers
                .iter()
                .map(|e| (e.source.retained_bytes(), e.introduced.len())),
        )?)?;
        add(self
            .inherited_items
            .len()
            .checked_mul(128)
            .ok_or_else(|| fault("inherited item metadata overflow"))?)?;
        for (indices, _) in self.compiler_test_prefixes.values() {
            add(indices
                .len()
                .checked_mul(64)
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("compiler test prefix metadata overflow"))?)?;
        }
        add(self
            .compiler_test_items
            .len()
            .checked_mul(128)
            .ok_or_else(|| fault("compiler test metadata overflow"))?)?;
        add(self
            .empty_derives
            .len()
            .checked_mul(std::mem::size_of::<EmptyDeriveCertificate>() + 128)
            .ok_or_else(|| fault("empty derive certificate metadata overflow"))?)?;
        for scope in self.module_scopes.keys().chain(self.call_scopes.values()) {
            add(scope
                .heap_bytes()
                .checked_add(160)
                .ok_or_else(|| fault("generated lexical scope metadata overflow"))?)?;
        }
        for declaration in self.declaration_scopes.values() {
            add(declaration.context.scope.heap_bytes()
                .checked_add(160 + std::mem::size_of::<CapturedDeclarationScope>())
                .ok_or_else(|| fault("generated declaration scope metadata overflow"))?)?;
        }
        for entry in &self.outward {
            add(entry.scope.heap_bytes())?;
        }
        for module in self
            .module_sources
            .values()
            .chain(self.completed_module_sources.values())
        {
            add(module
                .demand
                .metadata_bytes()?
                .checked_add(128)
                .ok_or_else(|| fault("module catalogue metadata overflow"))?)?;
        }
        add(self.declarations.metadata_bytes()?)?;
        add(self.helper_receipt_bytes)?;
        add(self.helper_scope_bytes)?;
        add(self.derive_output_bytes)?;
        add(self.facts.metadata_bytes()?)?;
        add(self.ancestry.metadata_bytes()?)?;
        for (count, size) in [
            (self.definitions.len(), 256),
            (self.calls.len(), 128),
            (self.call_sites.len(), 96),
            (self.call_frontiers.len(), 96),
            (self.completed.len(), 128),
            (
                self.inflight.len(),
                std::mem::size_of::<ExpansionKey>() + 96,
            ),
            (self.body_owners.len(), 128),
            (self.source_body_owners.len(), 128),
        ] {
            add(count
                .checked_mul(size)
                .ok_or_else(|| fault("generated catalogue index metadata overflow"))?)?;
        }
        add(self
            .outward
            .capacity()
            .checked_mul(std::mem::size_of::<OutwardMacro>())
            .ok_or_else(|| fault("outward catalogue metadata overflow"))?)?;
        add(self
            .route_children
            .capacity()
            .checked_mul(std::mem::size_of::<Arc<GeneratedRouteSnapshot>>())
            .ok_or_else(|| fault("child route index metadata overflow"))?)?;
        add(self
            .selection_children
            .capacity()
            .checked_mul(std::mem::size_of::<Arc<Self>>())
            .ok_or_else(|| fault("selection child index metadata overflow"))?)?;
        for entry in &self.outward {
            add(entry
                .name
                .as_str()
                .len()
                .checked_mul(4)
                .ok_or_else(|| fault("outward name metadata overflow"))?)?;
        }
        Ok(bytes)
    }
    pub(crate) fn call_frontiers(&self) -> &std::collections::BTreeSet<Digest> {
        &self.call_frontiers
    }
    /// Reuse an empty-derive receipt issued by ordinary item discovery. The
    /// immutable catalog and output must refer to the same actual record.
    pub(crate) fn completed_declaration_call(
        &self,
        output: &crate::expand::generated_declarations::GeneratedDeclarations,
        invocation: Digest,
    ) -> Result<Option<Arc<crate::expand::generated_declarations::GeneratedDeclarations>>, Diagnostic>
    {
        let mut ancestors = Vec::new();
        let Some(catalog) =
            self.find_catalogue(output.record().key().fingerprint(), &mut ancestors)?
        else {
            return Ok(None);
        };
        if catalog.items.record().descriptor().fingerprint()
            != output.record().descriptor().fingerprint()
            || catalog.cfg.fingerprint() != output.scan().cfg()
        {
            return Err(fault("completed declaration call owner differs"));
        }
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("completed declaration producer absent"))?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            || producer.grammar != crate::expand::identity::GrammarSlot::Items
        {
            return Ok(None);
        }
        let call = output.macro_call(invocation)?;
        let mut producers = Vec::new();
        producers
            .try_reserve_exact(output.scan().producers().len())
            .map_err(|_| fault("completed call producer allocation"))?;
        for row in output.scan().producers().iter().filter(|row| {
            row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                && row.grammar == crate::expand::identity::GrammarSlot::Items
        }) {
            if output.macro_call(row.identity)?.site == call.site {
                producers.push(row)
            }
        }
        producers.sort_by_key(|row| row.range.start);
        let ordinal = producers
            .iter()
            .position(|row| row.identity == invocation)
            .ok_or_else(|| fault("completed call producer ordinal absent"))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(catalog.completed.len())
            .map_err(|_| fault("completed call record allocation"))?;
        for record in catalog.completed.values() {
            let Some((_, GeneratedCallIndex::Function(index))) =
                catalog.call_sites.get(&record.key().invocation)
            else {
                continue;
            };
            let actual = catalog
                .items
                .calls()
                .get(*index)
                .ok_or_else(|| fault("completed call syntax absent"))?;
            if actual.site.0 == call.site.0 {
                if actual.key.fingerprint() != record.key().invocation
                    || actual.edition != record.key().call_edition
                {
                    return Err(fault(
                        "completed call record invocation or token edition differs",
                    ));
                }
                records.push((&actual.key.structural_path, record));
            }
        }
        if records.is_empty() {
            return Ok(None);
        }
        records.sort_by(|a, b| a.0.cmp(b.0));
        if records.len() != producers.len() {
            return Err(fault("completed call origin occurrence coverage differs"));
        }
        let record = records
            .get(ordinal)
            .ok_or_else(|| fault("completed call origin ordinal absent"))?
            .1;
        Ok(Some(Arc::new(crate::expand::generated_declarations::GeneratedDeclarations::admit_completed_item_call(record.clone(),output.epoch(),output.scan(),invocation,&catalog.cfg)?)))
    }
    /// Pair the two owned syntax materializations by token origin sequence and
    /// structural occurrence, never by their independently rendered offsets.
    pub(crate) fn completed_declaration_attribute(
        &self,
        output: &crate::expand::generated_declarations::GeneratedDeclarations,
        invocation: Digest,
    ) -> Result<Option<CompletedDeclarationAttribute>, Diagnostic> {
        let mut ancestors = Vec::new();
        let Some(catalog) =
            self.find_catalogue(output.record().key().fingerprint(), &mut ancestors)?
        else {
            return Ok(None);
        };
        if catalog.items.record().descriptor().fingerprint()
            != output.record().descriptor().fingerprint()
            || catalog.items.epoch() != output.epoch()
            || catalog.cfg.fingerprint() != output.scan().cfg()
        {
            return Err(fault("completed attribute declaration owner differs"));
        }
        let Some(range) = output.optional_attribute_item_range(invocation)? else {
            // File inner annotations have no item replacement attachment.
            return Ok(None);
        };
        let origin_identity = |origins: &[crate::syntax::spans::TokenOrigin],
                               range: crate::syntax::spans::SourceRange|
         -> Result<Digest, Diagnostic> {
            let start = origins.partition_point(|o| o.start < range.start);
            let end = origins.partition_point(|o| o.end <= range.end);
            let rows = origins
                .get(start..end)
                .filter(|r| !r.is_empty())
                .ok_or_else(|| fault("completed attribute origins absent or excessive"))?;
            let mut encoded = StableEncoder::new("lrr/v2/completed-attribute-origin-sequence");
            encoded.u64(
                u64::try_from(rows.len()).map_err(|_| fault("attribute origin count overflow"))?,
            );
            for row in rows {
                encoded.bytes(&row.span.0);
            }
            Ok(encoded.finish())
        };
        let origins = origin_identity(output.origins(), range)?;
        let selected = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("attribute declaration producer absent"))?;
        let meta_ordinal = output
            .scan()
            .producers()
            .iter()
            .filter(|p| {
                p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                    && p.path == selected.path
                    && range.start <= p.range.start
                    && p.range.end <= range.end
                    && p.range.start < selected.range.start
            })
            .count();

        let mut producers = Vec::new();
        producers
            .try_reserve_exact(output.scan().producers().len())
            .map_err(|_| fault("attribute producer allocation"))?;
        for p in output.scan().producers().iter().filter(|p| {
            p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
        }) {
            let Some(r) = output.optional_attribute_item_range(p.identity)? else {
                continue;
            };
            let ordinal = output
                .scan()
                .producers()
                .iter()
                .filter(|other| {
                    other.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                        && other.path == p.path
                        && r.start <= other.range.start
                        && other.range.end <= r.end
                        && other.range.start < p.range.start
                })
                .count();
            let same = p.path == selected.path
                && ordinal == meta_ordinal
                && origin_identity(output.origins(), r)? == origins;
            if same {
                producers.push(p);
            }
        }
        producers.sort_by_key(|p| p.range.start);
        let ordinal = producers
            .iter()
            .position(|p| p.identity == invocation)
            .ok_or_else(|| fault("attribute origin ordinal absent"))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(catalog.completed.len())
            .map_err(|_| fault("attribute record allocation"))?;
        for record in catalog.completed.values() {
            let Some((_, GeneratedCallIndex::Attribute(index))) =
                catalog.call_sites.get(&record.key().invocation)
            else {
                continue;
            };
            let header = catalog
                .items
                .items()
                .get(*index)
                .ok_or_else(|| fault("completed attribute header absent"))?;
            if origin_identity(
                catalog.items.origins(),
                crate::syntax::spans::SourceRange {
                    start: header.start,
                    end: header.end,
                },
            )? == origins
            {
                let Some(input) = crate::expand::generated_attribute_input::prepare_remaining(
                    &catalog.items,
                    *index,
                    &catalog.cfg,
                    &catalog.consumed_attribute_indices(record.key().invocation),
                    &catalog.inert_attribute_indices(record.key().invocation)?,
                    &catalog
                        .compiler_test_prefixes
                        .get(&record.key().invocation)
                        .map(|row| row.0.clone())
                        .unwrap_or_default(),
                )?
                else {
                    return Err(fault("completed attribute selected input absent"));
                };
                let parsed = ParsedFile::from_generated_items(&catalog.items)?;
                let facts = parsed.attribute_inputs_at(
                    crate::syntax::spans::SourceRange {
                        start: header.start,
                        end: header.end,
                    },
                    &catalog.cfg,
                )?;
                let actual = facts
                    .attributes
                    .iter()
                    .find(|attribute| attribute.range == input.syntax.range())
                    .ok_or_else(|| fault("completed attribute selected meta absent"))?;
                let ordinal = facts
                    .attributes
                    .iter()
                    .filter(|other| {
                        other.tokens == actual.tokens && other.range.start < actual.range.start
                    })
                    .count();
                if actual.tokens != selected.path || ordinal != meta_ordinal {
                    continue;
                }
                if header.key.fingerprint() != record.key().invocation
                    || !matches!(
                        &record.key().macro_identity,
                        crate::expand::identity::MacroIdentity::Procedural {
                            kind: crate::expand::identity::ProceduralKind::Attribute,
                            ..
                        }
                    )
                {
                    return Err(fault("completed attribute record identity differs"));
                }
                records.push((&header.key.structural_path, record));
            }
        }
        if records.is_empty() {
            return Ok(None);
        }
        records.sort_by(|a, b| a.0.cmp(b.0));
        if records.len() != producers.len() {
            return Err(fault(
                "completed attribute origin occurrence coverage differs",
            ));
        }
        let record = records
            .get(ordinal)
            .ok_or_else(|| fault("completed attribute record ordinal absent"))?
            .1;
        let child = crate::expand::generated_declarations::GeneratedDeclarations::admit_completed_item_call(record.clone(), output.epoch(), output.scan(), invocation, &catalog.cfg)?;
        Ok(Some((Arc::new(child), range)))
    }
    /// Reuse an inert helper decision only from the exact completed owned catalog.
    /// The receipt includes token origins and the hygienic helper environment.
    /// Actual ordered append records for a completed compiler annotation.
    pub(crate) fn completed_declaration_derives(
        &self,
        output: &crate::expand::generated_declarations::GeneratedDeclarations,
        invocation: Digest,
    ) -> Result<Option<CompletedDeclarationDerives>, Diagnostic> {
        let mut ancestors = Vec::new();
        let Some(catalog) =
            self.find_catalogue(output.record().key().fingerprint(), &mut ancestors)?
        else {
            return Ok(None);
        };
        if catalog.items.record().descriptor().fingerprint()
            != output.record().descriptor().fingerprint()
            || catalog.items.epoch() != output.epoch()
            || catalog.cfg.fingerprint() != output.scan().cfg()
        {
            return Err(fault("completed derive declaration owner differs"));
        }
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("derive declaration producer absent"))?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute {
            return Ok(None);
        }
        let Some(item_range) = output.optional_attribute_item_range(invocation)? else {
            return Ok(None);
        };
        let origin = |rows: &[crate::syntax::spans::TokenOrigin],
                      range: crate::syntax::spans::SourceRange|
         -> Result<Digest, Diagnostic> {
            let start = rows.partition_point(|r| r.start < range.start);
            let end = rows.partition_point(|r| r.end <= range.end);
            let rows = rows
                .get(start..end)
                .filter(|r| !r.is_empty())
                .ok_or_else(|| fault("derive origin range absent or excessive"))?;
            let mut e = StableEncoder::new("lrr/v2/completed-derive-item-origins");
            e.u64(u64::try_from(rows.len()).map_err(|_| fault("derive origin count overflow"))?);
            for row in rows {
                e.bytes(&row.span.0);
            }
            Ok(e.finish())
        };
        let target = origin(output.origins(), item_range)?;
        let ordinal = output
            .scan()
            .producers()
            .iter()
            .filter(|p| {
                p.kind == producer.kind
                    && p.path == producer.path
                    && item_range.start <= p.range.start
                    && p.range.end <= item_range.end
                    && p.range.start < producer.range.start
            })
            .count();
        let parsed = ParsedFile::from_generated_items(&catalog.items)?;
        let mut result = None;
        for (item, header) in catalog.items.items().iter().enumerate() {
            let range = crate::syntax::spans::SourceRange {
                start: header.start,
                end: header.end,
            };
            if !matches!(
                header.kind,
                crate::expand::generated_items::GeneratedHeaderKind::Struct
                    | crate::expand::generated_items::GeneratedHeaderKind::Enum
                    | crate::expand::generated_items::GeneratedHeaderKind::Union
            ) || origin(catalog.items.origins(), range)? != target
            {
                continue;
            }
            let facts = parsed.derive_item_inputs_at(range, &catalog.cfg)?;
            let Some((annotation, _)) = facts
                .item
                .attributes
                .iter()
                .enumerate()
                .filter(|(_, a)| a.tokens == producer.path)
                .nth(ordinal)
            else {
                continue;
            };
            let Some(helpers) = catalog.helper_scopes.get(&header.key.fingerprint()) else {
                continue;
            };
            if !helpers
                .completed_annotations(&catalog.appended_derives)
                .contains(&annotation)
            {
                continue;
            }
            let mut outputs = Vec::new();
            let mut e = StableEncoder::new("lrr/v2/completed-declaration-derive-append");
            e.digest(output.fingerprint());
            e.digest(invocation);
            e.digest(helpers.fingerprint());
            for (at, path) in helpers.declaration_occurrences() {
                if at != annotation {
                    continue;
                }
                let record = catalog
                    .completed
                    .values()
                    .find(|r| r.key().invocation == path)
                    .ok_or_else(|| fault("completed derive append record absent"))?;
                if !matches!(catalog.call_sites.get(&path), Some((_, GeneratedCallIndex::Derive(i, _))) if *i == item)
                    || !record.key().macro_identity.is_derive()
                {
                    return Err(fault("completed derive append site differs"));
                }
                outputs
                    .try_reserve(1)
                    .map_err(|_| fault("derive append receipt allocation"))?;
                e.digest(record.descriptor().fingerprint());
                outputs.push(Arc::new(crate::expand::generated_declarations::GeneratedDeclarations::admit_completed_item_call(record.clone(), output.epoch(), output.scan(), invocation, &catalog.cfg)?));
            }
            if outputs.is_empty() {
                return Err(fault("completed nonempty derive annotation has no records"));
            }
            if result.is_some() {
                return Err(fault("completed derive item origins ambiguous"));
            }
            result = Some((e.finish(), outputs));
        }
        Ok(result)
    }
    pub(crate) fn completed_helper_declaration(
        &self,
        output: &crate::expand::generated_declarations::GeneratedDeclarations,
        invocation: Digest,
    ) -> Result<Option<Digest>, Diagnostic> {
        let mut ancestors = Vec::new();
        let Some(catalog) =
            self.find_catalogue(output.record().key().fingerprint(), &mut ancestors)?
        else {
            return Ok(None);
        };
        if catalog.items.record().descriptor().fingerprint()
            != output.record().descriptor().fingerprint()
            || catalog.items.epoch() != output.epoch()
            || catalog.cfg.fingerprint() != output.scan().cfg()
        {
            return Err(fault("completed helper declaration owner differs"));
        }
        if let Some(empty) = catalog.completed_empty_declaration(output, invocation)? {
            return Ok(Some(empty));
        }
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("completed helper producer absent"))?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute {
            return Ok(None);
        }
        let origin = |rows: &[crate::syntax::spans::TokenOrigin],
                      range: crate::syntax::spans::SourceRange|
         -> Result<Digest, Diagnostic> {
            let start = rows.partition_point(|r| r.start < range.start);
            let end = rows.partition_point(|r| r.end <= range.end);
            let rows = rows
                .get(start..end)
                .filter(|r| !r.is_empty())
                .ok_or_else(|| fault("helper origin range absent or excessive"))?;
            let mut e = StableEncoder::new("lrr/v2/completed-helper-token-origins");
            e.u64(u64::try_from(rows.len()).map_err(|_| fault("helper origin count overflow"))?);
            for row in rows {
                e.bytes(&row.span.0);
            }
            Ok(e.finish())
        };
        let target = origin(output.origins(), producer.range)?;
        let parsed = ParsedFile::from_generated_items(&catalog.items)?;
        let mut selected = None;
        for (item, header) in catalog.items.items().iter().enumerate() {
            let range = crate::syntax::spans::SourceRange {
                start: header.start,
                end: header.end,
            };
            let mut groups = vec![parsed.attribute_inputs_at(range, &catalog.cfg)?];
            if matches!(
                header.kind,
                crate::expand::generated_items::GeneratedHeaderKind::Struct
                    | crate::expand::generated_items::GeneratedHeaderKind::Enum
                    | crate::expand::generated_items::GeneratedHeaderKind::Union
            ) {
                groups.extend(parsed.derive_item_inputs_at(range, &catalog.cfg)?.members);
            }
            let helpers = catalog.helper_scopes.get(&header.key.fingerprint());
            for group in groups.iter().filter(|g| g.enabled) {
                for (at, attribute) in group.attributes.iter().enumerate() {
                    if attribute.tokens != producer.path
                        || origin(catalog.items.origins(), attribute.range)? != target
                    {
                        continue;
                    }
                    let mut e = StableEncoder::new("lrr/v2/completed-helper-declaration-receipt");
                    e.digest(output.fingerprint());
                    e.digest(invocation);
                    e.digest(target);
                    e.digest(header.key.fingerprint());
                    let local = helpers.and_then(|h| {
                        if group.item == groups[0].item {
                            h.outer_helpers().get(&at)
                        } else {
                            h.member_helpers()
                                .get(&(group.item.start, group.item.end, at))
                        }
                    });
                    if let (Some(helpers), Some(owners)) = (helpers, local) {
                        if !owners
                            .iter()
                            .all(|id| catalog.appended_derives.contains(id))
                        {
                            continue;
                        }
                        e.digest(helpers.fingerprint());
                        for id in owners {
                            e.digest(*id);
                        }
                    } else if catalog
                        .inherited_items
                        .contains_key(&header.key.fingerprint())
                        && catalog.inherited_attribute(item, attribute, &parsed)?
                    {
                        e.digest(
                            *catalog
                                .inherited_items
                                .get(&header.key.fingerprint())
                                .ok_or_else(|| fault("inherited receipt absent"))?,
                        );
                    } else {
                        continue;
                    }
                    let receipt = e.finish();
                    if selected.is_some_and(|old| old != receipt) {
                        return Err(fault("completed helper origin occurrence ambiguous"));
                    }
                    selected = Some(receipt);
                }
            }
        }
        Ok(selected)
    }
    pub(crate) fn completed_empty_declaration(
        &self,
        output: &crate::expand::generated_declarations::GeneratedDeclarations,
        invocation: Digest,
    ) -> Result<Option<Digest>, Diagnostic> {
        if self.items.record().descriptor().fingerprint()
            != output.record().descriptor().fingerprint()
            || self.items.epoch() != output.epoch()
            || self.cfg.fingerprint() != output.scan().cfg()
        {
            return Err(fault("empty derive declaration owner differs"));
        }
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("empty derive declaration producer absent"))?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute {
            return Ok(None);
        }
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        for certificate in self.empty_derives.values() {
            let header = self
                .items
                .items()
                .get(certificate.item)
                .ok_or_else(|| fault("empty derive owned header absent"))?;
            let facts = parsed.derive_item_inputs_at(
                crate::syntax::spans::SourceRange {
                    start: header.start,
                    end: header.end,
                },
                &self.cfg,
            )?;
            let attribute = facts
                .item
                .attributes
                .get(certificate.annotation)
                .ok_or_else(|| fault("empty derive owned annotation absent"))?;
            if producer.range.start <= attribute.range.start
                && attribute.range.end <= producer.range.end
            {
                return Ok(Some(certificate.definition.identity));
            }
        }
        Ok(None)
    }
    pub(crate) fn ready_for_outward(&self) -> bool {
        self.calls.is_empty() && self.module_sources.is_empty()
    }
    pub fn next_call(&mut self) -> Result<GeneratedCallDemand, Diagnostic> {
        self.next_call_using(None, &[])
    }
    pub(crate) fn next_call_with_source(
        &mut self,
        context: CapturedMacroContext<'_>,
    ) -> Result<GeneratedCallDemand, Diagnostic> {
        self.next_call_using(Some(context), &[])
    }
    pub(crate) fn next_call_with_ancestors(
        &mut self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<GeneratedCallDemand, Diagnostic> {
        self.next_call_using(Some(context), ancestors)
    }
    fn next_call_using(
        &mut self,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
    ) -> Result<GeneratedCallDemand, Diagnostic> {
        if let Some(request) = &self.pending {
            if request.is_derive()
                && let Some(record) = self.derive_outputs.get(&request.key().invocation)
            {
                return Ok(GeneratedCallDemand::Append {
                    request: request.clone(),
                    record: record.clone(),
                });
            }
            return Ok(GeneratedCallDemand::Ready(request.clone()));
        }
        // Consume only certified empty annotations. Restart in deterministic
        // source order after each state change; no expansion record is invented.
        if let Some(context) = enclosing {
            loop {
                let mut candidates = Vec::new();
                candidates
                    .try_reserve_exact(self.calls.len())
                    .map_err(|_| fault("empty derive work allocation"))?;
                candidates.extend(self.calls.iter().map(|(&(at, id), &index)| (at, id, index)));
                let mut progressed = false;
                for (at, id, index) in candidates {
                    if let GeneratedCallIndex::Attribute(item) = index
                        && !self.inflight.contains_key(&id)
                        && (self.settle_compiler_test_item(at, id, item, context, ancestors)?
                            || self.settle_inherited_item(at, id, item)?
                            || self.consume_empty_derive(at, id, item, context, ancestors)?)
                    {
                        progressed = true;
                        break;
                    }
                }
                if !progressed {
                    break;
                }
            }
        }
        let mut blocked = None;
        for (&(at, identity), &index) in &self.calls {
            if self.inflight.contains_key(&identity) {
                blocked.get_or_insert(identity);
                continue;
            }
            let mut demand = self.prepare_call(at, identity, index, enclosing, ancestors, false)?;
            if enclosing.is_some() && matches!(demand, GeneratedCallDemand::NeedInput(_)) {
                demand = self.prepare_call(at, identity, index, enclosing, ancestors, true)?;
            }
            match demand {
                GeneratedCallDemand::NeedDeriveExecution {
                    occurrence,
                    helpers,
                    request,
                    append,
                } => {
                    let id = helpers.item_identity();
                    if let Some(old) = self.helper_scopes.get(&id) {
                        if old.fingerprint() != helpers.fingerprint() {
                            return Err(fault("generated helper decisions changed within epoch"));
                        }
                    } else {
                        let bytes = self
                            .helper_scope_bytes
                            .checked_add(helpers.retained_bytes())
                            .and_then(|n| n.checked_add(128))
                            .ok_or_else(|| fault("generated helper scope retention limit"))?;
                        self.helper_scopes.insert(id, helpers);
                        self.helper_scope_bytes = bytes;
                    }
                    if let Some(request) = request {
                        if append {
                            let (item, ordinal) = request.derive_location()?;
                            let header = &self.items.items()[item];
                            let scope = self
                                .call_scopes
                                .get(&header.key.fingerprint())
                                .ok_or_else(|| fault("derive original scope absent"))?
                                .clone();
                            let lexical = *self
                                .module_scopes
                                .get(&scope)
                                .ok_or_else(|| fault("derive prefix lexical scope absent"))?;
                            let view = self.scopes.prefix_view(
                                lexical,
                                u64::from(header.start),
                                identity,
                            )?;
                            let bytes = self
                                .derive_parent_view_bytes
                                .checked_add(view.metadata_bytes()?)
                                .and_then(|n| n.checked_add(128))
                                .ok_or_else(|| fault("derive parent view retention limit"))?;
                            if self
                                .derive_parent_views
                                .contains_key(&request.key().invocation)
                            {
                                return Err(fault("derive parent view already retained"));
                            }
                            self.call_sites.insert(
                                request.key().invocation,
                                (header.start, GeneratedCallIndex::Derive(item, ordinal)),
                            );
                            self.call_scopes.insert(request.key().invocation, scope);
                            self.derive_parent_views
                                .insert(request.key().invocation, Arc::new(view));
                            self.derive_parent_view_bytes = bytes;
                            let record = self
                                .derive_outputs
                                .get(&request.key().invocation)
                                .ok_or_else(|| fault("derive append record absent"))?
                                .clone();
                            self.pending = Some(request.clone());
                            return Ok(GeneratedCallDemand::Append { request, record });
                        }
                        self.pending = Some(request.clone());
                        return Ok(GeneratedCallDemand::Ready(request));
                    }
                    blocked.get_or_insert(occurrence);
                }
                GeneratedCallDemand::Append { .. } => {
                    return Err(fault("nested append preparation escaped helper decision"));
                }
                GeneratedCallDemand::NeedDeclaration(request) => {
                    return Ok(GeneratedCallDemand::NeedDeclaration(request));
                }
                GeneratedCallDemand::Ready(request) => {
                    if request.consumed_compiler_tests() != 0 {
                        if request.consumed_compiler_tests() != 1 {
                            return Err(fault(
                                "generated compiler Test registration count unsupported",
                            ));
                        }
                        let item = self
                            .test_registration_candidates
                            .get(&identity)
                            .ok_or_else(|| {
                                fault("generated compiler Test function metadata absent")
                            })?
                            .clone();
                        if self
                            .test_registrations
                            .insert(request.key().fingerprint(), item)
                            .is_some()
                        {
                            return Err(fault(
                                "generated compiler Test registration origin duplicated",
                            ));
                        }
                    }
                    if let GeneratedCallIndex::Attribute(item) = index {
                        let context = enclosing
                            .ok_or_else(|| fault("compiler test prefix context absent"))?;
                        let prefix =
                            self.compiler_test_indices(identity, item, context, ancestors)?;
                        if !prefix.0.is_empty() {
                            let retained = self
                                .compiler_test_prefix_bytes
                                .checked_add(
                                    prefix
                                        .0
                                        .len()
                                        .checked_mul(64)
                                        .and_then(|n| n.checked_add(160))
                                        .ok_or_else(|| {
                                            fault("compiler test prefix size overflow")
                                        })?,
                                )
                                .ok_or_else(|| fault("compiler test prefix retention limit"))?;
                            if self.compiler_test_prefixes.contains_key(&identity) {
                                return Err(fault(
                                    "compiler test prefix count or identity conflict",
                                ));
                            }
                            self.compiler_test_prefixes.insert(identity, prefix);
                            self.compiler_test_prefix_bytes = retained;
                        }
                    }
                    if matches!(index, GeneratedCallIndex::Attribute(_))
                        && let Some(helpers) = self.helper_scopes.get(&identity)
                        && helpers
                            .occurrences()
                            .any(|id| self.appended_derives.contains(&id))
                    {
                        let scope = self
                            .call_scopes
                            .get(&identity)
                            .ok_or_else(|| fault("later attribute scope absent"))?;
                        let lexical = *self
                            .module_scopes
                            .get(scope)
                            .ok_or_else(|| fault("later attribute lexical scope absent"))?;
                        let view = self.scopes.prefix_view(lexical, u64::from(at), identity)?;
                        let bytes = self
                            .derive_parent_view_bytes
                            .checked_add(view.metadata_bytes()?)
                            .and_then(|n| n.checked_add(128))
                            .ok_or_else(|| fault("later attribute parent view retention limit"))?;
                        if self.derive_parent_views.contains_key(&identity) {
                            return Err(fault("later attribute parent view already retained"));
                        }
                        self.derive_parent_views.insert(identity, Arc::new(view));
                        self.derive_parent_view_bytes = bytes;
                    }
                    self.pending = Some(request.clone());
                    return Ok(GeneratedCallDemand::Ready(request));
                }
                GeneratedCallDemand::NeedInput(input) => {
                    blocked.get_or_insert(input);
                    if enclosing.is_none() {
                        break;
                    }
                }
                GeneratedCallDemand::Complete => {
                    return Err(fault("generated call preparation returned completion"));
                }
            }
        }
        let blocked = blocked.or_else(|| self.module_sources.keys().next().copied());
        Ok(match blocked {
            Some(input) => GeneratedCallDemand::NeedInput(input),
            None => GeneratedCallDemand::Complete,
        })
    }
    fn prepare_call(
        &self,
        at: u32,
        identity: Digest,
        index: GeneratedCallIndex,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
        available: bool,
    ) -> Result<GeneratedCallDemand, Diagnostic> {
        let index = match index {
            GeneratedCallIndex::Function(index) => index,
            GeneratedCallIndex::Attribute(index) => {
                return self.prepare_attribute_call(identity, index, enclosing, ancestors);
            }
            GeneratedCallIndex::Derive(item, ordinal) => {
                return Ok(GeneratedCallDemand::Ready(Arc::new(self.derive_request(
                    item,
                    ordinal,
                    enclosing.ok_or_else(|| fault("derive replay context absent"))?,
                    ancestors,
                )?)));
            }
        };
        let call_scope = self
            .call_scopes
            .get(&identity)
            .ok_or_else(|| fault("generated call source scope absent"))?;
        let lexical = *self
            .module_scopes
            .get(call_scope)
            .ok_or_else(|| fault("generated call lexical scope absent"))?;
        let call = &self.items.calls()[index];
        let [crate::syntax::token_wire::TokenTree::Group { children, .. }] = call.path.roots()
        else {
            return Err(fault("generated call path root absent"));
        };
        let Some(crate::syntax::token_wire::TokenTree::Ident { text, raw, span }) = children.last()
        else {
            return Err(fault("generated call member identifier absent"));
        };
        let name = MacroName::parse(
            &if *raw {
                format!("r#{text}")
            } else {
                text.clone()
            },
            call.edition,
        )?;
        let context = call
            .path
            .spans()
            .iter()
            .find(|s| s.key() == *span)
            .ok_or_else(|| fault("generated call path origin absent"))?
            .context;
        let header = self
            .items
            .items()
            .iter()
            .find(|h| h.key == call.key)
            .ok_or_else(|| fault("generated call canonical header absent"))?;
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let syntax = parsed.generated_macro_call_at(crate::syntax::spans::SourceRange {
            start: header.start,
            end: header.end,
        })?;
        let bare = !syntax.absolute() && syntax.segments().len() == 1;
        let reference = MacroReference {
            scope: lexical,
            before: u64::from(at),
            name: &name,
            context,
            origin: Some(self.origin()),
        };
        let textual = if bare {
            Some(self.resolve_reference_using(reference, enclosing, ancestors, available)?)
        } else {
            None
        };
        let definition = match textual {
            Some(HygienicLookup::NeedInput(input)) => {
                return Ok(GeneratedCallDemand::NeedInput(input));
            }
            Some(HygienicLookup::Selection(selection))
                if selection != super::macro_origins::MacroSelection::Absent =>
            {
                match selection {
                    super::macro_origins::MacroSelection::Defined(definition) => {
                        if let Some(captured)=enclosing
                            && let Some(input)=self.check_path_shadow(&definition,&syntax,call.edition,call_scope,captured,ancestors)? {return Ok(GeneratedCallDemand::NeedInput(input));}
                        definition
                    },
                    super::macro_origins::MacroSelection::Ambiguous(_) => return Err(Diagnostic::new("LRR-E-MACRO-AMBIGUOUS", "generated textual macro selection is ambiguous between expansion origins".into())),
                    super::macro_origins::MacroSelection::Absent => return Err(fault("selected generated textual macro is absent")),
                }
            }
            _ => {
                let Some(captured) = enclosing else {
                    return Ok(GeneratedCallDemand::NeedInput(identity));
                };
                let comparison = self.facts.name_item(context)?;
                for token in children {
                    let token_span = match token {
                        crate::syntax::token_wire::TokenTree::Ident { span, .. }
                        | crate::syntax::token_wire::TokenTree::Punct { span, .. } => span,
                        _ => return Err(fault("generated call path has non-path tokens")),
                    };
                    let token_context = call
                        .path
                        .spans()
                        .iter()
                        .find(|s| s.key() == *token_span)
                        .ok_or_else(|| fault("generated path token origin absent"))?
                        .context;
                    if self.facts.name_item(token_context)? != comparison {
                        return Ok(GeneratedCallDemand::NeedInput(identity));
                    }
                }
                let names = self.call_namespaces(captured, ancestors)?;
                let route = if matches!(syntax.segments().first(),Some(crate::syntax::imports::ImportSegment::Name(name)) if name=="$crate")
                {
                    let Some(crate::syntax::token_wire::TokenTree::Ident { text, span, .. }) =
                        children.first()
                    else {
                        return Err(fault("generated dollar-crate path origin absent"));
                    };
                    if text != "$crate" {
                        return Err(fault(
                            "generated dollar-crate syntax/token identity differs",
                        ));
                    }
                    let mark = call
                        .path
                        .spans()
                        .iter()
                        .find(|s| s.key() == *span)
                        .ok_or_else(|| fault("generated dollar-crate token span absent"))?
                        .context;
                    let unit = Digest(self.facts.dollar_crate(mark)?.0);
                    names.source_segments_path(
                        &syntax.segments()[1..],
                        false,
                        &SourceScope {
                            crate_key: unit,
                            module_path: vec![],
                            owner: None,
                        },
                        call.edition,
                    )?
                } else {
                    names.source_segments_path(
                        syntax.segments(),
                        syntax.absolute(),
                        call_scope,
                        call.edition,
                    )?
                };
                let Some(route) = route else {
                    return Ok(GeneratedCallDemand::NeedInput(identity));
                };
                let selected = match names.lookup_from(&route, call_scope)? {
                    super::macro_names::PathLookup::Absent => names.lookup_invocation_segments(syntax.segments(),syntax.absolute(),call_scope,call.edition)?,
                    other => other,
                };
                match selected {
                    super::macro_names::PathLookup::Defined(definition) => definition,
                    super::macro_names::PathLookup::NeedInput(input) => {
                        return Ok(GeneratedCallDemand::NeedInput(input));
                    }
                    super::macro_names::PathLookup::Absent => {
                        return Ok(GeneratedCallDemand::NeedInput(identity));
                    }
                    super::macro_names::PathLookup::Ambiguous(_) => {
                        return Err(Diagnostic::new("LRR-E-MACRO-AMBIGUOUS", "generated macro route is ambiguous".into()));
                    }
                    super::macro_names::PathLookup::Inaccessible(_) => {
                        return Err(Diagnostic::new("LRR-E-MACRO-INACCESSIBLE", "generated macro route is inaccessible".into()));
                    }
                }
            }
        };
        let source_owner = std::iter::once(self)
            .chain(ancestors.iter().copied())
            .find_map(|catalog| catalog.source_body_owners.get(&definition.identity));
        let body = if source_owner.is_some() {
            None
        } else if self.definitions.contains_key(&definition.identity) {
            Some(self.body_for(definition.identity))
        } else {
            ancestors
                .iter()
                .find(|c| c.definitions.contains_key(&definition.identity))
                .map(|c| c.body_for(definition.identity))
        };
        let body =
            body.or_else(|| enclosing.and_then(|context| context.inherited_body(&definition)));
        let mut request = crate::expand::generated_call_request::GeneratedCallRequest::prepare(
            self.items.clone(),
            index,
            definition.clone(),
        )?;
        let parents = if let Some(context) = enclosing {
            self.diagnostic_origin_parents(context, ancestors)?
        } else {
            Vec::new()
        };
        if let Some(owner) = source_owner {
            request = request.with_enclosing(owner, parents)?;
        } else if let Some(body) = body {
            request = request.with_generated_body(body)?;
        } else {
            let context =
                enclosing.ok_or_else(|| fault("enclosing generated definition provider absent"))?;
            if let Some(procedural) = &context.provider.procedural
                && procedural
                    .select(
                        &definition,
                        crate::expand::identity::ProceduralKind::Function,
                    )?
                    .is_some()
            {
                let count = context
                    .owners
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fault("generated procedural source provider limit"))?;
                let mut providers = Vec::new();
                providers
                    .try_reserve_exact(count)
                    .map_err(|_| fault("generated procedural source provider allocation"))?;
                providers.push(context.provider);
                providers.extend(context.owners.iter());
                request = request
                    .with_procedural(procedural, call_scope, &providers)?
                    .with_diagnostic_parents(parents)?;
            } else if let Some(compiler) = &context.provider.compiler
                && compiler.select(&definition)?.is_some()
            {
                request = request.with_compiler(compiler)?;
            } else {
                let owner = context.definition_owner(&definition)?;
                request = request.with_enclosing(owner, parents)?;
            }
        }
        let request = Arc::new(request);
        Ok(GeneratedCallDemand::Ready(request))
    }
    fn derive_request(
        &self,
        item: usize,
        ordinal: usize,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<crate::expand::generated_call_request::GeneratedCallRequest, Diagnostic> {
        let id = self
            .items
            .items()
            .get(item)
            .ok_or_else(|| fault("derive replay item absent"))?
            .key
            .fingerprint();
        let scope = self
            .call_scopes
            .get(&id)
            .ok_or_else(|| fault("derive replay scope absent"))?;
        let names = self.call_namespaces(context, ancestors)?;
        let builtin = context
            .provider
            .compiler
            .as_ref()
            .and_then(|c| c.attribute_catalog())
            .ok_or_else(|| fault("derive replay compiler facet absent"))?;
        let selected = super::generated_derives::GeneratedDeriveResolution::resolve(
            &self.items,
            item,
            &self.cfg,
            scope.clone(),
            &names,
            builtin,
        )?;
        let plan = selected.plan(&self.items, item, &self.cfg)?;
        let exports =
            super::generated_derive_exports::GeneratedDeriveExports::resolve_with_compiler(
                &plan,
                &self.items,
                scope.clone(),
                &names,
                context.provider.procedural.as_deref(),
                context.provider.compiler.as_deref(),
            )?;
        let super::generated_derive_helpers::GeneratedHelperPoll::Ready(helpers) =
            super::generated_derive_helpers::GeneratedDeriveHelpers::resolve_with_catalog(
                plan,
                &self.items,
                item,
                &exports,
                context.provider.procedural.as_deref(),
                &self.helper_receipts,
            )?
        else {
            return Err(fault("derive replay helpers unresolved"));
        };
        self.prepare_derive_execution(item, ordinal, helpers, &exports, context, ancestors)
    }
    /// Initial execution and replay use the same selected export, helper input,
    /// compiler container facts and procedural authority boundary.
    fn prepare_derive_execution(
        &self,
        item: usize,
        ordinal: usize,
        helpers: Arc<super::generated_derive_helpers::GeneratedDeriveHelpers>,
        exports: &super::generated_derive_exports::GeneratedDeriveExports,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<crate::expand::generated_call_request::GeneratedCallRequest, Diagnostic> {
        let selected = exports
            .rows()
            .get(ordinal)
            .and_then(|row| row.selected())
            .ok_or_else(|| fault("derive export selection absent"))?;
        let request = crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
            self.items.clone(),
            item,
            &self.cfg,
            ordinal,
            selected.definition().clone(),
            helpers,
        )?;
        let request = if let Some(builtin) = selected.builtin() {
            use lrr_expand_adapter::builtin_macros::{BuiltinDeriveKind, DeriveContainerFacts};
            let mut facts = DeriveContainerFacts::default();
            for kind in exports.rows().iter().filter_map(|row| {
                row.selected()
                    .and_then(|selected| selected.builtin())
                    .map(|export| export.kind)
            }) {
                match kind {
                    BuiltinDeriveKind::Copy => facts.derives_copy = true,
                    BuiltinDeriveKind::Ord => facts.derives_ord = true,
                    _ => {}
                }
            }
            request.with_builtin_derive(builtin.kind, facts)?
        } else {
            let catalog = context
                .provider
                .procedural
                .as_deref()
                .ok_or_else(|| fault("procedural derive catalog absent"))?;
            let id = self
                .items
                .items()
                .get(item)
                .ok_or_else(|| fault("derive execution item absent"))?
                .key
                .fingerprint();
            let scope = self
                .call_scopes
                .get(&id)
                .ok_or_else(|| fault("derive execution scope absent"))?;
            let count = context
                .owners
                .len()
                .checked_add(1)
                .ok_or_else(|| fault("derive provider count overflow"))?;
            let mut providers = Vec::new();
            providers
                .try_reserve_exact(count)
                .map_err(|_| fault("derive provider allocation"))?;
            providers.push(context.provider);
            providers.extend(context.owners.iter());
            request.with_procedural(catalog, scope, &providers)?
        };
        request.with_diagnostic_parents(self.diagnostic_origin_parents(context, ancestors)?)
    }
    pub(crate) fn settled_derive_items(&self) -> &std::collections::BTreeSet<Digest> {
        &self.settled_derive_items
    }
    pub(crate) fn test_registrations(
        &self,
    ) -> impl Iterator<Item = (Digest, &crate::syntax::outline::ItemOutline)> {
        self.test_registrations
            .iter()
            .map(|(origin, item)| (*origin, item.as_ref()))
    }
    pub(crate) fn selection_catalog_children(&self) -> impl Iterator<Item = &Self> {
        self.selection_children.iter().map(Arc::as_ref)
    }
    fn consumed_attribute_indices(&self, identity: Digest) -> BTreeSet<usize> {
        let mut consumed = self
            .helper_scopes
            .get(&identity)
            .map(|helpers| helpers.completed_annotations(&self.appended_derives))
            .unwrap_or_default();
        consumed.extend(
            self.empty_derives
                .keys()
                .filter_map(|(item, at)| (*item == identity).then_some(*at)),
        );
        consumed
    }
    fn empty_member_fingerprint(
        &self,
        plan: &crate::expand::generated_derives::GeneratedDerivePlan,
    ) -> Result<Option<Digest>, Diagnostic> {
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let mut encoded = StableEncoder::new("lrr/v2/empty-derive-member-attributes");
        encoded.digest(plan.binding_digest());
        for member in plan.facts().members.iter().filter(|m| m.enabled) {
            for (at, attribute) in member.attributes.iter().enumerate() {
                if attribute.name == "cfg" {
                    continue;
                }
                let syntax = parsed.attribute_path_at(attribute.range)?;
                if syntax.absolute() || syntax.segments().len() != 1 {
                    return Ok(None);
                }
                let Some(descriptor) =
                    crate::expand::builtin_attributes::retained_descriptor(&attribute.name)?
                else {
                    return Ok(None);
                };
                encoded.u32(member.item.start);
                encoded.u32(member.item.end);
                encoded.u64(
                    u64::try_from(at)
                        .map_err(|_| fault("empty member attribute index overflow"))?,
                );
                encoded.digest(descriptor.definition_identity());
            }
        }
        Ok(Some(encoded.finish()))
    }
    fn consume_empty_derive(
        &mut self,
        at: u32,
        id: Digest,
        item: usize,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<bool, Diagnostic> {
        if !self.items.items().get(item).is_some_and(|h| {
            matches!(
                h.kind,
                crate::expand::generated_items::GeneratedHeaderKind::Struct
                    | crate::expand::generated_items::GeneratedHeaderKind::Enum
                    | crate::expand::generated_items::GeneratedHeaderKind::Union
            )
        }) {
            return Ok(false);
        }
        let Some(builtin) = context
            .provider
            .compiler
            .as_ref()
            .and_then(|c| c.attribute_catalog())
        else {
            return Ok(false);
        };
        let scope = self
            .call_scopes
            .get(&id)
            .ok_or_else(|| fault("empty derive scope absent"))?
            .clone();
        let names = self.call_namespaces(context, ancestors)?;
        let selected = super::generated_derives::GeneratedDeriveResolution::resolve(
            &self.items,
            item,
            &self.cfg,
            scope.clone(),
            &names,
            builtin,
        )?;
        let plan = selected.plan(&self.items, item, &self.cfg)?;
        let consumed = self.consumed_attribute_indices(id);
        let inert = self.inert_attribute_indices(id)?;
        let mut first = None;
        for (i, a) in plan.facts().item.attributes.iter().enumerate() {
            if !consumed.contains(&i)
                && !inert.contains(&i)
                && a.name != "cfg"
                && crate::expand::builtin_attributes::retained_descriptor(&a.name)?.is_none()
            {
                first = Some(i);
                break;
            }
        }
        let Some(annotation) = plan
            .annotations()
            .iter()
            .find(|a| Some(a.index()) == first && a.paths().is_empty())
        else {
            return Ok(false);
        };
        let super::macro_names::PathLookup::Defined(definition) = selected
            .candidates()
            .get(&annotation.index())
            .ok_or_else(|| fault("empty annotation selection absent"))?
        else {
            return Err(fault("empty annotation selection incomplete"));
        };
        let members = self.empty_member_fingerprint(&plan)?;
        let certificate = EmptyDeriveCertificate {
            item,
            annotation: annotation.index(),
            definition: definition.clone(),
            binding: plan.binding_digest(),
            members,
        };
        if self
            .empty_derives
            .insert((id, annotation.index()), certificate)
            .is_some()
        {
            return Err(fault("empty annotation already consumed"));
        }
        let consumed = self.consumed_attribute_indices(id);
        let groups_complete = plan
            .annotations()
            .iter()
            .all(|a| consumed.contains(&a.index()));
        let settled = groups_complete
            && if let Some(helpers) = self.helper_scopes.get(&id) {
                helpers
                    .occurrences()
                    .all(|p| self.appended_derives.contains(&p))
                    && helpers.unresolved_outer().is_empty()
                    && helpers.unresolved_members().is_empty()
            } else {
                plan.unresolved_attributes().is_empty() && members.is_some()
            };
        if settled {
            let lexical = *self
                .module_scopes
                .get(&scope)
                .ok_or_else(|| fault("empty derive lexical scope absent"))?;
            Arc::make_mut(&mut self.scopes).complete_prefix(lexical, u64::from(at), id)?;
            self.calls.remove(&(at, id));
            self.settled_derive_items.insert(id);
        }
        Ok(true)
    }
    fn compiler_test_indices(
        &self,
        identity: Digest,
        item: usize,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<(BTreeSet<usize>, Digest), Diagnostic> {
        let header = self
            .items
            .items()
            .get(item)
            .ok_or_else(|| fault("compiler test header absent"))?;
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let facts = parsed.attribute_inputs_at(
            crate::syntax::spans::SourceRange {
                start: header.start,
                end: header.end,
            },
            &self.cfg,
        )?;
        let scope = self
            .call_scopes
            .get(&identity)
            .ok_or_else(|| fault("compiler test scope absent"))?;
        let names = self
            .call_namespaces(context, ancestors)?
            .before_attribute(identity, identity);
        let mut indices = BTreeSet::new();
        let mut proof =
            StableEncoder::new("lrr/v2/generated-compiler-test-selected-action-and-inert-facets");
        proof.record(&header.key);
        proof.digest(self.items.record().descriptor().fingerprint());
        proof.digest(self.cfg.fingerprint());
        let consumed = self.consumed_attribute_indices(identity);
        let inherited = self.inert_attribute_indices(identity)?;
        let mut prefix_open = true;
        for (at, attribute) in facts.attributes.iter().enumerate() {
            let builtin_inert =
                crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_some();
            if attribute.name != "test" && !builtin_inert {
                if attribute.name != "cfg" && !consumed.contains(&at) && !inherited.contains(&at) {
                    prefix_open = false;
                }
                continue;
            }
            // An earlier replacing attribute can remove later annotations.
            // Resolve inert facets only when their effective prefix is reached.
            if builtin_inert && !prefix_open {
                continue;
            }
            let syntax = parsed.attribute_path_at(attribute.range)?;
            if syntax.absolute() || syntax.segments().len() != 1 {
                if builtin_inert {
                    return Ok((BTreeSet::new(), proof.finish()));
                }
                prefix_open = false;
                continue;
            }
            let path = crate::expand::generated_derives::annotation_path(
                &self.items,
                item,
                attribute.range,
            )?;
            let [crate::syntax::token_wire::TokenTree::Group { children, .. }] = path.roots()
            else {
                return Err(fault("compiler test path root absent"));
            };
            let mut class = None;
            let mut mixed = false;
            for token in children {
                let span = match token {
                    crate::syntax::token_wire::TokenTree::Ident { span, .. }
                    | crate::syntax::token_wire::TokenTree::Punct { span, .. } => span,
                    _ => return Err(fault("compiler test path contains non-path tokens")),
                };
                let row = path
                    .spans()
                    .iter()
                    .find(|row| row.key() == *span)
                    .ok_or_else(|| fault("compiler test path span absent"))?;
                let normalized = self.facts.name_item(row.context)?;
                mixed |= class.is_some_and(|old| old != normalized);
                class = Some(normalized);
            }
            if mixed || class.is_none() {
                if builtin_inert {
                    return Ok((BTreeSet::new(), proof.finish()));
                }
                prefix_open = false;
                continue;
            }
            let lookup =
                names.lookup_invocation_segments(syntax.segments(), false, scope, self.edition)?;
            if builtin_inert {
                let Some(binding) = crate::expand::builtin_attributes::certify_inert_binding(
                    &attribute.name,
                    &lookup,
                    context.provider.procedural.as_deref(),
                    context
                        .provider
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
                else {
                    return Ok((BTreeSet::new(), proof.finish()));
                };
                proof.digest(binding);
                proof.u64(
                    u64::try_from(at)
                        .map_err(|_| fault("compiler inert attribute index overflow"))?,
                );
                continue;
            }
            match lookup {
                super::macro_names::PathLookup::Defined(definition) => {
                    if !crate::expand::builtin_attributes::permits_builtin_test_marker(
                        &definition,
                        context.provider.procedural.as_deref(),
                        context
                            .provider
                            .compiler
                            .as_ref()
                            .and_then(|c| c.attribute_catalog()),
                    )? {
                        prefix_open = false;
                        continue;
                    }
                    proof.digest(definition.identity);
                    proof.digest(definition.defining_unit);
                }
                super::macro_names::PathLookup::Absent => {}
                _ => {
                    prefix_open = false;
                    continue;
                }
            }
            let descriptor = crate::expand::builtin_attributes::descriptor("test")?
                .ok_or_else(|| fault("compiler test facet absent"))?;
            proof.digest(descriptor.definition_identity());
            proof.u64(u64::try_from(at).map_err(|_| fault("compiler test index overflow"))?);
            indices.insert(at);
        }
        Ok((indices, proof.finish()))
    }
    fn settle_compiler_test_item(
        &mut self,
        at: u32,
        identity: Digest,
        item: usize,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<bool, Diagnostic> {
        let header = self
            .items
            .items()
            .get(item)
            .ok_or_else(|| fault("compiler test item absent"))?;
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let facts = parsed.attribute_inputs_at(
            crate::syntax::spans::SourceRange {
                start: header.start,
                end: header.end,
            },
            &self.cfg,
        )?;
        if !facts
            .attributes
            .iter()
            .any(|attribute| attribute.name == "test")
        {
            return Ok(false);
        }
        let (indices, proof) = self.compiler_test_indices(identity, item, context, ancestors)?;
        if indices.is_empty() {
            return Ok(false);
        }
        for (index, attribute) in facts.attributes.iter().enumerate() {
            if !indices.contains(&index)
                && attribute.name != "cfg"
                && crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_none()
            {
                return Ok(false);
            }
        }
        // The compiler facet and complete inert annotation prefix are proven
        // before target eligibility. A non-function cannot wait on its own
        // unresolved declaration or silently disappear from the final catalog.
        if header.kind != crate::expand::generated_items::GeneratedHeaderKind::Function {
            return Err(Diagnostic::new(
                "LRR-E-TEST-DISCOVERY",
                "compiler Test attribute requires a free function".into(),
            ));
        }
        let scope = self
            .call_scopes
            .get(&identity)
            .ok_or_else(|| fault("compiler test lexical scope absent"))?;
        let lexical = *self
            .module_scopes
            .get(scope)
            .ok_or_else(|| fault("compiler test lexical module absent"))?;
        Arc::make_mut(&mut self.scopes).complete_prefix(lexical, u64::from(at), identity)?;
        self.calls.remove(&(at, identity));
        self.compiler_test_items.insert(identity, proof);
        self.settled_derive_items.insert(identity);
        Ok(true)
    }
    fn inert_attribute_indices(&self, identity: Digest) -> Result<BTreeSet<usize>, Diagnostic> {
        let mut inert: BTreeSet<usize> = self
            .helper_scopes
            .get(&identity)
            .map(|helpers| helpers.outer_helpers().keys().copied().collect())
            .unwrap_or_default();
        if let Some((indices, _)) = self.compiler_test_prefixes.get(&identity) {
            inert.extend(indices.iter().copied());
        }
        if !self.inherited_helpers.is_empty() {
            let item = self
                .items
                .items()
                .iter()
                .position(|h| h.key.fingerprint() == identity)
                .ok_or_else(|| fault("inherited attribute item absent"))?;
            let parsed = ParsedFile::from_generated_items(&self.items)?;
            let header = &self.items.items()[item];
            let facts = parsed.attribute_inputs_at(
                crate::syntax::spans::SourceRange {
                    start: header.start,
                    end: header.end,
                },
                &self.cfg,
            )?;
            for (at, attribute) in facts.attributes.iter().enumerate() {
                if self.inherited_attribute(item, attribute, &parsed)? {
                    inert.insert(at);
                }
            }
        }
        Ok(inert)
    }
    fn inherited_attribute(
        &self,
        item: usize,
        attribute: &crate::syntax::attributes::AttributeInput,
        parsed: &ParsedFile,
    ) -> Result<bool, Diagnostic> {
        if self.inherited_helpers.is_empty() {
            return Ok(false);
        }
        let syntax = parsed.attribute_path_at(attribute.range)?;
        let [crate::syntax::imports::ImportSegment::Name(name)] = syntax.segments() else {
            return Ok(false);
        };
        if syntax.absolute() {
            return Ok(false);
        }
        let forest =
            crate::expand::generated_derives::annotation_path(&self.items, item, attribute.range)?;
        let [crate::syntax::token_wire::TokenTree::Group { children, .. }] = forest.roots() else {
            return Err(fault("inherited helper path absent"));
        };
        let [crate::syntax::token_wire::TokenTree::Ident { span, .. }] = children.as_slice() else {
            return Ok(false);
        };
        let row = forest
            .spans()
            .iter()
            .find(|s| s.key() == *span)
            .ok_or_else(|| fault("inherited helper span absent"))?;
        let class = self.items.hygiene_facts().name_item(row.context)?;
        Ok(self
            .inherited_helpers
            .iter()
            .any(|e| e.source.inherited_match(name, class, &e.introduced)))
    }
    fn inherited_item_fingerprint(&self, item: usize) -> Result<Option<Digest>, Diagnostic> {
        if self.inherited_helpers.is_empty() {
            return Ok(None);
        }
        let header = self
            .items
            .items()
            .get(item)
            .ok_or_else(|| fault("inherited item header absent"))?;
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let range = crate::syntax::spans::SourceRange {
            start: header.start,
            end: header.end,
        };
        let facts = parsed.attribute_inputs_at(range, &self.cfg)?;
        let mut groups = vec![facts];
        if matches!(
            header.kind,
            crate::expand::generated_items::GeneratedHeaderKind::Struct
                | crate::expand::generated_items::GeneratedHeaderKind::Enum
                | crate::expand::generated_items::GeneratedHeaderKind::Union
        ) {
            let members = parsed.derive_item_inputs_at(range, &self.cfg)?.members;
            let cfg_descendants = parsed.derive_cfg_inputs_at(range, &self.cfg)?.descendants;
            let mut owners = BTreeMap::new();
            for group in members.into_iter().chain(cfg_descendants) {
                let key = (group.item.start, group.item.end);
                if let Some(previous) = owners.get(&key) {
                    if previous != &group {
                        return Err(fault("inherited helper member facts differ"));
                    }
                } else {
                    owners.insert(key, group);
                }
            }
            groups.extend(owners.into_values());
        }
        let mut encoded = StableEncoder::new("lrr/v2/inherited-helper-item");
        encoded.digest(self.items.record().descriptor().fingerprint());
        encoded.digest(header.key.fingerprint());
        encoded.digest(self.cfg.fingerprint());
        for environment in &self.inherited_helpers {
            encoded.digest(environment.source.fingerprint());
            encoded.sequence(
                &environment.introduced.iter().copied().collect::<Vec<_>>(),
                |e, id| e.digest(*id),
            );
        }
        for group in groups.iter().filter(|g| g.enabled) {
            for (at, attribute) in group.attributes.iter().enumerate() {
                if attribute.name == "cfg"
                    || crate::expand::builtin_attributes::retained_descriptor(&attribute.name)?
                        .is_some()
                {
                    continue;
                }
                if !self.inherited_attribute(item, attribute, &parsed)? {
                    return Ok(None);
                }
                encoded.u32(group.item.start);
                encoded.u32(group.item.end);
                encoded.u64(
                    u64::try_from(at).map_err(|_| fault("inherited attribute index overflow"))?,
                );
            }
        }
        Ok(Some(encoded.finish()))
    }
    fn settle_inherited_item(
        &mut self,
        at: u32,
        id: Digest,
        item: usize,
    ) -> Result<bool, Diagnostic> {
        let Some(proof) = self.inherited_item_fingerprint(item)? else {
            return Ok(false);
        };
        let scope = self
            .call_scopes
            .get(&id)
            .ok_or_else(|| fault("inherited item scope absent"))?;
        let lexical = *self
            .module_scopes
            .get(scope)
            .ok_or_else(|| fault("inherited item lexical scope absent"))?;
        Arc::make_mut(&mut self.scopes).complete_prefix(lexical, u64::from(at), id)?;
        self.calls.remove(&(at, id));
        self.inherited_items.insert(id, proof);
        self.settled_derive_items.insert(id);
        Ok(true)
    }
    fn prepare_attribute_call(
        &self,
        identity: Digest,
        index: usize,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
    ) -> Result<GeneratedCallDemand, Diagnostic> {
        let Some(context) = enclosing else {
            return Ok(GeneratedCallDemand::NeedInput(identity));
        };
        let procedural = context.provider.procedural.as_deref();
        let skipped = self.consumed_attribute_indices(identity);
        let mut inert = self.inert_attribute_indices(identity)?;
        let compiler_tests = self
            .compiler_test_indices(identity, index, context, ancestors)?
            .0;
        inert.extend(compiler_tests.iter().copied());
        let Some(input) = crate::expand::generated_attribute_input::prepare_remaining(
            &self.items,
            index,
            &self.cfg,
            &skipped,
            &inert,
            &compiler_tests,
        )?
        else {
            if let Some(builtin) = context
                .provider
                .compiler
                .as_ref()
                .and_then(|compiler| compiler.attribute_catalog())
                && self.items.items().get(index).is_some_and(|header| {
                    matches!(
                        header.kind,
                        crate::expand::generated_items::GeneratedHeaderKind::Struct
                            | crate::expand::generated_items::GeneratedHeaderKind::Enum
                            | crate::expand::generated_items::GeneratedHeaderKind::Union
                    )
                })
            {
                let scope = self
                    .call_scopes
                    .get(&identity)
                    .ok_or_else(|| fault("generated derive source scope absent"))?;
                let names = self.call_namespaces(context, ancestors)?;
                let selected = super::generated_derives::GeneratedDeriveResolution::resolve(
                    &self.items,
                    index,
                    &self.cfg,
                    scope.clone(),
                    &names,
                    builtin,
                )?;
                let plan = selected.plan(&self.items, index, &self.cfg)?;
                let exports = super::generated_derive_exports::GeneratedDeriveExports::resolve_with_compiler(
                    &plan, &self.items, scope.clone(), &names, procedural, context.provider.compiler.as_deref(),
                )?;
                for (at, row) in exports.rows().iter().enumerate() {
                    if let Some(demand) = row.helper_demand() {
                        let request = exports.declaration_request(at, procedural
                            .ok_or_else(|| fault("procedural derive declaration catalog absent"))?)?;
                        if let Some(receipt) = self.helper_receipts.get(&demand) {
                            receipt.validate_request(&request)?;
                        } else {
                            return Ok(GeneratedCallDemand::NeedDeclaration(Arc::new(request)));
                        }
                    } else if let Some(demand) = exports.row_demand(at)? {
                        return Ok(GeneratedCallDemand::NeedInput(demand));
                    }
                }
                if let Some(row) = exports.rows().first() {
                    let helpers = match super::generated_derive_helpers::GeneratedDeriveHelpers::resolve_with_catalog(
                        plan, &self.items, index, &exports, procedural, &self.helper_receipts,
                    )? {
                        super::generated_derive_helpers::GeneratedHelperPoll::Ready(helpers) => {
                            helpers
                        }
                        super::generated_derive_helpers::GeneratedHelperPoll::NeedInput(input) => {
                            return Ok(GeneratedCallDemand::NeedInput(input));
                        }
                    };
                    let active = helpers.next_annotation(&self.appended_derives);
                    let mut eligible = Vec::new();
                    eligible
                        .try_reserve_exact(exports.rows().len())
                        .map_err(|_| fault("ordered derive row allocation"))?;
                    for (at, row) in exports.rows().iter().enumerate() {
                        if Some(helpers.annotation_index(at)?) == active {
                            eligible.push((at, row));
                        }
                    }
                    let next = eligible
                        .iter()
                        .copied()
                        .find(|(_, row)| !self.derive_outputs.contains_key(&row.occurrence()));
                    let append = next.is_none();
                    let next = next.or_else(|| {
                        eligible
                            .iter()
                            .copied()
                            .find(|(_, row)| !self.appended_derives.contains(&row.occurrence()))
                    });
                    let request = if let Some((at, next)) = next {
                        if self.inflight.contains_key(&next.occurrence())
                            || !helpers.can_invoke(at)?
                        {
                            None
                        } else {
                            Some(Arc::new(self.prepare_derive_execution(
                                index, at, helpers.clone(), &exports, context, ancestors,
                            )?))
                        }
                    } else {
                        None
                    };
                    return Ok(GeneratedCallDemand::NeedDeriveExecution {
                        occurrence: next.map_or(row.occurrence(), |(_, row)| row.occurrence()),
                        helpers,
                        request,
                        append,
                    });
                }
            }
            return Ok(GeneratedCallDemand::NeedInput(identity));
        };
        let Some(procedural) = procedural else {
            return Ok(GeneratedCallDemand::NeedInput(identity));
        };
        let site = input
            .path
            .spans()
            .iter()
            .find(|span| span.key() == input.site)
            .ok_or_else(|| fault("generated attribute path site absent"))?;
        let comparison = self.facts.name_item(site.context)?;
        let [crate::syntax::token_wire::TokenTree::Group { children, .. }] = input.path.roots()
        else {
            return Err(fault("generated attribute path root absent"));
        };
        for token in children {
            let span = match token {
                crate::syntax::token_wire::TokenTree::Ident { span, .. }
                | crate::syntax::token_wire::TokenTree::Punct { span, .. } => span,
                _ => return Err(fault("generated attribute path has non-path tokens")),
            };
            let context = input
                .path
                .spans()
                .iter()
                .find(|record| record.key() == *span)
                .ok_or_else(|| fault("generated attribute path span absent"))?
                .context;
            if self.facts.name_item(context)? != comparison {
                return Err(fault("generated attribute path contexts differ"));
            }
        }
        let scope = self
            .call_scopes
            .get(&identity)
            .ok_or_else(|| fault("generated attribute source scope absent"))?;
        let names = self
            .call_namespaces(context, ancestors)?
            .before_attribute(identity, identity);
        let definition = match names.lookup_invocation_segments(
            input.syntax.segments(),
            input.syntax.absolute(),
            scope,
            self.edition,
        )? {
            super::macro_names::PathLookup::Defined(definition) => definition,
            super::macro_names::PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "generated replacing attribute namespace is ambiguous".into(),
                ));
            }
            super::macro_names::PathLookup::Inaccessible(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "generated replacing attribute namespace is inaccessible".into(),
                ));
            }
            super::macro_names::PathLookup::NeedInput(input) => {
                return Ok(GeneratedCallDemand::NeedInput(input));
            }
            _ => return Ok(GeneratedCallDemand::NeedInput(identity)),
        };
        if procedural
            .select(
                &definition,
                crate::expand::identity::ProceduralKind::Attribute,
            )?
            .is_none()
        {
            return Ok(GeneratedCallDemand::NeedInput(identity));
        }
        let count = context
            .owners
            .len()
            .checked_add(1)
            .ok_or_else(|| fault("generated attribute source provider limit"))?;
        let mut providers = Vec::new();
        providers
            .try_reserve_exact(count)
            .map_err(|_| fault("generated attribute source provider allocation"))?;
        providers.push(context.provider);
        providers.extend(context.owners.iter());
        let request =
            crate::expand::generated_call_request::GeneratedCallRequest::prepare_attribute(
                self.items.clone(),
                index,
                definition,
                input,
            )?
            .with_procedural(procedural, scope, &providers)?
            .with_diagnostic_parents(self.diagnostic_origin_parents(context, ancestors)?)?;
        Ok(GeneratedCallDemand::Ready(Arc::new(request)))
    }
    fn check_path_shadow(
        &self,
        definition: &MacroDefinition,
        syntax: &crate::syntax::macro_calls::SourceMacroCall,
        call_edition: u16,
        call_scope: &SourceScope,
        captured: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<Option<Digest>, Diagnostic> {
        let mut ancestry = self.ancestry.as_ref().clone();
        let mut origin = self.declarations.origin_of(definition)?;
        for ancestor in ancestors {
            ancestry.extend(&ancestor.ancestry)?;
            if let Some(found) = ancestor.declarations.origin_of(definition)? {
                if origin.is_some_and(|old| old != found) {
                    return Err(fault("joined textual origin differs"));
                }
                origin = Some(found);
            }
        }
        for owner in std::iter::once(captured.provider).chain(captured.owners.iter()) {
            if let Some(found) = owner.macro_index().definition_origin(definition)? {
                if origin.is_some_and(|old| old != found) {
                    return Err(fault("captured textual origin differs"));
                }
                origin = Some(found);
                ancestry.extend(owner.macro_index().expansion_ancestry())?;
            }
        }
        if let Some(found) = captured.inherited_origin(definition, &mut ancestry)? {
            if origin.is_some_and(|old| old != found) {
                return Err(fault("inherited textual origin differs"));
            }
            origin = Some(found);
        }
        let origin = origin.ok_or_else(|| fault("textual origin metadata absent"))?;
        if ancestry.descendant_of(Some(self.origin()), origin)? {
            return Ok(None);
        }
        let names = self.call_namespaces(captured, ancestors)?;
        let Some(path) = names.source_segments_path(
            syntax.segments(),
            syntax.absolute(),
            call_scope,
            call_edition,
        )?
        else {
            return Ok(None);
        };
        let other = match names.lookup_from(&path, call_scope)? {
            super::macro_names::PathLookup::Defined(other) if other != *definition => other,
            super::macro_names::PathLookup::NeedInput(input) => return Ok(Some(input)),
            super::macro_names::PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "textual macro conflicts with ambiguous path bindings".into(),
                ));
            }
            _ => return Ok(None),
        };
        for binding in names.binding_origins_from(&path, &other, call_scope)? {
            if let Some(key) = binding
                && !ancestry.contains(key)
            {
                let owner = std::iter::once(captured.provider)
                    .chain(captured.owners.iter())
                    .find(|p| p.macro_index().expansion_ancestry().contains(key))
                    .ok_or_else(|| fault("path binding origin ancestry absent"))?;
                ancestry.extend(owner.macro_index().expansion_ancestry())?;
            }
            let candidates = [
                super::macro_origins::MacroCandidate {
                    definition: definition.clone(),
                    origin,
                },
                super::macro_origins::MacroCandidate {
                    definition: other.clone(),
                    origin: binding,
                },
            ];
            if matches!(
                ancestry.select_compatible(Some(self.origin()), &candidates)?,
                super::macro_origins::MacroSelection::Ambiguous(_)
            ) {
                return Err(Diagnostic::new("LRR-E-MACRO-AMBIGUOUS", "generated textual macro conflicts with an imported binding from another expansion origin".into()));
            }
        }
        Ok(None)
    }
    fn resolve_reference(
        &self,
        reference: MacroReference<'_>,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_reference_using(reference, enclosing, ancestors, false)
    }
    fn resolve_reference_using(
        &self,
        reference: MacroReference<'_>,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
        available: bool,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_reference_in(reference, enclosing, ancestors, available, &self.scopes)
    }
    fn resolve_reference_in(
        &self,
        reference: MacroReference<'_>,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
        available: bool,
        scopes: &TextualScopes,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.resolve_reference_with_facts(reference, enclosing, ancestors, available, scopes, None)
    }
    fn resolve_reference_with_facts(
        &self,
        reference: MacroReference<'_>,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
        available: bool,
        scopes: &TextualScopes,
        returned: Option<&HygieneFacts>,
    ) -> Result<HygienicLookup, Diagnostic> {
        if let Some(context) = enclosing {
            let index = context.provider.macro_index();
            let mut facts = index.hygiene_facts().clone();
            facts.extend(&self.facts)?;
            if let Some(returned) = returned {
                facts.extend(returned)?;
            }
            let mut ancestry = index.expansion_ancestry().clone();
            ancestry.extend(&self.ancestry)?;
            let mut outers = Vec::new();
            let count = ancestors
                .len()
                .checked_add(context.source_scope_count()?)                .ok_or_else(|| fault("generated scope ancestry limit"))?;
            outers
                .try_reserve_exact(count)
                .map_err(|_| fault("generated outer scope allocation"))?;
            for (ordinal, ancestor) in ancestors.iter().enumerate() {
                facts.extend(&ancestor.facts)?;
                ancestry.extend(&ancestor.ancestry)?;
                let child = if ordinal == 0 {
                    self
                } else {
                    ancestors[ordinal - 1]
                };
                let invocation = child.items.record().key().invocation;
                outers.push(ancestor.invocation_outer(invocation)?);
            }
            context.append_source_scopes(
                ancestors.last().map_or(&self.owner, |root| &root.owner),
                &mut facts,
                &mut ancestry,
                &mut outers,
            )?;
            if available {
                self.declarations
                    .resolve_for_discovery(scopes, reference, &outers, &facts, &ancestry)
            } else {
                self.declarations
                    .resolve_with_outers(scopes, reference, &outers, &facts, &ancestry)
            }
        } else {
            let mut facts = self.facts.as_ref().clone();
            if let Some(returned) = returned {
                facts.extend(returned)?;
            }
            self.declarations
                .resolve(scopes, reference, &facts, &self.ancestry)
        }
    }
    pub(crate) fn expand_string_operand(
        &self,
        definition: &MacroDefinition,
        key: &ExpansionKey,
        input: &TokenForest,
        site: Key,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<ExpansionRecord, Diagnostic> {
        if key.grammar != crate::expand::identity::GrammarSlot::Expression
            || key.macro_identity
                != (crate::expand::identity::MacroIdentity::Declaration {
                    definition: definition.identity,
                })
        {
            return Err(fault(
                "string operand requires a selected declarative expression",
            ));
        }
        let owner = std::iter::once(self)
            .chain(ancestors.iter().copied())
            .find(|catalog| catalog.definitions.get(&definition.identity) == Some(definition));
        if let Some(owner) = owner {
            if let Some(source) = owner.source_body_owners.get(&definition.identity) {
                return source
                    .generated_request(definition, key.clone(), input, site)?
                    .expand();
            }
            return owner.body_for(definition.identity).expand_macro_body(
                definition.identity,
                definition.defining_unit,
                key,
                input,
                site,
            );
        }
        if let Some(items) = context.inherited_body(definition) {
            return items.expand_macro_body(
                definition.identity,
                definition.defining_unit,
                key,
                input,
                site,
            );
        }
        context
            .definition_owner(definition)?
            .generated_request(definition, key.clone(), input, site)?
            .expand()
    }
    pub(crate) fn prepare_compiler<'a>(
        &'a self,
        request: &'a crate::expand::generated_call_request::GeneratedCallRequest,
        context: CapturedMacroContext<'a>,
        ancestors: &'a [&'a Self],
        authorities: &crate::source::include_paths::IncludeAuthorities,
        included: Option<&crate::source::include_view::IncludedSourceView>,
        source_base: &'a crate::syntax::token_wire::SpanRecord,
    ) -> Result<GeneratedCompilerInput<'a>, Diagnostic> {
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| pending.key() != request.key())
            || !request.is_compiler()
            || context.provider.compiler.is_none()
        {
            return Err(fault("generated compiler pending demand differs"));
        }
        let (_, input, site) = request.compiler_input();
        let identity = request.key().invocation;
        let (at, _) = self
            .call_sites
            .get(&identity)
            .ok_or_else(|| fault("generated compiler call position absent"))?;
        let scope = self
            .call_scopes
            .get(&identity)
            .ok_or_else(|| fault("generated compiler call scope absent"))?;
        let lexical = *self
            .module_scopes
            .get(scope)
            .ok_or_else(|| fault("generated compiler lexical scope absent"))?;
        let span = input
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("generated compiler site absent"))?;
        let mut profile = context.provider.profile().clone();
        profile.edition = request.key().call_edition;
        let caller = super::nested_compile_inputs::NestedLexicalContext {
            source: crate::syntax::parse::ParseKey {
                content: Digest(span.source.content.0),
                profile,
            },
            scope: scope.clone(),
            textual_scope: lexical,
            origin: Some(self.origin()),
        };
        let mut scopes = self.scopes.as_ref().clone();
        scopes.omit_invocation_position(lexical, u64::from(*at))?;
        let resolver = GeneratedEagerResolver {
            catalog: self,
            context,
            ancestors,
            namespaces: self.call_namespaces(context, ancestors)?,
            scopes,
            before: u64::from(*at) + 1,
            source_base,
            input,
        };
        let from = match included {
            Some(view) => view.location().clone(),
            None => crate::source::include_paths::IncludeLocation {
                unit: scope.crate_key,
                root: authorities.unique_source_root()?,
                path: crate::source::snapshot::SourcePath::new(&source_base.source.path)?,
                origin: source_base.origin,
            },
        };
        Ok(GeneratedCompilerInput {
            resolver,
            caller,
            from,
        })
    }
    pub(crate) fn routes(
        &self,
        context: CapturedMacroContext<'_>,
    ) -> Result<super::source_macro_routes::SourceMacroRoutes, Diagnostic> {
        self.routes_using(context, &[], false)
    }
    pub(crate) fn child_route_snapshot(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<Arc<GeneratedRouteSnapshot>, Diagnostic> {
        Ok(Arc::new(GeneratedRouteSnapshot {
            record: self.items.record().descriptor().fingerprint(),
            owner: self.owner.clone(),
            epoch: self.items.epoch(),
            origin: self.origin(),
            routes: Arc::new(self.routes_using(context, ancestors, false)?),
        }))
    }
    pub(crate) fn completed_child_routes(
        &self,
    ) -> impl Iterator<Item = Arc<super::source_macro_routes::SourceMacroRoutes>> + '_ {
        self.route_children
            .iter()
            .map(|snapshot| snapshot.routes.clone())
    }
    fn call_namespaces(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<super::macro_names::MacroNamespaces, Diagnostic> {
        self.call_namespaces_with_routes(context, ancestors, &[])
    }
    fn call_namespaces_with_routes(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
        extra: &[Arc<super::source_macro_routes::SourceMacroRoutes>],
    ) -> Result<super::macro_names::MacroNamespaces, Diagnostic> {
        let mut routes = Vec::new();
        let mut bytes = 0usize;
        for provider in if context.owners.is_empty() {
            std::slice::from_ref(context.provider)
        } else {
            context.owners
        } {
            for route in provider.routes_in(context.owners)? {
                bytes = bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("generated call route catalogue metadata budget"))?;
                routes
                    .try_reserve(1)
                    .map_err(|_| fault("generated source route allocation"))?;
                routes.push(route);
            }
        }
        let mut add =
            |route: Arc<super::source_macro_routes::SourceMacroRoutes>| -> Result<(), Diagnostic> {
                bytes = bytes
                    .checked_add(route.metadata_bytes()?)
                    .ok_or_else(|| fault("generated call route catalogue metadata budget"))?;
                routes
                    .try_reserve(1)
                    .map_err(|_| fault("generated call route catalogue allocation failed"))?;
                routes.push(route);
                Ok(())
            };
        add(Arc::new(self.routes_using(context, ancestors, true)?))?;
        for route in self.completed_child_routes() {
            add(route)?;
        }
        for (i, ancestor) in ancestors.iter().enumerate() {
            add(Arc::new(ancestor.routes_using(
                context,
                &ancestors[i + 1..],
                true,
            )?))?;
            for route in ancestor.completed_child_routes() {
                add(route)?;
            }
        }
        if let Some(compiler) = &context.provider.compiler {
            add(Arc::new(compiler.source_routes()?))?;
            for (_, unit) in compiler.facade_aliases() {
                add(Arc::new(compiler.source_exports(unit)?))?;
            }
        }
        if let Some(procedural) = &context.provider.procedural {
            for route in
                super::source_macro_routes::SourceMacroRoutes::from_procedural_catalog(procedural)?
            {
                add(Arc::new(route))?;
            }
        }
        for route in extra {
            add(route.clone())?;
        }
        let mut namespaces =
            super::source_macro_routes::SourceMacroRoutes::discovery_graph(&routes)?;
        if let Some(compiler) = &context.provider.compiler {
            for (name, definition) in compiler.compiler_globals()? {
                namespaces.compiler_global(self.owner.crate_key, name, definition)?;
            }
            let core = compiler
                .definitions()
                .next()
                .ok_or_else(|| fault("compiler facet empty"))?
                .defining_unit;
            for scope in self.module_scopes.keys() {
                namespaces.set_prelude(scope.clone(), core)?;
            }
        }
        Ok(namespaces)
    }
    pub(crate) fn has_inflight_attributes(&self) -> bool {
        self.inflight.values().any(|key| {
            matches!(
                key.macro_identity,
                crate::expand::identity::MacroIdentity::Procedural {
                    kind: crate::expand::identity::ProceduralKind::Attribute,
                    ..
                }
            )
        })
    }
    pub(crate) fn candidate_routes(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<super::source_macro_routes::SourceMacroRoutes, Diagnostic> {
        self.routes_using(context, ancestors, true)
    }
    pub(crate) fn check_inflight_attributes(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
        routes: &[Arc<super::source_macro_routes::SourceMacroRoutes>],
    ) -> Result<(), Diagnostic> {
        if !self.has_inflight_attributes() {
            return Ok(());
        }
        let names = self.call_namespaces_with_routes(context, ancestors, routes)?;
        for key in self.inflight.values() {
            if !matches!(
                key.macro_identity,
                crate::expand::identity::MacroIdentity::Procedural {
                    kind: crate::expand::identity::ProceduralKind::Attribute,
                    ..
                }
            ) {
                continue;
            }
            let Some((_, GeneratedCallIndex::Attribute(index))) =
                self.call_sites.get(&key.invocation)
            else {
                return Err(fault("inflight attribute selection attachment absent"));
            };
            let Some(input) = crate::expand::generated_attribute_input::prepare_remaining(
                &self.items,
                *index,
                &self.cfg,
                &self.consumed_attribute_indices(key.invocation),
                &self.inert_attribute_indices(key.invocation)?,
                &self
                    .compiler_test_prefixes
                    .get(&key.invocation)
                    .map(|row| row.0.clone())
                    .unwrap_or_default(),
            )?
            else {
                return Err(fault("inflight attribute input changed"));
            };
            let scope = self
                .call_scopes
                .get(&key.invocation)
                .ok_or_else(|| fault("inflight attribute scope absent"))?;
            match names.lookup_invocation_segments(
                input.syntax.segments(),
                input.syntax.absolute(),
                scope,
                self.edition,
            )? {
                super::macro_names::PathLookup::Ambiguous(_) => {
                    return Err(Diagnostic::new(
                        "LRR-E-MACRO-AMBIGUOUS",
                        "inflight generated attribute became ambiguous".into(),
                    ));
                }
                super::macro_names::PathLookup::Defined(definition) => {
                    let catalog = context
                        .provider
                        .procedural
                        .as_ref()
                        .ok_or_else(|| fault("inflight attribute catalog absent"))?;
                    if catalog
                        .select(
                            &definition,
                            crate::expand::identity::ProceduralKind::Attribute,
                        )?
                        .is_none_or(|selected| selected.identity() != &key.macro_identity)
                    {
                        return Err(fault("inflight attribute export changed"));
                    }
                }
                super::macro_names::PathLookup::Inaccessible(_) => {
                    return Err(Diagnostic::new(
                        "LRR-E-MACRO-INACCESSIBLE",
                        "inflight generated attribute became inaccessible".into(),
                    ));
                }
                _ => {} // Candidate absence/pending is not a completeness certificate.
            }
        }
        Ok(())
    }
    fn routes_using(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
        partial: bool,
    ) -> Result<super::source_macro_routes::SourceMacroRoutes, Diagnostic> {
        if !partial && !self.ready_for_outward() {
            return Err(fault("generated macro routes require completed calls"));
        }
        let parsed = ParsedFile::from_generated_items(&self.items)?;
        let outline = parsed.outline_item_fragment(&self.owner, &self.cfg)?;
        let plan = super::generated_headers::GeneratedHeaderPlan::prepare_catalog(self)?;
        let headers = self
            .items
            .items()
            .iter()
            .map(|h| ((h.start, h.end), h.key.fingerprint()))
            .collect::<BTreeMap<_, _>>();
        // Charged captured extern rows also cover this bounded temporary identity index.
        let extern_ids = outline
            .extern_crates
            .iter()
            .map(|row| row.identity)
            .collect::<BTreeSet<_>>();
        let ids = outline
            .items
            .iter()
            .filter(|i| {
                i.def_key.item_kind == ItemKind::Macro
                    || extern_ids.contains(&i.def_key.fingerprint())
            })
            .map(|i| {
                headers
                    .get(&(i.declaration_span.start, i.declaration_span.end))
                    .copied()
                    .map(|id| (i.def_key.fingerprint(), id))
                    .ok_or_else(|| fault("generated route macro header absent"))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let (dependencies, available) = context.provider.route_dependency_sets()?;
        let mut routes = super::source_macro_routes::SourceMacroRoutes::from_lookup_with_available(
            &outline,
            &dependencies,
            &available,
            &[],
            |raw| {
                ids.get(&raw)
                    .and_then(|id| self.definitions.get(id))
                    .cloned()
            },
            |module, at, name| {
                let source_scope = SourceScope {
                    crate_key: self.owner.crate_key,
                    module_path: module.to_vec(),
                    owner: None,
                };
                let scope = *self
                    .module_scopes
                    .get(&source_scope)
                    .ok_or_else(|| fault("generated macro import module scope absent"))?;
                let name = MacroName::parse(name, self.edition)?;
                self.resolve_reference(
                    MacroReference {
                        scope,
                        before: at,
                        name: &name,
                        context: self.root_context,
                        origin: Some(self.origin()),
                    },
                    Some(context),
                    ancestors,
                )
            },
        )?;
        if routes
            .extern_aliases
            .iter()
            .any(|row| row.macro_use.is_some())
        {
            let mut prefix = vec![
                u32::try_from(context.before)
                    .map_err(|_| fault("macro-use source order overflow"))?,
            ];
            for (index, ancestor) in ancestors.iter().enumerate() {
                let child = ancestors.get(index + 1).copied().unwrap_or(self);
                let (position, _) = ancestor
                    .call_sites
                    .get(&child.items.record().key().invocation)
                    .ok_or_else(|| fault("macro-use generated parent call order absent"))?;
                prefix.push(*position);
            }
            for row in &mut routes.extern_aliases {
                let mut order = Vec::new();
                order
                    .try_reserve_exact(prefix.len() + row.order.len())
                    .map_err(|_| fault("macro-use order allocation failed"))?;
                order.extend_from_slice(&prefix);
                order.extend_from_slice(&row.order);
                row.order = order;
            }
        }
        routes.export_pending = self
            .calls
            .keys()
            .map(|(_, id)| *id)
            .chain(self.inflight.keys().copied())
            .collect();
        routes.canonical_generated_extern_aliases(&ids)?;
        routes.binding_origin = Some(self.origin());
        routes.canonical_generated_imports(plan.discovery_imports())?;
        Ok(routes)
    }
    /// A nested producer must be integrated before its parent textual frontier can close.
    pub fn complete_call(
        &mut self,
        record: Arc<ExpansionRecord>,
    ) -> Result<GeneratedItems, Diagnostic> {
        self.complete_call_using(record, None)
    }
    pub(crate) fn reserve_child_routes(
        &mut self,
        child: &Self,
        remaining: usize,
    ) -> Result<usize, Diagnostic> {
        let child_bytes = child.metadata_bytes()?;
        let binding_bytes = child
            .outward
            .iter()
            .try_fold(0usize, |bytes, entry| {
                bytes
                    .checked_add(entry.scope.heap_bytes())
                    .and_then(|n| n.checked_add(entry.name.as_str().len()))
                    .and_then(|n| n.checked_add(160))
            })
            .ok_or_else(|| fault("child binding scope metadata overflow"))?;
        let outward_count = self
            .outward
            .len()
            .checked_add(child.outward.len())
            .ok_or_else(|| fault("child outward scope count limit"))?;
        let outward_growth = outward_count
            .saturating_sub(self.outward.capacity())
            .checked_mul(std::mem::size_of::<OutwardMacro>())
            .ok_or_else(|| fault("child outward scope allocation overflow"))?;

        let count = self
            .route_children
            .len()
            .checked_add(child.route_children.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| fault("child route snapshot count limit"))?;
        let selections = self
            .selection_children
            .len()
            .checked_add(1)
            .ok_or_else(|| fault("selection child count limit"))?;
        selections
            .saturating_sub(self.selection_children.capacity())
            .checked_mul(std::mem::size_of::<Arc<Self>>())
            .and_then(|n| {
                count
                    .saturating_sub(self.route_children.capacity())
                    .checked_mul(std::mem::size_of::<Arc<GeneratedRouteSnapshot>>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(child_bytes))
            .and_then(|n| n.checked_add(binding_bytes))
            .and_then(|n| n.checked_add(outward_growth))
            .filter(|n| *n <= remaining)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "retained selection and route metadata limit".into(),
                )
            })?;
        let old_outward = self.outward.capacity();
        self.outward
            .try_reserve_exact(child.outward.len())
            .map_err(|_| fault("child outward scope allocation"))?;
        let retained_bindings = self
            .outward
            .capacity()
            .checked_sub(old_outward)
            .and_then(|n| n.checked_mul(std::mem::size_of::<OutwardMacro>()))
            .and_then(|n| n.checked_add(binding_bytes))
            .ok_or_else(|| fault("child outward scope allocation overflow"))?;
        let previous = self.selection_children.capacity();
        self.selection_children
            .try_reserve_exact(1)
            .map_err(|_| fault("selection child allocation"))?;
        let selection_bytes = self
            .selection_children
            .capacity()
            .checked_sub(previous)
            .and_then(|n| n.checked_mul(std::mem::size_of::<Arc<Self>>()))
            .and_then(|n| n.checked_add(child_bytes))
            .ok_or_else(|| fault("selection child metadata overflow"))?;
        let count = self
            .route_children
            .len()
            .checked_add(child.route_children.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| fault("child route snapshot count limit"))?;
        let old = self.route_children.capacity();
        self.route_children
            .try_reserve_exact(count - self.route_children.len())
            .map_err(|_| fault("completed child route allocation failed"))?;
        self.route_children
            .capacity()
            .checked_sub(old)
            .and_then(|n| n.checked_mul(std::mem::size_of::<Arc<GeneratedRouteSnapshot>>()))
            .and_then(|n| n.checked_add(selection_bytes))
            .and_then(|n| n.checked_add(retained_bindings))
            .filter(|n| *n <= remaining)
            .ok_or_else(|| fault("child route index growth overflow"))
    }
    pub(crate) fn complete_child(
        &mut self,
        mut child: Self,
        snapshot: Arc<GeneratedRouteSnapshot>,
    ) -> Result<(), Diagnostic> {
        if snapshot.record != child.items.record().descriptor().fingerprint()
            || snapshot.owner != child.owner
            || snapshot.epoch != child.items.epoch()
            || snapshot.origin != child.origin()
        {
            return Err(fault(
                "child route snapshot owner/record/epoch/origin differs",
            ));
        }
        let count = self
            .route_children
            .len()
            .checked_add(child.route_children.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| fault("child route snapshot count limit"))?;
        if count > self.route_children.capacity() {
            return Err(fault(
                "child route index was not reserved before completion",
            ));
        }
        let snapshots = std::mem::take(&mut child.route_children);
        let record = child.items.record_input();
        if record.key().macro_identity.is_derive() {
            self.complete_derive_child(record, child)?;
        } else {
            self.complete_call_using(record, Some(child))?;
        }
        self.route_children.push(snapshot);
        self.route_children.extend(snapshots);
        Ok(())
    }
    fn complete_derive_child(
        &mut self,
        record: Arc<ExpansionRecord>,
        child: Self,
    ) -> Result<(), Diagnostic> {
        let key = record.key();
        if self.inflight.get(&key.invocation) != Some(key)
            || self.derive_outputs.get(&key.invocation).is_none_or(|old| {
                old.descriptor().fingerprint() != record.descriptor().fingerprint()
            })
        {
            return Err(fault("derive completion differs from retained output"));
        }
        let (_, GeneratedCallIndex::Derive(item, _)) = *self
            .call_sites
            .get(&key.invocation)
            .ok_or_else(|| fault("derive completion site absent"))?
        else {
            return Err(fault("derive completion site kind differs"));
        };
        let header = &self.items.items()[item];
        let original = header.key.fingerprint();
        let at = header.start;
        let scope = self
            .call_scopes
            .get(&original)
            .ok_or_else(|| fault("derive completion scope absent"))?
            .clone();
        let lexical = *self
            .module_scopes
            .get(&scope)
            .ok_or_else(|| fault("derive completion lexical scope absent"))?;
        let bindings = self.merge_child_metadata(&child, &scope, key, at, true)?;
        Arc::make_mut(&mut self.scopes).publish_prefix(
            lexical,
            u64::from(at),
            original,
            bindings,
        )?;
        self.selection_children.push(Arc::new(child));
        self.completed.insert(key.fingerprint(), record.clone());
        self.inflight.remove(&key.invocation);
        self.appended_derives.insert(key.invocation);
        let helpers = self
            .helper_scopes
            .get(&original)
            .ok_or_else(|| fault("derive completion helpers absent"))?;
        if helpers
            .occurrences()
            .all(|id| self.appended_derives.contains(&id))
            && helpers.unresolved_outer().is_empty()
            && {
                let consumed = self.consumed_attribute_indices(original);
                // Empty groups have separate compiler-only certificates.
                helpers
                    .empty_annotation_indices()
                    .all(|at| consumed.contains(&at))
            }
        {
            Arc::make_mut(&mut self.scopes).complete_prefix(lexical, u64::from(at), original)?;
            self.calls.remove(&(at, original));
            self.settled_derive_items.insert(original);
        }
        Ok(())
    }
    fn complete_call_using(
        &mut self,
        record: Arc<ExpansionRecord>,
        child: Option<Self>,
    ) -> Result<GeneratedItems, Diagnostic> {
        let expected = self
            .pending
            .as_ref()
            .map(|request| request.key())
            .or_else(|| self.inflight.get(&record.key().invocation))
            .ok_or_else(|| fault("no generated call demand is pending or parked"))?;
        if record.key() != expected {
            return Err(fault("generated call reply differs from exact demand"));
        }
        let wire_bytes = self
            .completed_wire_bytes
            .checked_add(record.descriptor().wire_bytes)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "fragment completed wire budget".into(),
                )
            })?;
        let allocated = self
            .completed_allocated_bytes
            .checked_add(record.estimated_bytes()?)
            .and_then(|n| n.checked_add(192))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "fragment completed record allocation budget".into(),
                )
            })?;
        let items = GeneratedItems::admit(expected, record.clone(), self.items.epoch())?;
        let headers = self
            .completed_headers
            .checked_add(items.items().len())
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "fragment completed header budget".into(),
                )
            })?;
        let call_scope = self
            .call_scopes
            .get(&record.key().invocation)
            .ok_or_else(|| fault("completed generated call scope absent"))?
            .clone();
        let lexical = *self
            .module_scopes
            .get(&call_scope)
            .ok_or_else(|| fault("completed generated lexical scope absent"))?;
        let parsed = ParsedFile::from_generated_items(&items)?;
        let outline = parsed.outline_item_fragment(&call_scope, &self.cfg)?;
        if child.is_none()
            && (!outline.imports.is_empty()
                || outline.scopes.values().any(|s| !s.blockers.is_empty())
                || outline.textual_events.iter().any(|e| {
                    matches!(
                        e.kind,
                        crate::syntax::outline::TextualEventKind::Producer { .. }
                    )
                })
                || outline.items.iter().any(|i| {
                    matches!(
                        i.def_key.item_kind,
                        ItemKind::Macro | ItemKind::Module | ItemKind::Impl | ItemKind::Trait
                    )
                }))
        {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "generated call output requires nested producer or import integration".into(),
            ));
        }
        let identity = expected.invocation;
        let &(at, index) = self
            .call_sites
            .get(&identity)
            .ok_or_else(|| fault("generated call frontier absent"))?;
        let position = (at, identity);
        if self.calls.get(&position) != Some(&index) {
            return Err(fault("generated call position differs from pending demand"));
        }
        let after_derive = matches!(index, GeneratedCallIndex::Attribute(_))
            && self.helper_scopes.contains_key(&identity);
        let bindings = match &child {
            Some(child) => self.merge_child_metadata(
                child,
                &call_scope,
                record.key(),
                position.0,
                after_derive,
            )?,
            None => Vec::new(),
        };
        if after_derive {
            Arc::make_mut(&mut self.scopes).publish_prefix(
                lexical,
                u64::from(position.0),
                position.1,
                bindings,
            )?;
            Arc::make_mut(&mut self.scopes).complete_prefix(
                lexical,
                u64::from(position.0),
                position.1,
            )?;
        } else {
            Arc::make_mut(&mut self.scopes).publish(
                lexical,
                u64::from(position.0),
                position.1,
                bindings,
            )?;
        }
        self.calls.remove(&position);
        if let Some(child) = child {
            self.selection_children.push(Arc::new(child));
        }
        self.completed.insert(record.key().fingerprint(), record);
        self.inflight.remove(&identity);
        self.pending = None;
        self.completed_wire_bytes = wire_bytes;
        self.completed_allocated_bytes = allocated;
        self.completed_headers = headers;
        Ok(items)
    }
    fn merge_child_metadata(
        &mut self,
        child: &Self,
        call_scope: &SourceScope,
        key: &ExpansionKey,
        position: u32,
        append: bool,
    ) -> Result<Vec<(MacroName, MacroDefinition)>, Diagnostic> {
        if self.selection_children.len() == self.selection_children.capacity() {
            return Err(fault("completed selection child was not reserved"));
        }
        if !child.ready_for_outward()
            || &child.owner != call_scope
            || child.items.record().key() != key
            || !child
                .ancestry
                .descendant_of(Some(child.origin()), Some(self.origin()))?
        {
            return Err(fault(
                "generated child completion owner/key/ancestry differs",
            ));
        }
        self.definitions
            .len()
            .checked_add(child.outward.len())
            .ok_or_else(|| fault("generated child definition count limit"))?;
        let bindings = child.root_bindings()?;
        self.outward
            .try_reserve(child.outward.len())
            .map_err(|_| fault("child outward allocation"))?;
        Arc::make_mut(&mut self.facts).extend(&child.facts)?;
        Arc::make_mut(&mut self.ancestry).extend(&child.ancestry)?;
        let insertion = self
            .outward
            .partition_point(|e| e.at < position || (append && e.at == position));
        let mut rows = Vec::new();
        rows.try_reserve_exact(child.outward.len())
            .map_err(|_| fault("child outward insertion allocation"))?;
        for entry in &child.outward {
            if self.definitions.contains_key(&entry.definition.identity) {
                return Err(fault("generated child definition already registered"));
            }
            Arc::make_mut(&mut self.declarations).register(
                entry.definition.clone(),
                entry.context,
                entry.origin,
                &self.facts,
                &self.ancestry,
            )?;
            self.definitions
                .insert(entry.definition.identity, entry.definition.clone());
            if let Some(owner) = child.source_body_owners.get(&entry.definition.identity) {
                self.source_body_owners
                    .insert(entry.definition.identity, owner.clone());
            } else {
                self.body_owners.insert(
                    entry.definition.identity,
                    child.body_for(entry.definition.identity),
                );
            }
            rows.push(OutwardMacro {
                scope: entry.scope.clone(),
                name: entry.name.clone(),
                definition: entry.definition.clone(),
                context: entry.context,
                origin: entry.origin,
                at: position,
            });
        }
        self.outward.splice(insertion..insertion, rows);
        Ok(bindings)
    }
    fn body_for(&self, definition: Digest) -> Arc<GeneratedItems> {
        self.body_owners
            .get(&definition)
            .cloned()
            .unwrap_or_else(|| self.items.clone())
    }
    pub(crate) fn revalidate_calls(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<(), Diagnostic> {
        if !self.ready_for_outward() || self.pending.is_some() {
            return Err(fault("generated selection revalidation context incomplete"));
        }
        for (id, prefix) in &self.compiler_test_prefixes {
            let item = self
                .items
                .items()
                .iter()
                .position(|h| h.key.fingerprint() == *id)
                .ok_or_else(|| fault("compiler test prefix replay item absent"))?;
            if self.compiler_test_indices(*id, item, context, ancestors)? != *prefix {
                return Err(fault("compiler test prefix changed before sealing"));
            }
        }
        for (id, proof) in &self.compiler_test_items {
            let item = self
                .items
                .items()
                .iter()
                .position(|h| h.key.fingerprint() == *id)
                .ok_or_else(|| fault("compiler test replay item absent"))?;
            if self.compiler_test_indices(*id, item, context, ancestors)?.1 != *proof {
                return Err(fault("compiler test proof changed before sealing"));
            }
        }
        for (id, proof) in &self.inherited_items {
            let item = self
                .items
                .items()
                .iter()
                .position(|h| h.key.fingerprint() == *id)
                .ok_or_else(|| fault("inherited replay item absent"))?;
            if self.inherited_item_fingerprint(item)? != Some(*proof) {
                return Err(fault("inherited helper proof changed before sealing"));
            }
        }
        if !self.empty_derives.is_empty() {
            let names = self.call_namespaces(context, ancestors)?;
            let builtin = context
                .provider
                .compiler
                .as_ref()
                .and_then(|c| c.attribute_catalog())
                .ok_or_else(|| fault("empty derive replay compiler absent"))?;
            for ((id, at), certificate) in &self.empty_derives {
                let scope = self
                    .call_scopes
                    .get(id)
                    .ok_or_else(|| fault("empty replay scope absent"))?;
                let selected = super::generated_derives::GeneratedDeriveResolution::resolve(
                    &self.items,
                    certificate.item,
                    &self.cfg,
                    scope.clone(),
                    &names,
                    builtin,
                )?;
                let plan = selected.plan(&self.items, certificate.item, &self.cfg)?;
                if *at != certificate.annotation
                    || plan.binding_digest() != certificate.binding
                    || !matches!(selected.candidates().get(at),Some(super::macro_names::PathLookup::Defined(d)) if d == &certificate.definition)
                    || !plan
                        .annotations()
                        .iter()
                        .any(|a| a.index() == *at && a.paths().is_empty())
                    || self.empty_member_fingerprint(&plan)? != certificate.members
                {
                    return Err(fault("empty derive certificate changed before sealing"));
                }
            }
        }
        for record in self.completed.values() {
            let identity = record.key().invocation;
            let &(at, index) = self
                .call_sites
                .get(&identity)
                .ok_or_else(|| fault("completed generated call identity absent"))?;
            match self.prepare_call(at, identity, index, Some(context), ancestors, false)? {
                GeneratedCallDemand::Ready(request) if request.key() == record.key() => {}
                _ => {
                    return Err(fault(
                        "generated macro selection changed before scope sealing",
                    ));
                }
            }
        }
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(ancestors.len() + 1)
            .map_err(|_| fault("generated selection ancestry allocation"))?;
        parents.push(self);
        parents.extend_from_slice(ancestors);
        for child in &self.selection_children {
            child.revalidate_calls(context, &parents)?;
        }
        Ok(())
    }
    pub(crate) fn ancestry(&self) -> &ExpansionAncestry {
        &self.ancestry
    }
    pub(crate) fn root_bindings(&self) -> Result<Vec<(MacroName, MacroDefinition)>, Diagnostic> {
        if !self.ready_for_outward() {
            return Err(fault("macro-use outward bindings require completed calls"));
        }
        match self.scopes.local_bindings(self.scope, u64::MAX)? {
            crate::expand::scopes::LocalBindings::Ready(bindings) => Ok(bindings),
            crate::expand::scopes::LocalBindings::NeedInput(input) => Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                format!("macro-use outward scope retains producer {input}"),
            )),
        }
    }
    fn diagnostic_origin_parents(
        &self,
        context: CapturedMacroContext<'_>,
        ancestors: &[&Self],
    ) -> Result<Vec<crate::syntax::origin::ExpansionOrigin>, Diagnostic> {
        let chain = self.ancestry.chain(self.origin())?;
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(chain.len())
            .map_err(|_| fault("diagnostic origin allocation"))?;
        for expansion in chain {
            let catalog = std::iter::once(self)
                .chain(ancestors.iter().copied())
                .find(|catalog| Digest(catalog.origin().0) == expansion);
            let identity = catalog.map(|catalog| catalog.items.record().key().invocation);
            let mut invocation: Option<crate::syntax::origin::SourceOrigin> = None;
            if let Some(identity) = identity {
                for provider in std::iter::once(context.provider).chain(context.owners) {
                    if let Some(at) = provider.original_invocation_origin(identity)? {
                        if let Some(old) = &invocation
                            && (old.source != at.source || old.range != at.range)
                        {
                            return Err(fault(
                                "diagnostic invocation has multiple captured owners",
                            ));
                        }
                        invocation = Some(at);
                    }
                }
            }
            // A generated producer is selected by its structural invocation
            // identity in an actual ancestor catalog. Retain its owned token
            // site when it has no whole original-source invocation attachment.
            if invocation.is_none()
                && let Some(identity) = identity
            {
                for ancestor in std::iter::once(self).chain(ancestors.iter().copied()) {
                    for call in ancestor
                        .items
                        .calls()
                        .iter()
                        .filter(|call| call.key.fingerprint() == identity)
                    {
                        let span = call
                            .input
                            .spans()
                            .iter()
                            .find(|span| span.key() == call.site)
                            .ok_or_else(|| fault("generated parent call site absent"))?;
                        let range =
                            crate::syntax::spans::SourceRange::checked(span.start, span.end)?;
                        if let Some(old) = &invocation {
                            if old.source != span.source || old.range != range {
                                return Err(fault("generated parent call site owners conflict"));
                            }
                            continue;
                        }
                        let mut path = String::new();
                        path.try_reserve_exact(span.source.path.len())
                            .map_err(|_| fault("generated parent call site allocation"))?;
                        path.push_str(&span.source.path);
                        invocation = Some(crate::syntax::origin::SourceOrigin {
                            source: crate::syntax::token_wire::SourceIdentity {
                                path,
                                unit: span.source.unit,
                                content: span.source.content,
                            },
                            range,
                        });
                    }
                }
            }
            parents.push(crate::syntax::origin::ExpansionOrigin {
                expansion,
                invocation_identity: identity,
                invocation,
            });
        }
        Ok(parents)
    }
    pub(crate) fn outward(&self) -> &[OutwardMacro] {
        &self.outward
    }
    pub(crate) fn owner(&self) -> &SourceScope {
        &self.owner
    }
    pub(crate) fn origin(&self) -> Key {
        Key(self.items.record().key().fingerprint().0)
    }
    pub(crate) fn invocation(&self) -> Digest {
        self.items.record().key().invocation
    }
    pub(crate) fn facts(&self) -> &HygieneFacts {
        &self.facts
    }
    pub(crate) fn module_source(
        &self,
        producer: Digest,
    ) -> Option<&super::generated_headers::GeneratedModuleSource> {
        self.module_sources
            .get(&producer)
            .or_else(|| self.completed_module_sources.get(&producer))
            .map(|module| &module.demand)
    }
    pub(crate) fn find_catalogue<'a>(
        &'a self,
        identity: Digest,
        ancestors: &mut Vec<&'a Self>,
    ) -> Result<Option<&'a Self>, Diagnostic> {
        if self.record_identity() == identity {
            return Ok(Some(self));
        }
        for child in &self.selection_children {
            ancestors
                .try_reserve(1)
                .map_err(|_| fault("completed catalogue ancestor allocation"))?;
            ancestors.push(self);
            if let Some(found) = child.find_catalogue(identity, ancestors)? {
                return Ok(Some(found));
            }
            ancestors.pop();
        }
        Ok(None)
    }
    pub(crate) fn complete_module_catalogue(
        &mut self,
        producer: Digest,
        child: Arc<crate::expand::source_declarative::SourceDeclarative>,
    ) -> Result<(), Diagnostic> {
        let module = self
            .module_sources
            .get(&producer)
            .ok_or_else(|| fault("module source catalogue is not pending"))?;
        let source = child.module_source();
        if &source.outline().root_scope != module.demand.scope()
            || source.outline().cfg != self.cfg.fingerprint()
        {
            return Err(fault("completed module source scope/cfg differs"));
        }
        let index = child.macro_index();
        let bindings = index.completed_root_bindings()?;
        let ids = bindings
            .iter()
            .map(|(_, definition)| definition.identity)
            .collect::<BTreeSet<_>>();
        let child_scope = *self
            .module_scopes
            .get(module.demand.scope())
            .ok_or_else(|| fault("completed module lexical scope absent"))?;
        let at = self
            .items
            .items()
            .iter()
            .find(|header| &header.key == module.demand.header())
            .ok_or_else(|| fault("completed module header absent"))?
            .end;
        let scope = module.demand.scope().clone();
        self.definitions
            .len()
            .checked_add(ids.len())
            .ok_or_else(|| fault("module macro definition count limit"))?;
        if ids.iter().any(|id| self.definitions.contains_key(id)) {
            return Err(fault("module macro definition already admitted"));
        }
        self.outward
            .try_reserve(bindings.len())
            .map_err(|_| fault("module outward binding allocation"))?;
        Arc::make_mut(&mut self.facts).extend(index.hygiene_facts())?;
        Arc::make_mut(&mut self.ancestry).extend(index.expansion_ancestry())?;
        Arc::make_mut(&mut self.declarations).copy_selected(
            index.declarations(),
            &ids,
            &self.facts,
            &self.ancestry,
        )?;
        for (name, definition) in &bindings {
            let (context, origin) = index.declarations().metadata_of(definition)?;
            self.definitions
                .insert(definition.identity, definition.clone());
            self.source_body_owners
                .insert(definition.identity, child.clone());
            self.outward.push(OutwardMacro {
                scope: scope.clone(),
                name: name.clone(),
                definition: definition.clone(),
                context,
                origin,
                at,
            });
        }
        self.outward.sort_by_key(|entry| entry.at);
        Arc::make_mut(&mut self.scopes).publish(child_scope, 0, producer, bindings)?;
        let module = self
            .module_sources
            .remove(&producer)
            .ok_or_else(|| fault("completed module context disappeared"))?;
        self.completed_module_sources.insert(producer, module);
        self.metadata_bytes()?;
        Ok(())
    }
    pub fn unresolved_module_sources(
        &self,
    ) -> impl Iterator<Item = &super::generated_headers::GeneratedModuleSource> {
        self.module_sources.values().map(|m| &m.demand)
    }
    pub fn lookup_module_parent(
        &self,
        producer: Digest,
        name: &str,
        context: Key,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_module_parent_using(producer, name, context, None, &[])
    }
    pub(crate) fn module_parent_outer(
        &self,
        producer: Digest,
    ) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        let module = self
            .module_sources
            .get(&producer)
            .or_else(|| self.completed_module_sources.get(&producer))
            .ok_or_else(|| fault("retained module parent context absent"))?;
        Ok(super::macro_scopes::OuterMacroScope {
            scopes: &self.scopes,
            scope: module.lexical,
            before: module.before,
            declarations: &self.declarations,
        })
    }
    pub(crate) fn invocation_outer(
        &self,
        invocation: Digest,
    ) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        let (position, index) = *self
            .call_sites
            .get(&invocation)
            .ok_or_else(|| fault("generated parent invocation absent"))?;
        let scope = *self
            .module_scopes
            .get(
                self.call_scopes
                    .get(&invocation)
                    .ok_or_else(|| fault("generated parent call scope absent"))?,
            )
            .ok_or_else(|| fault("generated parent lexical scope absent"))?;
        let (scopes, before) = if let Some(view) = self.derive_parent_views.get(&invocation) {
            (
                view.as_ref(),
                u64::from(position)
                    .checked_add(1)
                    .ok_or_else(|| fault("derive prefix position overflow"))?,
            )
        } else {
            if matches!(index, GeneratedCallIndex::Derive(_, _)) {
                return Err(fault("derive parent prefix view absent"));
            }
            (self.scopes.as_ref(), u64::from(position))
        };
        Ok(super::macro_scopes::OuterMacroScope {
            scopes,
            scope,
            before,
            declarations: &self.declarations,
        })
    }
    pub(crate) fn lookup_module_parent_using(
        &self,
        producer: Digest,
        name: &str,
        context: Key,
        enclosing: Option<CapturedMacroContext<'_>>,
        ancestors: &[&Self],
    ) -> Result<HygienicLookup, Diagnostic> {
        let module = self
            .module_sources
            .get(&producer)
            .or_else(|| self.completed_module_sources.get(&producer))
            .ok_or_else(|| fault("retained module parent context absent"))?;
        let name = MacroName::parse(name, self.edition)?;
        self.resolve_reference(
            MacroReference {
                scope: module.lexical,
                before: module.before,
                name: &name,
                context,
                origin: None,
            },
            enclosing,
            ancestors,
        )
    }
    pub fn lookup(
        &self,
        name: &str,
        before: u64,
        context: Key,
    ) -> Result<HygienicLookup, Diagnostic> {
        let name = MacroName::parse(name, self.edition)?;
        self.declarations.resolve(
            &self.scopes,
            MacroReference {
                scope: self.scope,
                before,
                name: &name,
                context,
                origin: None,
            },
            &self.facts,
            &self.ancestry,
        )
    }
    pub(crate) fn definition_origin(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<Option<Key>>, Diagnostic> {
        self.declarations.origin_of(definition)
    }
    pub(crate) fn module_definition_body(
        &self,
        definition: &MacroDefinition,
    ) -> Option<Arc<GeneratedItems>> {
        (self.definitions.get(&definition.identity) == Some(definition)
            && !self.source_body_owners.contains_key(&definition.identity))
        .then(|| self.body_for(definition.identity))
    }
    pub(crate) fn captured_definition_owner(
        &self,
        definition: &MacroDefinition,
    ) -> Option<&crate::expand::source_declarative::SourceDeclarative> {
        (self.definitions.get(&definition.identity) == Some(definition))
            .then(|| {
                self.source_body_owners
                    .get(&definition.identity)
                    .map(Arc::as_ref)
            })
            .flatten()
    }
    pub fn expand(
        &self,
        id: Digest,
        key: &ExpansionKey,
        input: &TokenForest,
        site: Key,
    ) -> Result<ExpansionRecord, Diagnostic> {
        let definition = self
            .definitions
            .get(&id)
            .ok_or_else(|| fault("generated macro declaration absent or inactive"))?;
        if let Some(owner) = self.source_body_owners.get(&id) {
            return owner
                .generated_request(definition, key.clone(), input, site)?
                .expand();
        }
        self.body_for(id)
            .expand_macro_body(id, definition.defining_unit, key, input, site)
    }
}

/// Checked storage for declaration-only registration metadata; body recipes are absent.
fn test_registration_bytes(
    item: &crate::syntax::outline::ItemOutline,
) -> Result<usize, Diagnostic> {
    use crate::syntax::ast_adapter::TokenRecipe;
    use crate::syntax::outline::Visibility;
    let mut bytes = std::mem::size_of_val(item)
        .checked_add(160)
        .ok_or_else(|| fault("test registration storage overflow"))?;
    let mut charge = |amount: usize| -> Result<(), Diagnostic> {
        bytes = bytes
            .checked_add(amount)
            .ok_or_else(|| fault("test registration storage limit"))?;
        Ok(())
    };
    charge(item.def_key.declared_name.capacity())?;
    charge(item.header.profile.semantics.capacity())?;
    for path in [&item.scope.module_path, &item.def_key.module_path] {
        charge(
            path.capacity()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or_else(|| fault("test registration path size overflow"))?,
        )?;
        for component in path {
            charge(component.capacity())?;
        }
    }
    charge(
        item.header
            .attributes
            .capacity()
            .checked_mul(std::mem::size_of::<crate::syntax::outline::AttributeRecipe>())
            .ok_or_else(|| fault("test registration attribute size overflow"))?,
    )?;
    let restricted = match &item.header.visibility {
        Visibility::Restricted(tokens) => Some(tokens),
        _ => None,
    };
    for tokens in std::iter::once(&item.header.tokens)
        .chain(std::iter::once(&item.header.context))
        .chain(restricted)
        .chain(
            item.header
                .attributes
                .iter()
                .map(|attribute| &attribute.tokens),
        )
    {
        charge(
            tokens
                .capacity()
                .checked_mul(std::mem::size_of::<TokenRecipe>())
                .ok_or_else(|| fault("test registration token size overflow"))?,
        )?;
        for token in tokens {
            charge(token.text.capacity())?;
        }
    }
    for attribute in &item.header.attributes {
        charge(attribute.name.capacity())?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod inherited_budget_tests {
    #[test]
    fn inherited_environment_preflight_refuses_only_arithmetic_overflow() {
        assert!(super::inherited_helper_bytes([(1024, 2)]).is_ok());
        for rows in [vec![(usize::MAX, 0)], vec![(0, usize::MAX)]] {
            assert_eq!(
                super::inherited_helper_bytes(rows).unwrap_err().code,
                "LRR-E-DISCOVERY-LIMIT"
            );
        }
        // E-R5-006: no aggregate or count ceiling.
        assert!(
            super::inherited_helper_bytes([(160 * 1024 * 1024, 0), (160 * 1024 * 1024, 0)])
                .is_ok()
        );
        assert!(super::inherited_helper_bytes(std::iter::repeat_n((0, 0), 4097)).is_ok());
    }
}
