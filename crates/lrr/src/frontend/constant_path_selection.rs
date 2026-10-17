//! Exact current original Value-member selection. No constant evaluation or provider work.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::BucketKey,
        memo::Database,
        seals::{DiscoveryInput, SealDomain, SealKey},
    },
    frontend::{
        constant_source_admission::ConstantRequest,
        names::{NameDefinition, NameLookup, NameNamespaces},
        source_locals::OrdinaryNameSite,
    },
    source::{
        changes::{PreparedSourceView, SourceSelection},
        manifest::{ItemKind, Namespace},
    },
    syntax::{
        constant_paths::ConstantPathSite,
        outline::{BodyHandle, DeclarationContext, ScopeKey},
        pure_constants::ConstantSource,
        spans::SourceRange,
    },
};
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-PATH-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "constant path selection budget".into(),
    )
}
#[derive(Debug)]
pub(crate) enum ConstantPathOutcome {
    Selected(Box<ConstantPathSelection>),
    LocalBinding(Digest),
    OtherValueMember(NameDefinition),
    /// Missing may be prelude/associated/foreign/generated; it is not absence.
    RequiredNamespace(NameLookup),
    RequiredLexicalConstant(NameDefinition),
    RequiredForeignConstant(NameDefinition),
}
impl ConstantPathOutcome {
    /// A bounded diagnostic retains the actual selected identity/frontier. No
    /// lookup list or source file is formatted wholesale into the error path.
    pub(crate) fn prerequisite_error(self) -> Diagnostic {
        let message = match self {
            Self::Selected(_) => "constant selection has no missing prerequisite".into(),
            Self::LocalBinding(identity) => {
                format!("constant path is shadowed by local binding {identity}")
            }
            Self::OtherValueMember(definition) => format!(
                "selected value member {} in unit {} (edition {}) is not a constant",
                definition.identity, definition.defining_unit, definition.definition_edition
            ),
            Self::RequiredLexicalConstant(definition) => format!(
                "constant member {} in unit {} requires lexical or associated source admission",
                definition.identity, definition.defining_unit
            ),
            Self::RequiredForeignConstant(definition) => format!(
                "constant member {} requires retained source for unit {} (edition {})",
                definition.identity, definition.defining_unit, definition.definition_edition
            ),
            Self::RequiredNamespace(lookup) => match lookup {
                NameLookup::Defined(definition) => format!(
                    "constant namespace member {} requires source admission",
                    definition.identity
                ),
                NameLookup::LocalBinding(identity) => {
                    format!("constant namespace resolved to local binding {identity}")
                }
                NameLookup::NeedInput(identity) => {
                    format!("constant namespace requires input {identity}")
                }
                NameLookup::ModuleRoot(unit) => {
                    format!("constant namespace resolved to module root {unit}")
                }
                NameLookup::Ambiguous(definitions) => format!(
                    "constant namespace has {} ambiguous definitions",
                    definitions.len()
                ),
                NameLookup::AmbiguousModules(paths) => format!(
                    "constant namespace has {} ambiguous module routes",
                    paths.len()
                ),
                NameLookup::Inaccessible(paths) => {
                    format!("constant namespace has {} inaccessible routes", paths.len())
                }
                NameLookup::Absent => "constant namespace has no selected source member".into(),
            },
        };
        Diagnostic::new("LRR-E-CONST-PATH-SOURCE", message)
    }
}
/// Privately minted source request. Caller names/descriptors never construct it.
#[derive(Debug)]
pub(crate) struct ConstantPathSelection {
    site: ConstantPathSite,
    target: SourceSelection,
    definition: NameDefinition,
    context: DeclarationContext,
    body: BodyHandle,
    caller_seal: SealKey,
    target_seal: SealKey,
    caller_fingerprint: Digest,
    target_fingerprint: Digest,
    sources: Vec<SourceSelection>,
    stamps: BTreeMap<InputKey, Digest>,
    bytes: usize,
    byte_limit: usize,
}
fn validate_scope(
    db: &Database,
    input: &DiscoveryInput,
    view: &PreparedSourceView,
    scope: &ScopeKey,
    views: &[&PreparedSourceView],
) -> Result<(), Diagnostic> {
    let certified = input
        .scope
        .as_ref()
        .ok_or_else(|| fault("constant path scope certificate absent"))?;
    let key = input.seal.key();
    key.validate()?;
    if certified.scope != *scope || certified.namespace != Namespace::Value || certified.edition != view.selection().outline.parse.profile.edition || !matches!(input.seal.domain(),SealDomain::Scope{id,namespace} if *id==scope.fingerprint() && *namespace==Namespace::Value) || !input.sources.contains(view.selection()) || input.seal.unresolved_frontiers() != 0 || input
            .sources
            .iter()
            .any(|source| !views.iter().any(|view| view.selection() == source)) {
        return Err(fault(
            "constant path actual source set/scope/world/edition is incomplete or differs",
        ));
    }
    if !matches!(db.published_input(&InputKey::ExpansionEpoch),Some((InputValue::ExpansionEpoch(epoch),_)) if *epoch==key.epoch)
        || !matches!(db.published_input(&InputKey::SourceDependencies),Some((InputValue::SourceDependencies(graph),_)) if graph.fingerprint()==input.source_dependencies)
    {
        return Err(fault("constant path epoch/source graph differs"));
    }
    for (key, fingerprint) in &input.compiler_inputs {
        if db
            .published_input(key)
            .is_none_or(|(_, actual)| actual != *fingerprint)
        {
            return Err(fault("constant path compiler input changed"));
        }
    }
    if !input.records.is_empty()
        || !input.procedural_catalogs.is_empty()
        || !input.helper_declarations.is_empty()
        || !input.argument_summaries.is_empty()
        || !input.argument_indexes.is_empty()
        || !input.argument_fragments.is_empty()
        || !input.source_replacements.is_empty()
        || !input.declaration_fragments.is_empty()
    {
        return Err(fault(
            "constant path generated/native namespace requires separate admission",
        ));
    }
    Ok(())
}
fn require_body(
    db: &Database,
    view: &PreparedSourceView,
    body: &BodyHandle,
) -> Result<DeclarationContext, Diagnostic> {
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.body.as_ref() == Some(body))
        .ok_or_else(|| fault("constant path declaration body absent"))?;
    let context = DeclarationContext {
        scope: item.scope.clone(),
        range: item.declaration_span,
    };
    let definition = item.def_key.fingerprint();
    if !matches!(db.published_input(&InputKey::DefinitionSource(definition)),Some((InputValue::DefinitionSource(parse),_)) if parse==&view.selection().outline.parse)
        || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)),Some((InputValue::DefinitionHeader(header),_)) if header==&item.header)
        || !matches!(db.published_input(&InputKey::DefinitionBody(definition)),Some((InputValue::DefinitionBody(actual),_)) if actual==body)
        || !matches!(db.published_input(&InputKey::DeclarationContext(definition)),Some((InputValue::DeclarationContext(actual),_)) if actual==&context)
    {
        return Err(fault(
            "constant path exact declaration/source/header/body/context changed",
        ));
    }
    Ok(context)
}
impl ConstantPathSelection {
    pub(crate) fn capture_current(
        db: &Database,
        caller: &PreparedSourceView,
        body: &BodyHandle,
        range: SourceRange,
        views: &[&PreparedSourceView],
        byte_limit: usize,
    ) -> Result<ConstantPathOutcome, Diagnostic> {
        if views.is_empty() || byte_limit
                < std::mem::size_of::<Self>() + std::mem::size_of::<ConstantPathOutcome>() + 256 {
            return Err(limit());
        }
        if !views
            .iter()
            .any(|v| v.same_publication(caller) && v.selection() == caller.selection())
        {
            return Err(fault("constant path current caller publication is absent"));
        }
        let mut selections = BTreeSet::new();
        let mut sources = Vec::new();
        let mut bytes =
            std::mem::size_of::<Self>() + std::mem::size_of::<ConstantPathOutcome>() + 256;
        sources
            .try_reserve_exact(views.len())
            .map_err(|_| limit())?;
        for view in views {
            view.require_original_hygiene()?;
            view.require_declaration_syntax()?;
            if view.selection().canonical()? != *view.selection() {
                return Err(fault(
                    "constant path original source selection is not canonical",
                ));
            }
            if !selections.insert(view.selection().location_identity()) {
                return Err(fault(
                    "constant path source selection is duplicate/ambiguous",
                ));
            }
            bytes = bytes
                .checked_add(view.selection().metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
            for (key, value) in view.prepared_inputs() {
                if db
                    .published_input(key)
                    .is_none_or(|(_, actual)| actual != value.fingerprint())
                {
                    return Err(fault(
                        "constant path actual prepared source/compiler facts changed",
                    ));
                }
            }
            sources.push(view.selection().clone());
        }
        sources.sort_by_key(SourceSelection::location_identity);
        let stamps = db.source_view_namespace_stamps()?;
        for key in stamps.keys() {
            bytes = bytes
                .checked_add(std::mem::size_of::<(InputKey, Digest)>() + 128)
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        // A scope seal can disclose a subset. Foreign selection must not treat
        // unretained published original sources as irrelevant name evidence.
        // Membership comes from actual inputs, never a caller completeness flag.
        let mut captured_count = 0usize;
        for key in stamps.keys() {
            if let InputKey::CapturedSource(location) = key {
                captured_count = captured_count
                    .checked_add(1)                    .ok_or_else(limit)?;
                let Some((InputValue::CapturedSource(selection), _)) = db.published_input(key)
                else {
                    return Err(fault("constant path captured source input variant differs"));
                };
                if selection.location_identity() != *location
                    || !views.iter().any(|view| view.selection() == selection)
                {
                    return Err(fault(
                        "constant path complete current original source set is not retained",
                    ));
                }
            }
        }
        if captured_count != views.len() {
            return Err(fault(
                "constant path retained set differs from actual current source membership",
            ));
        }
        let mut structural_reservation = 0usize;
        for view in views {
            structural_reservation = structural_reservation
                .checked_add(
                    view.captured_macro_blob()?
                        .bytes()
                        .len()
                        .checked_mul(128)
                        .ok_or_else(limit)?,
                )
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        if bytes
            .checked_add(structural_reservation)
            .is_none_or(|n| n > byte_limit)
        {
            return Err(limit());
        }
        let context = require_body(db, caller, body)?;
        let parsed = db
            .prepared_parse(&caller.selection().outline.parse)
            .ok_or_else(|| fault("constant path exact original parser lease absent"))?;
        let site = parsed.constant_path_site(caller, body, range)?;
        if site.context() != &context {
            return Err(fault("constant path captured caller context differs"));
        }
        bytes = bytes
            .checked_add(site.metadata_bytes()?)
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        let published = db.source_view_scope_certificate(caller.selection(), &context.scope)?;
        let InputValue::DiscoverySeal(caller_input) = published.as_ref() else {
            return Err(fault("constant path caller Value seal variant differs"));
        };
        validate_scope(db, caller_input, caller, &context.scope, views)?;
        // Fresh private original graph, never caller-supplied namespace authority.
        let mut graph =
            NameNamespaces::from_outlines(&views.iter().map(|v| v.outline()).collect::<Vec<_>>())?;
        let dependencies = caller.source_dependencies()?;
        let mut units = BTreeSet::new();
        for view in views {
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
        // Captured dependency aliases do not by themselves declare `extern
        // crate upstream as local`. Replay the genuine original module routes,
        // including edition-2015 declarations, before resolving qualified uses.
        let mut providers = views
            .iter()
            .map(|view| crate::expand::source_declarative::SourceDeclarative::new(view.share()))
            .collect::<Result<Vec<_>, _>>()?;
        crate::expand::source_declarative::SourceDeclarative::configure_captured_modules(
            &mut providers,
        )?;
        let mut route_bytes = 0usize;
        let mut alias_count = 0usize;
        for provider in &providers {
            for route in provider.routes_in(&providers)? {
                route_bytes = route_bytes
                    .checked_add(route.metadata_bytes()?)
                    .filter(|n| *n <= byte_limit)
                    .ok_or_else(limit)?;
                for row in &route.extern_aliases {
                    alias_count = alias_count
                        .checked_add(1)
                        .ok_or_else(limit)?;
                    graph.extern_alias(row, route.edition)?;
                }
            }
        }
        if bytes
            .checked_add(structural_reservation)
            .and_then(|n| n.checked_add(route_bytes))
            .is_none_or(|n| n > byte_limit)
        {
            return Err(limit());
        }
        let _ = graph.resolve_imports()?; // bounded pure fixed point; unresolved outcomes are retained.
        let replay = graph.certified_scope(
            &context.scope,
            Namespace::Value,
            caller.selection().outline.parse.profile.edition,
        )?;
        if replay.module_fingerprint()
            != caller_input
                .scope
                .as_ref()
                .ok_or_else(|| fault("constant path caller names absent"))?
                .module_fingerprint()
        {
            return Err(fault(
                "constant path original namespace replay differs from current seal",
            ));
        }
        let provider = crate::expand::source_declarative::SourceDeclarative::new(caller.share())?;
        let published_index = provider.completed_published_index()?;
        let mut entered = published_index.enter_body(caller.selection(), &body.key())?;
        for (_, proof) in caller_input
            .runtime_no_item_proofs
            .iter()
            .filter(|(_, p)| p.body() == &body.key())
        {
            let catalog=crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(&caller.selection().outline.parse.profile,proof.definition())?;
            entered.accept_no_item_proof(caller.selection(), &body.key(), proof, &catalog)?;
        }
        // Ambiguous identifier patterns remain NeedInput; no guessed Binding map.
        let locals = entered.local_index(caller.selection(), &body.key(), &BTreeMap::new(), &[])?;
        let ordinary = OrdinaryNameSite {
            source: caller.selection(),
            body: &body.key(),
            at: u64::from(range.start),
            name: site.member()?,
            context: locals.context(),
            namespace: Namespace::Value,
        };
        let found = if site.segments().len() == 1 && !site.absolute() {
            locals.lookup_ordinary(ordinary, &graph, None)?
        } else {
            locals.lookup_qualified_module(ordinary, site.segments(), site.absolute(), &graph)?
        };
        let definition = match found {
            NameLookup::Defined(definition) => definition,
            NameLookup::LocalBinding(identity) => {
                return Ok(ConstantPathOutcome::LocalBinding(identity));
            }
            other => return Ok(ConstantPathOutcome::RequiredNamespace(other)),
        };
        if definition.namespace != Namespace::Value {
            return Err(fault("constant path lookup returned another namespace"));
        }
        let foreign = definition.defining_unit != caller.selection().outline.crate_key;
        let mut targets = views
            .iter()
            .flat_map(|view| view.outline().items.iter().map(move |item| (*view, item)))
            .filter(|(_, item)| item.def_key.fingerprint() == definition.identity);
        let Some((target, item)) = targets.next() else {
            return Ok(if foreign {
                ConstantPathOutcome::RequiredForeignConstant(definition)
            } else {
                ConstantPathOutcome::RequiredLexicalConstant(definition)
            });
        };
        if targets.next().is_some() {
            return Err(fault("constant path target source identity is duplicated"));
        }
        if item.def_key.item_kind != ItemKind::Constant {
            return Ok(ConstantPathOutcome::OtherValueMember(definition));
        }
        if item.scope.owner.is_some() {
            return Ok(ConstantPathOutcome::RequiredLexicalConstant(definition));
        }
        if definition.definition_edition != target.selection().outline.parse.profile.edition
            || definition.defining_unit != item.scope.crate_key
        {
            return Err(fault("constant target owning edition/unit differs"));
        }
        let handle = item
            .body
            .as_ref()
            .ok_or_else(|| fault("selected constant body absent"))?;
        let target_context = require_body(db, target, handle)?;
        // Establish direct original module membership without evaluating/typechecking.
        let captured = ConstantSource::capture_prepared(target, definition.identity)?;
        let declaration = captured
            .declarations()
            .iter()
            .find(|d| d.definition() == definition.identity)
            .ok_or_else(|| fault("selected constant is not an original direct module member"))?;
        if declaration.context() != &target_context || declaration.body_handle() != handle {
            return Err(fault("selected constant direct member/body replay differs"));
        }
        let target_published =
            db.source_view_scope_certificate(target.selection(), &target_context.scope)?;
        let InputValue::DiscoverySeal(target_input) = target_published.as_ref() else {
            return Err(fault("constant target Value seal variant differs"));
        };
        validate_scope(db, target_input, target, &target_context.scope, views)?;
        if target_input.seal.key().unit_graph != caller_input.seal.key().unit_graph
            || target_input.seal.key().epoch != caller_input.seal.key().epoch
        {
            return Err(fault(
                "constant target belongs to another selected world/epoch",
            ));
        }
        let target_replay = graph.certified_scope(
            &target_context.scope,
            Namespace::Value,
            target.selection().outline.parse.profile.edition,
        )?;
        let certified = target_input
            .scope
            .as_ref()
            .ok_or_else(|| fault("constant target certified names absent"))?;
        if certified.module_fingerprint() != target_replay.module_fingerprint() {
            return Err(fault("constant target namespace replay differs"));
        }
        let bucket = BucketKey {
            scope: target_context.scope.clone(),
            namespace: Namespace::Value,
            name: item.def_key.declared_name.clone(),
        };
        if !matches!(certified.lookup(&bucket)?,NameLookup::Defined(found) if found==definition) {
            return Err(fault(
                "constant target sealed member does not select the exact declaration",
            ));
        }
        bytes = bytes
            .checked_add(target.selection().metadata_bytes()?)
            .and_then(|n| n.checked_add(target_context.scope.heap_bytes()))
            .and_then(|n| n.checked_add(captured.metadata_bytes().ok()?))
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        Ok(ConstantPathOutcome::Selected(Box::new(Self {
            site,
            target: target.selection().clone(),
            definition,
            context: target_context,
            body: handle.clone(),
            caller_seal: caller_input.seal.key(),
            target_seal: target_input.seal.key(),
            caller_fingerprint: published.fingerprint(),
            target_fingerprint: target_published.fingerprint(),
            sources,
            stamps,
            bytes,
            byte_limit,
        })))
    }
    pub(crate) fn target(&self) -> &SourceSelection {
        &self.target
    }
    pub(crate) fn definition(&self) -> Digest {
        self.definition.identity
    }
    pub(crate) fn request(&self) -> ConstantRequest {
        ConstantRequest::Constant
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn site(&self) -> &ConstantPathSite {
        &self.site
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        caller: &PreparedSourceView,
        views: &[&PreparedSourceView],
    ) -> Result<(), Diagnostic> {
        if caller.selection() != self.site.source()
            || db.source_view_namespace_stamps()? != self.stamps
        {
            return Err(fault("constant path caller/input membership changed"));
        }
        // capture_current checks the bounded complete source set before
        // copying selections. The reminted identity below compares that set;
        // do not allocate an unbounded caller view list before this admission.
        let fresh = match Self::capture_current(
            db,
            caller,
            self.site.body(),
            self.site.range(),
            views,
            self.byte_limit,
        )? {
            ConstantPathOutcome::Selected(fresh) => fresh,
            other => return Err(other.prerequisite_error()),
        };
        if self.identity() != fresh.identity() {
            return Err(fault(
                "constant path source/member/namespace identity changed",
            ));
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v3/current-source-constant-path-selection");
        e.record(&self.site);
        e.string(self.target.path.as_str());
        e.digest(self.target.location_identity());
        e.record(&self.target.outline.parse);
        e.digest(self.target.outline.cfg);
        e.record(&self.definition);
        e.record(&self.context);
        e.record(&self.body);
        e.record(&self.caller_seal);
        e.record(&self.target_seal);
        e.digest(self.caller_fingerprint);
        e.digest(self.target_fingerprint);
        e.u64(self.sources.len() as u64);
        for source in &self.sources {
            e.digest(source.location_identity());
            e.record(&source.outline.parse);
            e.digest(source.outline.cfg);
        }
        e.u64(self.stamps.len() as u64);
        for (key, value) in &self.stamps {
            e.record(key);
            e.digest(*value);
        }
        e.finish()
    }
}
