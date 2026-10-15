//! Same-frame admission for a single genuinely captured generated break.
//! Caller-provided names/dependencies carry no source or macro authority.
use super::PreparedDeclarativeExpression;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::Database,
    },
    expand::{
        identity::{GrammarSlot, MacroIdentity},
        source_declarative::{SourceDeclarative, group},
    },
    ir::{
        op::{HintRecipe, Op},
        pc::{NodeId, PcKey, Phase, SlotId},
        recipe::{ControlRecipe, Origin, RecipeDraft, RecipeExit},
    },
    source::changes::PreparedSourceView,
    syntax::{
        body_index::{BodyIndex, ExpressionKind, LabelId, NodeRole},
        generated_control::{
            GeneratedBreak, decode_break, target_continuation, target_label_range,
        },
        outline::BodyHandle,
        parse::ParseKey,
        spans::BodySpan,
        token_wire::{Key, TokenForest},
    },
};

pub(crate) struct DeclarativeControlInput<'a> {
    pub(crate) db: &'a Database,
    pub(crate) source: &'a PreparedSourceView,
    pub(crate) handle: &'a BodyHandle,
    pub(crate) body: &'a BodyIndex,
    pub(crate) pc: &'a PcKey,
    pub(crate) input: &'a InputValue,
    pub(crate) dependencies: Vec<DependencyStamp>,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated break source admission budget".into(),
    )
}

/// Private source-selected control target. Decoded spelling never constructs it.
pub(crate) struct GeneratedBreakAdmission {
    caller: PcKey,
    span: BodySpan,
    source: ParseKey,
    target: LabelId,
    target_node: NodeId,
    target_result: SlotId,
    destination: SlotId,
    resume: PcKey,
    expansion: Digest,
    dependency: DependencyStamp,
    captured_lifetime: Option<Key>,
    required: Box<[DependencyStamp]>,
}
impl GeneratedBreakAdmission {
    pub(crate) fn caller(&self) -> &PcKey {
        &self.caller
    }
    pub(crate) fn target(&self) -> LabelId {
        self.target
    }
    pub(crate) fn target_node(&self) -> NodeId {
        self.target_node
    }
    pub(crate) fn target_result(&self) -> SlotId {
        self.target_result
    }
    pub(crate) fn destination(&self) -> SlotId {
        self.destination
    }
    pub(crate) fn resume(&self) -> &PcKey {
        &self.resume
    }
    pub(crate) fn expansion(&self) -> Digest {
        self.expansion
    }
    pub(crate) fn dependency(&self) -> &DependencyStamp {
        &self.dependency
    }
    pub(crate) fn required_dependencies(&self) -> &[DependencyStamp] {
        &self.required
    }
    pub(crate) fn require(
        &self,
        body: &BodyIndex,
        output: &GeneratedBreak,
    ) -> Result<(), Diagnostic> {
        if body.body() != &self.caller.body
            || body.profile() != &self.source.profile
            || output.caller() != &self.caller
            || output.span() != &self.span
            || output.expansion() != self.expansion
            || output.dependency() != &self.dependency
            || output.lifetime().map(|l| l.span_key()) != self.captured_lifetime
            || body.pc(NodeId(self.caller.node), Phase::Entry)? != self.caller
            || body.span(NodeId(self.caller.node))? != self.span
        {
            return Err(invalid(
                "generated break source/caller/output proof differs",
            ));
        }
        let node = body
            .nodes()
            .get(self.caller.node as usize)
            .ok_or_else(|| invalid("generated break caller absent"))?;
        let target = body
            .labels()
            .get(self.target.0 as usize)
            .ok_or_else(|| invalid("generated break target absent"))?;
        let target_node = body
            .nodes()
            .get(target.node.0 as usize)
            .ok_or_else(|| invalid("generated break target node absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro)
            || node.value != Some(self.destination)
            || target.id != self.target
            || target.node != self.target_node
            || target_node.value != Some(self.target_result)
            || body.pc(
                NodeId(self.resume.node),
                Phase::from_code(self.resume.phase),
            )? != self.resume
        {
            return Err(invalid(
                "generated break source target/slots/continuation differ",
            ));
        }
        let mut ancestor = node.enclosing_syntax_target;
        for _ in 0..body.labels().len() {
            let Some(id) = ancestor else { break };
            if id == self.target {
                return Ok(());
            }
            ancestor = body
                .labels()
                .get(id.0 as usize)
                .ok_or_else(|| invalid("generated break target ancestry absent"))?
                .parent;
        }
        Err(invalid("generated break target is not a caller ancestor"))
    }
}
fn retain_input(
    db: &Database,
    required: &mut Vec<DependencyStamp>,
    key: InputKey,
) -> Result<(), Diagnostic> {
    let (_, fingerprint) = db
        .published_input(&key)
        .ok_or_else(|| invalid("generated break required publication absent"))?;
    let stamp = DependencyStamp::Input { key, fingerprint };
    if let Some(old) = required.iter().find(|old| old.same_key(&stamp)) {
        return if old == &stamp {
            Ok(())
        } else {
            Err(invalid("generated break dependency conflicts"))
        };
    }
    if required.len() >= 4096 {
        return Err(limit());
    }
    let bytes = required
        .iter()
        .chain(std::iter::once(&stamp))
        .try_fold(0usize, |bytes, stamp| {
            bytes
                .checked_add(std::mem::size_of::<DependencyStamp>())?
                .checked_add(stamp.heap_bytes())
        })
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(limit)?;
    let _ = bytes;
    required.try_reserve(1).map_err(|_| limit())?;
    required.push(stamp);
    Ok(())
}

impl GeneratedBreakAdmission {
    fn capture_current(
        args: &DeclarativeControlInput<'_>,
        output: &GeneratedBreak,
    ) -> Result<Self, Diagnostic> {
        let source = args.source;
        source.require_original_hygiene()?;
        if args.body.body() != &args.pc.body
            || args.handle.key() != args.pc.body
            || args.body.profile() != &source.selection().outline.parse.profile
            || args.pc.phase != Phase::Entry.code()?
            || output.caller() != args.pc
            || output.span() != &args.body.span(NodeId(args.pc.node))?
        {
            return Err(invalid(
                "generated break caller source/body/profile differs",
            ));
        }
        let item = source
            .outline()
            .items
            .iter()
            .find(|item| {
                item.def_key.fingerprint() == args.pc.body.definition
                    && item.body.as_ref() == Some(args.handle)
            })
            .ok_or_else(|| invalid("generated break exact source declaration absent"))?;
        let node = args
            .body
            .nodes()
            .get(args.pc.node as usize)
            .ok_or_else(|| invalid("generated break macro node absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro) {
            return Err(invalid("generated break requires caller Macro node"));
        }
        let start = args
            .handle
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(limit)?;
        let end = args
            .handle
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(limit)?;
        let range = crate::syntax::spans::SourceRange::checked(start, end)?;
        let InputValue::ExpansionRecord(record) = args.input else {
            return Err(invalid("generated break record variant differs"));
        };
        if record.key().grammar != GrammarSlot::Statements
            || !matches!(
                record.key().macro_identity,
                MacroIdentity::Declaration { .. }
            )
            || record.key().call_edition != source.selection().outline.parse.profile.edition
            || output.call_edition() != record.key().call_edition
            || output.definition_edition() != record.key().definition_edition
            || record.key().engine_version != crate::expand::source_declarative::engine_version()
            || record.descriptor().fingerprint() != output.expansion()
        {
            return Err(invalid(
                "generated break expansion identity/grammar/edition differs",
            ));
        }
        record.key().validate()?;
        let record_key = InputKey::ExpansionRecord(record.key().fingerprint());
        if args
            .db
            .published_input(&record_key)
            .is_none_or(|(_, fp)| fp != args.input.fingerprint())
        {
            return Err(invalid(
                "generated break record is not the current published input",
            ));
        }
        let mut required = Vec::new();
        for key in [
            InputKey::CapturedSource(source.selection().location_identity()),
            InputKey::DefinitionSource(item.def_key.fingerprint()),
            InputKey::DefinitionHeader(item.def_key.fingerprint()),
            InputKey::DefinitionBody(item.def_key.fingerprint()),
            InputKey::DeclarationContext(item.def_key.fingerprint()),
            InputKey::ExpansionEpoch,
            InputKey::SourceDependencies,
            record_key,
        ] {
            retain_input(args.db, &mut required, key)?;
        }
        for (key, expected) in source.prepared_inputs() {
            if args
                .db
                .published_input(key)
                .is_none_or(|(_, fp)| fp != expected.fingerprint())
            {
                return Err(invalid(
                    "generated break retained original source publication changed",
                ));
            }
            retain_input(args.db, &mut required, key.clone())?;
        }
        let certificate = args
            .db
            .source_view_scope_certificate(source.selection(), &item.scope)?;
        let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
            return Err(invalid("generated break scope certificate variant differs"));
        };
        if input.seal.unresolved_frontiers() != 0
            || !input
                .records
                .iter()
                .any(|d| d.fingerprint() == output.expansion())
        {
            return Err(invalid(
                "generated break record lacks the actual sealed owning source scope",
            ));
        }
        let seal_key = InputKey::DiscoverySeal(input.seal.key());
        if args
            .db
            .published_input(&seal_key)
            .is_none_or(|(_, fp)| fp != certificate.fingerprint())
        {
            return Err(invalid(
                "generated break owning scope is not currently published",
            ));
        }
        retain_input(args.db, &mut required, seal_key)?;
        if input.compiler_inputs.len() > 4096 {
            return Err(limit());
        }
        for key in input.compiler_inputs.keys() {
            retain_input(args.db, &mut required, key.clone())?;
        }
        // Replay the actual invocation input and producer. No name-only or
        // independently supplied record grants a reached macro target.
        let scan = source.declaration_inventory(Default::default())?;
        if !scan.producers().iter().any(|producer| {
            producer.range == range
                && producer.grammar == GrammarSlot::Statements
                && producer.identity == record.key().invocation
        }) {
            return Err(invalid(
                "generated break original statement producer differs",
            ));
        }
        let original = SourceDeclarative::new(source.share())?;
        let adapter_forest = original.owned_forest()?;
        let captured = group(&adapter_forest, source.macro_token_group(range, false)?)?;
        if Digest::of_bytes(&captured.encode().map_err(|e| invalid(&e.message))?)
            != record.key().input_tokens
        {
            return Err(invalid("generated break captured invocation tokens differ"));
        }
        let forest =
            TokenForest::decode(&adapter_forest.encode().map_err(|e| invalid(&e.message))?)
                .map_err(|e| invalid(&e.message))?;
        if forest.spans().len() > 100_000 {
            return Err(limit());
        }
        let root = crate::expand::source_hygiene::source_root(
            source.selection().outline.crate_key,
            source.selection().outline.parse.profile.edition,
        )?;
        if record.key().parent_context != Digest(root.context.0) {
            return Err(invalid("generated break parent context differs"));
        }
        let captured_lifetime = if let Some(lifetime) = output.lifetime() {
            let span = lifetime.span();
            if span.source.unit != Key(source.selection().outline.crate_key.0)
                || span.source.content != Key(source.selection().outline.parse.content.0)
                || span.source.path != source.selection().path.as_str()
                || span.start < range.start
                || span.end > range.end
                || !forest.spans().iter().any(|original| {
                    original.source == span.source
                        && original.start == span.start
                        && original.end == span.end
                        && original.origin == span.origin
                        && original.edition == span.edition
                        && original.context == root.context
                })
            {
                return Err(unsupported(
                    "generated break label requires an actual captured caller lifetime origin",
                ));
            }
            // Every materialized piece of the lifetime must retain a genuine
            // captured origin, not just the primary identifier span.
            let selected = lifetime.generated_range();
            let mut pieces = 0usize;
            if output.origins().len() > 100_000 || output.tokens().spans().len() > 100_000 {
                return Err(limit());
            }
            for origin in output
                .origins()
                .iter()
                .filter(|origin| origin.start < selected.end && selected.start < origin.end)
            {
                pieces = pieces
                    .checked_add(1)
                    .filter(|n| *n <= 16)
                    .ok_or_else(limit)?;
                let piece = output
                    .tokens()
                    .spans()
                    .iter()
                    .find(|span| span.key() == origin.span)
                    .ok_or_else(|| invalid("generated break lifetime piece origin absent"))?;
                if piece.context != span.context
                    || piece.edition != span.edition
                    || piece.source.unit != Key(source.selection().outline.crate_key.0)
                    || piece.source.content != Key(source.selection().outline.parse.content.0)
                    || piece.source.path != source.selection().path.as_str()
                    || piece.start < range.start
                    || piece.end > range.end
                    || !forest.spans().iter().any(|original| {
                        original.source == piece.source
                            && original.start == piece.start
                            && original.end == piece.end
                            && original.origin == piece.origin
                            && original.edition == piece.edition
                            && original.context == root.context
                    })
                {
                    return Err(unsupported(
                        "generated break lifetime contains a non-captured caller origin",
                    ));
                }
            }
            if pieces == 0 {
                return Err(invalid("generated break lifetime origin coverage absent"));
            }
            let facts = crate::expand::source_hygiene::output_facts(output.tokens())?;
            let roots = crate::frontend::hygiene::HygieneFacts::new(root.contexts, root.rows)?
                .with_name_contexts(root.names)?;
            if facts.name_local(lifetime.context())? != roots.name_local(root.context)? {
                return Err(unsupported(
                    "generated break label has foreign or definition-site local hygiene",
                ));
            }
            Some(lifetime.span_key())
        } else {
            None
        };
        let blob = source.captured_macro_blob()?;
        if blob.bytes().len() > 4 * 1024 * 1024 {
            return Err(limit());
        }
        let parsed = args
            .db
            .prepared_parse(&source.selection().outline.parse)
            .ok_or_else(|| invalid("generated break original prepared parse absent"))?;
        if parsed.key() != source.selection().outline.parse {
            return Err(invalid("generated break original parse identity differs"));
        }
        let wanted = output
            .lifetime()
            .map(|l| {
                crate::expand::scopes::MacroName::parse(
                    l.spelling().trim_start_matches('\''),
                    output.call_edition(),
                )
            })
            .transpose()?;
        let mut ancestor = node.enclosing_syntax_target;
        let mut selected = None;
        for _ in 0..args.body.labels().len().min(256) {
            let Some(id) = ancestor else { break };
            let label = args
                .body
                .labels()
                .get(id.0 as usize)
                .ok_or_else(|| invalid("generated break caller label ancestry absent"))?;
            let matches = if let Some(wanted) = &wanted {
                label.spelling.as_ref().is_some_and(|name| {
                    crate::expand::scopes::MacroName::parse(
                        name.trim_start_matches('\''),
                        output.call_edition(),
                    )
                    .is_ok_and(|name| &name == wanted)
                })
            } else {
                label.is_loop
            };
            if matches {
                selected = Some(label);
                break;
            }
            ancestor = label.parent;
        }
        let label = selected
            .ok_or_else(|| unsupported("generated break has no hygienic caller ancestor target"))?;
        let target_node = args
            .body
            .nodes()
            .get(label.node.0 as usize)
            .ok_or_else(|| invalid("generated break target node absent"))?;
        if output.payload().is_some()
            && matches!(
                target_node.role,
                NodeRole::Expression(ExpressionKind::While | ExpressionKind::For)
            )
        {
            return Err(unsupported(
                "generated break loop target does not accept a value",
            ));
        }
        if wanted.is_some() {
            let range = target_label_range(&parsed, args.handle, args.body, label.node)?;
            if !forest.spans().iter().any(|span| {
                span.start >= range.start && span.end <= range.end && span.context == root.context
            }) {
                return Err(invalid(
                    "generated break original target label context absent",
                ));
            }
        }
        let admission = Self {
            caller: args.pc.clone(),
            span: output.span().clone(),
            source: parsed.key(),
            target: label.id,
            target_node: label.node,
            target_result: target_node
                .value
                .ok_or_else(|| invalid("generated break target value slot absent"))?,
            destination: node
                .value
                .ok_or_else(|| invalid("generated break caller value slot absent"))?,
            resume: target_continuation(&parsed, args.handle, args.body, label.node)?,
            expansion: output.expansion(),
            dependency: output.dependency().clone(),
            captured_lifetime,
            required: {
                required.sort_unstable();
                required.into_boxed_slice()
            },
        };
        admission.require(args.body, output)?;
        Ok(admission)
    }
}

pub(crate) fn prepare_declarative_break(
    args: DeclarativeControlInput<'_>,
) -> Result<PreparedDeclarativeExpression, Diagnostic> {
    let output = decode_break(args.input, args.body.span(NodeId(args.pc.node))?, args.pc)?;
    let bytes = output.allocated_bytes()?;
    let admission = GeneratedBreakAdmission::capture_current(&args, &output)?;
    let mut operations = Vec::new();
    if let Some(literal) = output.payload() {
        operations.push(Op::NumericHint {
            parent: admission.target_result(),
            child: admission.destination(),
            left: None,
            operator: None,
        });
        operations.push(Op::Literal {
            literal: literal.clone(),
            hint: HintRecipe::None,
            destination: admission.destination(),
        });
    } else {
        operations.push(Op::Unit {
            destination: admission.destination(),
        });
    }
    let mut dependencies = args.dependencies;
    if dependencies.len() > 4096 {
        return Err(limit());
    }
    for dependency in &dependencies {
        let DependencyStamp::Input { key, fingerprint } = dependency else {
            return Err(invalid(
                "generated break preparation cannot establish an arbitrary query dependency",
            ));
        };
        if args
            .db
            .published_input(key)
            .is_none_or(|(_, fp)| fp != *fingerprint)
        {
            return Err(invalid("generated break supplied dependency is stale"));
        }
    }
    for dependency in admission.required_dependencies() {
        let DependencyStamp::Input { key, .. } = dependency else {
            return Err(invalid("generated break required dependency kind differs"));
        };
        retain_input(args.db, &mut dependencies, key.clone())?;
    }
    let recipe = crate::ir::validate::verify_generated_break(
        crate::frontend::thunks::with_temporary_scope_entry(
            args.body,
            RecipeDraft {
                key: args.pc.clone(),
                origin: Origin {
                    span: output.span().clone(),
                    expansion_ancestry: vec![output.expansion()],
                },
                operations,
                exits: vec![RecipeExit::Control(ControlRecipe::Break {
                    target: admission.target(),
                    value: Some(admission.destination()),
                    resume: admission.resume().clone(),
                })],
                dependencies,
            },
        )?,
        args.body,
        &output,
        &admission,
    )?;
    let metadata_bytes = bytes
        .checked_add(recipe.metadata_bytes())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PreparedDeclarativeExpression>()))
        .filter(|bytes| *bytes <= 64 * 1024 * 1024)
        .ok_or_else(limit)?;
    let (tokens, origins) = output.into_provenance();
    Ok(PreparedDeclarativeExpression {
        recipe: Box::new(recipe),
        tokens,
        origins,
        metadata_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::compile_input_catalog::CompileInputCatalog,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParsedFile,
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
        let unit = Digest::of_bytes(b"LRR generated break real source admission");
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("generated-break-original.rs").unwrap();
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
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("original source absent")
        };
        let handle = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == function)
            .unwrap()
            .body
            .clone()
            .unwrap();
        let parsed =
            ParsedFile::parse(view.captured_macro_blob().unwrap(), profile.clone()).unwrap();
        let body = parsed.index_body(&handle, Default::default()).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"LRR generated break sealed real world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"LRR generated break declarative environment"),
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
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => panic!("real source discovery incomplete: {diagnostic}; {blocked:?}"),
                _ => panic!("real source discovery requires another authorized producer"),
            }
        }
        assert!(sealed, "actual source discovery work budget");
        discovery.publish(&mut db).unwrap();
        (db, view, handle, body)
    }
    fn inputs<'a>(
        db: &'a Database,
        source: &PreparedSourceView,
        handle: &BodyHandle,
        body: &BodyIndex,
        name: &str,
    ) -> Vec<(PcKey, &'a InputValue)> {
        let text = source.captured_macro_blob().unwrap().text().unwrap();
        let scan = source.declaration_inventory(Default::default()).unwrap();
        let stamps = db.source_view_namespace_stamps().unwrap();
        body.nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
            .filter_map(|node| {
                let range = crate::syntax::spans::SourceRange {
                    start: handle.range.start.checked_add(node.range.start).unwrap(),
                    end: handle.range.start.checked_add(node.range.end).unwrap(),
                };
                // Selection only identifies the requested original test call;
                // admission separately proves its real producer/input/hygiene.
                if !text[range.start as usize..range.end as usize].starts_with(name) {
                    return None;
                }
                let producer = scan
                    .producers()
                    .iter()
                    .find(|producer| {
                        producer.range == range && producer.grammar == GrammarSlot::Statements
                    })
                    .expect("actual original Statements producer missing");
                let record = stamps
                    .keys()
                    .find_map(|key| {
                        let InputKey::ExpansionRecord(_) = key else {
                            return None;
                        };
                        let (input @ InputValue::ExpansionRecord(record), _) =
                            db.published_input(key)?
                        else {
                            return None;
                        };
                        (record.key().invocation == producer.identity
                            && record.key().grammar == GrammarSlot::Statements)
                            .then_some(input)
                    })
                    .expect("actual sealed original Statements expansion record missing");
                Some((body.pc(node.id, Phase::Entry).unwrap(), record))
            })
            .collect()
    }
    fn args<'a>(
        db: &'a Database,
        source: &'a PreparedSourceView,
        handle: &'a BodyHandle,
        body: &'a BodyIndex,
        pc: &'a PcKey,
        input: &'a InputValue,
    ) -> DeclarativeControlInput<'a> {
        DeclarativeControlInput {
            db,
            source,
            handle,
            body,
            pc,
            input,
            dependencies: Vec::new(),
        }
    }
    #[test]
    fn unchanged_mac1_captured_break_targets_keep_real_source_hygiene_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (db, source, handle, body) = world(ORIGINAL, "label_break_macro", edition);
            let calls = inputs(&db, &source, &handle, &body, "mac1!");
            assert_eq!(calls.len(), 2);
            for ((pc, input), wanted) in calls.iter().zip(["'b", "'a"]) {
                let prepared =
                    prepare_declarative_break(args(&db, &source, &handle, &body, pc, input))
                        .unwrap();
                let RecipeExit::Control(ControlRecipe::Break {
                    target,
                    value,
                    resume,
                }) = &prepared.recipe().recipe().exit
                else {
                    panic!("same-frame break exit absent")
                };
                assert_eq!(
                    body.labels()[target.0 as usize].spelling.as_deref(),
                    Some(wanted)
                );
                assert_eq!(*value, body.nodes()[pc.node as usize].value);
                body.pc(NodeId(resume.node), Phase::from_code(resume.phase))
                    .unwrap();
                assert!(
                    prepared.recipe().recipe().operations.iter().any(
                        |op| matches!(op, Op::Literal {literal, ..} if literal.spelling == "1")
                    )
                );
                assert!(
                    prepared
                        .recipe()
                        .recipe()
                        .dependencies
                        .iter()
                        .any(|stamp| matches!(
                            stamp,
                            DependencyStamp::Input {
                                key: InputKey::ExpansionEpoch,
                                ..
                            }
                        ))
                );
                assert!(
                    prepared
                        .recipe()
                        .recipe()
                        .dependencies
                        .iter()
                        .any(|stamp| matches!(
                            stamp,
                            DependencyStamp::Input {
                                key: InputKey::DiscoverySeal(_),
                                ..
                            }
                        ))
                );
                assert!(!prepared.tokens().roots().is_empty());
                assert!(!prepared.origins().is_empty());
            }
        }
    }
    #[test]
    fn genuine_private_break_verifier_rejects_control_dependency_and_hint_tampering() {
        let (db, source, handle, body) = world(ORIGINAL, "label_break_macro", 2024);
        let calls = inputs(&db, &source, &handle, &body, "mac1!");
        let (pc, input) = &calls[0];
        let output = decode_break(input, body.span(NodeId(pc.node)).unwrap(), pc).unwrap();
        let admission = GeneratedBreakAdmission::capture_current(
            &args(&db, &source, &handle, &body, pc, input),
            &output,
        )
        .unwrap();
        let prepared =
            prepare_declarative_break(args(&db, &source, &handle, &body, pc, input)).unwrap();
        let draft = || RecipeDraft {
            key: prepared.recipe().recipe().key.clone(),
            origin: prepared.recipe().recipe().origin.clone(),
            operations: prepared.recipe().recipe().operations.to_vec(),
            exits: vec![prepared.recipe().recipe().exit.clone()],
            dependencies: prepared.recipe().recipe().dependencies.to_vec(),
        };
        assert!(crate::ir::validate::verify(draft(), &body).is_err());
        let mut changed = draft();
        changed.exits = vec![RecipeExit::Control(ControlRecipe::Break {
            target: admission.target(),
            value: Some(admission.destination()),
            resume: pc.clone(),
        })];
        assert!(
            crate::ir::validate::verify_generated_break(changed, &body, &output, &admission)
                .is_err()
        );
        let mut changed = draft();
        changed.dependencies.retain(|d| {
            !matches!(
                d,
                DependencyStamp::Input {
                    key: InputKey::ExpansionEpoch,
                    ..
                }
            )
        });
        assert!(
            crate::ir::validate::verify_generated_break(changed, &body, &output, &admission)
                .is_err()
        );
        let mut changed = draft();
        let hint = changed
            .operations
            .iter_mut()
            .find(|op| matches!(op, Op::NumericHint { .. }))
            .unwrap();
        *hint = Op::NumericHint {
            parent: admission.destination(),
            child: admission.destination(),
            left: None,
            operator: None,
        };
        assert!(
            crate::ir::validate::verify_generated_break(changed, &body, &output, &admission)
                .is_err()
        );
    }
    #[test]
    fn definition_site_label_spelling_cannot_target_the_original_caller_label() {
        let text = "macro_rules! m{()=>{break 'a 1;};}fn main(){let _='a:{m!();0};}";
        let (db, source, handle, body) = world(text, "main", 2024);
        let calls = inputs(&db, &source, &handle, &body, "m!");
        let (pc, input) = &calls[0];
        let error =
            prepare_declarative_break(args(&db, &source, &handle, &body, pc, input)).unwrap_err();
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert!(error.message.contains("captured caller lifetime"));
    }
    #[test]
    fn a_new_publication_epoch_does_not_reuse_old_generated_break_authority() {
        let (mut db, source, handle, body) = world(ORIGINAL, "label_break_macro", 2024);
        let (pc, input) = {
            let calls = inputs(&db, &source, &handle, &body, "mac1!");
            (calls[0].0.clone(), (*calls[0].1).clone())
        };
        db.begin_expansion_epoch(2).unwrap();
        let error =
            prepare_declarative_break(args(&db, &source, &handle, &body, &pc, &input)).unwrap_err();
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    }
}
