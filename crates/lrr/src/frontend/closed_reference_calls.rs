//! Closed source reference contracts; caller regions come only from actual frames.
use super::*;
use crate::{
    ir::pc::SlotId,
    runtime::{
        cell::{CellId, GuestStore, ReferenceTarget, StoreSession},
        shape::TypeKey,
        value::ValueRepr,
    },
    syntax::{
        closed_reference_signatures::{ClosedReferenceSyntax, ReferenceHeaderPlan},
        signatures::{FunctionSignature, GenericKind},
    },
    traits::term::RegionTerm,
    vm::frame::Frame,
};
use std::sync::Arc;
#[path = "closed_reference_results.rs"]
mod results;
use results::ReferenceResultContract;
fn invalid(message: &str) -> Diagnostic {
    fault("LRR-E-CALL-CONTEXT", message)
}
fn limit() -> Diagnostic {
    fault(
        "LRR-E-METADATA-LIMIT",
        "closed reference source contract budget",
    )
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DeclaredRegion {
    Parameter(ParamKey),
    Elided { definition: Digest, ordinal: u32 },
}
/// Source-created parameter syntax plus current owning primitive lookup.
#[derive(Debug, Clone)]
pub struct ClosedReferenceParameter {
    syntax: Arc<ClosedReferenceSyntax>,
    plan: Arc<ReferenceHeaderPlan>,
    definition: Digest,
    signature: Digest,
    region: DeclaredRegion,
    source: TyTerm,
    pointee: super::super::source_types::SourceType,
    revision: u64,
    source_revision: u64,
    inputs: BTreeMap<InputKey, Digest>,
}
impl ClosedReferenceParameter {
    pub(super) fn capture(
        db: &mut Database,
        callee: &SourceCallable,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        ordinal: usize,
    ) -> Result<Option<Arc<Self>>, Diagnostic> {
        let signature = callee.signature();
        let plan = reference_header(db, callee)?;
        let Some(syntax) = plan.parameter(ordinal).cloned() else {
            return Ok(None);
        };
        syntax.require(signature)?;
        let pointee = match super::super::source_types::prepare_reference_pointee(
            db,
            graph,
            sources,
            signature,
            syntax.reference(),
        ) {
            QueryPoll::Ready(Ok(super::super::source_types::NamedTypePreparation::Ready(
                pointee,
            ))) => pointee,
            QueryPoll::Ready(Ok(_)) => return Ok(None),
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => {
                return Err(invalid("reference pointee source authority is not current"));
            }
        };
        let inputs = db.lexical_namespace_stamps()?;
        if inputs.len() > 4096 {
            return Err(limit());
        }
        let region = match syntax.lifetime() {
            Some("'static") => {
                return Err(fault(
                    "LRR-E-UNSUPPORTED",
                    "static reference parameter requires an actual static-origin producer",
                ));
            }
            Some("'_") | None => DeclaredRegion::Elided {
                definition: signature.definition,
                ordinal: syntax.ordinal(),
            },
            Some(name) => DeclaredRegion::Parameter(ParamKey {
                definition: signature.definition,
                ordinal: signature
                    .generics
                    .iter()
                    .find(|p| p.kind == GenericKind::Lifetime && p.name == name)
                    .ok_or_else(|| {
                        invalid("reference lifetime is not owned by selected definition")
                    })?
                    .ordinal,
            }),
        };
        let source_region = match region {
            DeclaredRegion::Parameter(key) => RegionTerm::Param(key),
            DeclaredRegion::Elided { ordinal, .. } => RegionTerm::Bound {
                depth: 0,
                index: ordinal,
            },
        };
        let source = TyTerm::Ref(
            source_region,
            syntax.mutable(),
            Box::new(pointee.source().clone()),
        );
        let proof = Self {
            syntax,
            plan,
            definition: signature.definition,
            signature: signature.fingerprint(),
            region,
            source,
            pointee,
            revision: db.revision(),
            source_revision: db.source_revision(),
            inputs,
        };
        if proof.allocated_bytes()? > 32 * 1024 * 1024 {
            return Err(limit());
        }
        Ok(Some(Arc::new(proof)))
    }
    pub fn pointee_source(&self) -> &TyTerm {
        self.pointee.source()
    }
    pub fn ordinal(&self) -> u32 {
        self.syntax.ordinal()
    }
    pub fn mutable(&self) -> bool {
        self.syntax.mutable()
    }
    pub(super) fn source_template(&self) -> &TyTerm {
        &self.source
    }
    pub fn require_current(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
    ) -> Result<(), Diagnostic> {
        callee.revalidate(db)?;
        self.syntax.require(callee.signature())?;
        self.plan.require(callee.signature(), self.syntax.parse())?;
        if self.definition != callee.signature().definition
            || self.signature != callee.signature().fingerprint()
            || self.revision != db.revision()
            || self.source_revision != db.source_revision()
            || db.lexical_namespace_stamps()? != self.inputs
        {
            return Err(invalid("reference parameter source world changed"));
        }
        Ok(())
    }
    pub fn validate_hint(&self, hint: &ArgumentHint) -> Result<(), Diagnostic> {
        if hint.source != self.source
            || hint.descriptor.is_some()
            || hint
                .closed_reference
                .as_ref()
                .is_none_or(|proof| !std::ptr::eq(proof.as_ref(), self))
        {
            return Err(invalid(
                "reference hint was altered or copied from another source proof",
            ));
        }
        Ok(())
    }
    /// Descriptor only: the caller slot must retain its actual borrow region.
    pub fn materialize_hint(&self, store: &mut GuestStore) -> Result<TypeId, Diagnostic> {
        let target = self.pointee.materialize(store)?;
        store.intern_type(TypeKey::Ref {
            mutable: self.syntax.mutable(),
            target,
        })
    }
    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.source.heap_bytes().ok_or_else(limit)?)
            .and_then(|n| {
                self.pointee
                    .heap_bytes()
                    .ok()
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(limit)?;
        for key in self.inputs.keys() {
            bytes = bytes
                .checked_add(128)
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .ok_or_else(limit)?;
        }
        Ok(bytes)
    }
}
/// Immutable source preparation is query-owned; Type and caller authority are
/// admitted separately after this lookup. A retained proof pins its whole plan.
fn reference_header(
    db: &mut Database,
    callee: &SourceCallable,
) -> Result<Arc<ReferenceHeaderPlan>, Diagnostic> {
    callee.revalidate(db)?;
    let signature = callee.signature();
    let plan = match db.fetch(
        &SyntaxProvider,
        QueryKey::ReferenceHeader(signature.definition),
    ) {
        QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
            crate::db::value::QueryValue::ReferenceHeader(plan) => Arc::clone(plan),
            crate::db::value::QueryValue::Error(error) => return Err(error.clone()),
            _ => return Err(invalid("reference header query variant differs")),
        },
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(invalid("reference header source input absent")),
    };
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(signature.definition))
    else {
        return Err(invalid("reference definition source absent"));
    };
    plan.require(signature, parse)?;
    Ok(plan)
}
/// The source term is read from an admitted frame slot, never supplied by caller.
#[derive(Debug, Clone)]
pub(crate) struct ClosedReferenceArgument {
    session: StoreSession,
    cell: CellId,
    value: ValueRepr,
    source: TyTerm,
    target: ReferenceTarget,
}
impl ClosedReferenceArgument {
    #[cfg(test)]
    pub(crate) fn capture(
        frame: &Frame,
        slot: SlotId,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        Self::capture_budgeted(frame, slot, store, 32 * 1024 * 1024)
    }
    pub(crate) fn capture_budgeted(
        frame: &Frame,
        slot: SlotId,
        store: &GuestStore,
        max_bytes: usize,
    ) -> Result<Self, Diagnostic> {
        frame.require_store(store)?;
        let source = frame
            .source_view_ref(slot)?
            .ok_or_else(|| invalid("reference argument has no retained caller source identity"))?;
        let TyTerm::Ref(region, _, target) = source else {
            return Err(invalid(
                "reference argument source is not an admitted reference",
            ));
        };
        if !matches!(target.as_ref(), TyTerm::Primitive(_))
            && !matches!(target.as_ref(), TyTerm::Nominal(_, args) if args.is_empty())
            || matches!(
                region,
                RegionTerm::Placeholder(_) | RegionTerm::Bound { .. }
            )
        {
            return Err(fault(
                "LRR-E-UNSUPPORTED",
                "reference caller pointee/region requires further source integration",
            ));
        }
        if source.heap_bytes().is_none_or(|n| n > 128 * 1024) {
            return Err(limit());
        }
        let cell = frame.cell(slot)?;
        let value = store.admit_move(cell)?;
        validate_value(store, cell, source, value)?;
        let target_bytes = store.reference_target_owned_bytes(cell)?;
        if std::mem::size_of::<Self>()
            .checked_add(target_bytes)
            .and_then(|n| source.heap_bytes().and_then(|m| n.checked_add(m)))
            .is_none_or(|bytes| bytes > max_bytes)
        {
            return Err(limit());
        }
        let proof = Self {
            session: store.pattern_binding_session(),
            cell,
            value,
            source: source.clone(),
            target: store.reference_target(cell)?,
        };
        if proof.allocated_bytes()? > max_bytes {
            return Err(limit());
        }
        Ok(proof)
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.target.owned_bytes()?)
            .and_then(|n| self.source.heap_bytes().and_then(|m| n.checked_add(m)))
            .ok_or_else(limit)
    }
    fn require(&self, store: &GuestStore, actual: &ArgumentType) -> Result<(), Diagnostic> {
        if self.session != store.pattern_binding_session()
            || self.source != actual.source
            || self.value.type_id != actual.descriptor.0
            || store.admit_move(self.cell)? != self.value
        {
            return Err(invalid(
                "reference actual caller cell/session/source changed",
            ));
        }
        validate_value(store, self.cell, &self.source, self.value)
    }
}
fn validate_value(
    store: &GuestStore,
    cell: CellId,
    source: &TyTerm,
    value: ValueRepr,
) -> Result<(), Diagnostic> {
    let TyTerm::Ref(_, mutable, target) = source else {
        return Err(invalid("reference source shape differs"));
    };
    store.reference_from_cell(cell)?; // actual lease/owner/access/target validation
    let descriptor = store.type_descriptor(TypeId(value.type_id))?;
    let TypeKey::Ref {
        mutable: actual,
        target: descriptor_target,
    } = descriptor.key.as_ref()
    else {
        return Err(invalid("reference value descriptor shape differs"));
    };
    if value.kind != 9 || actual != mutable {
        return Err(invalid(
            "reference source/access and actual descriptor disagree",
        ));
    }
    // Validate already admitted caller source identity. The callee's SourceType
    // proof separately checks the exact current nominal schema before admission.
    let pointee = store.type_descriptor(*descriptor_target)?;
    let matches = match (target.as_ref(), pointee.key.as_ref()) {
        (TyTerm::Primitive(primitive), TypeKey::Primitive(actual)) => {
            primitive_runtime_id(*primitive)? == actual.id()
        }
        (
            TyTerm::Nominal(identity, args),
            TypeKey::Nominal {
                definition,
                args: runtime_args,
                const_args,
                ..
            },
        ) => {
            args.is_empty()
                && runtime_args.is_empty()
                && const_args.is_empty()
                && definition.fingerprint() == *identity
        }
        _ => false,
    };
    if !matches {
        return Err(invalid(
            "reference caller source pointee differs from actual descriptor",
        ));
    }

    Ok(())
}
#[derive(Debug, Clone)]
struct ReferenceBinding {
    ordinal: usize,
    parameter: Arc<ClosedReferenceParameter>,
    argument: ClosedReferenceArgument,
}
/// Only a checked reached source+cell join can mint this public opaque type.
#[derive(Debug, Clone)]
pub struct ClosedReferenceContracts {
    definition: Digest,
    signature: Digest,
    bindings: Box<[ReferenceBinding]>,
    result: Option<ReferenceResultContract>,
}
impl ClosedReferenceContracts {
    fn require_store_session(&self, store: &GuestStore) -> Result<(), Diagnostic> {
        if self.bindings.is_empty()
            || self
                .bindings
                .iter()
                .any(|binding| binding.argument.session != store.pattern_binding_session())
        {
            return Err(invalid(
                "reference source certificate belongs to another store session",
            ));
        }
        Ok(())
    }
    pub(crate) fn parameter_source(&self, ordinal: usize) -> Option<&TyTerm> {
        self.bindings
            .iter()
            .find(|binding| binding.ordinal == ordinal)
            .map(|binding| &binding.argument.source)
    }
    pub(crate) fn require_parameter_descriptor(
        &self,
        ordinal: usize,
        store: &GuestStore,
        descriptor: TypeId,
    ) -> Result<(), Diagnostic> {
        self.require_store_session(store)?;
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.ordinal == ordinal)
            .ok_or_else(|| invalid("reference parameter source certificate absent"))?;
        let TypeKey::Ref { mutable, target } = store.type_descriptor(descriptor)?.key.as_ref()
        else {
            return Err(invalid("reference parameter descriptor shape differs"));
        };
        if *mutable != binding.parameter.mutable() {
            return Err(invalid("reference parameter descriptor access differs"));
        }
        binding.parameter.pointee.require_descriptor(store, *target)
    }
    pub(super) fn admits(&self, ordinal: usize) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.ordinal == ordinal)
    }
    pub fn validate_packet(
        &self,
        signature: &FunctionSignature,
        packet: &PreparedCallContracts,
    ) -> Result<(), Diagnostic> {
        if signature.parameters.len() > 100_000 || self.bindings.len() > signature.parameters.len()
        {
            return Err(limit());
        }
        let signature_identity = signature.fingerprint();
        if signature.definition != self.definition
            || signature_identity != self.signature
            || packet.definition != self.definition
            || packet.source_parameters.len() != signature.parameters.len()
            || packet.parameter_descriptors.len() != signature.parameters.len()
            || self.bindings.is_empty()
        {
            return Err(invalid(
                "reference packet definition/signature/arity differs",
            ));
        }
        // Exact ordered coverage: immutable permission for one selected Ref
        // parameter cannot authorize another public normalized table entry.
        let mut bindings = self.bindings.iter().peekable();
        for (ordinal, source) in packet.source_parameters.iter().enumerate() {
            let Some(binding) = bindings.peek() else {
                if matches!(source, TyTerm::Ref(..)) {
                    return Err(invalid(
                        "normalized reference parameter lacks its source certificate",
                    ));
                }
                continue;
            };
            if binding.ordinal < ordinal {
                return Err(invalid(
                    "reference certificate ordinals are duplicate or unordered",
                ));
            }
            if binding.ordinal != ordinal {
                if matches!(source, TyTerm::Ref(..)) {
                    return Err(invalid(
                        "normalized reference parameter lacks its source certificate",
                    ));
                }
                continue;
            }
            let parameter = &binding.parameter;
            // Capture already tied the private syntax proof to this fingerprint.
            // Repeating full-header encoding per binding is unnecessary work.
            if parameter.definition != signature.definition
                || parameter.signature != signature_identity
                || parameter.syntax.ordinal() as usize != ordinal
                || signature.parameters[ordinal].receiver
                || signature.parameters[ordinal].variadic
                || !matches!(source,TyTerm::Ref(_,mutable,target)
                    if *mutable==parameter.mutable() && target.as_ref()==parameter.pointee_source())
                || source != &binding.argument.source
                || packet.parameter_descriptors[ordinal] != TypeId(binding.argument.value.type_id)
            {
                return Err(invalid("reference packet parameter table was altered"));
            }
            bindings.next();
        }
        if bindings.next().is_some() {
            return Err(invalid(
                "reference certificate ordinal is outside the parameter table",
            ));
        }
        if let Some(result) = &self.result {
            result.validate_packet(signature, packet, &self.bindings)?;
        }
        Ok(())
    }
    pub fn require_current(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
    ) -> Result<(), Diagnostic> {
        if self.definition != callee.signature().definition
            || self.signature != callee.signature().fingerprint()
        {
            return Err(invalid("reference contracts copied to another callable"));
        }
        for binding in &self.bindings {
            binding.parameter.require_current(db, callee)?;
        }
        if let Some(result) = &self.result {
            result.require_signature(callee.signature())?;
        }
        Ok(())
    }
    /// Must run against actual physical call cells before ArgumentPacket::seal.
    pub(crate) fn require_argument_cells(
        &self,
        store: &GuestStore,
        cells: &[CellId],
        arguments: &[ArgumentType],
    ) -> Result<(), Diagnostic> {
        for binding in &self.bindings {
            if cells.get(binding.ordinal) != Some(&binding.argument.cell) {
                return Err(invalid("reference physical argument cell differs"));
            }
            binding.argument.require(
                store,
                arguments
                    .get(binding.ordinal)
                    .ok_or_else(|| invalid("reference physical argument absent"))?,
            )?;
        }
        Ok(())
    }
    pub fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.bindings
                    .len()
                    .checked_mul(std::mem::size_of::<ReferenceBinding>())
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        for binding in &self.bindings {
            let target_bytes = binding.argument.target.owned_bytes()?;
            bytes = bytes
                .checked_add(binding.parameter.allocated_bytes()?)
                .and_then(|n| {
                    binding
                        .argument
                        .source
                        .heap_bytes()
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| n.checked_add(target_bytes))
                .ok_or_else(limit)?;
        }
        if let Some(result) = &self.result {
            bytes = bytes
                .checked_add(result.allocated_bytes()?)
                .ok_or_else(limit)?;
        }
        Ok(bytes)
    }
}
pub(super) fn bind_references(
    db: &mut Database,
    callee: &SourceCallable,
    namespace: (&NameNamespaces, &[&PreparedSourceView]),
    hints: &mut [ArgumentHint],
    store: &GuestStore,
    actuals: &[ArgumentType],
    references: &[Option<ClosedReferenceArgument>],
) -> Result<Option<Box<ClosedReferenceContracts>>, Diagnostic> {
    let (graph, sources) = namespace;
    if hints.len() != actuals.len() || references.len() != actuals.len() {
        return Err(invalid("reference evidence arity differs"));
    }
    let mut retained = std::mem::size_of::<ClosedReferenceContracts>()
        .checked_add(
            hints
                .len()
                .checked_mul(std::mem::size_of::<ReferenceBinding>())
                .ok_or_else(limit)?,
        )
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(limit)?;
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(hints.len())
        .map_err(|_| limit())?;
    for (ordinal, (hint, actual)) in hints.iter_mut().zip(actuals).enumerate() {
        let Some(parameter) = hint.closed_reference.as_ref() else {
            if references[ordinal].is_some() {
                return Err(invalid(
                    "reference evidence supplied for a different source parameter",
                ));
            }
            continue;
        };
        parameter.validate_hint(hint)?;
        parameter.require_current(db, callee)?;
        let argument = references[ordinal].as_ref().ok_or_else(|| {
            invalid("reference source contract requires actual owning frame evidence")
        })?;
        argument.require(store, actual)?;
        let TyTerm::Ref(region, mutable, target) = &argument.source else {
            return Err(invalid("reference actual source absent"));
        };
        if *mutable != parameter.syntax.mutable() || target.as_ref() != parameter.pointee_source() {
            return Err(fault(
                "LRR-E-CALL-ARG-TYPE",
                "reference argument source differs from selected source pointee/access",
            ));
        }
        let TypeKey::Ref { target, .. } = store.type_descriptor(actual.descriptor)?.key.as_ref()
        else {
            return Err(invalid("reference argument descriptor shape differs"));
        };
        parameter.pointee.require_descriptor(store, *target)?;
        for previous in &bindings {
            let previous: &ReferenceBinding = previous;
            if parameter.region == previous.parameter.region {
                let TyTerm::Ref(previous_region, _, _) = &previous.argument.source else {
                    return Err(invalid("reference previous source absent"));
                };
                if previous_region != region {
                    return Err(fault(
                        "LRR-E-UNSUPPORTED",
                        "shared declared lifetime needs an unsupported region relation",
                    ));
                }
            }
        }
        let target_bytes = argument.target.owned_bytes()?;
        let parameter_bytes = parameter.allocated_bytes()?;
        retained = retained
            .checked_add(target_bytes)
            .and_then(|n| n.checked_add(parameter_bytes))
            .and_then(|n| argument.source.heap_bytes().and_then(|m| n.checked_add(m)))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(limit)?;
        bindings.push(ReferenceBinding {
            ordinal,
            parameter: parameter.clone(),
            argument: argument.clone(),
        });
        hint.source = argument.source.clone();
    }
    let result = ReferenceResultContract::capture(db, callee, graph, sources, &bindings, store)?;
    if bindings.is_empty() {
        return Ok(None);
    }
    let proof = ClosedReferenceContracts {
        definition: callee.signature().definition,
        signature: callee.signature().fingerprint(),
        bindings: bindings.into_boxed_slice(),
        result,
    };
    if proof.allocated_bytes()? > 32 * 1024 * 1024 {
        return Err(limit());
    }
    Ok(Some(Box::new(proof)))
}

/// Already evaluated actual caller metadata and opaque frame captures.
pub(crate) struct ReferenceCallArguments<'a> {
    pub(crate) store: &'a GuestStore,
    pub(crate) arguments: &'a [ArgumentType],
    pub(crate) references: &'a [Option<ClosedReferenceArgument>],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        frontend::callables::{CallResolver, CallSelection},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
    );
    pub(super) fn source(
        text: &str,
        edition: u16,
    ) -> (Database, PreparedSourceView, NameNamespaces) {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let unit = SourceUnitInput {
            path: SourcePath::new("regions-infer-call.rs").unwrap(),
            crate_key: Digest([181; 32]),
            module_path: vec![],
            profile: profile.clone(),
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        };
        let selection = SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile,
                },
                crate_key: unit.crate_key,
                module_path: vec![],
                cfg: unit.cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh_with_dependencies(
            None,
            vec![unit],
            Default::default(),
            vec![],
        )
        .unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
            panic!()
        };
        let mut graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
        let imports = graph.resolve_imports().unwrap();
        assert!(imports.pending.is_empty() && imports.missing.is_empty());
        (db, source, graph)
    }
    pub(super) fn callable(
        db: &mut Database,
        source: &PreparedSourceView,
        graph: &NameNamespaces,
        name: &str,
    ) -> Box<SourceCallable> {
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .def_key
            .fingerprint();
        let views = [source];
        let resolver = CallResolver::new(graph, &views).unwrap();
        let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
            resolver.module_function(db, main, name)
        else {
            panic!("genuine source callable unresolved")
        };
        callee
    }
    #[test]
    fn unchanged_reference_headers_keep_distinct_named_and_elided_region_owners_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, source, graph) = source(ORIGINAL, edition);
            for name in ["takes_two", "has_two"] {
                let callee = callable(&mut db, &source, &graph, name);
                for ordinal in 0..2 {
                    let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) =
                        prepare_argument_hint(&mut db, &callee, &graph, &[&source], ordinal)
                    else {
                        panic!()
                    };
                    let [hint] = hints.as_slice() else { panic!() };
                    let proof = hint.closed_reference.as_ref().unwrap();
                    proof.require_current(&mut db, &callee).unwrap();
                    proof.validate_hint(hint).unwrap();
                    let TyTerm::Ref(region, false, target) = &hint.source else {
                        panic!()
                    };
                    assert_eq!(target.as_ref(), &TyTerm::Primitive(PrimitiveType::Isize));
                    match (name, region) {
                        ("has_two", RegionTerm::Param(key)) => {
                            assert_eq!(key.definition, callee.signature().definition);
                            assert_eq!(key.ordinal, ordinal as u32);
                        }
                        ("takes_two", RegionTerm::Bound { depth: 0, index }) => {
                            assert_eq!(*index, ordinal as u32)
                        }
                        _ => panic!("source-owned lifetime context changed"),
                    }
                    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
                    let descriptor = proof.materialize_hint(&mut store).unwrap();
                    assert_eq!(
                        store.type_descriptor(descriptor).unwrap().key.as_ref(),
                        &TypeKey::Ref {
                            mutable: false,
                            target: primitive_runtime_id(PrimitiveType::Isize).unwrap()
                        }
                    );
                    assert!(hint.descriptor.is_none());
                }
            }
        }
        // Header component only. Whole original main/deref/add/assert awaits
        // root's actual engine+GenericEnv transport integration and execution.
    }
    #[test]
    fn reference_hint_cannot_use_primitive_spelling_after_current_type_shadow() {
        let (mut db, source, graph) =
            source("type isize=bool;fn take(x:&isize){}fn main(){}", 2024);
        let callee = callable(&mut db, &source, &graph, "take");
        let QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedType { .. })) =
            prepare_argument_hint(&mut db, &callee, &graph, &[&source], 0)
        else {
            panic!("shadowed source name cannot mint primitive reference")
        };
    }
    #[test]
    fn genuine_reference_hint_rejects_public_hint_tamper_and_epoch_change() {
        let (mut db, source, graph) = source(ORIGINAL, 2024);
        let callee = callable(&mut db, &source, &graph, "takes_two");
        let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) =
            prepare_argument_hint(&mut db, &callee, &graph, &[&source], 0)
        else {
            panic!()
        };
        let proof = hints[0].closed_reference.as_ref().unwrap();
        let mut forged = hints[0].clone();
        forged.source = TyTerm::Ref(
            RegionTerm::Static,
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Isize)),
        );
        assert_eq!(
            proof.validate_hint(&forged).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
        db.begin_expansion_epoch(1).unwrap();
        assert_eq!(
            proof.require_current(&mut db, &callee).unwrap_err().code,
            "LRR-E-CALL-CONTEXT"
        );
    }
    /// Genuine original caller source and ordinary verified borrow execution;
    /// no private reference/source/certificate field is fabricated for positives.
    struct Coverage {
        db: Database,
        source: PreparedSourceView,
        graph: NameNamespaces,
        callee: Box<SourceCallable>,
        frame: Frame,
        store: GuestStore,
        slot: SlotId,
    }
    fn coverage_world() -> Coverage {
        coverage_source("fn take(x:&bool,y:bool){}fn main(){&true;}")
    }
    fn coverage_source(text: &str) -> Coverage {
        use crate::{
            db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
            ir::{
                op::Op,
                pc::{NodeId, Phase},
            },
            vm::step::{StepOutcome, step},
        };
        let (mut db, source, graph) = source(text, 2024);
        let callee = callable(&mut db, &source, &graph, "take");
        let body = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let indexed = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
            panic!()
        };
        let mut store = GuestStore::new(1, 10_000, 4_000_000).unwrap();
        let mut frame = Frame::new(
            index.as_ref().clone(),
            index.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame.set_fuel_limit(100_000).unwrap();
        for _ in 0..128 {
            let fetched = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
                panic!()
            };
            let reference = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow { destination, .. } => Some(*destination),
                _ => None,
            });
            assert!(matches!(
                step(&mut frame, &mut store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(slot) = reference {
                let captured = ClosedReferenceArgument::capture(&frame, slot, &store).unwrap();
                assert!(
                    matches!(captured.source,TyTerm::Ref(RegionTerm::Local{..},false,ref target)
                    if target.as_ref()==&TyTerm::Primitive(PrimitiveType::Bool))
                );
                return Coverage {
                    db,
                    source,
                    graph,
                    callee,
                    frame,
                    store,
                    slot,
                };
            }
        }
        panic!("ordinary actual source did not reach the borrow")
    }
    fn coverage_packet(c: &mut Coverage) -> PreparedCallContracts {
        let reference = ClosedReferenceArgument::capture(&c.frame, c.slot, &c.store).unwrap();
        let actuals = [
            ArgumentType {
                source: reference.source.clone(),
                descriptor: TypeId(reference.value.type_id),
            },
            ArgumentType {
                source: TyTerm::Primitive(PrimitiveType::Bool),
                descriptor: primitive_runtime_id(PrimitiveType::Bool).unwrap(),
            },
        ];
        let references = [Some(reference), None];
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
            prepare_contracts_with_references(
                &mut c.db,
                &c.callee,
                &c.graph,
                &[&c.source],
                &[],
                None,
                ReferenceCallArguments {
                    store: &c.store,
                    arguments: &actuals,
                    references: &references,
                },
            )
        else {
            panic!("actual source reference producer failed")
        };
        packet
    }
    #[test]
    fn copied_reference_certificate_cannot_authorize_an_extra_normalized_ref_slot() {
        let mut c = coverage_world();
        let mut packet = coverage_packet(&mut c);
        let certificate = packet.closed_references.as_ref().unwrap().clone();
        certificate
            .validate_packet(c.callee.signature(), &packet)
            .unwrap();
        certificate.require_current(&mut c.db, &c.callee).unwrap();
        packet.source_parameters[1] = packet.source_parameters[0].clone();
        packet.parameter_descriptors[1] = packet.parameter_descriptors[0];
        assert_eq!(
            certificate
                .validate_packet(c.callee.signature(), &packet)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        // Descriptor agreement of this forged extra slot grants no source fact.
    }
    #[test]
    fn reference_coverage_rejects_hidden_removed_and_mismatched_bound_slots() {
        let mut c = coverage_world();
        let mut packet = coverage_packet(&mut c);
        let certificate = packet.closed_references.as_ref().unwrap().clone();
        let original = packet.source_parameters[0].clone();
        packet.source_parameters[0] = TyTerm::Primitive(PrimitiveType::Bool);
        assert!(
            certificate
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        packet.source_parameters[0] = TyTerm::Ref(
            RegionTerm::Static,
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Bool)),
        );
        assert_eq!(
            certificate
                .validate_packet(c.callee.signature(), &packet)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        packet.source_parameters[0] = original;
        let descriptor = packet.parameter_descriptors[0];
        packet.parameter_descriptors[0] = primitive_runtime_id(PrimitiveType::Bool).unwrap();
        assert!(
            certificate
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        packet.parameter_descriptors[0] = descriptor;
        certificate
            .validate_packet(c.callee.signature(), &packet)
            .unwrap();
        let mut removed = certificate.as_ref().clone();
        removed.bindings = Box::new([]);
        assert!(
            removed
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        let mut out_of_range = certificate.as_ref().clone();
        out_of_range.bindings[0].ordinal = packet.source_parameters.len();
        assert!(
            out_of_range
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        let mut duplicate = certificate.as_ref().clone();
        duplicate.bindings =
            vec![duplicate.bindings[0].clone(), duplicate.bindings[0].clone()].into_boxed_slice();
        assert!(
            duplicate
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        let mut different = c.callee.signature().clone();
        different.definition = Digest([87; 32]);
        assert!(certificate.validate_packet(&different, &packet).is_err());
    }
    #[test]
    fn reference_result_source_certificate_rejects_public_packet_tampering() {
        let mut c = coverage_source("fn take(x:&bool,y:bool)->&bool{x}fn main(){&true;}");
        let mut packet = coverage_packet(&mut c);
        let proof = packet.closed_references.as_ref().unwrap().clone();
        assert!(proof.has_result());
        proof
            .validate_packet(c.callee.signature(), &packet)
            .unwrap();
        crate::vm::context::GenericEnv::checked_bytes(c.callee.signature(), &packet).unwrap();
        let original = packet.source_result.clone();
        packet.source_result = Some(TyTerm::Ref(
            RegionTerm::Static,
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Bool)),
        ));
        assert!(
            proof
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
        assert!(
            crate::vm::context::GenericEnv::checked_bytes(c.callee.signature(), &packet).is_err()
        );
        packet.source_result = original;
        packet.result_descriptor = Some(TypeId(2));
        assert!(
            proof
                .validate_packet(c.callee.signature(), &packet)
                .is_err()
        );
    }
    #[test]
    fn reference_result_return_admission_checks_current_source_and_actual_provenance() {
        let mut c = coverage_source("fn take(x:&bool,y:bool)->&bool{x}fn main(){&true;}");
        let mut packet = coverage_packet(&mut c);
        let cell = c.frame.cell(c.slot).unwrap();
        let ticket = c.store.copy_from(cell).unwrap();
        let actual_source = c.frame.source_view(c.slot).unwrap().unwrap();
        packet
            .admit_reference_return(&mut c.db, &c.callee, &c.store, ticket, Some(&actual_source))
            .unwrap();
        packet
            .closed_references
            .as_ref()
            .unwrap()
            .recheck_result_ticket(&c.store, ticket)
            .unwrap();
        let value = c.store.peek_ticket(ticket).unwrap();
        let forged = TyTerm::Ref(
            RegionTerm::Static,
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Bool)),
        );
        assert!(
            packet
                .admit_reference_return(&mut c.db, &c.callee, &c.store, ticket, Some(&forged))
                .is_err()
        );
        assert_eq!(c.store.peek_ticket(ticket).unwrap(), value);
        c.db.begin_expansion_epoch(1).unwrap();
        assert!(
            packet
                .admit_reference_return(
                    &mut c.db,
                    &c.callee,
                    &c.store,
                    ticket,
                    Some(&actual_source)
                )
                .is_err()
        );
        assert_eq!(c.store.peek_ticket(ticket).unwrap(), value);
    }
    #[test]
    fn prepared_reference_result_hint_requires_its_actual_definition_and_root_entry() {
        use crate::{
            db::value::QueryValue,
            ir::pc::{NodeId, Phase},
            vm::context::GenericEnv,
        };
        let mut c = coverage_source("fn take(x:&bool,y:bool)->&bool{x}fn main(){&true;}");
        let packet = coverage_packet(&mut c);
        let context = GenericEnv::capture_current(&mut c.db, &c.callee, &packet).unwrap();
        let fetched =
            c.db.fetch(
                &SyntaxProvider,
                QueryKey::BodyStructure(c.callee.body().unwrap().key()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!()
        };
        let mut frame = Frame::new(
            index.clone(),
            index.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut c.store,
        )
        .unwrap();
        assert!(frame.definition_result_source().is_err());
        frame.install_context(context).unwrap();
        assert_eq!(
            Some(frame.definition_result_source().unwrap()),
            packet.source_result
        );
        assert!(
            frame
                .definition_result_descriptor(&c.store)
                .unwrap()
                .is_some()
        );
        let mut foreign = GuestStore::new(2, 1000, 1_000_000).unwrap();
        let before = (
            foreign.budget().handles(),
            foreign.budget().bytes(),
            foreign.type_metadata_bytes(),
        );
        assert!(frame.definition_result_descriptor(&foreign).is_err());
        assert!(
            frame
                .materialize_definition_result_descriptor(&mut foreign)
                .is_err()
        );
        assert_eq!(
            before,
            (
                foreign.budget().handles(),
                foreign.budget().bytes(),
                foreign.type_metadata_bytes()
            )
        );
        let counters = frame.counters.operations_started;
        frame.pc = index.pc(NodeId(0), Phase::Commit).unwrap();
        assert!(frame.definition_result_source().is_err());
        assert_eq!(frame.counters.operations_started, counters);
        let context = GenericEnv::capture_current(&mut c.db, &c.callee, &packet).unwrap();
        assert!(c.frame.install_context(context).is_err());
    }
    #[test]
    fn reference_target_evidence_budget_refuses_before_any_owner_or_counter_change() {
        let c = coverage_world();
        let before = (
            c.store.budget().handles(),
            c.store.budget().bytes(),
            c.store.admit_move(c.frame.cell(c.slot).unwrap()).unwrap(),
            c.frame.counters.operations_started,
        );
        assert!(ClosedReferenceArgument::capture_budgeted(&c.frame, c.slot, &c.store, 0).is_err());
        assert_eq!(
            before,
            (
                c.store.budget().handles(),
                c.store.budget().bytes(),
                c.store.admit_move(c.frame.cell(c.slot).unwrap()).unwrap(),
                c.frame.counters.operations_started
            )
        );
    }
    #[test]
    fn shared_header_plan_does_not_share_actual_caller_regions_or_target_receipts() {
        use crate::{
            db::value::QueryValue,
            ir::op::Op,
            vm::step::{StepOutcome, step},
        };
        let mut c = coverage_source("fn take(x:&bool,y:bool)->&bool{x}fn main(){&true;&false;}");
        let first = coverage_packet(&mut c);
        let first_plan = Arc::clone(
            &first.closed_references.as_ref().unwrap().bindings[0]
                .parameter
                .plan,
        );
        // Reach the second ordinary Borrow through the same VM continuation;
        // no source/region or ticket is installed directly by this control.
        let mut reached = false;
        for _ in 0..128 {
            let outcome =
                c.db.fetch(&SyntaxProvider, QueryKey::Thunk(c.frame.pc().clone()))
                    .into_ready()
                    .unwrap()
                    .unwrap();
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!()
            };
            let borrow = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow { destination, .. } => Some(*destination),
                _ => None,
            });
            assert!(matches!(
                step(&mut c.frame, &mut c.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(slot) = borrow {
                c.slot = slot;
                reached = true;
                break;
            }
        }
        assert!(reached);
        let second = coverage_packet(&mut c);
        let second_proof = second.closed_references.as_ref().unwrap();
        assert!(Arc::ptr_eq(
            &first_plan,
            &second_proof.bindings[0].parameter.plan
        ));
        assert_ne!(first.source_result, second.source_result);
        assert_eq!(
            c.db.trace.counts.get("ReferenceHeader:ProviderRun"),
            Some(&1)
        );
        let ticket = c.store.copy_from(c.frame.cell(c.slot).unwrap()).unwrap();
        second_proof
            .recheck_result_ticket(&c.store, ticket)
            .unwrap();
        assert!(
            first
                .closed_references
                .as_ref()
                .unwrap()
                .recheck_result_ticket(&c.store, ticket)
                .is_err()
        );
    }
}

#[cfg(test)]
#[path = "named_reference_source_tests.rs"]
mod named_reference_source_tests;
