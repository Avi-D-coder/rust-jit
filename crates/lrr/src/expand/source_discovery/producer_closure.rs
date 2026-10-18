//! Pure cold projection of actual published declaration expansion receipts.
//! Not registered yet: no namespace/frontier can consume this component alone.
//! A Ready contribution is not a lookup answer or permission to discard headers.
use super::SourceDiscovery;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    expand::{
        declaration_fragment::CompletedDeclarationFragment,
        generated_declarations::GeneratedDeclarations, identity::GrammarSlot,
    },
    frontend::source_body_context::PreparedBodyContext,
    source::manifest::Namespace,
    syntax::{
        declaration_scan::DeclarationProducerKind,
        entered_macros::BodyMacroEventKind,
        outline::{BlockerKind, Outline, ProducerKind},
        spans::SourceRange,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};


pub(crate) enum ProducerClosurePreparation {
    Ready(PreparedProducerContribution),
    /// No generic output exists. A compiler/runtime no-item summary is not one.
    MissingGenericExpansion {
        body_producer: Digest,
        declaration_producer: Digest,
    },
    NeedProducerOccurrence {
        body_producer: Digest,
    },
    /// Items outlines cannot prove absence inside expression/statement output.
    NeedScopedHeaderTraversal {
        expansion: Digest,
        grammar: GrammarSlot,
    },
    NeedRecursiveExpansion {
        expansion: Digest,
        producer: Digest,
    },
    NeedNamespaceBoundary {
        expansion: Digest,
    },
}

/// Empty certifies zero headers for the requested namespace only after checking
/// that no other namespace/header effects need installation either. Headers are
/// actual admitted facts whose names, lexical rebase and import effects STILL
/// need common installation, even if none directly names this namespace.
/// Neither case grants global impl completeness, Drop facts or member selection.
pub(crate) enum NamespaceContribution {
    Empty,
    Headers,
}
pub(crate) struct PreparedOutputContribution {
    output: Arc<GeneratedDeclarations>,
    outline: Arc<Outline>,
}
pub(crate) struct PreparedProducerContribution {
    context: Arc<PreparedBodyContext>,
    fragment: Arc<CompletedDeclarationFragment>,
    body_producer: Digest,
    declaration_producer: Digest,
    lexical_scope: u32,
    invocation: SourceRange,
    namespace: Namespace,
    contribution: NamespaceContribution,
    outputs: Vec<PreparedOutputContribution>,
    seal: QueryKey,
    seal_value: Digest,
    fingerprint: Digest,
    exclusive_bytes: usize,
}
impl PreparedProducerContribution {
    pub(crate) fn body_producer(&self) -> Digest {
        self.body_producer
    }
    pub(crate) fn declaration_producer(&self) -> Digest {
        self.declaration_producer
    }
    pub(crate) fn namespace(&self) -> Namespace {
        self.namespace
    }
    pub(crate) fn lexical_scope(&self) -> u32 {
        self.lexical_scope
    }
    pub(crate) fn invocation(&self) -> SourceRange {
        self.invocation
    }
    pub(crate) fn contribution(&self) -> &NamespaceContribution {
        &self.contribution
    }
    pub(crate) fn outputs(&self) -> &[PreparedOutputContribution] {
        &self.outputs
    }
    pub(crate) fn context(&self) -> &Arc<PreparedBodyContext> {
        &self.context
    }
    pub(crate) fn fragment(&self) -> &Arc<CompletedDeclarationFragment> {
        &self.fragment
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub(crate) fn dependency(&self) -> (&QueryKey, Digest) {
        (&self.seal, self.seal_value)
    }
    /// Cold composition guard. Observing immutable contribution metadata is not
    /// authority to install it after its source or completion lease changes.
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match self.context.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        match db.fetch(&SyntaxProvider, self.seal.clone()) {
            QueryPoll::Ready(Ok(current)) if current.fingerprint == self.seal_value => {
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Err(fault("producer contribution completion lease changed")))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
impl PreparedOutputContribution {
    pub(crate) fn output(&self) -> &Arc<GeneratedDeclarations> {
        &self.output
    }
    pub(crate) fn outline(&self) -> &Arc<Outline> {
        &self.outline
    }
}
impl StableEncode for PreparedProducerContribution {
    const DOMAIN: &'static str = "lrr/v1/body-producer-declaration-contribution";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.context.source().identity());
        e.record(self.context.source().index().body());
        e.digest(self.body_producer);
        e.digest(self.declaration_producer);
        e.u32(self.lexical_scope);
        e.u32(self.invocation.start);
        e.u32(self.invocation.end);
        e.u32(self.namespace.tag());
        e.u8(match self.contribution {
            NamespaceContribution::Empty => 0,
            NamespaceContribution::Headers => 1,
        });
        e.record(&self.seal);
        e.digest(self.seal_value);
        e.sequence(&self.outputs, |e, output| {
            e.digest(output.output.fingerprint());
            e.digest(output.outline.fingerprint());
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PRODUCER-CLOSURE", message.into())
}
fn resource() -> Diagnostic {
    crate::runtime::arena::resource()
}
fn charge(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .ok_or_else(resource)?;
    Ok(())
}
macro_rules! ready {
    ($poll:expr) => {
        match $poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl SourceDiscovery {
    /// Pure query/input reads only. Does not directly parse, expand, publish or
    /// install. Existing pure queries may replay evicted admitted source facts.
    /// Future integration must demand this once cold and retain its actual roots.
    pub(crate) fn prepare_body_producer_contribution(
        &self,
        db: &mut Database,
        context: Arc<PreparedBodyContext>,
        body_producer: Digest,
        namespace: Namespace,
    ) -> QueryPoll<ProducerClosurePreparation> {
        ready!(context.require_current(db));
        let key = QueryKey::DiscoverySeal(self.key.clone());
        let current = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let Some(expected) = self.body_boundary_seal_fingerprint else {
            return QueryPoll::Ready(Err(fault("producer closure discovery not completed")));
        };
        if current.fingerprint != expected
            || !matches!(current.value.as_ref(), QueryValue::DiscoverySeal(seal) if seal.key() == self.key)
        {
            return QueryPoll::Ready(Err(fault("producer closure current seal differs")));
        }
        let input = db.input(&InputKey::DiscoverySeal(self.key.clone()));
        let Some(InputValue::DiscoverySeal(publication)) = input.as_deref() else {
            return QueryPoll::Ready(Err(fault("producer closure publication absent")));
        };
        QueryPoll::Ready((|| {
            let need_occurrence =
                || ProducerClosurePreparation::NeedProducerOccurrence { body_producer };
            let source = context.source();
            let Some(original) = source.original_source() else {
                return Ok(need_occurrence());
            };
            // A flattened retained/generated offset is never an original occurrence.
            // Its structural occurrence transport remains a separate common join.
            if source.parsed().key() != original.selection().outline.parse {
                return Ok(need_occurrence());
            }
            let mut events = context.facts().events.iter().filter(|event| matches!(event.kind,
                BodyMacroEventKind::Producer { identity, kind: ProducerKind::ItemMacro } if identity == body_producer));
            let Some(event) = events.next() else {
                return Ok(need_occurrence());
            };
            if events.next().is_some()
                || !source.body().range.contains(event.range)
                || event.at != u64::from(event.range.start)
            {
                return Err(fault("producer body occurrence differs"));
            }
            let Some(scope) = context
                .facts()
                .scopes
                .iter()
                .find(|scope| scope.ordinal == event.scope)
            else {
                return Err(fault("producer lexical body scope absent"));
            };
            if !scope.range.contains(event.range) {
                return Err(fault("producer escapes lexical body scope"));
            }
            let mut scans = publication
                .declaration_inventory
                .scan_handles()
                .iter()
                .filter(|scan| {
                    scan.parse() == &original.selection().outline.parse
                        && scan.path() == &original.selection().path
                        && scan.scope() == &original.outline().root_scope
                        && scan.cfg() == original.selection().outline.cfg
                });
            let Some(scan) = scans.next() else {
                return Ok(need_occurrence());
            };
            if scans.next().is_some() {
                return Err(fault("producer source scan has duplicate owners"));
            }
            let mut producers = scan.producers().iter().filter(|producer| {
                producer.kind == DeclarationProducerKind::Macro
                    && producer.range == event.range
                    && producer.containers.contains(&source.body().block_range())
            });
            let Some(producer) = producers.next() else {
                return Ok(need_occurrence());
            };
            if producers.next().is_some() {
                return Err(fault("producer source occurrence is ambiguous"));
            }
            let scan_identity = scan.fingerprint();
            let mut fragments = publication.declaration_fragments.iter().filter(|fragment| {
                fragment.source_scan().parse() == scan.parse()
                    && fragment.source_scan().path() == scan.path()
                    && fragment.source_scan().scope() == scan.scope()
                    && fragment.source_scan().cfg() == scan.cfg()
                    && fragment.index().output().parent_invocation() == producer.identity
            });
            let Some(fragment) = fragments.next() else {
                return Ok(ProducerClosurePreparation::MissingGenericExpansion {
                    body_producer,
                    declaration_producer: producer.identity,
                });
            };
            if fragments.next().is_some()
                || !fragment.index().is_complete()
                || fragment.source_scan().fingerprint() != scan_identity
                || fragment.index().output().parent_inventory() != scan_identity
                || fragment.index().output().record().key().grammar != producer.grammar
            {
                return Err(fault("producer completed fragment attachment differs"));
            }
            project(
                db,
                Arc::clone(&context),
                fragment,
                body_producer,
                producer.identity,
                event.scope,
                event.range,
                namespace,
                key,
                current.fingerprint,
                &publication.records,
                self.key.epoch,
            )
        })())
    }
}

#[allow(clippy::too_many_arguments)]
fn project(
    db: &Database,
    context: Arc<PreparedBodyContext>,
    fragment: &Arc<CompletedDeclarationFragment>,
    body_producer: Digest,
    declaration_producer: Digest,
    lexical_scope: u32,
    invocation: SourceRange,
    namespace: Namespace,
    seal: QueryKey,
    seal_value: Digest,
    descriptors: &[crate::expand::record::ExpansionDescriptor],
    epoch: u64,
) -> Result<ProducerClosurePreparation, Diagnostic> {
    // syntax_outputs has its own checked 100,000-output/4,096-frame bound.
    // Preflight that maximum temporary Arc-vector allocation before invoking it.
    let outputs = fragment.syntax_outputs()?;
    if outputs.is_empty() {
        return Err(resource());
    }
    let mut bytes = std::mem::size_of::<PreparedProducerContribution>();
    charge(&mut bytes, seal.heap_bytes())?;
    charge(
        &mut bytes,
        outputs
            .capacity()
            .checked_mul(std::mem::size_of::<Arc<GeneratedDeclarations>>())
            .ok_or_else(resource)?,
    )?;
    let mut children = BTreeMap::new();
    let mut published = BTreeSet::new();
    for descriptor in descriptors {
        charge(&mut bytes, 96)?;
        if !published.insert(descriptor.fingerprint()) {
            return Err(fault("producer descriptor publication has duplicates"));
        }
    }
    let mut work = 0usize;
    for output in &outputs {
        charge(&mut bytes, 160)?;
        if children
            .insert(
                (output.parent_inventory(), output.parent_invocation()),
                output,
            )
            .is_some()
        {
            return Err(fault("producer child output has duplicate owners"));
        }
    }
    let mut projected = Vec::new();
    charge(
        &mut bytes,
        outputs
            .len()
            .checked_mul(std::mem::size_of::<PreparedOutputContribution>())
            .ok_or_else(resource)?,
    )?;
    projected
        .try_reserve_exact(outputs.len())
        .map_err(|_| resource())?;
    charge(
        &mut bytes,
        projected
            .capacity()
            .checked_sub(outputs.len())
            .and_then(|extra| extra.checked_mul(std::mem::size_of::<PreparedOutputContribution>()))
            .ok_or_else(resource)?,
    )?;
    let mut contribution = NamespaceContribution::Empty;
    for output in &outputs {
        let record = output.record();
        charge(
            &mut bytes,
            crate::expand::record::key_heap_bytes(record.key()),
        )?;
        let descriptor = record.descriptor();
        if output.epoch() != epoch
            || record
                .key()
                .procedural_epoch
                .is_some_and(|actual| actual != epoch)
            || !published.contains(&descriptor.fingerprint())
            || !matches!(db.published_input(&InputKey::ExpansionRecord(record.key().fingerprint())),
                Some((InputValue::ExpansionRecord(actual), _)) if actual.same_record(record))
        {
            return Err(fault("producer actual expansion publication differs"));
        }
        let scan = output.scan();
        let scan_identity = scan.fingerprint();
        for producer in scan.producers() {
            work = work
                .checked_add(1)
                .ok_or_else(resource)?;
            // Completed indexes may contain legacy intrinsic summary retirement.
            // This projection demands actual recursive output records instead.
            if !children.contains_key(&(scan_identity, producer.identity)) {
                return Ok(ProducerClosurePreparation::NeedRecursiveExpansion {
                    expansion: record.key().fingerprint(),
                    producer: producer.identity,
                });
            }
        }
        if record.key().grammar != GrammarSlot::Items {
            return Ok(ProducerClosurePreparation::NeedScopedHeaderTraversal {
                expansion: record.key().fingerprint(),
                grammar: record.key().grammar,
            });
        }
        let Some((InputValue::CrateCfg(cfg), _)) =
            db.published_input(&InputKey::CrateCfg(scan.scope().crate_key))
        else {
            return Err(fault("producer output owning cfg absent"));
        };
        if cfg.fingerprint() != scan.cfg() || output.admitted_parsed().key() != *scan.parse() {
            return Err(fault("producer admitted output source/cfg differs"));
        }
        // Conservatively preflight header allocation from actual bounded wire
        // metadata. No display-text parser or body executable plan is invoked.
        let bound = record
            .token_count()
            .checked_mul(1024)
            .and_then(|n| {
                output
                    .text()
                    .len()
                    .checked_mul(512)
                    .and_then(|text| n.checked_add(text))
            })
            .and_then(|n| {
                scan.scope()
                    .heap_bytes()
                    .checked_mul(1024)
                    .and_then(|scope| n.checked_add(scope))
            })
            .ok_or_else(resource)?;
        charge(&mut bytes, scan.scope().heap_bytes())?;
        let outline = output.admitted_parsed().outline(
            scan.scope().crate_key,
            scan.scope().module_path.clone(),
            cfg,
        )?;
        charge(
            &mut bytes,
            outline
                .estimated_bytes()
                .checked_add(2 * std::mem::size_of::<usize>())
                .ok_or_else(resource)?,
        )?;
        if outline.cfg != scan.cfg()
            || outline.content != scan.parse().content
            || outline.profile != scan.parse().profile
        {
            return Err(fault("producer actual output outline differs"));
        }
        if outline.scopes.values().any(|members| {
            members
                .blockers
                .iter()
                .any(|blocker| blocker.kind == BlockerKind::ParseBoundary)
        }) {
            return Ok(ProducerClosurePreparation::NeedNamespaceBoundary {
                expansion: record.key().fingerprint(),
            });
        }
        if !outline.items.is_empty()
            || !outline.item_invocations.is_empty()
            || !outline.imports.is_empty()
            || !outline.extern_crates.is_empty()
            || outline.scopes.values().any(|members| {
                !members.blockers.is_empty()
                    || members
                        .candidates
                        .keys()
                        .any(|(space, _)| *space == namespace)
            })
        {
            contribution = NamespaceContribution::Headers;
        }
        projected.push(PreparedOutputContribution {
            output: Arc::clone(output),
            outline: Arc::new(outline),
        });
    }
    let mut result = PreparedProducerContribution {
        context,
        fragment: Arc::clone(fragment),
        body_producer,
        declaration_producer,
        lexical_scope,
        invocation,
        namespace,
        contribution,
        outputs: projected,
        seal,
        seal_value,
        fingerprint: Digest([0; 32]),
        exclusive_bytes: bytes,
    };
    // Temporary work-map/output Arc capacity is charged conservatively above;
    // root accounting must separately retain the exposed source/fragment roots.
    result.fingerprint = StableEncode::fingerprint(&result);
    Ok(ProducerClosurePreparation::Ready(result))
}
