//! Bounded eager compiler worklist. Output admission and all external effects remain explicit.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{ExpansionRequest, InputRequest},
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        compile_input_identity::CompileInputKind,
        concat_arguments::{ConcatArguments, ConcatExpansion, ConcatExpansionPoll},
        environment_arguments::{EnvironmentExpansion, EnvironmentExpansionPoll},
        environment_plan::{AdoptedEnvironmentExpansionPoll, EnvironmentArgumentPlan},
        identity::{ExpansionKey, MacroIdentity},
        include_arguments::{IncludeArguments, NestedStringInvocation},
        include_string::{IncludeDataExpansion, IncludeDataPoll},
        record::ExpansionRecord,
        scopes::MacroDefinition,
    },
    frontend::nested_compile_inputs::{
        EagerMacroResolver, NestedExpansionContext, NestedLexicalContext, NestedMacroLookup,
    },
    source::include_paths::{IncludeAuthorities, IncludeLocation},
    syntax::token_wire::{Key, TokenForest},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
pub struct EagerLimits {
    pub depth: usize,
    pub work: usize,
    pub metadata_bytes: usize,
    pub wire_bytes: usize,
}
impl Default for EagerLimits {
    fn default() -> Self {
        Self {
            depth: usize::MAX,
            work: usize::MAX,
            metadata_bytes: usize::MAX,
            wire_bytes: usize::MAX,
        }
    }
}
#[derive(Clone, Copy)]
pub struct EagerCompilerRequest<'a> {
    pub selected: &'a MacroDefinition,
    pub key: &'a ExpansionKey,
    pub original: &'a TokenForest,
    pub site: Key,
    pub source_revision: u64,
    pub limits: EagerLimits,
}
#[derive(Clone, Copy)]
pub struct EagerSourceContext<'a> {
    pub authorities: &'a IncludeAuthorities,
    pub from: &'a IncludeLocation,
    pub included: Option<&'a crate::source::include_view::IncludedSourceView>,
}
/// Validated owned payload for an explicit handler outside this worklist.
pub struct EagerExternalCall {
    selected: MacroDefinition,
    key: ExpansionKey,
    original: TokenForest,
    site: Key,
}
impl EagerExternalCall {
    pub fn selected(&self) -> &MacroDefinition {
        &self.selected
    }
    pub fn key(&self) -> &ExpansionKey {
        &self.key
    }
    pub fn original(&self) -> &TokenForest {
        &self.original
    }
    pub fn site(&self) -> Key {
        self.site
    }
    pub fn input(&self) -> InputRequest {
        InputRequest::Expand(Box::new(ExpansionRequest {
            key: self.key.clone(),
        }))
    }
}
pub enum EagerCompileInputPoll {
    /// Computed output must pass existing coordinator admission/adoption before any parent resumes.
    Produced(ExpansionRecord),
    /// The root itself is already adopted in the frozen input universe.
    Complete(ExpansionRecord),
    NeedInput(InputRequest),
    NeedExternal(EagerExternalCall),
    NeedLookup {
        lookup: NestedMacroLookup,
        invocation: NestedStringInvocation,
    },
    NeedEnvironment {
        unit: Digest,
        name: String,
        environment: Digest,
        span: Key,
    },
    NeedOptionFacade {
        defining_unit: Digest,
        value: Option<String>,
        site: Key,
    },
}
enum Plan {
    Concat(ConcatArguments),
    Environment(EnvironmentArgumentPlan),
    IncludeData(Box<IncludeArguments>),
}
struct Frame {
    selected: MacroDefinition,
    key: ExpansionKey,
    original: TokenForest,
    site: Key,
    plan: Plan,
    selections: BTreeMap<usize, ExpansionKey>,
    returned: Option<crate::frontend::hygiene::HygieneFacts>,
    before: u64,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
impl Frame {
    fn new(
        selected: MacroDefinition,
        key: ExpansionKey,
        original: TokenForest,
        site: Key,
        resolver: &impl EagerMacroResolver,
        environment: &FrozenCompileEnvironment,
    ) -> Result<Self, Diagnostic> {
        key.validate()?;
        let descriptor = resolver
            .compiler()
            .select(&selected)?
            .ok_or_else(|| fault("eager call is not a compiler definition"))?;
        if key.macro_identity
            != (MacroIdentity::Builtin {
                definition: selected.identity,
            })
            || key.definition_edition != selected.definition_edition
            || key.consumer_compile_env != environment.fingerprint()
            || key.input_tokens
                != Digest::of_bytes(&original.encode().map_err(|e| fault(&e.to_string()))?)
        {
            return Err(fault(
                "eager call definition/input/environment/grammar mismatch",
            ));
        }
        let span = original
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("eager call site absent"))?;
        if key.parent_context != Digest(span.context.0) {
            return Err(fault("eager call parent context mismatch"));
        }
        let plan = match descriptor.kind() {
            CompileInputKind::Concat => {
                Plan::Concat(ConcatArguments::parse(descriptor, &original)?)
            }
            CompileInputKind::Env | CompileInputKind::OptionEnv => {
                Plan::Environment(EnvironmentArgumentPlan::parse(descriptor, &original)?)
            }
            CompileInputKind::Include
            | CompileInputKind::IncludeStr
            | CompileInputKind::IncludeBytes => {
                Plan::IncludeData(Box::new(IncludeArguments::parse(descriptor, &original)?))
            }
        };
        let before = u64::from(span.start);
        Ok(Self {
            selected,
            key,
            original,
            site,
            plan,
            selections: BTreeMap::new(),
            returned: None,
            before,
        })
    }
    fn nested(&self, index: usize) -> Result<&NestedStringInvocation, Diagnostic> {
        match &self.plan {
            Plan::Concat(p) => p.operands().get(index).and_then(|o| o.nested()),
            Plan::Environment(p) => p.operands().get(index).and_then(|o| o.nested()),
            Plan::IncludeData(p) if index == 0 => p.nested_invocation(),
            Plan::IncludeData(_) => None,
        }
        .ok_or_else(|| fault("eager nested operand absent"))
    }
    fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .original
            .allocated_bytes()
            .map_err(|e| fault(&e.to_string()))?
            .checked_add(crate::expand::record::key_heap_bytes(&self.key))
            .ok_or_else(|| fault("eager frame metadata overflow"))?;
        bytes = bytes
            .checked_add(match &self.plan {
                Plan::Concat(p) => p.allocated_bytes()?,
                Plan::Environment(p) => p.allocated_bytes()?,
                Plan::IncludeData(p) => p.allocated_bytes()?,
            })
            .ok_or_else(|| fault("eager frame metadata overflow"))?;
        if let Some(facts) = &self.returned {
            bytes = bytes
                .checked_add(facts.metadata_bytes()?)
                .ok_or_else(|| fault("returned hygiene metadata overflow"))?;
        }
        for key in self.selections.values() {
            // Conservative node allowance in addition to key-owned heap storage.
            bytes = bytes
                .checked_add(2 * std::mem::size_of::<(usize, ExpansionKey)>())
                .and_then(|n| n.checked_add(crate::expand::record::key_heap_bytes(key)))
                .ok_or_else(|| fault("eager selection metadata overflow"))?;
        }
        Ok(bytes)
    }
}
enum Step {
    Output(ExpansionRecord),
    Select(usize),
    Selected(Box<PendingString>),
    Input(InputRequest),
    Environment(EnvironmentExpansionPoll),
    External,
}
struct EagerWorkBudget {
    remaining: usize,
    retained: usize,
}
impl EagerWorkBudget {
    fn spend(&mut self) -> Result<(), Diagnostic> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| fault("eager expansion work limit"))?;
        Ok(())
    }
}
struct PendingString {
    invocation: NestedStringInvocation,
    selection: crate::frontend::nested_compile_inputs::NestedExpansionSelection,
    facts: Option<crate::frontend::hygiene::HygieneFacts>,
    before: u64,
}
enum ResolvedInclude {
    Ready(Box<IncludeArguments>),
    Pending(Box<PendingString>),
}
/// Replay only immutable adopted outputs. A missing producer returns explicit work.
fn resolve_include_chain(
    db: &mut Database,
    frame: &Frame,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
    limits: EagerLimits,
    budget: &mut EagerWorkBudget,
) -> Result<ResolvedInclude, Diagnostic> {
    let descriptor = resolver
        .compiler()
        .select(&frame.selected)?
        .ok_or_else(|| fault("include chain definition absent"))?;
    let mut arguments = IncludeArguments::parse(descriptor, &frame.original)?;
    let mut facts = frame.returned.clone();
    for _ in 0..limits.depth {
        let Some(nested) = arguments.nested_invocation() else {
            return Ok(ResolvedInclude::Ready(Box::new(arguments)));
        };
        budget.spend()?;
        let context = NestedExpansionContext {
            environment,
            engine_version: frame.key.engine_version,
        };
        let selection = match &facts {
            Some(facts) => resolver.resolve_returned_expansion(
                nested,
                caller,
                &context,
                facts,
                frame.before,
            )?,
            None => resolver.resolve_expansion(nested, caller, &context)?,
        };
        let Some(key) = selection.key() else {
            return Ok(ResolvedInclude::Pending(Box::new(PendingString {
                invocation: nested.clone(),
                selection,
                facts,
                before: frame.before,
            })));
        };
        let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint())) else {
            return Ok(ResolvedInclude::Pending(Box::new(PendingString {
                invocation: nested.clone(),
                selection,
                facts,
                before: frame.before,
            })));
        };
        let InputValue::ExpansionRecord(record) = input.as_ref() else {
            return Err(fault("include chain adopted variant mismatch"));
        };
        if record.key() != key {
            return Err(fault("include chain adopted key mismatch"));
        }
        let next = crate::expand::source_hygiene::output_facts(&record.tokens()?)?;
        match &mut facts {
            Some(facts) => facts.extend(&next)?,
            None => facts = Some(next),
        }
        arguments = match arguments.resume_adopted(db, descriptor, key) {
            crate::db::poll::QueryPoll::Ready(result) => result?,
            crate::db::poll::QueryPoll::NeedInput(_) => {
                return Err(fault("adopted include producer disappeared"));
            }
        };
        arguments
            .allocated_bytes()?
            .checked_add(budget.retained)
            .and_then(|n| n.checked_add(facts.as_ref().map_or(Ok(0), |f| f.metadata_bytes()).ok()?))
            .filter(|n| *n <= limits.metadata_bytes)
            .ok_or_else(|| fault("include chain metadata limit"))?;
    }
    Err(fault("include operand expansion depth limit"))
}
enum ResolvedOperands {
    Ready(BTreeMap<usize, crate::expand::string_chain::StringExpansionChain>),
    Pending(Box<PendingString>),
}
fn resolve_frame_operands(
    db: &mut Database,
    frame: &Frame,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
    limits: EagerLimits,
    budget: &mut EagerWorkBudget,
) -> Result<ResolvedOperands, Diagnostic> {
    use crate::expand::string_chain::{StringExpansionChain, StringOutput, classify_output};
    let count = match &frame.plan {
        Plan::Concat(plan) => plan.operands().len(),
        Plan::Environment(plan) => plan.operands().len(),
        Plan::IncludeData(_) => return Ok(ResolvedOperands::Ready(BTreeMap::new())),
    };
    let mut chains = BTreeMap::new();
    let mut bytes = budget.retained;
    for index in 0..count {
        let nested = match &frame.plan {
            Plan::Concat(plan) => plan.operands()[index].nested(),
            Plan::Environment(plan) => plan.operands()[index].nested(),
            Plan::IncludeData(_) => None,
        };
        let Some(nested) = nested else {
            continue;
        };
        let mut nested = nested.clone();
        let mut facts = frame.returned.clone();
        let mut chain = StringExpansionChain::new();
        for depth in 0..limits.depth {
            budget.spend()?;
            let context = NestedExpansionContext {
                environment,
                engine_version: frame.key.engine_version,
            };
            let selection = match &facts {
                Some(facts) => resolver.resolve_returned_expansion(
                    &nested,
                    caller,
                    &context,
                    facts,
                    frame.before,
                )?,
                None => resolver.resolve_expansion(&nested, caller, &context)?,
            };
            let Some(key) = selection.key() else {
                return Ok(ResolvedOperands::Pending(Box::new(PendingString {
                    invocation: nested,
                    selection,
                    facts,
                    before: frame.before,
                })));
            };
            let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint())) else {
                return Ok(ResolvedOperands::Pending(Box::new(PendingString {
                    invocation: nested,
                    selection,
                    facts,
                    before: frame.before,
                })));
            };
            let InputValue::ExpansionRecord(record) = input.as_ref() else {
                return Err(fault("string operand adopted variant mismatch"));
            };
            if record.key() != key {
                return Err(fault("string operand adopted key mismatch"));
            }
            chain.push(key.clone(), record.output())?;
            let tokens = record.tokens()?;
            let output = classify_output(key, &tokens)?;
            let next = crate::expand::source_hygiene::output_facts(&tokens)?;
            match &mut facts {
                Some(facts) => facts.extend(&next)?,
                None => facts = Some(next),
            }
            bytes
                .checked_add(chain.allocated_bytes()?)
                .and_then(|n| n.checked_add(nested.allocated_bytes().ok()?))
                .and_then(|n| n.checked_add(tokens.allocated_bytes().ok()?))
                .and_then(|n| {
                    n.checked_add(facts.as_ref().map_or(Ok(0), |f| f.metadata_bytes()).ok()?)
                })
                .filter(|n| *n <= limits.metadata_bytes)
                .ok_or_else(|| fault("recursive string operand metadata limit"))?;
            match output {
                StringOutput::Literal => {
                    bytes = bytes
                        .checked_add(chain.allocated_bytes()?)
                        .and_then(|n| n.checked_add(128))
                        .filter(|n| *n <= limits.metadata_bytes)
                        .ok_or_else(|| fault("recursive operand aggregate metadata limit"))?;
                    chains.insert(index, chain);
                    break;
                }
                StringOutput::Nested(next) => {
                    if depth + 1 == limits.depth {
                        return Err(fault("recursive string operand depth limit"));
                    }
                    nested = *next;
                }
            }
        }
    }
    Ok(ResolvedOperands::Ready(chains))
}
/// Stateless across adoption boundaries: rebuild the bounded worklist against current immutable inputs.
/// Never publishes output, executes a user/procedural macro, or reads ambient environment/files.
pub fn poll_eager_compile_input(
    db: &mut Database,
    request: EagerCompilerRequest<'_>,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
) -> Result<EagerCompileInputPoll, Diagnostic> {
    poll_worklist(db, request, resolver, caller, environment, None)
}
/// Include data requires a separately supplied immutable source authority and caller location.
pub fn poll_eager_compile_input_with_sources(
    db: &mut Database,
    request: EagerCompilerRequest<'_>,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
    sources: EagerSourceContext<'_>,
) -> Result<EagerCompileInputPoll, Diagnostic> {
    poll_worklist(db, request, resolver, caller, environment, Some(sources))
}
fn poll_worklist(
    db: &mut Database,
    request: EagerCompilerRequest<'_>,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
    sources: Option<EagerSourceContext<'_>>,
) -> Result<EagerCompileInputPoll, Diagnostic> {
    let limits = request.limits;
    if limits.depth == 0 || limits.work == 0 || limits.metadata_bytes == 0 || limits.wire_bytes == 0
    {
        return Err(fault("eager worklist limits invalid"));
    }
    if request.source_revision != db.source_revision()
        || caller.scope.crate_key != environment.unit()
        || request.key.call_edition != caller.source.profile.edition
    {
        return Err(fault("eager frozen caller/source revision mismatch"));
    }
    caller.source.profile.validate_source()?;
    if request
        .original
        .allocated_bytes()
        .map_err(|e| fault(&e.to_string()))?
        > limits.metadata_bytes
    {
        return Err(fault("eager root metadata limit"));
    }
    let site = request
        .original
        .spans()
        .iter()
        .find(|s| s.key() == request.site)
        .ok_or_else(|| fault("eager root site absent"))?;
    let site_edition = match site.edition {
        crate::syntax::token_wire::Edition::E2015 => 2015,
        crate::syntax::token_wire::Edition::E2018 => 2018,
        crate::syntax::token_wire::Edition::E2021 => 2021,
        crate::syntax::token_wire::Edition::E2024 => 2024,
    };
    if site.source.unit.0 != caller.scope.crate_key.0
        || site.source.content.0 != caller.source.content.0
        || site_edition != caller.source.profile.edition
    {
        return Err(fault("eager root source context mismatch"));
    }
    if let Some(sources) = sources
        && (sources.from.unit != environment.unit()
            || sources.authorities.unit() != environment.unit()
            || sources.authorities.root(sources.from.root).is_none())
    {
        return Err(fault("eager source authority caller location mismatch"));
    }
    if let Some(sources) = sources {
        resolver.validate_include_location(site, sources.from, sources.included)?;
    }
    if let Some(sources) = sources
        && let Some(view) = sources.included
    {
        if !view.matches_authorities(sources.authorities) {
            return Err(fault("eager included source view mismatch"));
        }
        match view.validate(db) {
            crate::db::poll::QueryPoll::Ready(result) => result?,
            crate::db::poll::QueryPoll::NeedInput(input) => {
                return Ok(EagerCompileInputPoll::NeedInput(input));
            }
        }
    }
    let root = Frame::new(
        request.selected.clone(),
        request.key.clone(),
        request.original.clone(),
        request.site,
        resolver,
        environment,
    )?;
    let mut frames = Vec::new();
    frames.push(root);
    let mut budget = EagerWorkBudget {
        remaining: limits.work,
        retained: 0,
    };
    for _ in 0..limits.work {
        budget.spend()?;
        let mut bytes = frames
            .capacity()
            .checked_mul(std::mem::size_of::<Frame>())
            .ok_or_else(|| fault("eager stack metadata overflow"))?;
        for frame in &frames {
            bytes = bytes
                .checked_add(frame.allocated_bytes()?)
                .filter(|n| *n <= limits.metadata_bytes)
                .ok_or_else(|| fault("eager worklist metadata limit"))?;
        }
        let frame = frames
            .last_mut()
            .ok_or_else(|| fault("eager worklist is empty"))?;
        if let Some(input) = db.input(&InputKey::ExpansionRecord(frame.key.fingerprint())) {
            let InputValue::ExpansionRecord(record) = input.as_ref() else {
                return Err(fault("eager adopted input variant mismatch"));
            };
            if record.key() != &frame.key {
                return Err(fault("eager adopted output key mismatch"));
            }
            if frames.len() == 1 {
                return Ok(EagerCompileInputPoll::Complete(record.clone()));
            }
            frames.pop();
            continue;
        }
        budget.retained = bytes;
        let operands = resolve_frame_operands(
            db,
            frame,
            resolver,
            caller,
            environment,
            limits,
            &mut budget,
        )?;
        let step = match operands {
            ResolvedOperands::Pending(pending) => Step::Selected(pending),
            ResolvedOperands::Ready(chains) => {
                for (index, chain) in &chains {
                    frame.selections.insert(*index, chain.root()?.clone());
                }
                match &frame.plan {
                    Plan::Concat(plan) => match plan.expand_resolved(
                        db,
                        ConcatExpansion {
                            catalog: resolver.compiler(),
                            selected: &frame.selected,
                            key: &frame.key,
                            original: &frame.original,
                            site: frame.site,
                            selections: &frame.selections,
                            max_input_bytes: usize::MAX,
                            max_output_bytes: usize::MAX,
                            max_wire_bytes: limits.wire_bytes,
                        },
                        &chains,
                    ) {
                        ConcatExpansionPoll::Ready(result) => Step::Output(result?),
                        ConcatExpansionPoll::NeedSelection { operand } => Step::Select(operand),
                        ConcatExpansionPoll::NeedInput(input) => Step::Input(input),
                    },
                    Plan::Environment(plan) => match environment.expand_environment_resolved(
                        db,
                        plan,
                        EnvironmentExpansion {
                            catalog: resolver.compiler(),
                            selected: &frame.selected,
                            key: &frame.key,
                            original: &frame.original,
                            site: frame.site,
                            max_wire_bytes: limits.wire_bytes,
                        },
                        &frame.selections,
                        &chains,
                    )? {
                        AdoptedEnvironmentExpansionPoll::Output(output) => {
                            Step::Environment(output)
                        }
                        AdoptedEnvironmentExpansionPoll::NeedSelection { operand } => {
                            Step::Select(operand)
                        }
                        AdoptedEnvironmentExpansionPoll::NeedInput(input) => Step::Input(input),
                    },
                    Plan::IncludeData(_) => match sources {
                        None => Step::External,
                        Some(sources) => {
                            let emitter = IncludeDataExpansion {
                                catalog: resolver.compiler(),
                                selected: &frame.selected,
                                key: &frame.key,
                                original: &frame.original,
                                site: frame.site,
                                authorities: sources.authorities,
                                from: sources.from,
                                max_wire_bytes: limits.wire_bytes,
                            };
                            match resolve_include_chain(
                                db,
                                frame,
                                resolver,
                                caller,
                                environment,
                                limits,
                                &mut budget,
                            )? {
                                ResolvedInclude::Pending(pending) => {
                                    // Resolve the returned call, never reselect the original operand.
                                    Step::Selected(pending)
                                }
                                ResolvedInclude::Ready(resolved) => {
                                    let result = match sources.included {
                                        Some(view) => emitter.poll_included(db, &resolved, view),
                                        None => emitter.poll_resolved(db, &resolved),
                                    };
                                    match result {
                                        IncludeDataPoll::Ready(result) => Step::Output(result?),
                                        IncludeDataPoll::NeedInput(input) => Step::Input(input),
                                        IncludeDataPoll::NeedExpansion { .. } => {
                                            return Err(fault(
                                                "resolved include still needs expansion",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
            }
        };
        let mut pending = None;
        let index = match step {
            Step::External => {
                let frame = frames
                    .pop()
                    .ok_or_else(|| fault("eager external frame absent"))?;
                return Ok(EagerCompileInputPoll::NeedExternal(EagerExternalCall {
                    selected: frame.selected,
                    key: frame.key,
                    original: frame.original,
                    site: frame.site,
                }));
            }
            Step::Output(record) => return Ok(EagerCompileInputPoll::Produced(record)),
            Step::Environment(EnvironmentExpansionPoll::Ready(result)) => {
                return Ok(EagerCompileInputPoll::Produced(result?));
            }
            Step::Environment(EnvironmentExpansionPoll::NeedCapture {
                unit,
                name,
                environment,
                span,
            }) => {
                return Ok(EagerCompileInputPoll::NeedEnvironment {
                    unit,
                    name,
                    environment,
                    span,
                });
            }
            Step::Environment(EnvironmentExpansionPoll::NeedOptionFacade {
                defining_unit,
                value,
                site,
            }) => {
                return Ok(EagerCompileInputPoll::NeedOptionFacade {
                    defining_unit,
                    value,
                    site,
                });
            }
            Step::Environment(EnvironmentExpansionPoll::NeedExpansion { .. }) => {
                return Err(fault("materialized environment retained a nested operand"));
            }
            Step::Input(InputRequest::Expand(input)) => frame
                .selections
                .iter()
                .find(|(_, key)| **key == input.key)
                .map(|(index, _)| *index)
                .map(Some)
                .ok_or_else(|| fault("eager expansion demand has no operand"))?,
            Step::Input(input) => return Ok(EagerCompileInputPoll::NeedInput(input)),
            Step::Select(index) => Some(index),
            Step::Selected(selected) => {
                pending = Some(selected);
                None
            }
        };
        let pending = match pending {
            Some(pending) => *pending,
            None => {
                let nested = frame
                    .nested(index.ok_or_else(|| fault("eager operand index absent"))?)?
                    .clone();
                let context = NestedExpansionContext {
                    environment,
                    engine_version: request.key.engine_version,
                };
                let selection = match &frame.returned {
                    Some(facts) => resolver.resolve_returned_expansion(
                        &nested,
                        caller,
                        &context,
                        facts,
                        frame.before,
                    )?,
                    None => resolver.resolve_expansion(&nested, caller, &context)?,
                };
                PendingString {
                    invocation: nested,
                    selection,
                    facts: frame.returned.clone(),
                    before: frame.before,
                }
            }
        };
        let nested = &pending.invocation;
        let selected = pending.selection;
        let (lookup, key) = selected.into_parts();
        let (definition, external) = match lookup {
            NestedMacroLookup::Compiler { definition, .. } => (definition, false),
            NestedMacroLookup::Other(definition) => (definition, true),
            lookup => {
                return Ok(EagerCompileInputPoll::NeedLookup {
                    lookup,
                    invocation: nested.clone(),
                });
            }
        };
        let key = key.ok_or_else(|| fault("nested macro selection omitted its key"))?;
        let original = nested.arguments().clone();
        let site = nested.reference_span();
        if let Some(index) = index {
            if frame.selections.get(&index).is_some_and(|old| old != &key) {
                return Err(fault("eager selection changed within frozen worklist"));
            }
            frame.selections.insert(index, key.clone());
        }
        if external {
            if let Some(input) = db.input(&InputKey::ExpansionRecord(key.fingerprint())) {
                let InputValue::ExpansionRecord(record) = input.as_ref() else {
                    return Err(fault("adopted external string macro variant differs"));
                };
                if record.key() != &key {
                    return Err(fault("adopted external string macro key differs"));
                }
                continue;
            }
            return Ok(EagerCompileInputPoll::NeedExternal(EagerExternalCall {
                selected: definition,
                key,
                original,
                site,
            }));
        }

        if frames.len() >= limits.depth {
            return Err(fault("eager expansion depth limit"));
        }
        if frames.iter().any(|frame| frame.key == key) {
            return Err(fault("eager expansion cycle"));
        }
        let mut child = Frame::new(definition, key, original, site, resolver, environment)?;
        child.returned = pending.facts;
        child.before = pending.before;
        frames.push(child);
    }
    Err(fault("eager expansion work limit"))
}

/// Prepared environment dependency entry; never captures ambient state or publishes inputs.
pub fn poll_eager_compile_input_from_database(
    db: &mut Database,
    request: EagerCompilerRequest<'_>,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    sources: Option<EagerSourceContext<'_>>,
) -> Result<EagerCompileInputPoll, Diagnostic> {
    request.key.validate()?;
    if request.source_revision != db.source_revision() {
        return Err(fault("eager prepared environment source revision mismatch"));
    }
    let environment = match crate::db::provider::SyntaxProvider::compile_environment(
        db,
        caller.scope.crate_key,
    ) {
        crate::db::poll::QueryPoll::Ready(result) => result?,
        crate::db::poll::QueryPoll::NeedInput(input) => {
            return Ok(EagerCompileInputPoll::NeedInput(input));
        }
    };
    match sources {
        Some(sources) => poll_eager_compile_input_with_sources(
            db,
            request,
            resolver,
            caller,
            &environment,
            sources,
        ),
        None => poll_eager_compile_input(db, request, resolver, caller, &environment),
    }
}

/// Reproduce an already adopted syntax include and retain its exact source leases.
/// This performs no capture or publication, including for nested string operands.
pub fn adopted_eager_include_view(
    db: &mut Database,
    request: EagerCompilerRequest<'_>,
    resolver: &impl EagerMacroResolver,
    caller: &NestedLexicalContext,
    environment: &FrozenCompileEnvironment,
    sources: EagerSourceContext<'_>,
) -> crate::db::poll::QueryPoll<Option<crate::source::include_view::IncludedSourceView>> {
    use crate::{db::poll::QueryPoll, expand::include_arguments::IncludePathPoll};
    let mut work = || -> Result<QueryPoll<Option<crate::source::include_view::IncludedSourceView>>, Diagnostic> {
        match poll_eager_compile_input_with_sources(db,request,resolver,caller,environment,sources)? {
            EagerCompileInputPoll::Complete(_)=>{},
            EagerCompileInputPoll::NeedInput(input)=>return Ok(QueryPoll::NeedInput(input)),
            _=>return Err(fault("include view requires fully adopted compiler output")),
        }
        let frame = Frame::new(request.selected.clone(), request.key.clone(), request.original.clone(), request.site, resolver, environment)?;
        let descriptor = resolver.compiler().select(request.selected)?.ok_or_else(||fault("include view definition absent"))?;
        if descriptor.kind()!=CompileInputKind::Include {return Ok(QueryPoll::Ready(Ok(None)));}
        let mut budget = EagerWorkBudget {remaining:request.limits.work,retained:frame.allocated_bytes()?};
        let arguments = match resolve_include_chain(db, &frame, resolver, caller, environment, request.limits, &mut budget)? {
            ResolvedInclude::Ready(arguments) => arguments,
            ResolvedInclude::Pending(pending) => {
                let key = pending.selection.key().ok_or_else(||fault("include view nested selection absent"))?;
                return Ok(QueryPoll::NeedInput(InputRequest::Expand(Box::new(ExpansionRequest {key:key.clone()}))));
            }
        };
        let arguments = arguments.as_ref();
        let routed=match sources.included {
            Some(view)=>arguments.route_from_included(descriptor,sources.authorities,view,request.site)?,
            None=>arguments.route(descriptor,sources.authorities,sources.from,request.site)?,
        };
        let IncludePathPoll::Capture(capture)=routed else {return Err(fault("include view retained unresolved path"));};
        let capture=InputRequest::CaptureInclude {request:capture,compile_environment:request.key.consumer_compile_env};
        let emitter=crate::expand::include_string::IncludeExpansion {
            catalog:resolver.compiler(),selected:request.selected,key:request.key,original:request.original,site:request.site,authorities:sources.authorities,from:sources.from,max_wire_bytes:request.limits.wire_bytes,
        };
        let poll=match sources.included {
            Some(view)=>emitter.adopted_source_view_included(db,&capture,Some(arguments),view),
            None=>emitter.adopted_source_view(db,&capture,Some(arguments)),
        };
        Ok(match poll {QueryPoll::Ready(result)=>QueryPoll::Ready(result.map(Some)), QueryPoll::NeedInput(input)=>QueryPoll::NeedInput(input)})
    };
    match work() {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
