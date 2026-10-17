//! Reached local arithmetic constraints from prepared compiler comparisons.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{DependencyStamp, InputKey},
        memo::Database,
        poll::QueryPoll,
        seals::DiscoveryInput,
    },
    facade::macros::RuntimeMacroKind,
    frontend::{
        call_contracts::{ArgumentHintPreparation, prepare_argument_hint},
        callables::{CallResolver, CallSelection},
        names::NameNamespaces,
        runtime_macro_catalog::RuntimeMacroCatalog,
    },
    ir::{
        op::{DefinitionNumericContext, PrimitiveType},
        pc::PcKey,
        recipe::{RecipeDraft, VerifiedRecipe},
    },
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::{MacroNumericSite, body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile},
    traits::term::TyTerm,
};
use std::collections::BTreeMap;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}

fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "runtime macro numeric relation budget".into(),
    )
}
fn compatible(primitive: PrimitiveType, floating: bool) -> bool {
    matches!(
        (primitive, floating),
        (PrimitiveType::F32 | PrimitiveType::F64, true)
            | (
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::I128
                    | PrimitiveType::Isize
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::U128
                    | PrimitiveType::Usize,
                false
            )
    )
}
pub(crate) struct RuntimeMacroNumericAdmission {
    site: MacroNumericSite,
    context: DefinitionNumericContext,
    dependencies: Vec<DependencyStamp>,
    namespace: std::sync::Arc<BTreeMap<InputKey, Digest>>,
}
impl RuntimeMacroNumericAdmission {
    pub(crate) fn capture(
        db: &mut Database,
        source: &PreparedSourceView,
        published: &DiscoveryInput,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
        site: MacroNumericSite,
    ) -> QueryPoll<Option<Self>> {
        match Self::capture_inner(db, source, published, body, index, pc, site) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    fn capture_inner(
        db: &mut Database,
        source: &PreparedSourceView,
        published: &DiscoveryInput,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
        site: MacroNumericSite,
    ) -> Result<QueryPoll<Option<Self>>, Diagnostic> {
        source.require_original_hygiene()?;
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .ok_or_else(|| fault("numeric relation original parser lease absent"))?;
        site.require(&parsed, body, index, pc)?;
        let owner = source
            .outline()
            .items
            .iter()
            .find(|item| item.body.as_ref() == Some(body))
            .ok_or_else(|| fault("numeric relation owning function absent"))?;
        // Collect only genuine already-resolved compiler comparison relations.
        // Without one, an initializer keeps the existing unresolved literal
        // route; no completed namespace or primitive source absence is claimed.
        let mut ordinals = Vec::new();
        for (_, proof) in &published.runtime_no_item_proofs {
            if proof.body() != &pc.body || proof.source() != &parsed.key() {
                continue;
            }
            let catalogue =
                RuntimeMacroCatalog::standard_for_definition(index.profile(), proof.definition())?;
            let Some(kind) = catalogue.select(proof.definition())? else {
                continue;
            };
            if !matches!(
                kind,
                RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe
            ) {
                continue;
            }
            let invocation = parsed.macro_call_at(proof.invocation())?;
            let arguments = parsed.runtime_macro_arguments(&invocation, kind)?;
            if let Some(ordinal) =
                site.comparison_parameter(&parsed, body, index, &invocation, &arguments)?
                && !ordinals.contains(&ordinal)
            {
                ordinals.try_reserve(1).map_err(|_| limit())?;
                ordinals.push(ordinal);
            }
        }
        if ordinals.is_empty() {
            return if site.is_initializer() {
                Ok(QueryPoll::Ready(Ok(None)))
            } else {
                Err(fault(
                    "compound assignment has no admitted compiler comparison numeric relation",
                ))
            };
        }
        let namespace = db.lexical_namespace_stamps()?;
        // Account the retained map plus bounded Type receipt/graph replay work.
        let mut bytes = 32usize * 1024 * 1024;
        for key in namespace.keys() {
            // Retained membership, receipt replay scratch, and dependency-key
            // copies can coexist. Preflight them before the later allocations.
            let row = key
                .heap_bytes()
                .checked_add(256)
                .and_then(|n| n.checked_mul(3))
                .ok_or_else(limit)?;
            bytes = bytes
                .checked_add(row)
                .ok_or_else(limit)?;
            if db
                .input(key)
                .is_none_or(|value| namespace.get(key) != Some(&value.fingerprint()))
            {
                return Err(fault("numeric relation namespace publication changed"));
            }
        }
        let mut receipts = published.type_scopes.iter().filter(|receipt| {
            receipt.scope().scope == owner.scope
                && receipt.scope().namespace == Namespace::Type
                && receipt.scope().edition == index.profile().edition
        });
        let receipt = match receipts.next() {
            Some(receipt) => receipt,
            None if site.is_initializer() => return Ok(QueryPoll::Ready(Ok(None))),
            None => {
                return Err(fault(
                    "numeric relation current Type namespace receipt absent",
                ));
            }
        };
        if receipts.next().is_some() {
            return Err(fault(
                "numeric relation current Type namespace is ambiguous",
            ));
        }
        receipt.require_owner(&owner.scope, index.profile())?;
        receipt.require_current(db, published)?;
        let mut views = Vec::new();
        views
            .try_reserve_exact(published.sources.len())
            .map_err(|_| limit())?;
        for selection in &published.sources {
            let view = db
                .prepared_source(selection)?
                .ok_or_else(|| fault("numeric relation captured source lease absent"))?;
            view.require_original_hygiene()?;
            // Retained source catalogues and namespace reconstruction paths are
            // byte bounded before graph construction, not just source counted.
            let graph_work = view
                .outline()
                .estimated_bytes()
                .checked_mul(8)
                .ok_or_else(limit)?;
            bytes = bytes
                .checked_add(view.prepared_metadata_bytes()?)
                .and_then(|n| n.checked_add(graph_work))
                .ok_or_else(limit)?;
            for (key, expected) in view.prepared_inputs() {
                let copies = key
                    .heap_bytes()
                    .checked_add(256)
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(limit)?;
                bytes = bytes
                    .checked_add(copies)
                    .ok_or_else(limit)?;
                if db
                    .input(key)
                    .is_none_or(|value| value.fingerprint() != expected.fingerprint())
                {
                    return Err(fault("numeric relation captured source input changed"));
                }
            }
            views.push(view);
        }
        let borrowed = views.iter().collect::<Vec<_>>();
        let mut graph = NameNamespaces::from_outlines(
            &views.iter().map(|view| view.outline()).collect::<Vec<_>>(),
        )?;
        let dependencies = source.source_dependencies()?;
        for view in &views {
            for (alias, unit) in dependencies
                .aliases_for(
                    view.selection().outline.crate_key,
                    view.outline().profile.edition,
                )?
                .iter()
            {
                graph.dependency(
                    view.selection().outline.crate_key,
                    alias.as_str(),
                    *unit,
                    view.outline().profile.edition,
                )?;
            }
        }
        let imports = graph.resolve_imports()?;
        if !imports.pending.is_empty()
            || !imports.missing.is_empty()
            || !imports.inaccessible.is_empty()
            || !imports.ambiguous.is_empty()
            || graph
                .certified_scope(&owner.scope, Namespace::Type, index.profile().edition)?
                .fingerprint()
                != receipt.scope().fingerprint()
        {
            return Err(fault(
                "numeric relation complete Type namespace replay differs",
            ));
        }
        let resolver = CallResolver::new(&graph, &borrowed)?;
        let callee =
            match resolver.module_function(db, pc.body.definition, &owner.def_key.declared_name) {
                QueryPoll::Ready(Ok(CallSelection::Source(callee))) => callee,
                QueryPoll::Ready(Ok(_)) => {
                    return Err(fault("numeric relation owning callable remains unresolved"));
                }
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            };
        if callee.signature().definition != pc.body.definition {
            return Err(fault("numeric relation selected another callable"));
        }
        let mut selected: Option<(PrimitiveType, u32)> = None;
        for ordinal in ordinals {
            let hints =
                match prepare_argument_hint(db, &callee, &graph, &borrowed, ordinal as usize) {
                    QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) => hints,
                    QueryPoll::Ready(Ok(_)) => continue,
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                };
            let [hint] = hints.as_slice() else {
                return Err(fault("numeric relation parameter hint arity differs"));
            };
            let TyTerm::Primitive(primitive) = &hint.source else {
                continue;
            };
            if !compatible(*primitive, site.floating()) {
                continue;
            }
            selected = match selected {
                Some((previous, previous_ordinal)) if previous == *primitive => {
                    Some((previous, previous_ordinal.min(ordinal)))
                }
                Some(_) => return Err(fault("numeric comparison parameter source types disagree")),
                None => Some((*primitive, ordinal)),
            };
        }
        let (_, ordinal) = match selected {
            Some(selected) => selected,
            None if site.is_initializer() => return Ok(QueryPoll::Ready(Ok(None))),
            None => {
                return Err(fault(
                    "compound assignment has no admitted compiler comparison numeric relation",
                ));
            }
        };
        let context = DefinitionNumericContext::Parameter(ordinal);
        let mut stamps = BTreeMap::new();
        for (key, fingerprint) in namespace.iter() {
            stamps.insert(key.clone(), *fingerprint);
        }
        for view in &views {
            for (key, value) in view.prepared_inputs() {
                stamps.insert(key.clone(), value.fingerprint());
            }
        }
        let dependencies = stamps
            .into_iter()
            .map(|(key, fingerprint)| DependencyStamp::Input { key, fingerprint })
            .collect();
        let proof = Self {
            site,
            context,
            dependencies,
            namespace,
        };
        proof.revalidate(db)?;
        Ok(QueryPoll::Ready(Ok(Some(proof))))
    }
    pub(crate) fn context(
        &self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<(crate::ir::pc::NodeId, DefinitionNumericContext), Diagnostic> {
        self.site.require(parsed, body, index, pc)?;
        Ok((self.site.binding(), self.context))
    }
    pub(crate) fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        if db.lexical_namespace_stamps()? != self.namespace {
            return Err(fault("numeric relation namespace membership changed"));
        }
        for stamp in &self.dependencies {
            let DependencyStamp::Input { key, fingerprint } = stamp else {
                return Err(fault("numeric relation unexpected query dependency"));
            };
            if db
                .input(key)
                .is_none_or(|value| value.fingerprint() != *fingerprint)
            {
                return Err(fault("numeric relation source input changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn attach(
        &self,
        verified: VerifiedRecipe,
        index: &BodyIndex,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let recipe = verified.recipe();
        let count = recipe
            .dependencies
            .len()
            .checked_add(self.dependencies.len())
            .ok_or_else(limit)?;
        let mut bytes = count
            .checked_mul(std::mem::size_of::<DependencyStamp>())
            .ok_or_else(limit)?;
        for stamp in recipe.dependencies.iter().chain(&self.dependencies) {
            bytes = bytes
                .checked_add(stamp.heap_bytes())
                .ok_or_else(limit)?;
        }
        let mut dependencies = Vec::new();
        dependencies.try_reserve_exact(count).map_err(|_| limit())?;
        dependencies.extend(recipe.dependencies.iter().cloned());
        dependencies.extend(self.dependencies.iter().cloned());
        dependencies.sort_unstable();
        dependencies.dedup();
        crate::ir::validate::verify(
            RecipeDraft {
                key: recipe.key.clone(),
                origin: recipe.origin.clone(),
                operations: recipe.operations.to_vec(),
                exits: vec![recipe.exit.clone()],
                dependencies,
            },
            index,
        )
    }
}
