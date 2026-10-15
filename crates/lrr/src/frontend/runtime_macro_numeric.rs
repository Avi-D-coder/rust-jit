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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::{dependency::InputValue, key::OutlineKey},
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::compile_input_catalog::CompileInputCatalog,
        ir::{op::Op, pc::Phase},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    use std::sync::Arc;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    fn world(edition: u16) -> (Database, PreparedSourceView, BodyHandle, BodyIndex) {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
        let cfg = CfgSet::guest_default();
        let path = SourcePath::new("original-comparison-numeric.rs").unwrap();
        let crate_key = Digest::of_bytes(b"original comparison numeric source");
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key,
                module_path: vec![],
                profile: profile.clone(),
                cfg: cfg.clone(),
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
            panic!("original source selection absent")
        };
        let handle = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "label_break_match")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let parsed =
            ParsedFile::parse(source.captured_macro_blob().unwrap(), profile.clone()).unwrap();
        let body = parsed
            .index_body_with_cfg(&handle, &cfg, Default::default())
            .unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"original comparison numeric world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"original comparison numeric compiler"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
        )
        .unwrap();
        let mut sealed = false;
        for _ in 0..100_000 {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => {
                    sealed = true;
                    break;
                }
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                _ => panic!("actual original discovery is incomplete"),
            }
        }
        assert!(sealed);
        discovery.publish(&mut db).unwrap();
        (db, source, handle, body)
    }
    fn publication(db: &Database, source: &PreparedSourceView) -> Box<DiscoveryInput> {
        let certificate = db
            .lexical_scope_certificate(source.selection(), &source.outline().root_scope)
            .unwrap();
        let InputValue::DiscoverySeal(publication) = certificate.as_ref() else {
            panic!()
        };
        publication.clone()
    }
    fn capture(
        db: &mut Database,
        source: &PreparedSourceView,
        handle: &BodyHandle,
        body: &BodyIndex,
        pc: &PcKey,
        publication: &DiscoveryInput,
    ) -> Result<RuntimeMacroNumericAdmission, Diagnostic> {
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .unwrap();
        let site = parsed
            .runtime_macro_numeric_request(handle, body, pc)
            .unwrap()
            .unwrap();
        match RuntimeMacroNumericAdmission::capture(db, source, publication, handle, body, pc, site)
        {
            QueryPoll::Ready(result) => result.and_then(|proof| {
                proof.ok_or_else(|| fault("expected original comparison relation absent"))
            }),
            QueryPoll::NeedInput(_) => panic!("closed original preparation unexpectedly suspended"),
        }
    }
    fn sites(
        db: &Database,
        source: &PreparedSourceView,
        handle: &BodyHandle,
        body: &BodyIndex,
    ) -> Vec<PcKey> {
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .unwrap();
        body.nodes()
            .iter()
            .filter_map(|node| {
                let pc = body.pc(node.id, Phase::Entry).unwrap();
                parsed
                    .runtime_macro_numeric_request(handle, body, &pc)
                    .unwrap()
                    .map(|_| pc)
            })
            .collect()
    }
    #[test]
    fn unchanged_compound_relations_require_real_standard_comparison_and_type_scope_all_editions() {
        // Source/recipe authority control, not complete upstream main execution.
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, handle, body) = world(edition);
            let published = publication(&db, &source);
            let keys = sites(&db, &source, &handle, &body);
            assert_eq!(keys.len(), 3);
            for pc in keys {
                let proof = capture(&mut db, &source, &handle, &body, &pc, &published).unwrap();
                let parsed = db
                    .prepared_parse(&source.selection().outline.parse)
                    .unwrap();
                assert_eq!(
                    proof.context(&parsed, &handle, &body, &pc).unwrap().1,
                    DefinitionNumericContext::Parameter(1)
                );
                let recipe = parsed
                    .initial_thunk_with_runtime_macro_numeric(&handle, &body, &pc, &proof)
                    .unwrap();
                if body.nodes()[pc.node as usize].role
                    == crate::syntax::body_index::NodeRole::Statement
                {
                    assert!(recipe.recipe().operations.iter().any(|op| matches!(
                        op,
                        Op::SetHint {
                            hint: crate::ir::op::HintRecipe::DefinitionNumeric(
                                DefinitionNumericContext::Parameter(1)
                            ),
                            ..
                        }
                    )));
                } else {
                    assert!(recipe.recipe().operations.iter().any(|op| matches!(
                        op,
                        Op::CompoundHint {
                            kind: crate::ir::op::AssignKind::DefinitionNumeric {
                                context: DefinitionNumericContext::Parameter(1),
                                ..
                            },
                            ..
                        }
                    )));
                }
                proof.revalidate(&mut db).unwrap();
            }
        }
    }
    #[test]
    fn original_comparison_relation_cannot_survive_missing_type_receipt_or_changed_epoch() {
        let (mut db, source, handle, body) = world(2024);
        let published = publication(&db, &source);
        let pc = sites(&db, &source, &handle, &body).remove(0);
        let proof = capture(&mut db, &source, &handle, &body, &pc, &published).unwrap();
        let mut missing = published.as_ref().clone();
        missing.type_scopes.clear();
        assert!(capture(&mut db, &source, &handle, &body, &pc, &missing).is_err());
        let mut missing = published.as_ref().clone();
        missing.runtime_no_item_proofs.clear();
        assert!(capture(&mut db, &source, &handle, &body, &pc, &missing).is_err());
        db.begin_expansion_epoch(2).unwrap();
        assert!(proof.revalidate(&mut db).is_err());
    }
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
    namespace: BTreeMap<InputKey, Digest>,
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
        if published.runtime_no_item_proofs.len() > 4096 {
            return Err(limit());
        }
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
        if namespace.len() > 4096 {
            return Err(limit());
        }
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
                .filter(|n| *n <= 64 * 1024 * 1024)
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
        if published.sources.len() > 256 {
            return Err(limit());
        }
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
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(limit)?;
            for (key, expected) in view.prepared_inputs() {
                let copies = key
                    .heap_bytes()
                    .checked_add(256)
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(limit)?;
                bytes = bytes
                    .checked_add(copies)
                    .filter(|n| *n <= 64 * 1024 * 1024)
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
        for (key, fingerprint) in &namespace {
            stamps.insert(key.clone(), *fingerprint);
        }
        for view in &views {
            for (key, value) in view.prepared_inputs() {
                stamps.insert(key.clone(), value.fingerprint());
            }
        }
        if stamps.len() > 4096 {
            return Err(limit());
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
            .filter(|n| *n <= 4096)
            .ok_or_else(limit)?;
        let mut bytes = count
            .checked_mul(std::mem::size_of::<DependencyStamp>())
            .ok_or_else(limit)?;
        for stamp in recipe.dependencies.iter().chain(&self.dependencies) {
            bytes = bytes
                .checked_add(stamp.heap_bytes())
                .filter(|n| *n <= 32 * 1024 * 1024)
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
