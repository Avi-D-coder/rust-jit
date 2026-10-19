//! Coordinator-prepared declaration-only input for the ordinary local Type prefix.
//! It owns metadata and real hygiene; it grants no executable entry or body lease.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::QueryPoll,
    },
    frontend::{
        associated_declarations::{CapturedDeclarationScope, RetainedDeclarationScope},
        impl_index::{DeclarationMacroIndex, OriginalDeclarationNameContext},
        type_name_namespace::SourceTypeOwner,
    },
    syntax::outline::ScopeKey as SourceScope,
};
use ra_ap_syntax::ast;

enum ColdNameStep {
    Pending,
    Name(crate::frontend::names::NameLookup),
    Module(crate::db::key::CurrentModuleNameKey),
}
pub struct ColdDeclarationNames {
    scope: RetainedDeclarationScope,
    context: OriginalDeclarationNameContext,
    tables: DeclarationNameTables,
    names: owned_plan::LocalTypeNames,
    scopes: BTreeMap<SourceScope, u32>,
    seal: crate::db::seals::SealKey,
    seal_value: Digest,
    identity: Digest,
    completion: Digest,
}
impl std::fmt::Debug for ColdDeclarationNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColdDeclarationNames")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
impl ColdDeclarationNames {
    pub(crate) fn prepare(
        scope: &CapturedDeclarationScope<'_>,
        parent: &DeclarationMacroIndex,
        seal: &crate::db::seals::Seal,
        publication: u64,
        completions: &BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>,
    ) -> Result<Arc<Self>, Diagnostic> {
        let context = parent.original_name_context(scope.source())?;
        let completion = Self::completion_identity(scope, completions)?;
        let names = owned_plan::LocalTypeNames::prepare_captured(scope, &context)?;
        let mut retired = BTreeSet::new();
        for row in &scope.facts().items {
            if let Some(receipt) = completions.get(&row.item.def_key.fingerprint())
                && receipt.name_effects().is_some() {
                retired.extend(receipt.retired_names());
                retired.extend(row.attribute_input);
            }
        }
        let mut tables = DeclarationNameTables::prepare(
            scope.facts(),
            scope.parsed()?.key().profile.edition,
            Some(&names),
            |producer| retired.contains(&producer),
        )?;
        let mut generated_names = Vec::new();
        for row in &scope.facts().items {
            let Some(receipt) = completions.get(&row.item.def_key.fingerprint()) else { continue };
            generated_names.extend(tables.extend_annotation_names(row.scope, &row.item.scope, receipt)?);
        }
        let names = names.with_declared(&generated_names)?;
        let mut scopes = BTreeMap::new();
        for row in &scope.facts().items {
            if scopes
                .insert(row.item.scope.clone(), row.scope)
                .is_some_and(|old| old != row.scope)
            {
                return Err(error(
                    "cold declaration scope has conflicting lexical ordinals",
                ));
            }
        }
        let mut identity = StableEncoder::new("lrr/v1/cold-declaration-name-input");
        identity.digest(scope.fingerprint());
        identity.string(scope.source().selection().path.as_str());
        identity.record(&scope.source().selection().outline);
        identity.digest(names.fingerprint());
        identity.digest(Digest(context.root().0));
        identity.digest(Digest(context.comparison().0.0));
        identity.record(&seal.key());
        identity.digest(seal.fingerprint());
        identity.u64(publication);
        identity.digest(completion);
        for (scope, ordinal) in &scopes {
            identity.record(scope);
            identity.u32(*ordinal);
        }
        Ok(Arc::new(Self {
            scope: scope.retain(),
            context,
            tables,
            names,
            scopes,
            seal: seal.key().clone(),
            seal_value: seal.fingerprint(),
            identity: identity.finish(),
            completion,
        }))
    }
    fn completion_identity(
        scope: &CapturedDeclarationScope<'_>,
        completions: &BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>,
    ) -> Result<Digest, Diagnostic> {
        let mut e = StableEncoder::new("lrr/v1/cold-local-name-completions");
        for row in &scope.facts().items {
            if let Some(receipt) = completions.get(&row.item.def_key.fingerprint()) {
                receipt.require_source(scope.source())?;
                receipt.require_local_declaration(scope.parsed()?, scope.body(), scope.scope(), row)?;
                e.digest(receipt.item()); e.digest(receipt.name_fingerprint());
            }
        }
        Ok(e.finish())
    }
    pub(crate) fn reusable(
        &self,
        scope: &CapturedDeclarationScope<'_>,
        parent: &DeclarationMacroIndex,
        seal: &crate::db::seals::Seal,
        completions: &BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>,
    ) -> Result<bool, Diagnostic> {
        let context = parent.original_name_context(scope.source())?;
        let completion = Self::completion_identity(scope, completions)?;
        Ok(self.scope.captured().fingerprint() == scope.fingerprint()
            && self
                .scope
                .captured()
                .source()
                .same_publication(scope.source())
            && self.context.root() == context.root()
            && self.context.comparison() == context.comparison()
            && self.seal == seal.key()
            && self.seal_value == seal.fingerprint()
            && self.completion == completion)
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn scopes(&self) -> impl Iterator<Item = &SourceScope> {
        self.scopes.keys()
    }
    pub(crate) fn supports(&self, scope: &SourceScope) -> bool {
        self.scopes.contains_key(scope)
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> Result<(), Diagnostic> {
        let scope = self.scope.captured();
        scope.require_current(db)?;
        self.context.require_source(scope.source())?;
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),
            Some(InputValue::ExpansionEpoch(epoch)) if *epoch == self.seal.epoch)
            || !matches!(db.input(&InputKey::DiscoverySeal(self.seal.clone())).as_deref(),
                Some(InputValue::DiscoverySeal(publication)) if publication.seal.key() == self.seal
                    && publication.seal.fingerprint() == self.seal_value
                    && matches!(publication.declaration_inventory.syntax_source(self.context.scan()),
                        Some(crate::frontend::impl_index::ImplSyntaxSource::Original { source, .. })
                            if source.selection() == scope.source().selection()))
        {
            return Err(error("cold declaration name epoch/completion changed"));
        }
        Ok(())
    }
    fn require_scope_slot(&self, db: &mut Database, scope: &SourceScope) -> Result<(), Diagnostic> {
        if !self.supports(scope)
            || !matches!(db.input(&InputKey::DeclarationNames(scope.clone())).as_deref(),
            Some(InputValue::DeclarationNames(current)) if current.identity == self.identity)
        {
            return Err(error(
                "cold declaration name publication revoked or replaced",
            ));
        }
        Ok(())
    }
    fn covers_declaration_source(
        &self,
        db: &mut Database,
        identity: Digest,
    ) -> Result<bool, Diagnostic> {
        Ok(
            matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),
            Some(InputValue::DefinitionSource(source)) if *source == self.scope.captured().parsed()?.key()),
        )
    }
    fn declaration(
        &self,
        db: &mut Database,
        identity: Digest,
    ) -> Result<Option<ColdNamedDeclaration>, Diagnostic> {
        if !self.covers_declaration_source(db, identity)? {
            return Ok(None);
        }
        self.require_current(db)?;
        let Some(context) = db.input(&InputKey::DeclarationContext(identity)) else {
            return Ok(None);
        };
        let Some(header) = db.input(&InputKey::DefinitionHeader(identity)) else {
            return Ok(None);
        };
        let Some(source) = db.input(&InputKey::DefinitionSource(identity)) else {
            return Ok(None);
        };
        let (
            InputValue::DeclarationContext(context),
            InputValue::DefinitionHeader(header),
            InputValue::DefinitionSource(source),
        ) = (context.as_ref(), header.as_ref(), source.as_ref())
        else {
            return Err(error("cold canonical declaration variants differ"));
        };
        self.require_scope_slot(db, &context.scope)?;
        let scope = self.scope.captured();
        if *source != scope.parsed()?.key() {
            return Ok(None);
        }
        // A different admitted source must continue through its own canonical
        // service. Its lack of an original-body owner is not an invalid row.
        if !matches!(db.input(&InputKey::LocalDeclarationOwner(identity)).as_deref(),
            Some(InputValue::LocalDeclarationOwner(owner)) if owner == &scope.body().key())
        {
            return Err(error("cold canonical declaration owning body differs"));
        }

        let Some(row) = self.scope.item_at(context.range) else {
            return Ok(None);
        };
        if row.item.def_key.fingerprint() != identity
            || row.item.scope != context.scope
            || row.item.header != *header
        {
            return Err(error("cold canonical declaration rows differ"));
        }
        let Some((name, edition)) = self.names.item_spelling(identity) else {
            return Ok(None);
        };
        let index = DeclarationNameIndex::new(&self.tables, scope.facts(), scope.scope(), edition);
        let lookup = match index.lookup_prefix::<crate::frontend::names::NameNamespaces>(
            row.scope,
            crate::source::manifest::Namespace::Type,
            name,
            || Ok(self.context.comparison()),
            None,
            Some(&self.names),
            |_| None,
        )? {
            PrefixResolution::Name(lookup) => lookup,
            PrefixResolution::NeedImport(id) | PrefixResolution::NeedDeclaration(id) => {
                crate::frontend::names::NameLookup::NeedInput(id)
            }
            _ => return Ok(None),
        };
        Ok(Some(ColdNamedDeclaration {
            definition: row.item.def_key.clone(),
            scope: row.item.scope.clone(),
            header: row.item.header.clone(),
            lookup,
        }))
    }
    pub(crate) fn lookup(
        &self,
        db: &mut Database,
        declaration: SourceTypeOwner<'_>,
        name: &ast::NameRef,
    ) -> QueryPoll<Option<crate::frontend::names::NameLookup>> {
        let step = (|| {
            if !self.covers_declaration_source(db, declaration.definition)? {
                return Ok(ColdNameStep::Pending);
            }
            self.require_current(db)?;
            self.require_scope_slot(db, declaration.scope)?;
            let scope = self.scope.captured();
            if !matches!(db.input(&InputKey::LocalDeclarationOwner(declaration.definition)).as_deref(),
                Some(InputValue::LocalDeclarationOwner(owner)) if owner == &scope.body().key())
            {
                // A generated original-origin join must supply its own actual
                // local declaration owner, never borrow this original receipt.
                return Ok(ColdNameStep::Pending);
            }

            let parsed = scope.parsed()?;
            let Some(source) = db.input(&InputKey::DefinitionSource(declaration.definition)) else {
                return Ok(ColdNameStep::Pending);
            };
            let Some(header) = db.input(&InputKey::DefinitionHeader(declaration.definition)) else {
                return Ok(ColdNameStep::Pending);
            };
            let Some(context) = db.input(&InputKey::DeclarationContext(declaration.definition))
            else {
                return Ok(ColdNameStep::Pending);
            };
            let (
                InputValue::DefinitionSource(source),
                InputValue::DefinitionHeader(header),
                InputValue::DeclarationContext(context),
            ) = (source.as_ref(), header.as_ref(), context.as_ref())
            else {
                return Err(error("cold Type declaration rows differ"));
            };
            // Generated declarations require their real origin/insertion join;
            // this original receipt is never a matching-text substitute.
            if source != &parsed.key() {
                return Ok(ColdNameStep::Pending);
            }
            if header != declaration.header || &context.scope != declaration.scope {
                return Err(error("cold Type declaration owner differs"));
            }
            let proof =
                parsed.declaration_type_name(declaration.definition, header, context, name)?;
            proof.require(source, declaration.definition, header, context, name.text())?;
            let Some(row) = scope.item_at(context.range) else {
                return Ok(ColdNameStep::Pending);
            };
            if row.item.def_key.fingerprint() != declaration.definition
                || row.item.scope != context.scope
                || row.item.declaration_span != context.range
                || row.item.header != *header
            {
                return Err(error("cold Type canonical row differs"));
            }
            let ordinal = *self
                .scopes
                .get(declaration.scope)
                .ok_or_else(|| error("cold Type lexical scope absent"))?;
            if ordinal != row.scope {
                return Err(error("cold Type lexical ordinal differs"));
            }
            let name = MacroName::local(name.text(), parsed.token_edition(proof.range())?)?;
            let index = DeclarationNameIndex::new(
                &self.tables,
                scope.facts(),
                scope.scope(),
                parsed.key().profile.edition,
            );
            let found = index.lookup_prefix::<crate::frontend::names::NameNamespaces>(
                ordinal,
                crate::source::manifest::Namespace::Type,
                &name,
                || Ok(self.context.comparison()),
                None,
                Some(&self.names),
                |_| None,
            )?;
            Ok(match found {
                PrefixResolution::Name(found) => ColdNameStep::Name(found),
                PrefixResolution::NeedImport(id) | PrefixResolution::NeedDeclaration(id) => {
                    ColdNameStep::Name(crate::frontend::names::NameLookup::NeedInput(id))
                }
                PrefixResolution::ModuleTail if scope.scope().owner.is_none() => {
                    ColdNameStep::Module(crate::db::key::CurrentModuleNameKey {
                        source: scope.source().selection().outline.clone(),
                        path: scope.source().selection().path.as_str().into(),
                        bucket: crate::db::key::BucketKey {
                            scope: scope.scope().clone(),
                            namespace: crate::source::manifest::Namespace::Type,
                            name: name.as_str().into(),
                        },
                    })
                }
                PrefixResolution::ModuleTail | PrefixResolution::NeedOwnerGeneric => {
                    ColdNameStep::Pending
                }
                PrefixResolution::Parameter(_) => {
                    return Err(error("cold lexical tail cannot invent a header parameter"));
                }
            })
        })();
        match step {
            Err(error) => QueryPoll::Ready(Err(error)),
            Ok(ColdNameStep::Pending) => QueryPoll::Ready(Ok(None)),
            Ok(ColdNameStep::Name(found)) => QueryPoll::Ready(Ok(Some(found))),
            Ok(ColdNameStep::Module(key)) => {
                match db.fetch(
                    &crate::db::provider::SyntaxProvider,
                    crate::db::key::QueryKey::CurrentModuleName(Box::new(key)),
                ) {
                    QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                        crate::db::value::QueryValue::CurrentModuleName(observation) => {
                            QueryPoll::Ready(Ok(observation.lookup().cloned()))
                        }
                        _ => QueryPoll::Ready(Err(error("cold Type module query variant differs"))),
                    },
                    QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                }
            }
        }
    }
}
/// One observed input, including absence. Pure queries never prepare roots or
/// bodies, publish metadata, or mistake an unfinished prefix for module absence.
pub(crate) fn lookup(
    db: &mut Database,
    declaration: SourceTypeOwner<'_>,
    name: &ast::NameRef,
) -> QueryPoll<Option<crate::frontend::names::NameLookup>> {
    let Some(input) = db.input(&InputKey::DeclarationNames(declaration.scope.clone())) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::DeclarationNames(input) = input.as_ref() else {
        return QueryPoll::Ready(Err(error("cold declaration name input variant differs")));
    };
    input.lookup(db, declaration, name)
}

/// Canonical local declaration projected by the same ordinary name kernel.
/// This is a source fact, not a use-site or executable body grant.
pub(crate) struct ColdNamedDeclaration {
    pub(crate) definition: crate::source::manifest::DefKey,
    pub(crate) scope: SourceScope,
    pub(crate) header: crate::syntax::outline::HeaderRecipe,
    pub(crate) lookup: crate::frontend::names::NameLookup,
}
pub(crate) fn declaration(
    db: &mut Database,
    identity: Digest,
) -> QueryPoll<Option<ColdNamedDeclaration>> {
    let Some(context) = db.input(&InputKey::DeclarationContext(identity)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::DeclarationContext(context) = context.as_ref() else {
        return QueryPoll::Ready(Err(error("cold canonical context variant differs")));
    };
    let Some(input) = db.input(&InputKey::DeclarationNames(context.scope.clone())) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::DeclarationNames(input) = input.as_ref() else {
        return QueryPoll::Ready(Err(error("cold canonical input variant differs")));
    };
    QueryPoll::Ready(input.declaration(db, identity))
}
