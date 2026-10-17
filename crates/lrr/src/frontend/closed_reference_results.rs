//! Declared reference-result linkage and actual ticket authority; no evaluation.
use super::*;
use crate::{
    runtime::cell::TicketId, syntax::closed_reference_signatures::ClosedReferenceTypeSyntax,
};

#[derive(Debug, Clone)]
pub(super) struct ReferenceResultContract {
    syntax: Arc<ClosedReferenceTypeSyntax>,
    plan: Arc<ReferenceHeaderPlan>,
    source: TyTerm,
    pointee: super::super::super::source_types::SourceType,
    inputs: Box<[usize]>,
    descriptor: Option<TypeId>,
}
impl ReferenceResultContract {
    pub(super) fn capture(
        db: &mut Database,
        callee: &SourceCallable,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        bindings: &[ReferenceBinding],
        store: &GuestStore,
    ) -> Result<Option<Self>, Diagnostic> {
        let signature = callee.signature();
        if !signature
            .result
            .first()
            .is_some_and(|token| token.text == "&")
        {
            return Ok(None);
        }
        let plan = reference_header(db, callee)?;
        let Some(syntax) = plan.result().cloned() else {
            return Ok(None);
        };
        syntax.require_result(signature)?;
        let pointee = match super::super::super::source_types::prepare_reference_pointee(
            db, graph, sources, signature, &syntax,
        ) {
            QueryPoll::Ready(Ok(
                super::super::super::source_types::NamedTypePreparation::Ready(pointee),
            )) => pointee,
            QueryPoll::Ready(Ok(_)) => return Ok(None),
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => {
                return Err(invalid(
                    "reference result pointee source authority is not current",
                ));
            }
        };
        if syntax.lifetime() == Some("'static") {
            return Err(fault(
                "LRR-E-UNSUPPORTED",
                "static reference result requires an actual static-origin producer",
            ));
        }
        let input_count = plan.input_references();
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(bindings.len())
            .map_err(|_| limit())?;
        for (index, binding) in bindings.iter().enumerate() {
            let related = match syntax.lifetime() {
                Some("'_") | None => input_count == 1,
                Some(name) => binding.parameter.syntax.lifetime() == Some(name),
            };
            if related && (!syntax.mutable() || binding.parameter.mutable()) {
                inputs.push(index);
            }
        }
        // No closed reference argument carries the result's region (for
        // example `&self` of a generic `Self`). Region validity is the borrow
        // checker's, which LRR does not run; the return boundary still checks
        // the returned reference against the closed declared result Type.
        let Some(first) = inputs.first().and_then(|index| bindings.get(*index)) else {
            return Ok(None);
        };
        let TyTerm::Ref(region, _, _) = &first.argument.source else {
            return Err(invalid("reference result input source absent"));
        };
        let source = TyTerm::Ref(
            region.clone(),
            syntax.mutable(),
            Box::new(pointee.source().clone()),
        );
        for index in &inputs {
            let TyTerm::Ref(other, _, _) = &bindings[*index].argument.source else {
                return Err(invalid("reference result region source absent"));
            };
            if other != region {
                return Err(fault(
                    "LRR-E-UNSUPPORTED",
                    "reference result input regions require further region reasoning",
                ));
            }
        }
        let descriptor = pointee.find_descriptor(store).and_then(|target| {
            store.find_type(&TypeKey::Ref {
                mutable: syntax.mutable(),
                target,
            })
        });
        let proof = Self {
            syntax,
            plan,
            source,
            pointee,
            inputs: inputs.into_boxed_slice(),
            descriptor,
        };
        Ok(Some(proof))
    }
    pub(super) fn require_signature(
        &self,
        signature: &FunctionSignature,
    ) -> Result<(), Diagnostic> {
        self.plan.require(signature, self.syntax.parse())?;
        self.syntax.require_result(signature)
    }
    pub(super) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.source.heap_bytes().ok_or_else(limit)?)
            .and_then(|n| {
                self.pointee
                    .heap_bytes()
                    .ok()
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                self.inputs
                    .len()
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(limit)
    }
    pub(super) fn validate_packet(
        &self,
        signature: &FunctionSignature,
        packet: &PreparedCallContracts,
        bindings: &[ReferenceBinding],
    ) -> Result<(), Diagnostic> {
        self.require_signature(signature)?;
        if packet.source_result.as_ref() != Some(&self.source)
            || packet.result_descriptor != self.descriptor
            || self.inputs.is_empty()
        {
            return Err(invalid("reference result packet was altered"));
        }
        for index in &self.inputs {
            let binding = bindings
                .get(*index)
                .ok_or_else(|| invalid("reference result input certificate absent"))?;
            let TyTerm::Ref(region, _, _) = &binding.argument.source else {
                return Err(invalid("reference result actual source absent"));
            };
            if !matches!(&self.source, TyTerm::Ref(expected, mutable, target)
                if expected == region && *mutable == self.syntax.mutable()
                    && target.as_ref() == self.pointee.source())
            {
                return Err(invalid("reference result region certificate differs"));
            }
        }
        Ok(())
    }
    fn check_ticket(
        &self,
        store: &GuestStore,
        ticket: TicketId,
        bindings: &[ReferenceBinding],
    ) -> Result<TypeId, Diagnostic> {
        let mut failure = None;
        for index in &self.inputs {
            let binding = bindings
                .get(*index)
                .ok_or_else(|| invalid("reference result input certificate absent"))?;
            let pointee = self
                .pointee
                .find_descriptor(store)
                .ok_or_else(|| invalid("reference result pointee descriptor absent"))?;
            let checked = if binding.parameter.pointee.same_type(&self.pointee) {
                store.require_reference_return(
                    ticket,
                    &binding.argument.target,
                    self.syntax.mutable(),
                    pointee,
                )
            } else {
                let origin = binding
                    .parameter
                    .pointee
                    .find_descriptor(store)
                    .ok_or_else(|| invalid("reference result input descriptor absent"))?;
                store.require_reference_projection_return(
                    ticket,
                    &binding.argument.target,
                    self.syntax.mutable(),
                    origin,
                    pointee,
                )
            };
            match checked {
                Ok(descriptor) => {
                    if self
                        .descriptor
                        .is_some_and(|expected| expected != descriptor)
                    {
                        return Err(invalid("reference result descriptor changed"));
                    }
                    return Ok(descriptor);
                }
                Err(error) => failure = Some(error),
            }
        }
        Err(failure.unwrap_or_else(|| invalid("reference result target absent")))
    }
}
impl ClosedReferenceContracts {
    pub(crate) fn result_hint_descriptor(
        &self,
        store: &GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        self.require_store_session(store)?;
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?;
        Ok(result.pointee.find_descriptor(store).and_then(|target| {
            store.find_type(&TypeKey::Ref {
                mutable: result.syntax.mutable(),
                target,
            })
        }))
    }
    pub(crate) fn materialize_result_hint(
        &self,
        store: &mut GuestStore,
    ) -> Result<TypeId, Diagnostic> {
        self.require_store_session(store)?;
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?;
        let target = result.pointee.materialize(store)?;
        store.intern_type(TypeKey::Ref {
            mutable: result.syntax.mutable(),
            target,
        })
    }
    pub(crate) fn has_result(&self) -> bool {
        self.result.is_some()
    }
    pub(crate) fn result_source(&self) -> Option<&TyTerm> {
        self.result.as_ref().map(|proof| &proof.source)
    }
    pub(crate) fn result_descriptor(&self) -> Option<TypeId> {
        self.result.as_ref().and_then(|proof| proof.descriptor)
    }
    pub fn validate_result_packet(
        &self,
        signature: &FunctionSignature,
        packet: &PreparedCallContracts,
    ) -> Result<(), Diagnostic> {
        self.result
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?
            .validate_packet(signature, packet, &self.bindings)
    }
    pub(crate) fn recheck_result_ticket(
        &self,
        store: &GuestStore,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        self.result
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?
            .check_ticket(store, ticket, &self.bindings)?;
        Ok(())
    }
}
impl PreparedCallContracts {
    /// This is the only path that can publish a previously unknown reference
    /// result descriptor: actual current source, ticket and target are required.
    pub(crate) fn admit_reference_return(
        &mut self,
        db: &mut Database,
        callee: &SourceCallable,
        store: &GuestStore,
        ticket: TicketId,
        source: Option<&TyTerm>,
    ) -> Result<(), Diagnostic> {
        let proof = self
            .closed_references
            .as_ref()
            .filter(|proof| proof.has_result())
            .ok_or_else(|| invalid("reference result source certificate absent"))?;
        proof.validate_packet(callee.signature(), self)?;
        callee.revalidate(db)?;
        // The existing parameter proof pins exact database/source revisions.
        // Equal revisions imply its retained immutable input stamps cannot have
        // changed; do not rebuild a namespace census at the return boundary.
        if proof.bindings.iter().any(|binding| {
            binding.parameter.revision != db.revision()
                || binding.parameter.source_revision != db.source_revision()
        }) {
            return Err(invalid("reference result source world changed"));
        }
        let result = proof
            .result
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?;
        let Some(TyTerm::Ref(region, mutable, target)) = source else {
            return Err(invalid("returned reference lacks actual source provenance"));
        };
        if *mutable != result.syntax.mutable() || target.as_ref() != result.pointee.source() {
            return Err(invalid("returned reference source pointee/access differs"));
        }
        let expected_region = match &result.source {
            TyTerm::Ref(region, _, _) => region,
            _ => return Err(invalid("reference result source shape differs")),
        };
        if region != expected_region
            && !matches!(region, RegionTerm::Local { definition, start, end }
            if *definition == callee.signature().definition && start < end && callee.body().and_then(|body| body.range.end.checked_sub(body.range.start)).is_some_and(|length| *end <= length))
        {
            return Err(invalid(
                "returned reference region is not its actual input or reborrow",
            ));
        }
        let descriptor = result.check_ticket(store, ticket, &proof.bindings)?;
        let result = self
            .closed_references
            .as_mut()
            .and_then(|proof| proof.result.as_mut())
            .ok_or_else(|| invalid("reference result source certificate absent"))?;
        result.descriptor = Some(descriptor);
        self.result_descriptor = Some(descriptor);
        self.closed_references
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?
            .validate_packet(callee.signature(), self)
    }
}
