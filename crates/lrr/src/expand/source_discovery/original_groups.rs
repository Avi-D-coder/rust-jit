//! Original local annotation groups use the ordinary generated item/declaration
//! closure. A selected occurrence is acknowledged only after actual adoption.
use super::*;
use crate::{
    expand::{
        declaration_fragment::CompletedDeclarationFragment,
        generated_declarations::GeneratedDeclarations,
        source_derives::{SourceDeriveCompletionProof, SourceDerivePoll},
    },
    frontend::{impl_index::DeclarationMacroIndex, macro_names::MacroNamespaces},
    service::source_derive_requests::SourceDeriveSelection,
    source::original_items::OriginalAnnotationGroup,
    syntax::outline::AttributeClass,
};
use std::sync::Arc;

pub(super) struct OriginalGroupProgress {
    pub(super) ordinal: usize,
    pub(super) group: Arc<OriginalAnnotationGroup>,
    leader: Digest,
    output_job: Option<Digest>,
    proof: Option<SourceDeriveCompletionProof>,
}
pub(super) struct OriginalGroupOutput {
    group: Digest,
    pub(super) parent: Arc<DeclarationMacroIndex>,
    selection: Arc<SourceDeriveSelection>,
}

impl SourceDiscovery {
    pub(super) fn capture_original_derive_groups(
        sources: &[PreparedSourceView],
        parents: &BTreeMap<usize, Arc<DeclarationMacroIndex>>,
    ) -> Result<
        (
            BTreeMap<Digest, OriginalGroupProgress>,
            BTreeMap<Digest, Digest>,
        ),
        Diagnostic,
    > {
        let mut groups = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        for (ordinal, parent) in parents {
            let source = &sources[*ordinal];
            for producer in parent.scan().producers() {
                if producer.kind
                    != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                {
                    continue;
                }
                let Some(group) = source.original_annotation_group(producer.identity)? else {
                    continue;
                };
                let identity = group.call().item_input;
                if group.call().scope.owner.is_none() || groups.contains_key(&identity) {
                    continue;
                }
                let target = source
                    .original_annotation_producer(identity)?
                    .ok_or_else(|| fault("original group declaration target absent"))?;
                let attributes = &target.item().header.attributes;
                // Replacing attributes retain their existing coordinator lane;
                // they must not be acknowledged as completed derive groups.
                if !attributes.iter().any(|a| a.class == AttributeClass::Derive)
                    || attributes
                        .iter()
                        .any(|a| a.class == AttributeClass::TransformUnsupported)
                {
                    continue;
                }
                parent.require_pending_original_group(source, &group)?;
                let leader = *group
                    .annotations()
                    .first()
                    .ok_or_else(|| fault("original group annotations absent"))?;
                for annotation in group.annotations() {
                    if aliases.insert(*annotation, identity).is_some() {
                        return Err(fault("original annotation belongs to multiple groups"));
                    }
                }
                groups.insert(
                    identity,
                    OriginalGroupProgress {
                        ordinal: *ordinal,
                        group,
                        leader,
                        output_job: None,
                        proof: None,
                    },
                );
            }
        }
        Ok((groups, aliases))
    }

    pub(super) fn poll_original_derive_group(
        &mut self,
        ticket: &JobTicket,
        identity: Digest,
    ) -> Result<Option<SourceDiscoveryStep>, Diagnostic> {
        let state = self
            .original_groups
            .get(&identity)
            .ok_or_else(|| fault("original derive group absent"))?;
        let ordinal = state.ordinal;
        let group = state.group.clone();
        if let Some(proof) = &state.proof {
            self.work.complete(
                ticket,
                DiscoveryUpdate {
                    members: vec![ticket.job().identity, identity, proof.fingerprint],
                    ..Default::default()
                },
            )?;
            self.pending_nested = None;
            return Ok(None);
        }
        let wait = if ticket.job().identity != state.leader {
            Some(state.leader)
        } else {
            state
                .output_job
                .or_else(|| self.source_derives_inflight.get(&identity).copied())
        };
        if let Some(wait) = wait {
            self.work.block(ticket, vec![wait])?;
            self.pending_nested = None;
            return Ok(None);
        }
        let Some((_, _, providers)) = &self.declarative else {
            self.work.block(ticket, vec![identity])?;
            self.pending_nested = None;
            return Ok(None);
        };
        let namespaces = Self::install_macro_routes_with_modules(
            unique_graph(&mut self.graph)?,
            providers,
            &self.module_routes,
            &self.nested_sources,
            &self.nested_inert_proofs,
            &mut self.route_install,
        )?;
        let poll = super::super::source_derives::prepare_group(
            &self.sources[ordinal],
            &providers[ordinal],
            identity,
            &namespaces,
            &self.completed_source_derives,
            &self.helper_declarations,
        )?;
        self.pending_nested = None;
        match poll {
            SourceDerivePoll::NeedDeclaration(request) => {
                self.pending_declaration = Some((ticket.clone(), request.clone()));
                Ok(Some(SourceDiscoveryStep::NeedDeriveDeclaration { request }))
            }
            SourceDerivePoll::Ready(selection) if selection.is_builtin() => {
                self.expand_builtin_source_derive(ticket.clone(), selection)?;
                Ok(None)
            }
            SourceDerivePoll::Ready(selection) => {
                self.procedural_selection_bytes = self
                    .procedural_selection_bytes
                    .checked_add(selection.metadata_bytes()?)
                    .ok_or_else(|| fault("original derive selection metadata overflow"))?;
                self.pending_source_derive = Some(PendingSourceDerive {
                    ticket: ticket.clone(),
                    selection: selection.clone(),
                    request: None,
                    record: None,
                });
                Ok(Some(SourceDiscoveryStep::NeedSourceDeriveSelection {
                    selection,
                }))
            }
            SourceDerivePoll::Complete(proof) => {
                let mut update = DiscoveryUpdate {
                    members: vec![
                        ticket.job().identity,
                        identity,
                        group.fingerprint(),
                        proof.fingerprint,
                    ],
                    ..Default::default()
                };
                self.work.preflight_complete(ticket, &mut update)?;
                let mut parent = self
                    .nested_sources
                    .get(&ordinal)
                    .ok_or_else(|| fault("original group live parent absent"))?
                    .as_ref()
                    .clone();
                parent.complete_original_annotation_group(
                    &self.sources[ordinal],
                    &group,
                    &proof,
                )?;
                self.nested_sources.insert(ordinal, Arc::new(parent));
                self.original_groups.get_mut(&identity).unwrap().proof = Some(proof);
                self.work.complete(ticket, update)?;
                Ok(None)
            }
            SourceDerivePoll::Attribute(selection)
                if !selection.has_completed_derives()
                    && !self
                        .original_group_outputs
                        .values()
                        .any(|output| output.group == identity) =>
            {
                // Syntax classification is not an Attribute-facet decision.
                // No derive output has been admitted: let the existing cold
                // attribute coordinator own the actual replacing annotation.
                for annotation in group.annotations() {
                    self.original_group_aliases.remove(annotation);
                }
                self.original_groups.remove(&identity);
                self.poll_nested_declarations(ticket)
            }
            // Mixed replacement after real derive output remains an unresolved
            // integration prerequisite, never an empty successful completion.
            SourceDerivePoll::Attribute(_) | SourceDerivePoll::Pending => {
                self.work.block(ticket, vec![identity])?;
                Ok(None)
            }
        }
    }

    pub(super) fn prepare_original_group_output(
        &self,
        identity: Digest,
        selection: Arc<SourceDeriveSelection>,
        output: &GeneratedDeclarations,
    ) -> Result<OriginalGroupOutput, Diagnostic> {
        let state = self
            .original_groups
            .get(&identity)
            .ok_or_else(|| fault("original output group absent"))?;
        state.group.require_source(&self.sources[state.ordinal])?;
        if state.output_job.is_some()
            || state.proof.is_some()
            || !state
                .group
                .annotations()
                .contains(&output.parent_invocation())
        {
            return Err(fault("original output group state differs"));
        }
        let parent = self
            .nested_sources
            .get(&state.ordinal)
            .ok_or_else(|| fault("original output declaration parent absent"))?;
        Ok(OriginalGroupOutput {
            group: identity,
            selection,
            parent: Arc::new(parent.derive_prefix_view(output.parent_invocation())?),
        })
    }
    pub(super) fn register_original_group_output(
        &mut self,
        job: Digest,
        output: OriginalGroupOutput,
    ) -> Result<(), Diagnostic> {
        let state = self
            .original_groups
            .get_mut(&output.group)
            .ok_or_else(|| fault("original output group absent"))?;
        if state.output_job.is_some() || self.original_group_outputs.contains_key(&job) {
            return Err(fault("original output declaration job conflict"));
        }
        state.output_job = Some(job);
        self.original_group_outputs.insert(job, output);
        Ok(())
    }
    pub(super) fn complete_original_group_output(
        &mut self,
        job: Digest,
        output: &CompletedDeclarationFragment,
    ) -> Result<(), Diagnostic> {
        let Some(pending) = self.original_group_outputs.get(&job) else {
            return Ok(());
        };
        let state = self
            .original_groups
            .get(&pending.group)
            .ok_or_else(|| fault("completed original group absent"))?;
        let record = output.index().output().record();
        if state.output_job != Some(job)
            || state.proof.is_some()
            || record.key().invocation != pending.selection.input().identity()
            || self.source_derives_inflight.get(&pending.group) != Some(&record.key().fingerprint())
            || self
                .completed_source_derives
                .contains_key(&record.key().invocation)
        {
            return Err(fault("completed original occurrence differs"));
        }
        let mut parent = self
            .nested_sources
            .get(&state.ordinal)
            .ok_or_else(|| fault("completed original group parent absent"))?
            .as_ref()
            .clone();
        parent.adopt_completed_child_prefix(record.key(), output.index(), false)?;
        self.nested_sources.insert(state.ordinal, Arc::new(parent));
        self.completed_source_derives
            .insert(record.key().invocation, pending.selection.clone());
        self.source_derives_inflight.remove(&pending.group);
        self.original_groups
            .get_mut(&pending.group)
            .unwrap()
            .output_job = None;
        Ok(())
    }
    pub(super) fn revalidate_original_groups(
        &self,
        names: &MacroNamespaces,
    ) -> Result<(), Diagnostic> {
        let Some((_, _, providers)) = &self.declarative else {
            return Ok(());
        };
        for (identity, state) in &self.original_groups {
            let proof = state
                .proof
                .as_ref()
                .ok_or_else(|| fault("original derive group incomplete before sealing"))?;
            proof.require_group(&state.group, &self.sources[state.ordinal])?;
            match super::super::source_derives::prepare_group(
                &self.sources[state.ordinal],
                &providers[state.ordinal],
                *identity,
                names,
                &self.completed_source_derives,
                &self.helper_declarations,
            )? {
                SourceDerivePoll::Complete(current) if &current == proof => {}
                _ => return Err(fault("original group selection changed before sealing")),
            }
        }
        Ok(())
    }
}
