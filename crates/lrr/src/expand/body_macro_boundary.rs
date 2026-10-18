//! Discovery-issued owning macro boundaries. Scope roots are retained from the
//! actual selector; no diagnostic origin or independently reconstructed resolver.
use super::generated_items::GeneratedItems;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::seals::SealKey,
    frontend::macro_boundary::RetainedMacroScope,
    source::changes::PreparedSourceView,
    syntax::outline::{DeclarationContext, ScopeKey as SourceScope},
};
use std::{collections::BTreeMap, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}

pub(super) struct RetainedBoundaryParents {
    pub(super) scopes: Vec<RetainedMacroScope>,
    pub(super) sources: Vec<PreparedSourceView>,
    pub(super) items: Vec<Arc<GeneratedItems>>,
}
impl RetainedBoundaryParents {
    pub(super) fn new() -> Result<Self, Diagnostic> {
        let mut result = Self {
            scopes: vec![],
            sources: vec![],
            items: vec![],
        };
        result
            .scopes
            .try_reserve_exact(256)
            .map_err(|_| crate::runtime::arena::resource())?;
        result
            .sources
            .try_reserve_exact(256)
            .map_err(|_| crate::runtime::arena::resource())?;
        result
            .items
            .try_reserve_exact(256)
            .map_err(|_| crate::runtime::arena::resource())?;
        Ok(result)
    }
    pub(super) fn require_bound(&self) -> Result<(), Diagnostic> {
        let mut shared = 0usize;
        for root in &self.scopes {
            shared = shared
                .checked_add(root.shared_metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        for source in &self.sources {
            shared = shared
                .checked_add(source.prepared_metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        for item in &self.items {
            shared = shared
                .checked_add(item.metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        // These already-admitted roots can alias. The query owner applies its
        // configured cap after deduplicating actual retained allocations; this
        // conservative sum must not become a new origin-specific admission cap.
        let _ = shared;
        Ok(())
    }
}

/// Evidence that the actual completed item reply required no module macro
/// catalogue. Created only alongside complete_without_macro_bindings, after the
/// common header plan's producer/import/call/attribute facts were checked.
pub(super) struct CompletedEmptyBoundary {
    pub(super) record: Arc<super::record::ExpansionRecord>,
    pub(super) source_ordinal: usize,
    pub(super) call: crate::syntax::outline::ItemInvocation,
}

pub(crate) struct GeneratedBodyBoundary {
    output: Arc<GeneratedItems>,
    scope: SourceScope,
    selected: Option<RetainedMacroScope>,
    declarations:
        Arc<BTreeMap<Digest, crate::frontend::generated_macros::CapturedDeclarationScope>>,
    expansion_definitions: BTreeMap<Digest, crate::expand::scopes::MacroDefinition>,
    parents: RetainedBoundaryParents,
    seal: SealKey,
    seal_value: Digest,
    fingerprint: Digest,
    exclusive_bytes: usize,
}
impl std::fmt::Debug for GeneratedBodyBoundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneratedBodyBoundary")
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}
impl GeneratedBodyBoundary {
    pub(super) fn prepare(
        output: Arc<GeneratedItems>,
        scope: SourceScope,
        selected: Option<RetainedMacroScope>,
        declarations: Arc<
            BTreeMap<Digest, crate::frontend::generated_macros::CapturedDeclarationScope>,
        >,
        parents: RetainedBoundaryParents,
        expansion_definitions: BTreeMap<Digest, crate::expand::scopes::MacroDefinition>,
        seal: SealKey,
        seal_value: Digest,
    ) -> Result<Arc<Self>, Diagnostic> {
        parents.require_bound()?;
        if output.epoch() != seal.epoch
            || parents.sources.is_empty()
            || (selected.is_none() && !declarations.is_empty())
        {
            return Err(fault("generated macro boundary owner/epoch differs"));
        }
        let records: BTreeMap<_, _> = std::iter::once(&output)
            .chain(parents.items.iter())
            .map(|items| (items.record().key().fingerprint(), items.record()))
            .collect();
        for (key, definition) in &expansion_definitions {
            let record = records
                .get(key)
                .ok_or_else(|| fault("expansion definition escapes owning record closure"))?;
            definition.validate()?;
            if record.key().definition_edition != definition.definition_edition
                || matches!(record.key().macro_identity,
                    crate::expand::identity::MacroIdentity::Declaration { definition: actual }
                    | crate::expand::identity::MacroIdentity::Builtin { definition: actual }
                    | crate::expand::identity::MacroIdentity::BuiltinDerive { definition: actual }
                    if actual != definition.identity)
            {
                return Err(fault(
                    "owning expansion definition differs from output record",
                ));
            }
        }
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(scope.heap_bytes())
            .and_then(|n| {
                n.checked_add(
                    parents
                        .scopes
                        .capacity()
                        .checked_mul(std::mem::size_of::<RetainedMacroScope>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    parents
                        .sources
                        .capacity()
                        .checked_mul(std::mem::size_of::<PreparedSourceView>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    parents
                        .items
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<GeneratedItems>>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        bytes = bytes
            .checked_add(256)
            .ok_or_else(crate::runtime::arena::resource)?;
        // PreparedSourceView shares the publication but owns its selection and
        // location keys. Account conservative storage for both shallow keys.
        for source in &parents.sources {
            bytes = bytes
                .checked_add(
                    source
                        .selection()
                        .metadata_bytes()?
                        .checked_mul(2)
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        let mut e = StableEncoder::new("lrr/v3/generated-owning-macro-boundary");
        e.record(&output.record().descriptor());
        e.record(&scope);
        e.record(&seal);
        e.digest(seal_value);
        e.bool(selected.is_some());
        e.u64(u64::try_from(parents.scopes.len()).map_err(|_| crate::runtime::arena::resource())?);
        for parent in &parents.scopes {
            let outer = parent.outer();
            e.record(&outer.scope);
            e.u64(outer.before);
        }
        e.u64(u64::try_from(parents.sources.len()).map_err(|_| crate::runtime::arena::resource())?);
        for source in &parents.sources {
            e.string(source.selection().path.as_str());
            e.record(&source.selection().outline);
        }
        e.u64(u64::try_from(parents.items.len()).map_err(|_| crate::runtime::arena::resource())?);
        for item in &parents.items {
            e.record(&item.record().descriptor());
        }
        e.u64(u64::try_from(declarations.len()).map_err(|_| crate::runtime::arena::resource())?);
        for (definition, declaration) in declarations.iter() {
            e.digest(*definition);
            e.record(&declaration.context);
            e.record(&declaration.lexical);
        }
        e.u64(
            u64::try_from(expansion_definitions.len())
                .map_err(|_| crate::runtime::arena::resource())?,
        );
        for (record, definition) in &expansion_definitions {
            e.digest(*record);
            e.digest(definition.identity);
            e.digest(definition.defining_unit);
            e.u16(definition.definition_edition);
        }
        Ok(Arc::new(Self {
            output,
            scope,
            selected,
            declarations,
            expansion_definitions,
            parents,
            seal,
            seal_value,
            fingerprint: e.finish(),
            exclusive_bytes: bytes,
        }))
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn seal(&self) -> &SealKey {
        &self.seal
    }
    pub(crate) fn seal_value(&self) -> Digest {
        self.seal_value
    }
    pub(crate) fn output(&self) -> &Arc<GeneratedItems> {
        &self.output
    }
    pub(crate) fn scopes(&self) -> &[RetainedMacroScope] {
        &self.parents.scopes
    }
    pub(crate) fn selected_root(&self) -> Option<&RetainedMacroScope> {
        self.selected.as_ref()
    }
    pub(crate) fn source_roots(&self) -> &[PreparedSourceView] {
        &self.parents.sources
    }
    pub(crate) fn expansion_definitions(
        &self,
    ) -> &BTreeMap<Digest, crate::expand::scopes::MacroDefinition> {
        &self.expansion_definitions
    }
    pub(crate) fn generated_roots(&self) -> &[Arc<GeneratedItems>] {
        &self.parents.items
    }
    pub(crate) fn declaration_scope_roots(
        &self,
    ) -> &Arc<BTreeMap<Digest, crate::frontend::generated_macros::CapturedDeclarationScope>> {
        &self.declarations
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    /// Conservative checked size of separately retained allocation roots. A
    /// memo owner deduplicates the actual Arcs; this is not exclusive storage.
    pub(crate) fn shared_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self.output.metadata_bytes()?;
        for scope in self.selected.iter().chain(self.parents.scopes.iter()) {
            bytes = bytes
                .checked_add(scope.shared_metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        for source in &self.parents.sources {
            bytes = bytes
                .checked_add(source.prepared_metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        for item in &self.parents.items {
            bytes = bytes
                .checked_add(item.metadata_bytes()?)
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        bytes = bytes
            .checked_add(std::mem::size_of_val(self.declarations.as_ref()))
            .ok_or_else(crate::runtime::arena::resource)?;
        for declaration in self.declarations.values() {
            bytes = bytes
                .checked_add(
                    128 + std::mem::size_of::<
                        crate::frontend::generated_macros::CapturedDeclarationScope,
                    >(),
                )
                .and_then(|n| n.checked_add(declaration.context.scope.heap_bytes()))
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        Ok(bytes)
    }
    /// Cold admission/query dependency check. Installed execution must use the
    /// owning query's retained versions rather than call source preparation.
    pub(crate) fn require_current(
        &self,
        db: &mut crate::db::memo::Database,
        unit: &super::generated_syntax::GeneratedSyntaxInput,
    ) -> crate::db::poll::QueryPoll<()> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        };
        let check = || -> Result<(), Diagnostic> {
            // Type and Value discovery can publish the same generated source
            // record under different completion domains. Like same_unit and
            // retained generated admission, source correspondence is tied to
            // its graph/epoch, scope and actual record. The boundary's own
            // full discovery seal is still validated below.
            if self.seal.unit_graph != unit.seal.unit_graph
                || self.seal.epoch != unit.seal.epoch
                || self.scope != unit.scope
                || !self.output.record().same_record(unit.items.record())
            {
                return Err(fault("generated owning boundary source unit differs"));
            }
            Ok(())
        };
        if let Err(error) = check() {
            return QueryPoll::Ready(Err(error));
        }
        if !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.seal.epoch)
        {
            return QueryPoll::Ready(Err(fault("generated owning boundary epoch changed")));
        }
        if !matches!(db.input(&InputKey::GeneratedSyntax(unit.key.content)).as_deref(),Some(InputValue::GeneratedSyntax(current)) if current.same_unit(unit))
        {
            return QueryPoll::Ready(Err(fault("generated owning boundary input changed")));
        }
        for source in &self.parents.sources {
            match db.prepared_source(source.selection()) {
                Ok(Some(current)) if source.same_publication(&current) => {}
                Ok(_) => {
                    return QueryPoll::Ready(Err(fault(
                        "owning boundary source publication changed",
                    )));
                }
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        }
        for item in std::iter::once(&self.output).chain(self.parents.items.iter()) {
            let record = item.record();
            if !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(current)) if current.same_record(record))
            {
                return QueryPoll::Ready(Err(fault("owning boundary expansion record changed")));
            }
        }
        match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(self.seal.clone())) {
            QueryPoll::Ready(Ok(value))
                if value.fingerprint == self.seal_value
                    && matches!(value.value.as_ref(),QueryValue::DiscoverySeal(seal) if seal.key()==self.seal) =>
            {
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Err(fault("owning boundary discovery proof changed")))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(crate) fn declaration_outer(
        &self,
        definition: Digest,
        context: &DeclarationContext,
    ) -> Result<Option<RetainedMacroScope>, Diagnostic> {
        if context.scope.crate_key != self.scope.crate_key
            || !context
                .scope
                .module_path
                .starts_with(&self.scope.module_path)
        {
            return Err(fault("generated owning macro boundary declaration differs"));
        }
        match &self.selected {
            Some(root) => {
                let declaration = self
                    .declarations
                    .get(&definition)
                    .ok_or_else(|| fault("generated declaration macro scope absent"))?;
                if &declaration.context != context {
                    return Err(fault("generated declaration macro context differs"));
                }
                root.at(declaration.lexical, u64::from(context.range.start))
                    .map(Some)
            }
            // The completed header proof covers every module macro/import/
            // producer in this actual output. Inline modules without any such
            // declarations add no macro candidates; body-local facts remain a
            // separate demand and are never assumed empty here.
            None => {
                if !self.output.items().iter().any(|item| {
                    item.key.fingerprint() == definition
                        && item.start == context.range.start
                        && item.end == context.range.end
                }) {
                    return Err(fault("generated owning macro boundary declaration differs"));
                }
                Ok(None)
            }
        }
    }
}
