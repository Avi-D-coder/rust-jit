//! Source-resolved item requests. Assembly and worklist admission execute no tool.
use super::{
    expansion::DemandContext,
    item_macro_inputs::{
        AttributeInvocationInput, DeriveInvocationInput, DeriveOccurrenceInput,
        OriginalAttributeSelection,
    },
    macro_session::MacroSession,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::{
        identity::ExpansionKey,
        worklist::{AttributeSlot, ItemTransformWorklist, TransformStep},
    },
};
use lrr_expand_adapter::{
    proc_client::{CapturedSpanSource, ProcForm, ProcGrammar, ProcInvocation},
    token_wire::{Key, TokenForest},
};
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-TRANSFORM", message.into())
}
fn wire(error: impl std::fmt::Display) -> Diagnostic {
    fault(error.to_string())
}

/// Resolver supplies exact export and owned site records, including removed annotations.
/// This is data, never an execution grant; the frozen session validates the target.
pub struct ResolvedItemTarget<'a> {
    pub artifact: Key,
    pub export: String,
    pub scope: Key,
    pub parent_origin: Key,
    pub registry: &'a TokenForest,
    pub definition_site: Key,
    pub call_site: Key,
    pub mixed_site: Key,
    pub sources: Vec<CapturedSpanSource>,
}
pub struct PreparedItemRequest {
    original_attribute: Option<OriginalAttributeSelection>,
    selection: Digest,
    request: ProcInvocation,
    key: ExpansionKey,
}
impl PreparedItemRequest {
    pub fn attribute(
        session: &MacroSession,
        context: &DemandContext,
        input: &AttributeInvocationInput,
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        if input.source_selection.identity != input.identity {
            return Err(fault("original attribute selection identity differs"));
        }
        input.source_selection.parse.profile.validate_live()?;
        let attributes = input
            .arguments
            .as_ref()
            .map(|a| TokenForest::decode(&a.encode().map_err(wire)?).map_err(wire))
            .transpose()?;
        let mut prepared = Self::assemble(
            session,
            context,
            input.identity,
            &input.item,
            attributes,
            ProcForm::Attribute,
            target,
        )?;
        // The fixed live profile is the only heap-owned field in this provenance.
        // Reserve its copied string bytes in the existing request allowance.
        let total = prepared.retained_bytes()?;
        bounded(total, input.source_selection.parse.profile.semantics.len())?;
        prepared.original_attribute = Some(input.source_selection.clone());
        Ok(prepared)
    }
    fn retained_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self.request.input.allocated_bytes().map_err(wire)?;
        if let Some(a) = &self.request.attributes {
            bytes = bounded(bytes, a.allocated_bytes().map_err(wire)?)?;
        }
        bytes = bounded(
            bytes,
            self.request
                .sources
                .capacity()
                .checked_mul(std::mem::size_of::<CapturedSpanSource>())
                .ok_or_else(|| fault("source allocation overflow"))?,
        )?;
        for source in &self.request.sources {
            bytes = bounded(bytes, source.heap_bytes().map_err(wire)?)?;
        }
        bytes = bounded(bytes, self.request.export.capacity())?;
        bytes = bounded(bytes, self.request.export.len())?;
        bounded(bytes, std::mem::size_of::<Self>())
    }
    pub fn derive(
        session: &MacroSession,
        context: &DemandContext,
        input: &DeriveInvocationInput,
        selection: usize,
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        Self::assemble(
            session,
            context,
            input.selection_identity(selection)?,
            &input.item,
            None,
            ProcForm::Derive,
            target,
        )
    }
    /// The selected original occurrence supplies the key; callers cannot swap
    /// its ordinal while retaining the previously prepared input identity.
    pub fn derive_occurrence(
        session: &MacroSession,
        context: &DemandContext,
        input: &DeriveOccurrenceInput,
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        Self::assemble(
            session,
            context,
            input.identity(),
            input.item(),
            None,
            ProcForm::Derive,
            target,
        )
    }
    /// Exact cold declaration grammar; construction grants no macro execution.
    pub fn function_like(
        session: &MacroSession,
        context: &DemandContext,
        selection: Digest,
        input: &crate::syntax::token_wire::TokenForest,
        grammar: crate::expand::identity::GrammarSlot,
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        Self::assemble_in_slot(
            session,
            context,
            selection,
            input,
            None,
            (ProcForm::FunctionLike, grammar),
            target,
        )
    }
    pub(crate) fn assemble(
        session: &MacroSession,
        context: &DemandContext,
        selection: Digest,
        input: &crate::syntax::token_wire::TokenForest,
        attributes: Option<TokenForest>,
        form: ProcForm,
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        Self::assemble_in_slot(
            session,
            context,
            selection,
            input,
            attributes,
            (form, crate::expand::identity::GrammarSlot::Items),
            target,
        )
    }
    pub(crate) fn assemble_in_slot(
        session: &MacroSession,
        context: &DemandContext,
        selection: Digest,
        input: &crate::syntax::token_wire::TokenForest,
        attributes: Option<TokenForest>,
        form_and_grammar: (ProcForm, crate::expand::identity::GrammarSlot),
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        let grammar = match form_and_grammar.1 {
            crate::expand::identity::GrammarSlot::Items => ProcGrammar::Items,
            crate::expand::identity::GrammarSlot::Expression => ProcGrammar::Expression,
            crate::expand::identity::GrammarSlot::Pattern => ProcGrammar::Pattern,
            crate::expand::identity::GrammarSlot::Type => ProcGrammar::Type,
            crate::expand::identity::GrammarSlot::TraitItems => ProcGrammar::TraitItems,
            crate::expand::identity::GrammarSlot::ImplItems => ProcGrammar::ImplItems,
            crate::expand::identity::GrammarSlot::Statements => ProcGrammar::Statements,
        };
        Self::assemble_form(
            session,
            context,
            selection,
            input,
            attributes,
            (form_and_grammar.0, grammar),
            target,
        )
    }
    fn assemble_form(
        session: &MacroSession,
        context: &DemandContext,
        selection: Digest,
        input: &crate::syntax::token_wire::TokenForest,
        attributes: Option<TokenForest>,
        form_and_grammar: (ProcForm, ProcGrammar),
        target: ResolvedItemTarget<'_>,
    ) -> Result<Self, Diagnostic> {
        let (form, grammar) = form_and_grammar;
        let input = TokenForest::decode(&input.encode().map_err(wire)?).map_err(wire)?;
        let input = with_sites(input, &target)?;
        let mut bytes = input.allocated_bytes().map_err(wire)?;
        if let Some(a) = &attributes {
            bytes = bounded(bytes, a.allocated_bytes().map_err(wire)?)?;
        }
        bytes = bounded(
            bytes,
            target
                .sources
                .capacity()
                .checked_mul(std::mem::size_of::<CapturedSpanSource>())
                .ok_or_else(|| fault("captured source table allocation overflow"))?,
        )?;
        for source in &target.sources {
            bytes = bounded(bytes, source.heap_bytes().map_err(wire)?)?;
        }
        bytes = bounded(bytes, target.export.capacity())?;
        bytes = bounded(bytes, target.export.len())?;
        bounded(bytes, std::mem::size_of::<Self>())?;
        let request = ProcInvocation {
            identity: Key(selection.0),
            unit: Key(context.unit.0),
            epoch: context.epoch,
            artifact: target.artifact,
            export: target.export,
            form,
            grammar,
            scope: target.scope,
            parent_origin: target.parent_origin,
            input,
            attributes,
            definition_site: target.definition_site,
            call_site: target.call_site,
            mixed_site: target.mixed_site,
            sources: target.sources,
        };
        let key = session.request_key(context, &request)?;
        Ok(Self {
            original_attribute: None,
            selection,
            request,
            key,
        })
    }
    /// Pure lexical admission. Namespace completion and execution authority remain
    /// separate requirements; resolution cannot start the helper or adopt output.
    pub fn admit_original_attribute(
        &self,
        cursor: &crate::frontend::original_attributes::OriginalItemAttributeCursor,
        resolver: &crate::frontend::original_attributes::OriginalAttributeResolver<'_>,
        parsed: &crate::syntax::parse::ParsedFile,
        worklist: &mut ItemTransformWorklist,
        revision: u64,
    ) -> Result<(), Diagnostic> {
        use crate::frontend::{
            builtin_attributes::AttributeDecision, item_attributes::CustomAttributeDecision,
            original_attributes::OriginalAttributePoll,
        };
        let source = self
            .original_attribute
            .as_ref()
            .ok_or_else(|| fault("request has no original attribute selection"))?;
        if !worklist.matches_item_context(source.owner, source.cfg) {
            return Err(fault("worklist differs from original attribute owner/cfg"));
        }
        if !cursor.matches_selection(
            &source.parse,
            source.owner,
            source.cfg,
            source.item,
            source.meta,
            source.index,
        ) {
            return Err(fault(
                "request differs from current lexical attribute selection",
            ));
        }
        match cursor.poll(resolver, parsed)? {
            OriginalAttributePoll::Candidate {
                index,
                decision: AttributeDecision::Custom(CustomAttributeDecision::Procedural(identity)),
            } if index == source.index && identity == self.key.macro_identity => {
                self.admit(worklist, index, revision)
            }
            _ => Err(fault(
                "request differs from current resolved attribute definition",
            )),
        }
    }
    pub fn request(&self) -> &ProcInvocation {
        &self.request
    }
    pub fn key(&self) -> &ExpansionKey {
        &self.key
    }
    /// Admit only the currently suspended source selection. No coordinator is started.
    pub fn admit(
        &self,
        worklist: &mut ItemTransformWorklist,
        index: usize,
        revision: u64,
    ) -> Result<(), Diagnostic> {
        let matched = match (self.request.form, worklist.poll()) {
            (
                ProcForm::Attribute,
                TransformStep::NeedAttribute {
                    index: i,
                    revision: r,
                    identity,
                },
            )
            | (
                ProcForm::Derive,
                TransformStep::NeedDerive {
                    index: i,
                    revision: r,
                    identity,
                },
            ) => i == index && r == revision && identity == self.selection,
            _ => false,
        };
        if !matched {
            return Err(fault(
                "prepared request differs from current source selection",
            ));
        }
        match self.request.form {
            ProcForm::Attribute => {
                worklist.resolve_attribute(index, revision, AttributeSlot::expand(self.key.clone()))
            }
            ProcForm::Derive => worklist.resolve_derive(index, revision, self.key.clone()),
            ProcForm::FunctionLike => {
                Err(fault("function macro cannot transform an item annotation"))
            }
        }
    }
}
fn bounded(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .ok_or_else(|| fault("item request input limit"))
}
fn with_sites(
    input: TokenForest,
    target: &ResolvedItemTarget<'_>,
) -> Result<TokenForest, Diagnostic> {
    target.registry.validate().map_err(wire)?;
    let mut spans = input.spans().to_vec();
    let mut contexts = input.contexts().to_vec();
    for key in [target.definition_site, target.call_site, target.mixed_site] {
        if spans.iter().any(|span| span.key() == key) {
            continue;
        }
        let span = target
            .registry
            .spans()
            .iter()
            .find(|span| span.key() == key)
            .ok_or_else(|| fault("resolved item site has no owned span record"))?;
        let mut context = Some(span.context);
        while let Some(key) = context {
            if contexts.iter().any(|record| record.key() == key) {
                break;
            }
            let record = target
                .registry
                .contexts()
                .iter()
                .find(|record| record.key() == key)
                .ok_or_else(|| fault("resolved item site has no owned context record"))?;
            context = record.parent;
            contexts.push(record.clone());
        }
        spans.push(span.clone());
    }
    TokenForest::new(input.roots().to_vec(), contexts, spans).map_err(wire)
}
