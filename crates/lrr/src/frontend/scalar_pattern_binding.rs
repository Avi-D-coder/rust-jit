//! Reached plain-pattern binding admission from current original source facts.
//! No discovery, query fetch, macro expansion, descriptor inference or static check.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::Database,
        seals::SealDomain,
    },
    frontend::{
        names::{NameLookup, NameNamespaces},
        source_locals::OrdinaryNameSite,
        source_macros::PublishedMacroIndex,
    },
    ir::pc::{NodeId, PcKey},
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::{
        body_index::BodyIndex,
        outline::BodyHandle,
        parse::{ParseKey, ParsedFile},
        spans::SourceRange,
    },
};

use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar pattern source admission budget".into(),
    )
}
fn charge(bytes: &mut usize, n: usize, cap: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= cap)
        .ok_or_else(limit)?;
    Ok(())
}
/// Caller names, graphs and Boolean absence assertions cannot construct this.
#[derive(Debug, Clone)]
pub(crate) struct ScalarPatternBindingAdmission {
    parse: ParseKey,
    body: crate::source::manifest::BodyKey,
    pattern: NodeId,
    canonical: NodeId,
    range: SourceRange,
    name: String,
    index: Digest,
    dependencies: Box<[DependencyStamp]>,
    namespace: BTreeMap<InputKey, Digest>,
    bytes: usize,
}
fn plain_pattern(
    parsed: &ParsedFile,
    handle: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
    pattern: NodeId,
) -> Result<(SourceRange, String), Diagnostic> {
    let site = parsed
        .scalar_pattern_binding_site(handle, index, pc)?
        .ok_or_else(|| fault("reached plain match binding site absent"))?;
    if site.pattern() != pattern {
        return Err(fault("plain match binding node differs"));
    }
    site.require(parsed, handle, index, pc)?;
    Ok((site.range(), site.name().to_owned()))
}

impl ScalarPatternBindingAdmission {
    /// Already selected current immutable source/index are data, not grants.
    /// This replays their published owning namespace and all retained sources.
    #[cfg(test)]
    pub(crate) fn capture_current(
        db: &mut Database,
        source: &PreparedSourceView,
        published: &PublishedMacroIndex,
        supplied_graph: &NameNamespaces,
        index: &BodyIndex,
        pc: &PcKey,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::capture_inner(
            db,
            source,
            published,
            Some(supplied_graph),
            index,
            pc,
            byte_limit,
        )
    }
    pub(crate) fn capture_query_current(
        db: &mut Database,
        source: &PreparedSourceView,
        published: &PublishedMacroIndex,
        index: &BodyIndex,
        pc: &PcKey,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::capture_inner(db, source, published, None, index, pc, byte_limit)
    }
    fn capture_inner(
        db: &mut Database,
        source: &PreparedSourceView,
        published: &PublishedMacroIndex,
        supplied_graph: Option<&NameNamespaces>,
        index: &BodyIndex,
        pc: &PcKey,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if byte_limit > 64 * 1024 * 1024 {
            return Err(limit());
        }
        let mut bytes = std::mem::size_of::<Self>();
        charge(&mut bytes, source.prepared_metadata_bytes()?, byte_limit)?;
        source.require_original_hygiene()?;
        if index.cfg().fingerprint() != source.selection().outline.cfg {
            return Err(fault("plain pattern owning cfg differs from body index"));
        }
        published.require_publication(source)?;
        if let Some(supplied_graph) = supplied_graph {
            supplied_graph.require_source(source.selection(), source.outline())?;
        }
        let item = source
            .outline()
            .items
            .iter()
            .find(|item| item.body.as_ref().is_some_and(|body| body.key() == pc.body))
            .ok_or_else(|| fault("plain pattern owning definition absent"))?;
        let handle = item
            .body
            .as_ref()
            .ok_or_else(|| fault("plain pattern owning body absent"))?;
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .ok_or_else(|| fault("plain pattern original parse lease absent"))?;
        let site = parsed
            .scalar_pattern_binding_site(handle, index, pc)?
            .ok_or_else(|| fault("reached direct pattern binding site absent"))?;
        let pattern = site.pattern();
        let canonical = site.canonical();
        let (range, name) = plain_pattern(&parsed, handle, index, pc, pattern)?;
        let namespace = db.lexical_namespace_stamps()?;
        for key in namespace.keys() {
            charge(
                &mut bytes,
                std::mem::size_of::<(InputKey, Digest)>()
                    .checked_add(128)
                    .and_then(|n| n.checked_add(key.heap_bytes()))
                    .ok_or_else(limit)?,
                byte_limit,
            )?;
        }
        let certificate = db.lexical_scope_certificate(source.selection(), &item.scope)?;
        let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
            return Err(fault("plain pattern scope certificate variant differs"));
        };
        let seal_key = input.seal.key();
        seal_key.validate()?;
        let scope = input
            .scope
            .as_ref()
            .ok_or_else(|| fault("plain pattern Value scope absent"))?;
        if input.seal.unresolved_frontiers() != 0
            || scope.scope != item.scope
            || scope.namespace != Namespace::Value
            || scope.edition != index.profile().edition
            || !input.sources.contains(source.selection())
            || !matches!(input.seal.domain(), SealDomain::Scope { id, namespace } if *id == item.scope.fingerprint() && *namespace == Namespace::Value)
        {
            return Err(fault(
                "plain pattern Value scope owner/edition/completion differs",
            ));
        }
        if input.sources.len() > 256 {
            return Err(limit());
        }
        charge(
            &mut bytes,
            input
                .sources
                .len()
                .checked_mul(std::mem::size_of::<PreparedSourceView>())
                .ok_or_else(limit)?,
            byte_limit,
        )?;
        for selected in &input.sources {
            charge(&mut bytes, selected.metadata_bytes()?, byte_limit)?;
        }
        let mut deps = BTreeMap::new();
        let mut read = |db: &mut Database,
                        key: InputKey,
                        expected: Option<Digest>|
         -> Result<std::sync::Arc<InputValue>, Diagnostic> {
            let value = db
                .input(&key)
                .ok_or_else(|| fault("plain pattern current source/namespace input absent"))?;
            let fingerprint = value.fingerprint();
            if expected.is_some_and(|expected| fingerprint != expected) {
                return Err(fault(
                    "plain pattern current source/namespace input changed",
                ));
            }
            if !deps.contains_key(&key) {
                charge(
                    &mut bytes,
                    std::mem::size_of::<DependencyStamp>()
                        .checked_add(128)
                        .and_then(|n| n.checked_add(key.heap_bytes()))
                        .ok_or_else(limit)?,
                    byte_limit,
                )?;
                if deps.len() >= 100_000 {
                    return Err(limit());
                }
            }
            deps.insert(key, fingerprint);
            Ok(value)
        };
        read(
            db,
            InputKey::DiscoverySeal(seal_key.clone()),
            Some(certificate.fingerprint()),
        )?;
        if !matches!(read(db, InputKey::ExpansionEpoch, None)?.as_ref(), InputValue::ExpansionEpoch(epoch) if *epoch == seal_key.epoch)
        {
            return Err(fault("plain pattern expansion epoch differs"));
        }
        if !matches!(read(db, InputKey::SourceDependencies, None)?.as_ref(), InputValue::SourceDependencies(graph) if graph.fingerprint() == input.source_dependencies)
        {
            return Err(fault("plain pattern dependency source world differs"));
        }
        for (key, fingerprint) in &input.compiler_inputs {
            read(db, key.clone(), Some(*fingerprint))?;
        }
        for descriptor in &input.records {
            let value = read(
                db,
                InputKey::ExpansionRecord(descriptor.key.fingerprint()),
                None,
            )?;
            if !matches!(value.as_ref(), InputValue::ExpansionRecord(record) if record.descriptor().fingerprint() == descriptor.fingerprint())
            {
                return Err(fault("plain pattern expansion record changed"));
            }
        }
        let mut views = Vec::new();
        let mut locations = BTreeSet::new();
        for selected in &input.sources {
            if views.len() >= 256 || !locations.insert(selected.location_identity()) {
                return Err(limit());
            }
            let view = db.prepared_source(selected)?.ok_or_else(|| {
                fault("plain pattern retained original source missing or changed")
            })?;
            view.require_original_hygiene()?;
            for (key, value) in view.prepared_inputs() {
                read(db, key.clone(), Some(value.fingerprint()))?;
            }
            views.push(view);
        }
        // All current captured source membership must be represented. A module
        // subset cannot prove that an unknown imported source has no candidate.
        for key in namespace
            .keys()
            .filter(|key| matches!(key, InputKey::CapturedSource(_)))
        {
            let value = read(db, key.clone(), None)?;
            if !matches!(value.as_ref(), InputValue::CapturedSource(selected) if input.sources.contains(selected))
            {
                return Err(fault(
                    "plain pattern retained source universe is incomplete",
                ));
            }
        }
        drop(read);
        // Reserve conservative bounded AST/graph/local-index work before these
        // pure helpers allocate. The retained packet is charged separately.
        for view in &views {
            charge(
                &mut bytes,
                view.captured_macro_blob()?
                    .bytes()
                    .len()
                    .checked_mul(128)
                    .ok_or_else(limit)?,
                byte_limit,
            )?;
        }
        charge(&mut bytes, 32 * 1024 * 1024, byte_limit)?;
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
        let _ = graph.resolve_imports()?;
        if graph
            .certified_scope(&item.scope, Namespace::Value, index.profile().edition)?
            .fingerprint()
            != scope.fingerprint()
            || supplied_graph
                .map(|graph| {
                    graph.certified_scope(&item.scope, Namespace::Value, index.profile().edition)
                })
                .transpose()?
                .is_some_and(|actual| actual.fingerprint() != scope.fingerprint())
        {
            return Err(fault(
                "plain pattern current original namespace replay differs",
            ));
        }
        let mut entered = published.enter_body(source.selection(), &handle.key())?;
        for (_, proof) in input
            .runtime_no_item_proofs
            .iter()
            .filter(|(_, proof)| proof.body() == &handle.key())
        {
            let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(&source.outline().profile, proof.definition())?;
            entered.accept_no_item_proof(source.selection(), &handle.key(), proof, &catalog)?;
        }
        // At the pattern's start its own binding is not yet visible. Ambiguous
        // earlier candidates and unknown item producers remain genuine NeedInput.
        let binding = entered
            .facts()
            .bindings
            .iter()
            .find(|binding| binding.name_range == site.name_range() && binding.name == name)
            .ok_or_else(|| fault("plain pattern exact lexical binding candidate absent"))?;
        if binding.visible_from <= u64::from(range.start) {
            return Err(fault("plain pattern binding is prematurely visible"));
        }
        let locals = entered.local_index_with_pattern_index(
            source.selection(),
            handle,
            &BTreeMap::new(),
            &[],
            index,
        )?;
        let found = locals.lookup_ordinary(
            OrdinaryNameSite {
                source: source.selection(),
                body: &handle.key(),
                at: u64::from(range.start),
                name: &name,
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )?;
        if found != NameLookup::Absent {
            return Err(fault(
                "plain pattern has a local/module/imported Value candidate or unresolved producer",
            ));
        }
        let mut preludes = input
            .value_binding_preludes
            .iter()
            .filter(|proof| proof.source() == source.selection());
        let prelude = preludes
            .next()
            .ok_or_else(|| fault("plain pattern actual compiler Value prelude receipt absent"))?;
        if preludes.next().is_some() {
            return Err(fault(
                "plain pattern compiler Value prelude receipt ambiguous",
            ));
        }
        prelude.require_absent(source, &item.scope, &name)?;
        charge(
            &mut bytes,
            name.capacity()
                .checked_add(parsed.key().profile.semantics.capacity())
                .ok_or_else(limit)?,
            byte_limit,
        )?;
        let dependencies = deps
            .into_iter()
            .map(|(key, fingerprint)| DependencyStamp::Input { key, fingerprint })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            parse: parsed.key(),
            body: pc.body.clone(),
            pattern,
            canonical,
            range,
            name,
            index: index.fingerprint(),
            dependencies,
            namespace,
            bytes,
        })
    }
    pub(crate) fn require(
        &self,
        parsed: &ParsedFile,
        handle: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
        pattern: NodeId,
    ) -> Result<(), Diagnostic> {
        let (range, name) = plain_pattern(parsed, handle, index, pc, pattern)?;
        if parsed.key() != self.parse
            || pc.body != self.body
            || pattern != self.pattern
            || range != self.range
            || name != self.name
            || index.fingerprint() != self.index
            || parsed
                .scalar_pattern_binding_site(handle, index, pc)?
                .is_none_or(|site| site.canonical() != self.canonical_binding())
        {
            return Err(fault(
                "plain pattern binding certificate belongs to another source/body/node",
            ));
        }
        Ok(())
    }
    /// Required before cached certificate reuse; membership equality also detects
    /// newly published source/namespace facts, not only changed existing rows.
    pub(crate) fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        if db.lexical_namespace_stamps()? != self.namespace {
            return Err(fault("plain pattern namespace membership changed"));
        }
        for dependency in &self.dependencies {
            let DependencyStamp::Input { key, fingerprint } = dependency else {
                return Err(fault("plain pattern unexpected query dependency"));
            };
            if db
                .input(key)
                .is_none_or(|value| value.fingerprint() != *fingerprint)
            {
                return Err(fault("plain pattern source dependency changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn dependencies(&self) -> &[DependencyStamp] {
        &self.dependencies
    }
    pub(crate) fn canonical_binding(&self) -> NodeId {
        self.canonical
    }
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::compile_input_catalog::CompileInputCatalog,
        ir::pc::Phase,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::body_index::NodeRole,
    };
    use std::sync::Arc;
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    fn world(
        text: &str,
        function: &str,
        edition: u16,
    ) -> (Database, PreparedSourceView, BodyHandle, BodyIndex) {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let unit = Digest::of_bytes(b"actual scalar IdentPat source admission");
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("binding-source.rs").unwrap();
        let cfg = CfgSet::guest_default();
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
                profile: profile.clone(),
                cfg: cfg.clone(),
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
            panic!("actual original source absent")
        };
        let handle = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == function)
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
            Digest::of_bytes(b"real scalar binding source world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"scalar binding actual compile environment"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
        )
        .unwrap();
        let mut complete = false;
        for _ in 0..100_000 {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => {
                    complete = true;
                    break;
                }
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => panic!("actual source preparation incomplete: {diagnostic}; {blocked:?}"),
                _ => panic!("actual source preparation requires another authorized producer"),
            }
        }
        assert!(complete);
        discovery.publish(&mut db).unwrap();
        (db, source, handle, body)
    }
    fn sites(
        source: &PreparedSourceView,
        handle: &BodyHandle,
        body: &BodyIndex,
        name: &str,
    ) -> Vec<PcKey> {
        let parsed = ParsedFile::parse(
            source.captured_macro_blob().unwrap(),
            source.outline().profile.clone(),
        )
        .unwrap();
        body.nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Pattern)
            .filter_map(|node| {
                let key = body.pc(node.id, Phase::Entry).unwrap();
                plain_pattern(&parsed, handle, body, &key, node.id)
                    .ok()
                    .filter(|(_, actual)| actual == name)
                    .map(|_| key)
            })
            .collect()
    }
    fn capture(
        db: &mut Database,
        source: &PreparedSourceView,
        body: &BodyIndex,
        key: &PcKey,
        cap: usize,
    ) -> Result<ScalarPatternBindingAdmission, Diagnostic> {
        let scope = &source
            .outline()
            .items
            .iter()
            .find(|item| {
                item.body
                    .as_ref()
                    .is_some_and(|handle| handle.key() == key.body)
            })
            .unwrap()
            .scope;
        let published = db.lexical_scope_certificate(source.selection(), scope)?;
        let InputValue::DiscoverySeal(input) = published.as_ref() else {
            panic!("actual published Value seal differs")
        };
        let macros = source.replay_published_macro_index(input)?;
        let mut graph = NameNamespaces::from_outlines(&[source.outline()])?;
        let _ = graph.resolve_imports()?;
        ScalarPatternBindingAdmission::capture_current(db, source, &macros, &graph, body, key, cap)
    }
    #[test]
    fn unchanged_guarded_v_bindings_use_actual_source_compiler_and_value_seals_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, handle, body) = world(ORIGINAL, "label_break_match", edition);
            let keys = sites(&source, &handle, &body, "v");
            assert_eq!(keys.len(), 2);
            let parsed = db
                .prepared_parse(&source.selection().outline.parse)
                .unwrap();
            for key in keys {
                let revision = db.revision();
                let proof = capture(&mut db, &source, &body, &key, 64 * 1024 * 1024).unwrap();
                proof
                    .require(&parsed, &handle, &body, &key, NodeId(key.node))
                    .unwrap();
                let mut commit = key.clone();
                commit.phase = Phase::Commit.code().unwrap();
                proof
                    .require(&parsed, &handle, &body, &commit, NodeId(key.node))
                    .unwrap();
                proof.revalidate(&mut db).unwrap();
                assert_eq!(db.revision(), revision);
                assert!(proof.allocated_bytes() <= 64 * 1024 * 1024);
                assert!(proof.dependencies().iter().any(|stamp| matches!(
                    stamp,
                    DependencyStamp::Input {
                        key: InputKey::DiscoverySeal(_),
                        ..
                    }
                )));
                assert!(proof.dependencies().iter().any(|stamp| matches!(
                    stamp,
                    DependencyStamp::Input {
                        key: InputKey::ExpansionEpoch,
                        ..
                    }
                )));
            }
        }
    }
    #[test]
    fn real_module_local_and_imported_constants_do_not_become_plain_bindings() {
        for text in [
            "const v:u8=1;fn selected(c:u8){match c{v=>(),_=>()}}fn main(){}",
            "fn selected(c:u8){const v:u8=1;match c{v=>(),_=>()}}fn main(){}",
            "mod values{pub const K:u8=1;}fn selected(c:u8){use values::K as v;match c{v=>(),_=>()}}fn main(){}",
        ] {
            let (mut db, source, handle, body) = world(text, "selected", 2024);
            let key = sites(&source, &handle, &body, "v").remove(0);
            assert_eq!(
                capture(&mut db, &source, &body, &key, 64 * 1024 * 1024)
                    .unwrap_err()
                    .code,
                "LRR-E-PATTERN-SOURCE"
            );
        }
    }
    #[test]
    fn actual_prelude_variant_and_source_owned_disabling_stay_distinct_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, handle, body) = world(
                "fn selected(c:u8){match c{None=>(),_=>()}}fn main(){}",
                "selected",
                edition,
            );
            let key = sites(&source, &handle, &body, "None").remove(0);
            assert_eq!(
                capture(&mut db, &source, &body, &key, 64 * 1024 * 1024)
                    .unwrap_err()
                    .code,
                "LRR-E-PATTERN-PRELUDE"
            );
            let (mut db, source, handle, body) = world(
                "#![no_implicit_prelude]fn selected(c:u8){match c{None=>(),_=>()}}fn main(){}",
                "selected",
                edition,
            );
            let key = sites(&source, &handle, &body, "None").remove(0);
            capture(&mut db, &source, &body, &key, 64 * 1024 * 1024).unwrap();
        }
    }
    #[test]
    fn unchanged_conditional_binding_header_uses_the_same_actual_namespace_admission() {
        const SOURCE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/rfcs/rfc-2497-if-let-chains/ast-lowering-does-not-wrap-let-chains.rs"
        );
        let (mut db, source, handle, body) = world(SOURCE, "main", 2024);
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .unwrap();
        let sites: Vec<_> = body
            .nodes()
            .iter()
            .filter(|node| node.clause_scope.is_some())
            .filter_map(|node| {
                let pc = body.pc(node.id, Phase::Entry).unwrap();
                parsed
                    .scalar_pattern_binding_site(&handle, &body, &pc)
                    .unwrap()
                    .map(|site| (pc, site))
            })
            .collect();
        assert_eq!(sites.len(), 2);
        for (pc, site) in sites {
            assert_ne!(site.pattern(), NodeId(pc.node));
            let proof = capture(&mut db, &source, &body, &pc, 64 * 1024 * 1024).unwrap();
            proof
                .require(&parsed, &handle, &body, &pc, site.pattern())
                .unwrap();
            let mut commit = pc.clone();
            commit.phase = Phase::Commit.code().unwrap();
            proof
                .require(&parsed, &handle, &body, &commit, site.pattern())
                .unwrap();
            let recipe = match db.fetch(
                &crate::db::provider::SyntaxProvider,
                crate::db::key::QueryKey::Thunk(commit),
            ) {
                crate::db::poll::QueryPoll::Ready(Ok(value)) => value,
                other => panic!("actual conditional binding query: {other:?}"),
            };
            let crate::db::value::QueryValue::Thunk(recipe) = recipe.value.as_ref() else {
                panic!("actual conditional binding recipe absent")
            };
            assert!(recipe.recipe().operations.iter().any(|operation| matches!(
                operation,
                crate::ir::op::Op::Pattern {
                    operation: crate::ir::op::PatternOperation::StageBinding { binding, .. }
                } if *binding == site.pattern()
            )));
            assert!(!recipe.recipe().dependencies.is_empty());
            proof.revalidate(&mut db).unwrap();
        }
        // This checks source/namespace/recipe authority without executing the
        // original aggregate/library program. It is not a stock runtime pass.
    }
    #[test]
    fn unchanged_nested_or_and_at_names_keep_occurrence_and_canonical_authority_distinct() {
        const SOURCE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, handle, body) = world(SOURCE, "search", edition);
            let parsed = db
                .prepared_parse(&source.selection().outline.parse)
                .unwrap();
            let mut groups: BTreeMap<String, (NodeId, usize)> = BTreeMap::new();
            let mut distinct_occurrence = false;
            for node in body
                .nodes()
                .iter()
                .filter(|node| node.role == NodeRole::Pattern)
            {
                let pc = body.pc(node.id, Phase::Entry).unwrap();
                let Some(site) = parsed
                    .scalar_pattern_binding_site(&handle, &body, &pc)
                    .unwrap()
                else {
                    continue;
                };
                let proof = capture(&mut db, &source, &body, &pc, 64 * 1024 * 1024).unwrap();
                proof
                    .require(&parsed, &handle, &body, &pc, node.id)
                    .unwrap();
                let canonical = body
                    .pattern_bindings()
                    .group_for(node.id)
                    .unwrap()
                    .canonical_candidate()
                    .unwrap();
                assert_eq!(proof.canonical_binding(), canonical);
                distinct_occurrence |= node.id != canonical;
                let entry = groups
                    .entry(site.name().to_owned())
                    .or_insert((canonical, 0));
                assert_eq!(entry.0, canonical);
                entry.1 += 1;
                let mut commit = pc.clone();
                commit.phase = Phase::Commit.code().unwrap();
                proof
                    .require(&parsed, &handle, &body, &commit, node.id)
                    .unwrap();
            }
            assert_eq!(groups.len(), 3);
            assert!(groups.values().all(|(_, occurrences)| *occurrences == 2));
            assert!(distinct_occurrence);
        }
        // Namespace proof only. Guard ordering, tuple projection and whole
        // original main execution require the shared product transaction join.
    }
    #[test]
    fn unchanged_if_let_none_does_not_acquire_binding_authority_all_editions() {
        const SOURCE: &str =
            include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/if-let.rs");
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, handle, body) = world(SOURCE, "main", edition);
            let parsed = db
                .prepared_parse(&source.selection().outline.parse)
                .unwrap();
            let sites: Vec<_> = body
                .nodes()
                .iter()
                .filter(|node| node.clause_scope.is_some())
                .filter_map(|node| {
                    let pc = body.pc(node.id, Phase::Entry).unwrap();
                    parsed
                        .scalar_pattern_binding_site(&handle, &body, &pc)
                        .unwrap()
                        .filter(|site| site.name() == "None")
                        .map(|site| (pc, site))
                })
                .collect();
            assert_eq!(sites.len(), 1);
            for (pc, site) in sites {
                assert_ne!(site.pattern(), NodeId(pc.node));
                for phase in [Phase::Entry, Phase::Commit] {
                    let pc = body.pc(NodeId(pc.node), phase).unwrap();
                    assert_eq!(
                        capture(&mut db, &source, &body, &pc, 64 * 1024 * 1024)
                            .unwrap_err()
                            .code,
                        "LRR-E-PATTERN-PRELUDE"
                    );
                    let error = match db.fetch(
                        &crate::db::provider::SyntaxProvider,
                        crate::db::key::QueryKey::Thunk(pc),
                    ) {
                        crate::db::poll::QueryPoll::Ready(Err(error)) => error,
                        crate::db::poll::QueryPoll::Ready(Ok(value)) => {
                            let crate::db::value::QueryValue::Error(error) = value.value.as_ref()
                            else {
                                panic!("conditional prelude name acquired a binding recipe")
                            };
                            error.clone()
                        }
                        other => panic!("actual conditional prelude query: {other:?}"),
                    };
                    assert_eq!(error.code, "LRR-E-PATTERN-PRELUDE");
                }
            }
        }
        // Safe refusal is not correct execution of None's constant/variant
        // pattern contract. That common selection/execution remains required.
    }
    #[test]
    fn source_certificate_refuses_wrong_phase_budget_and_current_epoch() {
        let (mut db, source, handle, body) = world(
            "fn selected(c:u8){match c{v if v==0=>(),_=>()}}fn main(){}",
            "selected",
            2024,
        );
        let key = sites(&source, &handle, &body, "v").remove(0);
        assert_eq!(
            capture(&mut db, &source, &body, &key, 1).unwrap_err().code,
            "LRR-E-METADATA-LIMIT"
        );
        let proof = capture(&mut db, &source, &body, &key, 64 * 1024 * 1024).unwrap();
        let parsed = db
            .prepared_parse(&source.selection().outline.parse)
            .unwrap();
        let mut wrong = key.clone();
        wrong.phase = Phase::Exit.code().unwrap();
        assert_eq!(
            proof
                .require(&parsed, &handle, &body, &wrong, NodeId(key.node))
                .unwrap_err()
                .code,
            "LRR-E-PATTERN-SOURCE"
        );
        db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            proof.revalidate(&mut db).unwrap_err().code,
            "LRR-E-PATTERN-SOURCE"
        );
        assert!(capture(&mut db, &source, &body, &key, 64 * 1024 * 1024).is_err());
    }
    #[test]
    fn partial_compiler_macro_catalogue_cannot_mint_a_value_prelude_facet() {
        let profile = ProfileIdentity::current();
        let partial =
            CompileInputCatalog::new(crate::facade::unit(&profile, "core"), profile.edition, [])
                .unwrap();
        assert_eq!(
            partial.value_binding_facet(&profile).unwrap_err().code,
            "LRR-E-COMPILE-INPUT"
        );
        assert_ne!(
            CompileInputCatalog::standalone(&profile)
                .unwrap()
                .value_binding_facet(&profile)
                .unwrap(),
            Digest::of_bytes(b"printed v")
        );
    }
}
