//! Read-only preparation evidence. This does not run queries or infer output/native
//! observations; incomplete traces, sources or origin mappings fail collection.
use super::{
    fixture::{FixtureManifest, FixtureObservation, MarkerObservation},
    run::RunMachine,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{key::QueryKey, memo::Database, trace::Action, value::QueryValue},
    source::{changes::PreparedSourceView, manifest::BodyKey},
    syntax::{
        fixture_markers::{FixtureMarkers, MarkerTarget},
        spans::{BodySpan, SourceRange},
    },
};
use std::collections::BTreeMap;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-FIXTURE-OBSERVATION", message.into())
}
struct BodyLocation {
    key: BodyKey,
    range: SourceRange,
    target: bool,
}
pub struct FixturePreparationObserver {
    revision: u64,
    query_revision: u64,
    entry: BodyKey,
    trace_start: usize,
    bodies: BTreeMap<Digest, BodyLocation>,
    wanted_bodies: BTreeMap<Digest, String>,
    markers: BTreeMap<(u32, u32), Vec<String>>,
    sources: Vec<PreparedSourceView>,
}
fn increment(value: &mut u64, amount: u64) -> Result<(), Diagnostic> {
    *value = value
        .checked_add(amount)
        .ok_or_else(|| fault("fixture observation counter overflow"))?;
    Ok(())
}
impl FixturePreparationObserver {
    pub fn start(
        db: &Database,
        sources: &[PreparedSourceView],
        target: &PreparedSourceView,
        markers: &FixtureMarkers,
        contract: &FixtureManifest,
    ) -> Result<Self, Diagnostic> {
        contract.validate()?;
        if !db.trace.detailed || db.trace.truncated != 0 {
            return Err(fault("fixture requires complete detailed query tracing"));
        }
        markers.require_source(&target.selection().outline.parse)?;
        if sources.len() > 10_000 {
            return Err(fault("fixture source count limit"));
        }
        let mut bodies = BTreeMap::new();
        let mut named: BTreeMap<String, Vec<Digest>> = BTreeMap::new();
        let mut target_present = false;
        let mut retained_sources = Vec::new();
        retained_sources
            .try_reserve_exact(sources.len())
            .map_err(|_| fault("fixture source atlas allocation"))?;
        let mut bytes = retained_sources
            .capacity()
            .checked_mul(std::mem::size_of::<PreparedSourceView>())
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("fixture source atlas metadata limit"))?;
        for source in sources {
            let selection = source.selection();
            bytes = selection
                .outline
                .module_path
                .iter()
                .try_fold(bytes, |bytes, component| {
                    bytes
                        .checked_add(component.len())
                        .and_then(|bytes| bytes.checked_add(32))
                })
                .and_then(|bytes| bytes.checked_add(selection.path.as_str().len()))
                .and_then(|bytes| {
                    bytes.checked_add(selection.outline.parse.profile.semantics.len())
                })
                .filter(|bytes| *bytes <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("fixture source selection metadata limit"))?;
            retained_sources.push(source.share());
            let selected = source.selection() == target.selection();
            target_present |= selected;
            for item in &source.outline().items {
                let Some(body) = &item.body else { continue };
                let key = body.key();
                let identity = key.fingerprint();
                bytes = bytes
                    .checked_add(512)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| fault("fixture body atlas metadata limit"))?;
                if bodies
                    .insert(
                        identity,
                        BodyLocation {
                            key,
                            range: body.range,
                            target: selected,
                        },
                    )
                    .is_some()
                {
                    return Err(fault("duplicate fixture body source identity"));
                }
                if selected && item.scope.owner.is_none() {
                    let name = item
                        .scope
                        .module_path
                        .iter()
                        .chain(std::iter::once(&item.def_key.declared_name))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("::");
                    bytes = bytes
                        .checked_add(
                            name.len()
                                .checked_add(128)
                                .ok_or_else(|| fault("fixture name size overflow"))?,
                        )
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| fault("fixture name atlas metadata limit"))?;
                    named.entry(name).or_default().push(identity);
                }
            }
        }
        if !target_present {
            return Err(fault(
                "fixture marker source absent from supplied source atlas",
            ));
        }
        let entry_identity = named
            .get(&contract.entry)
            .filter(|entries| entries.len() == 1)
            .ok_or_else(|| fault("fixture entry body absent or ambiguous"))?[0];
        let entry = bodies
            .get(&entry_identity)
            .ok_or_else(|| fault("fixture entry identity absent"))?
            .key
            .clone();
        let mut wanted_bodies = BTreeMap::new();
        for name in &contract.laziness.no_body_preparation {
            let candidates = named
                .get(name)
                .filter(|v| v.len() == 1)
                .ok_or_else(|| fault("fixture body name absent or ambiguous"))?;
            wanted_bodies.insert(candidates[0], name.clone());
        }
        let mut target_ranges: BTreeMap<u32, u32> = BTreeMap::new();
        for body in bodies.values().filter(|b| b.target) {
            target_ranges
                .entry(body.range.start)
                .and_modify(|end| *end = (*end).max(body.range.end))
                .or_insert(body.range.end);
        }
        let mut prefix_end = 0;
        for end in target_ranges.values_mut() {
            prefix_end = prefix_end.max(*end);
            *end = prefix_end;
        }
        let mut ranges: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        for name in &contract.laziness.no_marker_preparation {
            let marker = markers
                .get(name)
                .ok_or_else(|| fault("requested fixture marker is absent"))?;
            let MarkerTarget::Expression(r) = marker.target else {
                return Err(fault("fixture marker still needs macro expansion"));
            };
            if !target_ranges
                .range(..=r.start)
                .next_back()
                .is_some_and(|(_, end)| *end >= r.end)
            {
                return Err(fault("fixture marker lacks an owned body attachment"));
            }
            ranges
                .entry((r.start, r.end))
                .or_default()
                .push(name.clone());
        }
        Ok(Self {
            revision: db.source_revision(),
            query_revision: db.revision(),
            entry,
            trace_start: db.trace.events.len(),
            bodies,
            wanted_bodies,
            markers: ranges,
            sources: retained_sources,
        })
    }
    fn absolute(&self, span: &BodySpan) -> Result<Option<SourceRange>, Diagnostic> {
        let location = self
            .bodies
            .get(&span.body_key.fingerprint())
            .filter(|b| b.key == span.body_key)
            .ok_or_else(|| fault("fixture operation body is absent or changed"))?;
        if span.relative_start > span.relative_end || span.relative_end > location.range.len() {
            return Err(fault("fixture operation span escapes its owned body"));
        }
        if !location.target {
            return Ok(None);
        }
        let start = location
            .range
            .start
            .checked_add(span.relative_start)
            .ok_or_else(|| fault("fixture span offset overflow"))?;
        let end = location
            .range
            .start
            .checked_add(span.relative_end)
            .ok_or_else(|| fault("fixture span offset overflow"))?;
        Ok(Some(SourceRange { start, end }))
    }
    pub fn collect(
        &self,
        db: &Database,
        run: &RunMachine,
    ) -> Result<FixtureObservation, Diagnostic> {
        self.collect_with_declarative_expressions(db, run, std::iter::empty())
    }
    /// Explicit exact prepared literal witnesses permit their retained ancestry.
    /// Unproved expanded recipes retain collect's ordinary blanket refusal.
    pub fn collect_with_declarative_expressions<'p>(
        &self,
        db: &Database,
        run: &RunMachine,
        expressions: impl IntoIterator<
            Item = &'p crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression,
        >,
    ) -> Result<FixtureObservation, Diagnostic> {
        let mut expressions_by_pc = BTreeMap::new();
        let mut expression_bytes = 0usize;
        for expression in expressions {
            expression_bytes = expression_bytes
                .checked_add(expression.metadata_bytes())
                .and_then(|bytes| bytes.checked_add(256))
                .filter(|bytes| *bytes <= 64 * 1024 * 1024)
                .ok_or_else(|| fault("fixture expression proof metadata limit"))?;
            if expressions_by_pc.len() >= 4096
                || expressions_by_pc
                    .insert(expression.recipe().recipe().key.clone(), expression)
                    .is_some()
            {
                return Err(fault("fixture expression proofs duplicate or excessive"));
            }
        }
        if run.root_body() != &self.entry || run.query_trace_start() < self.trace_start {
            return Err(fault(
                "fixture execution entry or observation start differs",
            ));
        }
        if db.source_revision() != self.revision
            || run.installed_recipes().source_revision() != self.revision
        {
            return Err(fault("fixture observation publication is stale"));
        }
        if !db.trace.detailed || db.trace.truncated != 0 || db.trace.events.len() < self.trace_start
        {
            return Err(fault("fixture observation trace is incomplete"));
        }
        let mut result = FixtureObservation::default();
        for name in self.wanted_bodies.values() {
            result.body_preparations.insert(name.clone(), 0);
        }
        for names in self.markers.values() {
            for name in names {
                result.markers.insert(
                    name.clone(),
                    MarkerObservation {
                        thunk_executions: Some(0),
                        installed_operations: Some(0),
                        native_operations: None,
                    },
                );
            }
        }
        let current_query_revision = db.revision();
        if current_query_revision < self.query_revision {
            return Err(fault("fixture query event publication is stale"));
        }
        let mut prior_query_revision = self.query_revision;
        for event in &db.trace.events[self.trace_start..] {
            // Derived publication can advance the query revision while a
            // suspended run retains the immutable source revision checked above.
            // The complete trace must remain ordered within this observation.
            if event.revision < prior_query_revision || event.revision > current_query_revision {
                return Err(fault("fixture query event publication is stale"));
            }
            prior_query_revision = event.revision;
            if let QueryKey::BodyStructure(body) = &event.key
                && matches!(
                    event.action,
                    Action::ProviderRun | Action::MemoHit | Action::ValidatedReuse
                )
                && let Some(name) = self.wanted_bodies.get(&body.fingerprint())
            {
                increment(
                    result
                        .body_preparations
                        .get_mut(name)
                        .expect("registered body observation"),
                    1,
                )?;
            }
            if event.action != Action::ProviderRun {
                continue;
            }
            let QueryKey::Thunk(pc) = &event.key else {
                continue;
            };
            let QueryValue::BodyStructure(index) = db
                .memo(&QueryKey::BodyStructure(pc.body.clone()))
                .map(|m| m.value.as_ref())
                .ok_or_else(|| fault("fixture thunk body mapping was evicted"))?
            else {
                return Err(fault("fixture thunk body mapping has no structural value"));
            };
            if index.body() != &pc.body {
                return Err(fault("fixture thunk body mapping identity differs"));
            }
            let span = index.span(crate::ir::pc::NodeId(pc.node))?;
            if let Some(r) = self.absolute(&span)?
                && let Some(names) = self.markers.get(&(r.start, r.end))
            {
                for name in names {
                    increment(
                        result
                            .markers
                            .get_mut(name)
                            .expect("registered marker")
                            .thunk_executions
                            .as_mut()
                            .expect("measured channel"),
                        1,
                    )?;
                }
            }
        }
        for installed in run.installed_recipes().recipes() {
            let recipe = installed.recipe();
            if !recipe.origin.expansion_ancestry.is_empty() {
                let proof = expressions_by_pc.remove(&recipe.key).ok_or_else(|| {
                    fault("fixture expanded operation origin needs source mapping integration")
                })?;
                if proof.recipe().recipe().fingerprint() != recipe.fingerprint() {
                    return Err(fault(
                        "fixture installed expanded recipe differs from literal proof",
                    ));
                }
                let QueryValue::BodyStructure(body) = db
                    .memo(&QueryKey::BodyStructure(recipe.key.body.clone()))
                    .map(|memo| memo.value.as_ref())
                    .ok_or_else(|| fault("fixture literal body mapping was evicted"))?
                else {
                    return Err(fault("fixture literal body mapping is not structural"));
                };
                observe_declarative_literal(db, &self.sources, body, proof)?;
            }
            if let Some(r) = self.absolute(&recipe.origin.span)?
                && let Some(names) = self.markers.get(&(r.start, r.end))
            {
                let count = u64::try_from(recipe.operations.len())
                    .map_err(|_| fault("fixture operation count overflow"))?;
                for name in names {
                    increment(
                        result
                            .markers
                            .get_mut(name)
                            .expect("registered marker")
                            .installed_operations
                            .as_mut()
                            .expect("measured channel"),
                        count,
                    )?;
                }
            }
        }
        if !expressions_by_pc.is_empty() {
            return Err(fault(
                "fixture literal proof has no installed expanded recipe",
            ));
        }
        if let Some(facts) = run.terminal_facts() {
            use super::{fixture::FixtureOutcome, run::TerminalExecution};
            match &facts.outcome {
                TerminalExecution::Completed { unit } => {
                    result.outcome = Some(FixtureOutcome::Completed);
                    result.status = Some(if *unit { "passed" } else { "failed" }.into());
                }
                TerminalExecution::GuestPanic { message } => {
                    result.outcome = Some(FixtureOutcome::GuestPanic);
                    result.status = Some("panicked".into());
                    result.panic_message = Some(message.clone());
                }
                TerminalExecution::Fault { code } => {
                    result.outcome = Some(FixtureOutcome::Fault);
                    result.status = Some("fault".into());
                    result.diagnostic_code = Some((*code).into());
                }
            }
            result.cleanup_ran = facts.cleanup_ran;
        }
        Ok(result)
    }
}

fn current_source_blob<'a>(
    db: &'a Database,
    source: &PreparedSourceView,
) -> Result<&'a crate::source::snapshot::SourceBlob, Diagnostic> {
    use crate::db::dependency::{InputKey, InputValue};
    let selection = source.selection();
    let Some((InputValue::CapturedSource(current), _)) =
        db.published_input(&InputKey::CapturedSource(selection.location_identity()))
    else {
        return Err(fault("expression origin captured source absent"));
    };
    if current != selection {
        return Err(fault("expression origin captured source changed"));
    }
    let Some((InputValue::SourceBlob(blob), _)) =
        db.published_input(&InputKey::SourceBlob(selection.outline.parse.content))
    else {
        return Err(fault("expression origin captured bytes absent"));
    };
    if blob.content() != selection.outline.parse.content {
        return Err(fault("expression origin captured bytes changed"));
    }
    Ok(blob)
}
fn source_identity(source: &PreparedSourceView) -> crate::syntax::token_wire::SourceIdentity {
    let selection = source.selection();
    crate::syntax::token_wire::SourceIdentity {
        unit: crate::syntax::token_wire::Key(selection.outline.crate_key.0),
        content: crate::syntax::token_wire::Key(selection.outline.parse.content.0),
        path: selection.path.as_str().into(),
    }
}
/// Validate the privately prepared literal against current immutable publication,
/// then retain exact caller/definition/token facts for diagnostic or fixture use.
/// This performs no queries, source capture, pin mutation or guest operation.
pub fn observe_declarative_literal(
    db: &Database,
    sources: &[PreparedSourceView],
    body: &crate::syntax::body_index::BodyIndex,
    plan: &crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression,
) -> Result<crate::syntax::origin::GeneratedOperationOrigin, Diagnostic> {
    use crate::{
        db::dependency::{DependencyStamp, InputKey, InputValue},
        expand::{
            identity::{GrammarSlot, MacroIdentity},
            source_declarative::{SourceDeclarative, group},
        },
        ir::{pc::Phase, recipe::RecipeDraft, validate::verify_declarative_literal},
        source::manifest::ItemKind,
        syntax::{
            body_index::{ExpressionKind, NodeRole},
            origin::{DefinitionOrigin, GeneratedOperationOrigin, SourceOrigin},
        },
    };
    if sources.len() > 10_000 {
        return Err(fault("expression origin source count limit"));
    }
    let recipe = plan.recipe().recipe();
    if body.body() != &recipe.key.body
        || recipe.key.phase != Phase::Entry.code()?
        || recipe.origin.expansion_ancestry.len() != 1
    {
        return Err(fault(
            "expression origin caller body/phase/ancestry differs",
        ));
    }
    let node = body
        .nodes()
        .get(recipe.key.node as usize)
        .ok_or_else(|| fault("expression origin node absent"))?;
    if node.role != NodeRole::Expression(ExpressionKind::Macro)
        || recipe.origin.span != body.span(node.id)?
    {
        return Err(fault(
            "expression origin requires its exact caller macro node",
        ));
    }
    let mut callers = sources.iter().filter_map(|source| {
        source
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|handle| handle.key() == recipe.key.body)
            .map(|handle| (source, handle))
    });
    let (caller, handle) = callers
        .next()
        .ok_or_else(|| fault("expression origin caller source absent"))?;
    if callers.next().is_some() {
        return Err(fault("expression origin caller source ambiguous"));
    }
    current_source_blob(db, caller)?;
    let range = SourceRange {
        start: handle
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| fault("expression origin start overflow"))?,
        end: handle
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| fault("expression origin end overflow"))?,
    };
    let expected_source_keys = [
        InputKey::CapturedSource(caller.selection().location_identity()),
        InputKey::DefinitionSource(recipe.key.body.definition),
        InputKey::DefinitionBody(recipe.key.body.definition),
    ];
    if recipe.dependencies.len() != 4 {
        return Err(fault("expression origin literal dependency set differs"));
    }
    let mut record_input = None;
    for dependency in recipe.dependencies.iter() {
        let DependencyStamp::Input { key, fingerprint } = dependency else {
            return Err(fault(
                "expression origin literal dependency is not captured input",
            ));
        };
        let (input, current) = db
            .published_input(key)
            .ok_or_else(|| fault("expression origin dependency absent"))?;
        if current != *fingerprint {
            return Err(fault("expression origin dependency changed"));
        }
        match input {
            InputValue::ExpansionRecord(record)
                if *key == InputKey::ExpansionRecord(record.key().fingerprint())
                    && record_input.is_none() =>
            {
                record_input = Some((input, record))
            }
            InputValue::CapturedSource(selected)
                if expected_source_keys.contains(key) && selected == caller.selection() => {}
            InputValue::DefinitionSource(parse)
                if *key == expected_source_keys[1]
                    && parse == &caller.selection().outline.parse => {}
            InputValue::DefinitionBody(current_body)
                if *key == expected_source_keys[2] && current_body == handle => {}
            _ => return Err(fault("expression origin dependency kind or source differs")),
        }
    }
    let (input, record) =
        record_input.ok_or_else(|| fault("expression origin record dependency absent"))?;
    let MacroIdentity::Declaration { definition } = record.key().macro_identity else {
        return Err(fault("expression origin is not a declarative record"));
    };
    if record.key().grammar != GrammarSlot::Expression
        || record.key().call_edition != caller.selection().outline.parse.profile.edition
        || record.descriptor().fingerprint() != recipe.origin.expansion_ancestry[0]
    {
        return Err(fault(
            "expression origin current record grammar/edition/ancestry differs",
        ));
    }
    let inventory = caller.declaration_inventory(Default::default())?;
    let producer = inventory
        .producers()
        .iter()
        .find(|producer| producer.range == range && producer.grammar == GrammarSlot::Expression)
        .ok_or_else(|| fault("expression origin caller producer absent"))?;
    let original = SourceDeclarative::new(caller.share())?;
    let call_input = group(
        &original.owned_forest()?,
        caller.macro_token_group(range, false)?,
    )?;
    if record.key().invocation != producer.identity
        || record.key().input_tokens
            != Digest::of_bytes(&call_input.encode().map_err(|error| fault(&error.message))?)
        || record.key().parent_context
            != Digest(
                crate::expand::source_hygiene::source_root(
                    caller.selection().outline.crate_key,
                    record.key().call_edition,
                )?
                .context
                .0,
            )
    {
        return Err(fault(
            "expression origin caller invocation/input/context differs",
        ));
    }
    let decoded = crate::syntax::generated_expression::literal(
        input,
        recipe.origin.span.clone(),
        &recipe.key,
    )?;
    let checked = verify_declarative_literal(
        RecipeDraft {
            key: recipe.key.clone(),
            origin: recipe.origin.clone(),
            operations: recipe.operations.to_vec(),
            exits: vec![recipe.exit.clone()],
            dependencies: recipe.dependencies.to_vec(),
        },
        body,
        &decoded,
    )?;
    if checked.recipe().fingerprint() != recipe.fingerprint() {
        return Err(fault("expression origin private literal recipe differs"));
    }
    let (tokens, origins) = decoded.into_provenance();
    if tokens.encode().map_err(|error| fault(&error.message))?
        != plan
            .tokens()
            .encode()
            .map_err(|error| fault(&error.message))?
        || origins.as_slice() != plan.origins()
    {
        return Err(fault(
            "expression origin output tokens or origin projection differ",
        ));
    }
    let mut owners = sources.iter().filter_map(|source| {
        source
            .outline()
            .items
            .iter()
            .find(|item| {
                item.def_key.item_kind == ItemKind::Macro
                    && item.def_key.fingerprint() == definition
            })
            .map(|item| (source, item))
    });
    let (owner, item) = owners
        .next()
        .ok_or_else(|| fault("expression origin captured definition absent"))?;
    if owners.next().is_some()
        || owner.selection().outline.parse.profile.edition != record.key().definition_edition
    {
        return Err(fault(
            "expression origin definition owner ambiguous or edition differs",
        ));
    }
    current_source_blob(db, owner)?;
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(definition))
    else {
        return Err(fault("expression origin definition publication absent"));
    };
    if parse != &owner.selection().outline.parse {
        return Err(fault("expression origin definition publication changed"));
    }
    let (definition_range, definition_name) = owner.macro_definition_header(definition)?;
    if definition_name != item.def_key.declared_name {
        return Err(fault("expression origin definition label differs"));
    }
    let _span_bytes = tokens
        .spans()
        .len()
        .checked_mul(96)
        .filter(|bytes| *bytes <= 64 * 1024 * 1024)
        .ok_or_else(|| fault("expression origin span lookup metadata limit"))?;
    let spans = tokens
        .spans()
        .iter()
        .map(|span| (span.key(), span))
        .collect::<BTreeMap<_, _>>();
    let mut seen = std::collections::BTreeSet::new();
    let mut selected = Vec::new();
    let mut bytes = std::mem::size_of::<GeneratedOperationOrigin>();
    for origin in origins {
        if !seen.insert(origin.span) {
            continue;
        }
        let span = spans
            .get(&origin.span)
            .ok_or_else(|| fault("expression origin output span absent"))?;
        let source = sources
            .iter()
            .find(|source| {
                let selection = source.selection();
                span.source.unit.0 == selection.outline.crate_key.0
                    && span.source.content.0 == selection.outline.parse.content.0
                    && span.source.path == selection.path.as_str()
            })
            .ok_or_else(|| fault("expression origin token captured source absent"))?;
        let blob = current_source_blob(db, source)?;
        let edition = match source.selection().outline.parse.profile.edition {
            2015 => crate::syntax::token_wire::Edition::E2015,
            2018 => crate::syntax::token_wire::Edition::E2018,
            2021 => crate::syntax::token_wire::Edition::E2021,
            2024 => crate::syntax::token_wire::Edition::E2024,
            _ => return Err(fault("expression origin source edition unsupported")),
        };
        if span.start > span.end
            || usize::try_from(span.end)
                .map_err(|_| fault("expression origin token offset conversion"))?
                > blob.bytes().len()
            || span.edition != edition
            || !tokens
                .contexts()
                .iter()
                .any(|context| context.key() == span.context)
        {
            return Err(fault(
                "expression origin token range or owning/context edition differs",
            ));
        }
        bytes = bytes
            .checked_add(std::mem::size_of::<crate::syntax::token_wire::SpanRecord>())
            .and_then(|bytes| bytes.checked_add(span.source.path.len()))
            .filter(|bytes| *bytes <= 64 * 1024)
            .ok_or_else(|| fault("expression diagnostic origin retention limit"))?;
        selected
            .try_reserve_exact(1)
            .map_err(|_| fault("expression diagnostic token allocation"))?;
        selected.push((*span).clone());
    }
    if selected.is_empty() {
        return Err(fault(
            "expression origin has no selected output token spans",
        ));
    }
    bytes
        .checked_add(caller.selection().path.as_str().len())
        .and_then(|bytes| bytes.checked_add(owner.selection().path.as_str().len()))
        .and_then(|bytes| bytes.checked_add(definition_name.len()))
        .filter(|bytes| *bytes <= 64 * 1024)
        .ok_or_else(|| fault("expression diagnostic labels metadata limit"))?;
    let origin = GeneratedOperationOrigin {
        body: recipe.origin.span.clone(),
        expansion: record.descriptor().fingerprint(),
        invocation_identity: record.key().invocation,
        invocation: Some(SourceOrigin {
            source: source_identity(caller),
            range,
        }),
        definition: Some(DefinitionOrigin {
            identity: definition,
            name: definition_name,
            location: SourceOrigin {
                source: source_identity(owner),
                range: definition_range,
            },
        }),
        tokens: selected,
        parents: Vec::new(),
    };
    origin
        .heap_bytes()
        .filter(|bytes| *bytes <= 64 * 1024)
        .ok_or_else(|| fault("expression diagnostic origin metadata limit"))?;
    Ok(origin)
}
