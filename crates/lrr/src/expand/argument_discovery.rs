//! Coordinator-owned argument producer assembly; no argument evaluation occurs.
use super::{
    argument_declarations::ArgumentDeclarations,
    budget::SharedExpansionBudget,
    declaration_fragment::{
        CompletedDeclarationFragment, DeclarationFragment, DeclarationFragmentInput,
        DeclarationFragmentStep,
    },
    record::ExpansionRecord,
    source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    frontend::impl_index::{ArgumentDeclarationMacroIndex, DeclarationMacroIndex},
};
use std::sync::Arc;
pub(crate) enum ArgumentDiscoveryStep {
    NeedAttributeSelection(
        Arc<crate::service::source_attribute_requests::SourceAttributeSelection>,
    ),
    NeedProceduralSelection(Arc<super::source_discovery::ProceduralSourceSelection>),
    NeedExpansion(Arc<super::source_declarative::SourceDeclarativeRequest>),
    NeedInput(Digest),
    Complete,
}
struct PendingArgumentProcedural {
    ordinal: usize,
    selection: Arc<super::source_discovery::ProceduralSourceSelection>,
    parent: Arc<ArgumentDeclarationMacroIndex>,
    input: DeclarationFragmentInput,
    budget: SharedExpansionBudget,
}
pub(crate) struct ArgumentDiscovery {
    pub arguments: Vec<Arc<ArgumentDeclarations>>,
    pub indexes: Vec<ArgumentDeclarationMacroIndex>,
    pub completed: Vec<Arc<CompletedDeclarationFragment>>,
    active: Option<(usize, DeclarationFragment)>,
    pending_procedural: Option<PendingArgumentProcedural>,
    bytes: usize,
}
fn fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-DISCOVERY-LIMIT",
        "intrinsic argument assembly metadata limit".into(),
    )
}
impl ArgumentDiscovery {
    pub fn new(
        arguments: Vec<Arc<ArgumentDeclarations>>,
        source: Arc<DeclarationMacroIndex>,
    ) -> Result<Self, Diagnostic> {
        Self::with_parent(
            arguments,
            crate::frontend::impl_index::DeclarationMacroParent::Source(source),
        )
    }
    pub(crate) fn with_parent(
        arguments: Vec<Arc<ArgumentDeclarations>>,
        parent: crate::frontend::impl_index::DeclarationMacroParent,
    ) -> Result<Self, Diagnostic> {
        let mut indexes = Vec::new();
        indexes
            .try_reserve_exact(arguments.len())
            .map_err(|_| fault())?;
        let mut bytes = arguments
            .capacity()
            .checked_mul(std::mem::size_of::<Arc<ArgumentDeclarations>>())
            .and_then(|n| {
                indexes
                    .capacity()
                    .checked_mul(std::mem::size_of::<ArgumentDeclarationMacroIndex>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(fault)?;
        for argument in &arguments {
            bytes = bytes
                .checked_add(argument.metadata_bytes())
                .ok_or_else(fault)?;
            let index =
                ArgumentDeclarationMacroIndex::with_parent(argument.clone(), parent.clone())?;
            bytes = bytes
                .checked_add(index.metadata_bytes())
                .ok_or_else(fault)?;
            indexes.push(index);
        }
        Ok(Self {
            arguments,
            indexes,
            completed: Vec::new(),
            active: None,
            pending_procedural: None,
            bytes,
        })
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn pending_request(
        &self,
    ) -> Option<&Arc<super::source_declarative::SourceDeclarativeRequest>> {
        self.active
            .as_ref()
            .and_then(|(_, fragment)| fragment.pending_request())
    }
    pub(crate) fn bind_attribute_request(
        &mut self,
        request: Arc<super::source_declarative::SourceDeclarativeRequest>,
    ) -> Result<(), Diagnostic> {
        self.active
            .as_mut()
            .ok_or_else(fault)?
            .1
            .bind_attribute_request(request)
    }
    pub(crate) fn bind_procedural_request(
        &mut self,
        request: Arc<super::source_declarative::SourceDeclarativeRequest>,
    ) -> Result<(), Diagnostic> {
        if let Some(PendingArgumentProcedural {
            ordinal,
            selection,
            parent,
            input,
            budget,
        }) = &self.pending_procedural
        {
            if !selection.validates_binding(request.key())? {
                return Err(fault());
            }
            let fragment = DeclarationFragment::from_prepared_argument(
                parent.clone(),
                DeclarationFragmentInput {
                    invocation: input.invocation,
                    request: input.request,
                    graph: input.graph,
                    compile_env: input.compile_env,
                    engine: input.engine,
                    epoch: input.epoch,
                    cfg: input.cfg.clone(),
                },
                budget.clone(),
                request,
                selection.clone(),
            )?;
            self.active = Some((*ordinal, fragment));
            self.pending_procedural = None;
            return Ok(());
        }
        self.active
            .as_mut()
            .ok_or_else(fault)?
            .1
            .bind_procedural_request(request)
    }
    pub fn admit(&mut self, record: Arc<ExpansionRecord>) -> Result<(), Diagnostic> {
        self.active.as_mut().ok_or_else(fault)?.1.admit(record)
    }
    pub fn poll(
        &mut self,
        provider: &SourceDeclarative,
        input: &DeclarationFragmentInput,
        budget: &SharedExpansionBudget,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<ArgumentDiscoveryStep, Diagnostic> {
        loop {
            if let Some(PendingArgumentProcedural { selection, .. }) = &self.pending_procedural {
                return Ok(ArgumentDiscoveryStep::NeedProceduralSelection(
                    selection.clone(),
                ));
            }
            if let Some((ordinal, fragment)) = &mut self.active {
                match fragment.poll_with_namespaces(provider, namespaces)? {
                    DeclarationFragmentStep::NeedAttributeSelection(selection) => {
                        return Ok(ArgumentDiscoveryStep::NeedAttributeSelection(selection));
                    }
                    DeclarationFragmentStep::NeedProceduralSelection(selection) => {
                        return Ok(ArgumentDiscoveryStep::NeedProceduralSelection(selection));
                    }
                    DeclarationFragmentStep::Complete(output) => {
                        let mut index = self.indexes[*ordinal].clone();
                        index.adopt_completed_child(
                            output.index().output().record().key(),
                            output.index(),
                        )?;
                        let bytes = self
                            .bytes
                            .checked_add(output.metadata_bytes())
                            .ok_or_else(fault)?;
                        self.completed.try_reserve(1).map_err(|_| fault())?;
                        self.indexes[*ordinal] = index;
                        self.completed.push(output);
                        self.bytes = bytes;
                        self.active = None;
                        continue;
                    }
                    DeclarationFragmentStep::NeedExpansion(request) => {
                        return Ok(ArgumentDiscoveryStep::NeedExpansion(request));
                    }
                    DeclarationFragmentStep::NeedInput(input) => {
                        return Ok(ArgumentDiscoveryStep::NeedInput(input));
                    }
                }
            }
            let Some((ordinal, index)) = self
                .indexes
                .iter()
                .enumerate()
                .find(|(_, index)| !index.is_complete())
            else {
                return Ok(ArgumentDiscoveryStep::Complete);
            };
            let invocation = index.next_pending_producer().ok_or_else(fault)?;
            if let Some(definition) =
                super::nested_inert::selected_definition(index.argument().scan(), invocation)?
            {
                let bytes = self
                    .bytes
                    .checked_add(128)
                    .ok_or_else(fault)?;
                let mut index = index.clone();
                index.retire_inert(invocation, definition)?;
                self.indexes[ordinal] = index;
                self.bytes = bytes;
                continue;
            }
            if let Some(selection) = provider.argument_procedural_selection(
                Arc::new(index.clone()),
                invocation,
                namespaces,
                std::slice::from_ref(provider),
            )? {
                let bytes = self
                    .bytes
                    .checked_add(selection.metadata_bytes()?)
                    .and_then(|n| n.checked_add(std::mem::size_of::<PendingArgumentProcedural>()))
                    .ok_or_else(fault)?;
                let selection = Arc::new(selection);
                self.pending_procedural = Some(PendingArgumentProcedural {
                    ordinal,
                    selection: selection.clone(),
                    parent: Arc::new(index.clone()),
                    input: DeclarationFragmentInput {
                        invocation,
                        request: input.request,
                        graph: input.graph,
                        compile_env: input.compile_env,
                        engine: input.engine,
                        epoch: input.epoch,
                        cfg: input.cfg.clone(),
                    },
                    budget: budget.clone(),
                });
                self.bytes = bytes;
                return Ok(ArgumentDiscoveryStep::NeedProceduralSelection(selection));
            }
            if let Some(request) = provider.argument_compiler_request(
                Arc::new(index.clone()),
                invocation,
                namespaces,
                input.compile_env,
                input.engine,
            )? {
                self.active = Some((
                    ordinal,
                    DeclarationFragment::from_compiler_argument(
                        Arc::new(index.clone()),
                        DeclarationFragmentInput {
                            invocation,
                            request: input.request,
                            graph: input.graph,
                            compile_env: input.compile_env,
                            engine: input.engine,
                            epoch: input.epoch,
                            cfg: input.cfg.clone(),
                        },
                        budget.clone(),
                        Arc::new(request),
                    )?,
                ));
                continue;
            }
            match provider.argument_declaration_demand(
                index,
                invocation,
                input.compile_env,
                input.engine,
            )? {
                SourceDeclarativeDemand::NeedInput(input) => {
                    return Ok(ArgumentDiscoveryStep::NeedInput(input));
                }
                SourceDeclarativeDemand::Ready(_) => {}
            }
            self.active = Some((
                ordinal,
                DeclarationFragment::from_argument(
                    provider,
                    Arc::new(index.clone()),
                    DeclarationFragmentInput {
                        invocation,
                        request: input.request,
                        graph: input.graph,
                        compile_env: input.compile_env,
                        engine: input.engine,
                        epoch: input.epoch,
                        cfg: input.cfg.clone(),
                    },
                    budget.clone(),
                )?,
            ));
        }
    }
}
