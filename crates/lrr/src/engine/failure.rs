//! Bounded owned dispatcher facts for a terminal internal defect. Capturing
//! these facts cannot query, compile, perform I/O or retain guest values.
use super::run::RunMachine;
use crate::{
    core::digest::{Digest, StableEncode},
    ir::recipe::{ControlRecipe, RecipeExit},
    vm::frame::OpcodeCounters,
};
use serde::Serialize;

const MAX_BYTES: usize = 64 * 1024;
const MAX_FRAMES: usize = 64;
const MAX_RECIPES: usize = 64;
const MAX_OPS: usize = 1024;
const MAX_SOURCES: usize = 128;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FrameRecord {
    pub pc: Digest,
    pub body: Digest,
    pub node: u32,
    pub phase: u16,
    pub counters: OpcodeCounters,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReducedRecipe {
    pub pc: Digest,
    pub node: u32,
    pub phase: u16,
    pub opcodes: Vec<u8>,
    pub omitted_operations: usize,
    pub exit: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourceContext {
    pub unit: Digest,
    pub content: Digest,
    pub source_edition: u16,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExecutionRecord {
    pub source_revision: u64,
    pub source_contexts: Vec<SourceContext>,
    pub omitted_source_contexts: usize,
    pub frames: Vec<FrameRecord>,
    pub omitted_frames: usize,
    pub lrir: Vec<ReducedRecipe>,
    pub omitted_recipes: usize,
    pub installed_lrir_bytes: usize,
    pub current_recipe_available: bool,
    pub clif: Option<&'static str>,
    pub native_region_ids: Vec<Digest>,
    pub native_context_availability: &'static str,
}
#[derive(Debug, Clone)]
pub(crate) enum FailureContext {
    Available(ExecutionRecord),
    ResourceUnavailable,
}
#[derive(Debug)]
pub(crate) struct CaptureLimit;

/// Read already published inputs and the entered body's index only. This is
/// outside query providers and cannot expand, parse, publish, compile or do I/O.
pub(crate) fn generated_origin(
    run: &RunMachine,
    db: &mut crate::db::memo::Database,
) -> Result<
    Option<crate::syntax::origin::GeneratedOperationOrigin>,
    crate::core::diagnostic::Diagnostic,
> {
    use crate::{
        core::diagnostic::Diagnostic,
        db::dependency::{InputKey, InputValue},
        syntax::{origin::GeneratedOperationOrigin, token_wire::SourceIdentity},
    };
    let invalid = || {
        Diagnostic::new(
            "LRR-E-INTERNAL-SOURCE-MAP",
            "generated fault publication differs".into(),
        )
    };
    let limit = || {
        Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "generated fault origin retention limit".into(),
        )
    };
    let Some((pc, _)) = run.diagnostic_frames().next() else {
        return Err(invalid());
    };
    let Some(source) = db.input(&InputKey::DefinitionSource(pc.body.definition)) else {
        return Ok(None);
    };
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return Err(invalid());
    };
    let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) else {
        return Ok(None);
    };
    let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
        return Err(invalid());
    };
    if &unit.key != parse || !unit.definitions.contains_key(&pc.body.definition) {
        return Err(invalid());
    }
    let body_input = db
        .input(&InputKey::DefinitionBody(pc.body.definition))
        .ok_or_else(invalid)?;
    let InputValue::DefinitionBody(body) = body_input.as_ref() else {
        return Err(invalid());
    };
    let span = run.diagnostic_current_span()?;
    let selected = unit.items.project_body_source_spans(body, &span)?;
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(selected.len())
        .map_err(|_| limit())?;
    let mut bytes = std::mem::size_of::<GeneratedOperationOrigin>()
        .checked_add(
            tokens
                .capacity()
                .checked_mul(std::mem::size_of::<crate::syntax::token_wire::SpanRecord>())
                .ok_or_else(limit)?,
        )
        .ok_or_else(limit)?;
    for original in selected {
        bytes = bytes
            .checked_add(original.source.path.len())
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        let mut path = String::new();
        path.try_reserve_exact(original.source.path.len())
            .map_err(|_| limit())?;
        path.push_str(&original.source.path);
        tokens.push(crate::syntax::token_wire::SpanRecord {
            source: SourceIdentity {
                path,
                unit: original.source.unit,
                content: original.source.content,
            },
            start: original.start,
            end: original.end,
            context: original.context,
            edition: original.edition,
            origin: original.origin,
        });
    }
    let (invocation, definition) = if let Some(producer) = &unit.producer_origin {
        if producer.invocation_identity != unit.items.record().key().invocation {
            return Err(invalid());
        }
        let invocation = producer
            .invocation
            .as_ref()
            .map(|at| at.copy_with_limit(MAX_BYTES.saturating_sub(bytes)))
            .transpose()?;
        if let Some(at) = &invocation {
            bytes = bytes
                .checked_add(at.source.path.capacity())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or_else(limit)?;
        }
        let definition = producer
            .definition
            .as_ref()
            .map(|original| {
                let remaining = MAX_BYTES.saturating_sub(bytes);
                if original.name.len() > remaining {
                    return Err(limit());
                }
                let mut name = String::new();
                name.try_reserve_exact(original.name.len())
                    .map_err(|_| limit())?;
                name.push_str(&original.name);
                let location = original
                    .location
                    .copy_with_limit(remaining.saturating_sub(name.capacity()))?;
                Ok::<_, Diagnostic>(crate::syntax::origin::DefinitionOrigin {
                    identity: original.identity,
                    name,
                    location,
                })
            })
            .transpose()?;
        (invocation, definition)
    } else {
        (
            run.diagnostic_invocation_origin(unit.items.record().key().invocation)?,
            run.diagnostic_definition_origin(&unit.items.record().key().macro_identity)?,
        )
    };
    let mut parents = Vec::new();
    if let Some(producer) = &unit.producer_origin {
        if producer.parents.len() > 256 {
            return Err(limit());
        }
        parents
            .try_reserve_exact(producer.parents.len())
            .map_err(|_| limit())?;
        bytes = bytes
            .checked_add(
                parents
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::syntax::origin::ExpansionOrigin>())
                    .ok_or_else(limit)?,
            )
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        for parent in &producer.parents {
            let invocation = parent
                .invocation
                .as_ref()
                .map(|at| at.copy_with_limit(MAX_BYTES.saturating_sub(bytes)))
                .transpose()?;
            if let Some(at) = &invocation {
                bytes = bytes
                    .checked_add(at.source.path.capacity())
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(limit)?;
            }
            parents.push(crate::syntax::origin::ExpansionOrigin {
                expansion: parent.expansion,
                invocation_identity: parent.invocation_identity,
                invocation,
            });
        }
    }
    let record = GeneratedOperationOrigin {
        body: span,
        expansion: unit.items.record().key().fingerprint(),
        invocation_identity: unit.items.record().key().invocation,
        invocation,
        definition,
        tokens,
        parents,
    };
    if record.heap_bytes().is_none_or(|n| n > MAX_BYTES) {
        return Err(limit());
    }
    Ok(Some(record))
}

/// Capture only entered frame indexes and immutable published source facts.
/// A single shared byte budget covers every frame and copied source path.
pub(crate) fn guest_stack(
    run: &RunMachine,
    db: &mut crate::db::memo::Database,
) -> Result<crate::syntax::origin::GuestStackOrigin, crate::core::diagnostic::Diagnostic> {
    use crate::{
        core::diagnostic::Diagnostic,
        db::dependency::{InputKey, InputValue},
        syntax::{
            origin::{GuestFrameOrigin, GuestStackOrigin, SourceOrigin},
            spans::SourceRange,
            token_wire::SourceIdentity,
        },
    };
    let invalid = || {
        Diagnostic::new(
            "LRR-E-INTERNAL-SOURCE-MAP",
            "guest frame publication differs".into(),
        )
    };
    let limit = || {
        Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "guest stack origin retention limit".into(),
        )
    };
    let mut stack = GuestStackOrigin {
        frames: Vec::new(),
        omitted_frames: 0,
    };
    stack
        .frames
        .try_reserve_exact(MAX_FRAMES)
        .map_err(|_| limit())?;
    let mut bytes = stack.heap_bytes().ok_or_else(limit)?;
    for indexed in run.diagnostic_frame_spans() {
        if stack.frames.len() == MAX_FRAMES {
            stack.omitted_frames = stack.omitted_frames.checked_add(1).ok_or_else(limit)?;
            continue;
        }
        let (pc, body) = indexed?;
        let mut locations = Vec::new();
        if let Some(input) = db.input(&InputKey::DefinitionSource(body.body_key.definition)) {
            let InputValue::DefinitionSource(parse) = input.as_ref() else {
                return Err(invalid());
            };
            if let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) {
                let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
                    return Err(invalid());
                };
                if &unit.key != parse || !unit.definitions.contains_key(&body.body_key.definition) {
                    return Err(invalid());
                }
                let input = db
                    .input(&InputKey::DefinitionBody(body.body_key.definition))
                    .ok_or_else(invalid)?;
                let InputValue::DefinitionBody(handle) = input.as_ref() else {
                    return Err(invalid());
                };
                let spans = unit.items.project_body_source_spans(handle, &body)?;
                locations
                    .try_reserve_exact(spans.len())
                    .map_err(|_| limit())?;
                bytes = bytes
                    .checked_add(
                        locations
                            .capacity()
                            .checked_mul(std::mem::size_of::<SourceOrigin>())
                            .ok_or_else(limit)?,
                    )
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(limit)?;
                for span in spans {
                    bytes = bytes
                        .checked_add(span.source.path.len())
                        .filter(|n| *n <= MAX_BYTES)
                        .ok_or_else(limit)?;
                    let mut path = String::new();
                    path.try_reserve_exact(span.source.path.len())
                        .map_err(|_| limit())?;
                    path.push_str(&span.source.path);
                    locations.push(SourceOrigin {
                        source: SourceIdentity {
                            path,
                            unit: span.source.unit,
                            content: span.source.content,
                        },
                        range: SourceRange::checked(span.start, span.end)?,
                    });
                }
            } else if let Some(at) =
                run.diagnostic_source_origin(&body, MAX_BYTES.saturating_sub(bytes))?
            {
                if at.source.content.0 != parse.content.0 {
                    return Err(invalid());
                }
                locations.try_reserve_exact(1).map_err(|_| limit())?;
                bytes = bytes
                    .checked_add(
                        locations
                            .capacity()
                            .checked_mul(std::mem::size_of::<SourceOrigin>())
                            .ok_or_else(limit)?,
                    )
                    .and_then(|n| n.checked_add(at.source.path.capacity()))
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(limit)?;
                locations.push(at);
            }
        }
        stack.frames.push(GuestFrameOrigin {
            pc,
            body,
            locations,
        });
    }
    if stack.heap_bytes().is_none_or(|n| n > MAX_BYTES) {
        return Err(limit());
    }
    Ok(stack)
}

fn reserve<T>(vector: &mut Vec<T>, count: usize) -> Result<(), CaptureLimit> {
    vector.try_reserve_exact(count).map_err(|_| CaptureLimit)
}
fn exit_tag(exit: &RecipeExit) -> &'static str {
    match exit {
        RecipeExit::Backedge(_) => "backedge",
        RecipeExit::Call { .. } => "call",
        RecipeExit::Effect { .. } => "effect",
        RecipeExit::Next(_) => "next",
        RecipeExit::Branch { .. } => "branch",
        RecipeExit::PatternRetry { .. } => "pattern_retry",
        RecipeExit::PatternNext { .. } => "pattern_next",
        RecipeExit::Control(ControlRecipe::Return { .. }) => "return",
        RecipeExit::Control(ControlRecipe::Break { .. }) => "break",
        RecipeExit::Control(ControlRecipe::Continue { .. }) => "continue",
        RecipeExit::Fault(_) => "fault",
        RecipeExit::MatchNoArm => "match_no_arm",
    }
}
impl ExecutionRecord {
    pub(crate) fn capture(run: &RunMachine) -> Result<Self, CaptureLimit> {
        let mut frames = Vec::new();
        reserve(&mut frames, MAX_FRAMES)?;
        let mut total_frames = 0usize;
        for (pc, counters) in run.diagnostic_frames() {
            total_frames = total_frames.checked_add(1).ok_or(CaptureLimit)?;
            if frames.len() < MAX_FRAMES {
                frames.push(FrameRecord {
                    pc: pc.fingerprint(),
                    body: pc.body.fingerprint(),
                    node: pc.node,
                    phase: pc.phase,
                    counters: counters.clone(),
                });
            }
        }
        let mut source_contexts = Vec::new();
        reserve(&mut source_contexts, MAX_SOURCES)?;
        let mut total_sources = 0usize;
        for (unit, content, edition) in run.diagnostic_source_contexts() {
            total_sources = total_sources.checked_add(1).ok_or(CaptureLimit)?;
            if source_contexts.len() < MAX_SOURCES {
                source_contexts.push(SourceContext {
                    unit: *unit,
                    content: *content,
                    source_edition: edition,
                });
            }
        }
        let current = run.diagnostic_frames().next().map(|(pc, _)| pc);
        let installed = run.installed_recipes();
        let primary = installed
            .recipes()
            .find(|recipe| Some(&recipe.recipe().key) == current);
        let mut lrir = Vec::new();
        reserve(&mut lrir, installed.len().min(MAX_RECIPES))?;
        let mut remaining_ops = MAX_OPS;
        for verified in primary
            .into_iter()
            .chain(
                installed
                    .recipes()
                    .filter(|recipe| Some(&recipe.recipe().key) != current),
            )
            .take(MAX_RECIPES)
        {
            let recipe = verified.recipe();
            let count = recipe.operations.len().min(remaining_ops);
            let mut opcodes = Vec::new();
            reserve(&mut opcodes, count)?;
            opcodes.extend(
                recipe
                    .operations
                    .iter()
                    .take(count)
                    .map(|op| op.opcode_tag()),
            );
            remaining_ops -= count;
            lrir.push(ReducedRecipe {
                pc: recipe.key.fingerprint(),
                node: recipe.key.node,
                phase: recipe.key.phase,
                opcodes,
                omitted_operations: recipe.operations.len() - count,
                exit: exit_tag(&recipe.exit),
            });
        }
        let record = Self {
            source_revision: installed.source_revision(),
            omitted_source_contexts: total_sources - source_contexts.len(),
            source_contexts,
            omitted_frames: total_frames - frames.len(),
            frames,
            omitted_recipes: installed.len() - lrir.len(),
            lrir,
            installed_lrir_bytes: installed.metadata_bytes(),
            current_recipe_available: primary.is_some(),
            clif: None,
            native_region_ids: Vec::new(),
            native_context_availability: "guest native emission is not implemented",
        };
        if record.heap_bytes().is_none_or(|bytes| bytes > MAX_BYTES) {
            return Err(CaptureLimit);
        }
        Ok(record)
    }
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.frames
                    .capacity()
                    .checked_mul(std::mem::size_of::<FrameRecord>())?,
            )?
            .checked_add(
                self.source_contexts
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceContext>())?,
            )?
            .checked_add(
                self.lrir
                    .capacity()
                    .checked_mul(std::mem::size_of::<ReducedRecipe>())?,
            )?
            .checked_add(
                self.native_region_ids
                    .capacity()
                    .checked_mul(std::mem::size_of::<Digest>())?,
            )?;
        for recipe in &self.lrir {
            bytes = bytes.checked_add(recipe.opcodes.capacity())?;
        }
        Some(bytes)
    }
}
impl FailureContext {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        match self {
            Self::Available(record) => std::mem::size_of::<Self>().checked_add(
                record
                    .heap_bytes()?
                    .checked_sub(std::mem::size_of::<ExecutionRecord>())?,
            ),
            Self::ResourceUnavailable => Some(std::mem::size_of::<Self>()),
        }
    }
}
