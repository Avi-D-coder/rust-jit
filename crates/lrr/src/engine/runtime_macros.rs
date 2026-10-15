//! Dispatcher-owned demand for compiler runtime macros; no expansion side effects.
use crate::{
    core::diagnostic::Diagnostic,
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::{MacroNamespaces, PathLookup},
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{RuntimeMacroPreparation, StaticPanicRecipeLowering},
        source_macros::PublishedMacroIndex,
    },
    ir::{
        pc::{NodeId, PcKey, Phase},
        recipe::VerifiedRecipe,
    },
    source::changes::PreparedSourceView,
    syntax::{body_index::BodyIndex, spans::SourceRange},
};
#[path = "runtime_floating_constants.rs"]
pub(crate) mod floating_constants;

#[path = "runtime_lexical_calls.rs"]
mod lexical_calls;

fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
/// Borrowed immutable resolution facts. Constructing this context neither enters
/// guest bodies nor captures invocation arguments, and conveys no macro authority.
pub struct RuntimeMacroContext<'a> {
    source: &'a PreparedSourceView,
    index: &'a PublishedMacroIndex,
    namespaces: &'a MacroNamespaces,
    catalog: &'a RuntimeMacroCatalog,
    compiler_exports: &'a [crate::frontend::source_macro_routes::SourceMacroRoutes],
}
pub(crate) struct PreparedRuntimeMacro {
    pub recipe: Box<VerifiedRecipe>,
    pub no_item: Option<crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    pub call: Option<std::sync::Arc<crate::syntax::runtime_macro_scalars::RuntimeMacroCall>>,
}
impl<'a> RuntimeMacroContext<'a> {
    pub fn new(
        source: &'a PreparedSourceView,
        index: &'a PublishedMacroIndex,
        namespaces: &'a MacroNamespaces,
        catalog: &'a RuntimeMacroCatalog,
    ) -> Result<Self, Diagnostic> {
        index.require_publication(source)?;
        Ok(Self {
            source,
            index,
            namespaces,
            catalog,
            compiler_exports: &[],
        })
    }
    pub(crate) fn with_compiler_exports(
        mut self,
        exports: &'a [crate::frontend::source_macro_routes::SourceMacroRoutes],
    ) -> Self {
        self.compiler_exports = exports;
        self
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    /// Reuse the completed publication and no-item certificates for ordinary
    /// constructor lookup; this conveys no macro execution authority.
    pub fn bind_record_constructor<'p>(
        &self,
        db: &mut crate::db::memo::Database,
        graph: &crate::frontend::names::NameNamespaces,
        sources: &[&PreparedSourceView],
        body: &BodyIndex,
        plan: &crate::syntax::constructors::ConstructorPlan,
        proofs: impl Iterator<Item = &'p crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    ) -> crate::db::poll::QueryPoll<crate::frontend::constructors::RecordConstructorBinding> {
        let mut entered = match self.index.enter_body(self.source.selection(), body.body()) {
            Ok(entered) => entered,
            Err(error) => return crate::db::poll::QueryPoll::Ready(Err(error)),
        };
        for proof in proofs.filter(|proof| proof.body() == body.body()) {
            if let Err(error) = entered.accept_no_item_proof(
                self.source.selection(),
                body.body(),
                proof,
                self.catalog,
            ) {
                return crate::db::poll::QueryPoll::Ready(Err(error));
            }
        }
        crate::frontend::constructors::bind_record_constructor(
            db,
            graph,
            sources,
            self.source,
            &entered,
            body,
            plan,
        )
    }
    /// Add only the exact compiler facet and its disclosed dependency aliases.
    /// The ordinary import engine can then classify a macro-only import without
    /// leaving unrelated source function names behind an unresolved frontier.
    pub(crate) fn contribute_names(
        &self,
        graph: &mut crate::frontend::names::NameNamespaces,
    ) -> Result<(), Diagnostic> {
        use crate::{
            frontend::names::NameDefinition, source::manifest::Namespace,
            syntax::visibility::VisibilityScope,
        };
        let root = self.catalog.source_routes()?;
        let exports = std::iter::once(&root).chain(self.compiler_exports.iter());
        for routes in exports {
            for (unit, path) in &routes.modules {
                graph.declare_module(
                    *unit,
                    &path.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
                    routes.edition,
                    VisibilityScope::Public,
                )?;
            }
            for route in routes.definitions() {
                graph.define(
                    route.path.clone(),
                    NameDefinition {
                        namespace: Namespace::Macro,
                        identity: route.definition.identity,
                        defining_unit: route.definition.defining_unit,
                        definition_edition: route.definition.definition_edition,
                    },
                    VisibilityScope::Public,
                )?;
            }
            for ((consumer, alias), provider) in self.namespaces.dependency_bindings() {
                if routes.modules.contains(&(*provider, vec![]))
                    && *consumer == self.source.selection().outline.crate_key
                {
                    graph.dependency(
                        *consumer,
                        alias.as_str(),
                        *provider,
                        self.source.selection().outline.parse.profile.edition,
                    )?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn resolve<'p>(
        &self,
        body: &BodyIndex,
        pc: &PcKey,
        proofs: impl Iterator<Item = &'p crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    ) -> Result<Option<PreparedRuntimeMacro>, Diagnostic> {
        let Some(handle) = self
            .source
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|handle| handle.key() == pc.body)
        else {
            return Ok(None);
        };
        let node = body
            .nodes()
            .get(pc.node as usize)
            .ok_or_else(|| fault("LRR-E-RUNTIME-MACRO", "reached macro node is absent"))?;
        let range = SourceRange {
            start: handle
                .range
                .start
                .checked_add(node.range.start)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "macro source start overflow"))?,
            end: handle
                .range
                .start
                .checked_add(node.range.end)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "macro source end overflow"))?,
        };
        let mut entered = self.index.enter_body(self.source.selection(), &pc.body)?;
        for proof in proofs.filter(|proof| proof.body() == &pc.body) {
            entered.accept_no_item_proof(self.source.selection(), &pc.body, proof, self.catalog)?;
        }
        match entered.prepare_runtime_macro(
            self.source.selection(),
            &pc.body,
            range,
            self.namespaces,
            self.catalog,
        )? {
            RuntimeMacroPreparation::Intrinsic(plan) => {
                if let Some(name) = plan.arguments().zero_arg_predicate_call() {
                    self.source
                        .require_module_predicate_call(handle, body, pc, name)?;
                }
                let scalar_assertion = matches!(
                    plan.arguments().kind(),
                    RuntimeMacroKind::Assert
                        | RuntimeMacroKind::AssertEq
                        | RuntimeMacroKind::AssertNe
                ) && !(plan.arguments().kind() == RuntimeMacroKind::Assert
                    && (plan.arguments().literal_predicate().is_some()
                        || plan.arguments().zero_arg_predicate_call().is_some()
                        || plan.arguments().binding_predicate().is_some()));
                let call = if scalar_assertion {
                    plan.scalar_call_witness(self.source, body, pc)?
                } else {
                    None
                };
                let lowering = match plan.arguments().kind() {
                    RuntimeMacroKind::Vec => {
                        return Err(fault(
                            "LRR-E-UNSUPPORTED",
                            "reached vec requires pending Vec allocation and selected Clone integration",
                        ));
                    }
                    RuntimeMacroKind::Println => {
                        return Err(fault(
                            "LRR-E-UNSUPPORTED",
                            "reached println requires pending formatter and managed output integration",
                        ));
                    }
                    RuntimeMacroKind::Panic
                    | RuntimeMacroKind::Unreachable
                    | RuntimeMacroKind::Unimplemented => plan.lower_static_panic_recipe(
                        self.source,
                        body,
                        pc,
                        body.pc(NodeId(0), Phase::Exit)?,
                    ),
                    RuntimeMacroKind::Assert
                        if plan.arguments().literal_predicate().is_some()
                            || plan.arguments().zero_arg_predicate_call().is_some()
                            || plan.arguments().binding_predicate().is_some() =>
                    {
                        plan.lower_assertion_recipe(
                            self.source,
                            body,
                            pc,
                            self.source.runtime_macro_successor(handle, body, pc)?,
                        )
                    }
                    RuntimeMacroKind::Assert
                    | RuntimeMacroKind::AssertEq
                    | RuntimeMacroKind::AssertNe => plan.lower_scalar_assertion_recipe(
                        self.source,
                        body,
                        pc,
                        self.source.runtime_macro_successor(handle, body, pc)?,
                    ),
                };
                match lowering? {
                    StaticPanicRecipeLowering::Recipe(recipe) => Ok(Some(PreparedRuntimeMacro {
                        recipe,
                        no_item: plan.no_item_proof(&pc.body),
                        call,
                    })),
                    StaticPanicRecipeLowering::NeedsFormatting => Err(fault(
                        "LRR-E-UNSUPPORTED",
                        "reached runtime macro formatting requires pending formatter integration",
                    )),
                }
            }
            RuntimeMacroPreparation::SourceExpansion {
                definition,
                invocation,
            } => Err(pending_source_expansion(
                self.source,
                pc,
                invocation.key(),
                invocation.range(),
                definition.identity,
            )?),
            RuntimeMacroPreparation::Resolution { lookup, .. } => Err(match lookup {
                PathLookup::NeedInput(_) => fault(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "reached macro lookup requires pending expansion input",
                ),
                PathLookup::Absent => fault("LRR-E-MACRO-ABSENT", "reached macro name is absent"),
                PathLookup::Ambiguous(_) => {
                    fault("LRR-E-MACRO-AMBIGUOUS", "reached macro name is ambiguous")
                }
                PathLookup::Inaccessible(_) => fault(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "reached macro name is inaccessible",
                ),
                PathLookup::Defined(_) => {
                    fault("LRR-E-INTERNAL-IR", "defined macro retained as unresolved")
                }
            }),
        }
    }
}

/// Diagnostic-only selected source evidence, shared by unresolved runtime
/// expansion and a sealed expansion outside the closed executable control lane.
fn pending_source_expansion(
    source: &PreparedSourceView,
    pc: &PcKey,
    parse: &crate::syntax::parse::ParseKey,
    range: SourceRange,
    definition: crate::core::digest::Digest,
) -> Result<Diagnostic, Diagnostic> {
    use crate::{
        core::digest::{StableEncode, StableEncoder},
        source::manifest::ItemKind,
        syntax::{
            origin::{DefinitionOrigin, MacroOrigin, SourceOrigin},
            token_wire::{Key, SourceIdentity},
        },
    };
    let selected = source.selection();
    if parse != &selected.outline.parse {
        return Err(fault(
            "LRR-E-RUNTIME-MACRO",
            "pending diagnostic source/profile differs",
        ));
    }
    let identity = SourceIdentity {
        unit: Key(selected.outline.crate_key.0),
        content: Key(selected.outline.parse.content.0),
        path: selected.path.as_str().into(),
    };
    let header = if source.outline().items.iter().any(|item| {
        item.def_key.item_kind == ItemKind::Macro && item.def_key.fingerprint() == definition
    }) {
        Some(source.macro_definition_header(definition)?)
    } else {
        // Sealed declaration records use the captured declaration identity,
        // not the outline member's DefKey. Follow the same original Source
        // body-owner/header join as SourceDeclarative::declaration_demand.
        let scan = source.declaration_inventory(Default::default())?;
        if scan.parse() != &selected.outline.parse
            || scan.path() != &selected.path
            || scan.scope() != &source.outline().root_scope
            || scan.cfg() != selected.outline.cfg
            || scan.generated_origin().is_some()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "pending definition declaration source attachment differs",
            ));
        }
        let mut definitions = scan
            .macro_definitions()
            .iter()
            .filter(|header| header.identity == definition);
        let captured = definitions.next();
        if definitions.next().is_some() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "pending definition declaration identity is ambiguous",
            ));
        }
        if let Some(captured) = captured {
            let shape = source.macro_definition_shape_at(captured.range)?;
            let header = source.macro_definition_header_at(captured.range)?;
            let edition = selected.outline.parse.profile.edition;
            if shape.body() != captured.body
                || crate::expand::scopes::MacroName::parse(&header.1, edition)?
                    != crate::expand::scopes::MacroName::parse(&captured.name, edition)?
            {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "pending definition captured body/header differs",
                ));
            }
            Some(header)
        } else {
            // No same-source identity is not permission to guess a foreign or
            // generated definition's location from its spelling.
            None
        }
    };
    let definition_origin = header.map(|(range, name)| DefinitionOrigin {
        identity: definition,
        name,
        location: SourceOrigin {
            source: identity.clone(),
            range,
        },
    });
    let mut invocation_identity = StableEncoder::new("lrr/v2/reached-pending-macro-diagnostic");
    invocation_identity.record(pc);
    invocation_identity.digest(definition);
    let mut diagnostic = fault(
        "LRR-E-CATALOG-INCOMPLETE",
        "reached user macro requires source expansion integration",
    );
    diagnostic.macro_origin = Some(Box::new(MacroOrigin {
        invocation_identity: invocation_identity.finish(),
        invocation: Some(SourceOrigin {
            source: identity,
            range,
        }),
        definition: definition_origin,
        failure_kind: None,
        failure_span_key: None,
        failure_span: None,
        parents: Vec::new(),
    }));
    Ok(diagnostic)
}

/// Prepare an exact original-source call using its sealed discovery descriptor.
/// This consumes published immutable input or suspends; it never invokes a macro
/// and creates no compiler no-item certificate for a user declaration.
pub fn prepare_declarative_expression(
    db: &mut crate::db::memo::Database,
    source: &PreparedSourceView,
    body: &BodyIndex,
    pc: &PcKey,
    expected: &crate::expand::record::ExpansionDescriptor,
) -> crate::db::poll::QueryPoll<crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression>
{
    use crate::{
        core::digest::{Digest, StableEncode},
        db::{
            dependency::{DependencyStamp, InputKey, InputValue},
            poll::{ExpansionRequest, InputRequest, QueryPoll},
        },
        expand::{
            identity::{GrammarSlot, MacroIdentity},
            source_declarative::{SourceDeclarative, group},
        },
        ir::{
            op::{HintRecipe, Op},
            recipe::{Origin, RecipeDraft, RecipeExit},
            validate::verify_declarative_literal,
        },
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    let validate = || -> Result<_, Diagnostic> {
        expected.key.validate()?;
        if expected.wire_bytes > 32 * 1024 * 1024
            || !matches!(
                expected.key.macro_identity,
                MacroIdentity::Declaration { .. }
            )
            || !matches!(
                expected.key.grammar,
                GrammarSlot::Expression | GrammarSlot::Statements
            )
            || expected.key.call_edition != source.selection().outline.parse.profile.edition
            || expected.key.engine_version != crate::expand::source_declarative::engine_version()
            || body.body() != &pc.body
            || pc.phase != Phase::Entry.code()?
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached declarative expression key/profile/phase differs",
            ));
        }
        let handle = source
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|handle| handle.key() == pc.body)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "reached declarative body source absent",
                )
            })?;
        let node = body
            .nodes()
            .get(pc.node as usize)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "reached declarative node absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached declarative node is not a macro",
            ));
        }
        let range = SourceRange {
            start: handle
                .range
                .start
                .checked_add(node.range.start)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "macro start overflow"))?,
            end: handle
                .range
                .start
                .checked_add(node.range.end)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "macro end overflow"))?,
        };
        let scan = source.declaration_inventory(Default::default())?;
        let producer = scan
            .producers()
            .iter()
            .find(|producer| producer.range == range && producer.grammar == expected.key.grammar)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "reached expression producer absent",
                )
            })?;
        let original = SourceDeclarative::new(source.share())?;
        let input = group(
            &original.owned_forest()?,
            source.macro_token_group(range, false)?,
        )?;
        if producer.identity != expected.key.invocation
            || Digest::of_bytes(
                &input
                    .encode()
                    .map_err(|error| fault(error.code, &error.message))?,
            ) != expected.key.input_tokens
            || Digest(
                crate::expand::source_hygiene::source_root(
                    source.selection().outline.crate_key,
                    expected.key.call_edition,
                )?
                .context
                .0,
            ) != expected.key.parent_context
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached expression invocation/input/context differs",
            ));
        }
        Ok((handle, node))
    };
    let (handle, node) = match validate() {
        Ok(value) => value,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let mut dependencies = Vec::new();
    // Retain all immutable publication stamps consumed by this preparation.
    for key in [
        InputKey::CapturedSource(source.selection().location_identity()),
        InputKey::DefinitionSource(pc.body.definition),
        InputKey::DefinitionBody(pc.body.definition),
    ] {
        let Some(value) = db.input(&key) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached expression source publication absent",
            )));
        };
        let matches = match value.as_ref() {
            InputValue::CapturedSource(selected) => selected == source.selection(),
            InputValue::DefinitionSource(parse) => parse == &source.selection().outline.parse,
            InputValue::DefinitionBody(published) => published == handle,
            _ => false,
        };
        if !matches {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached expression source publication differs",
            )));
        }
        dependencies.push(DependencyStamp::Input {
            key,
            fingerprint: value.fingerprint(),
        });
    }
    let key = InputKey::ExpansionRecord(expected.key.fingerprint());
    let Some(value) = db.input(&key) else {
        return QueryPoll::NeedInput(InputRequest::Expand(Box::new(ExpansionRequest {
            key: expected.key.clone(),
        })));
    };
    let result = (|| {
        let InputValue::ExpansionRecord(record) = value.as_ref() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached expansion input kind differs",
            ));
        };
        if record.descriptor().fingerprint() != expected.fingerprint() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "reached expansion descriptor differs from sealed discovery",
            ));
        }
        dependencies.push(DependencyStamp::Input {
            key,
            fingerprint: value.fingerprint(),
        });
        if expected.key.grammar == GrammarSlot::Statements {
            let epoch_key = InputKey::ExpansionEpoch;
            let Some(epoch) = db.input(&epoch_key) else {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated control requires its published expansion epoch",
                ));
            };
            if !matches!(epoch.as_ref(), InputValue::ExpansionEpoch(_)) {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated control expansion epoch input differs",
                ));
            }
            dependencies.push(DependencyStamp::Input {
                key: epoch_key,
                fingerprint: epoch.fingerprint(),
            });
            if let Some(prepared) = crate::frontend::runtime_macro_plan::prepare_declarative_break(
                crate::frontend::runtime_macro_plan::DeclarativeControlInput {
                    db,
                    source,
                    handle,
                    body,
                    pc,
                    input: value.as_ref(),
                    dependencies,
                },
            )? {
                return Ok(prepared);
            }
            let MacroIdentity::Declaration { definition } = &expected.key.macro_identity else {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "pending source macro identity differs",
                ));
            };
            return Err(pending_source_expansion(
                source,
                pc,
                &source.selection().outline.parse,
                SourceRange {
                    start: handle
                        .range
                        .start
                        .checked_add(node.range.start)
                        .ok_or_else(|| {
                            fault("LRR-E-METADATA-LIMIT", "pending source start overflow")
                        })?,
                    end: handle
                        .range
                        .start
                        .checked_add(node.range.end)
                        .ok_or_else(|| {
                            fault("LRR-E-METADATA-LIMIT", "pending source end overflow")
                        })?,
                },
                *definition,
            )?);
        }
        let span = body.span(NodeId(pc.node))?;
        let output =
            crate::syntax::generated_expression::literal(value.as_ref(), span.clone(), pc)?;
        let recipe = verify_declarative_literal(
            crate::frontend::thunks::with_temporary_scope_entry(
                body,
                RecipeDraft {
                    key: pc.clone(),
                    origin: Origin {
                        span,
                        expansion_ancestry: vec![expected.fingerprint()],
                    },
                    operations: vec![Op::Literal {
                        literal: output.literal().clone(),
                        hint: HintRecipe::None,
                        destination: node.value.ok_or_else(|| {
                            fault("LRR-E-EXPANSION-INPUT", "reached macro value slot absent")
                        })?,
                    }],
                    exits: vec![RecipeExit::Next(
                        source.runtime_macro_successor(handle, body, pc)?,
                    )],
                    dependencies,
                },
            )?,
            body,
            &output,
        )?;
        let (tokens, origins) = output.into_provenance();
        let token_bytes = tokens
            .allocated_bytes()
            .map_err(|error| fault(error.code, &error.message))?;
        let metadata_bytes = origins
            .capacity()
            .checked_mul(std::mem::size_of::<crate::syntax::spans::TokenOrigin>())
            .and_then(|bytes| bytes.checked_add(token_bytes))
            .and_then(|bytes| bytes.checked_add(recipe.metadata_bytes()))
            .and_then(|bytes| {
                bytes.checked_add(std::mem::size_of::<
                    crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression,
                >())
            })
            .filter(|bytes| *bytes <= 64 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "prepared declarative expression metadata limit",
                )
            })?;
        Ok(
            crate::frontend::runtime_macro_plan::PreparedDeclarativeExpression {
                recipe: Box::new(recipe),
                tokens,
                origins,
                metadata_bytes,
            },
        )
    })();
    QueryPoll::Ready(result)
}
