//! Reached original lexical callable selection; no guest or macro execution.
use super::RuntimeMacroContext;
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    expand::scopes::MacroName,
    frontend::{
        callables::SourceCallable,
        names::{NameLookup, NameNamespaces},
        source_locals::OrdinaryNameSite,
    },
    ir::pc::PcKey,
    source::manifest::{ItemKind, Namespace},
    syntax::{body_index::BodyIndex, outline::DeclarationContext},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
impl RuntimeMacroContext<'_> {
    pub(crate) fn bind_lexical_numeric_function<'p>(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        body: &BodyIndex,
        key: &PcKey,
        name: &str,
        proofs: impl Iterator<Item = &'p crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
    ) -> QueryPoll<Option<Box<SourceCallable>>> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        if name.contains("::") {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(caller) = self.source.outline().items.iter().find(|item| {
            item.body
                .as_ref()
                .is_some_and(|handle| handle.key() == *body.body())
        }) else {
            // No context-chain guessing for a newly admitted lexical caller.
            return QueryPoll::Ready(Ok(None));
        };
        if caller.scope.owner.is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        if key.body != *body.body() || self.source.outline().cfg != body.cfg().fingerprint() {
            return QueryPoll::Ready(Err(fault("lexical caller PC/cfg/source context differs")));
        }
        let expected = ready!(MacroName::parse(
            name,
            self.source.selection().outline.parse.profile.edition
        ));
        let mut entered = ready!(self.index.enter_body(self.source.selection(), body.body()));
        if !entered.facts().items.iter().any(|row| {
            row.item.def_key.item_kind == ItemKind::Function
                && MacroName::parse(
                    &row.item.def_key.declared_name,
                    self.source.selection().outline.parse.profile.edition,
                )
                .is_ok_and(|candidate| candidate == expected)
        }) {
            return QueryPoll::Ready(Ok(None));
        }
        for proof in proofs.filter(|proof| proof.body() == body.body()) {
            ready!(entered.accept_no_item_proof(
                self.source.selection(),
                body.body(),
                proof,
                self.catalog
            ));
        }
        let locals = ready!(entered.local_index(
            self.source.selection(),
            body.body(),
            &Default::default(),
            &[]
        ));
        let node = match body.nodes().get(key.node as usize) {
            Some(node) => node,
            None => return QueryPoll::Ready(Err(fault("lexical reached node absent"))),
        };
        let handle = caller.body.as_ref().expect("matched owning caller body");
        let at = match handle.range.start.checked_add(node.range.start) {
            Some(at) => at,
            None => return QueryPoll::Ready(Err(fault("lexical call offset overflow"))),
        };
        let found = ready!(locals.lookup_ordinary(
            OrdinaryNameSite {
                source: self.source.selection(),
                body: body.body(),
                at: u64::from(at),
                name,
                context: locals.context(),
                namespace: Namespace::Value,
            },
            graph,
            None
        ));
        let NameLookup::Defined(selected) = found else {
            return QueryPoll::Ready(Err(fault(
                "lexical callable identity is incomplete, shadowed or ambiguous",
            )));
        };
        let Some(item) = entered
            .facts()
            .items
            .iter()
            .find(|row| row.item.def_key.fingerprint() == selected.identity)
        else {
            // Existing namespace lookup explicitly selected a module definition.
            return QueryPoll::Ready(Ok(None));
        };
        if item.item.def_key.item_kind != ItemKind::Function {
            return QueryPoll::Ready(Err(fault(
                "lexical selected value is not an ordinary function",
            )));
        }
        // Exact public parent rows must match before any private retained plan is minted.
        let context = DeclarationContext {
            scope: caller.scope.clone(),
            range: caller.declaration_span,
        };
        for (input, expected) in [
            (
                InputKey::DefinitionSource(body.body().definition),
                InputValue::DefinitionSource(self.source.selection().outline.parse.clone()),
            ),
            (
                InputKey::DefinitionHeader(body.body().definition),
                InputValue::DefinitionHeader(caller.header.clone()),
            ),
            (
                InputKey::DeclarationContext(body.body().definition),
                InputValue::DeclarationContext(context.clone()),
            ),
            (
                InputKey::DefinitionBody(body.body().definition),
                InputValue::DefinitionBody(handle.clone()),
            ),
        ] {
            if db
                .input(&input)
                .is_none_or(|actual| actual.fingerprint() != expected.fingerprint())
            {
                return QueryPoll::Ready(Err(fault("lexical owning caller publication differs")));
            }
        }
        let outcome = match db.fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(self.source.selection().outline.parse.clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("lexical captured parser outcome differs")));
        };
        let plan = ready!(parsed.lexical_numeric_function_plan(
            handle,
            body,
            key,
            &caller.header,
            &context,
            selected.identity,
            name,
            db.source_revision(),
            db.revision()
        ));
        let declaration_scope = ready!(
            entered
                .facts()
                .scopes
                .iter()
                .find(|scope| scope.ordinal == plan.scope())
                .ok_or_else(|| fault("lexical declaring scope absent"))
        );
        // Block-wide Type items/imports are looked up in the captured parent
        // scope, without entering the opaque selected child's signature/body.
        let type_site = u64::from(declaration_scope.range.start);
        for name in plan.primitive_names() {
            let found = ready!(locals.lookup_ordinary(
                OrdinaryNameSite {
                    source: self.source.selection(),
                    body: body.body(),
                    at: type_site,
                    name,
                    context: locals.context(),
                    namespace: Namespace::Type,
                },
                graph,
                None
            ));
            if found != NameLookup::Absent {
                return QueryPoll::Ready(Err(fault(
                    "lexical numeric header requires owning type namespace resolution",
                )));
            }
        }
        // Root-owned hook verifies current private facts and publishes only
        // matched derived Definition* rows, retaining source_revision. It must
        // fetch FunctionSignature only; body structure/thunks begin at entry.
        match crate::frontend::callables::admit_lexical_function(db, self.source, &plan) {
            QueryPoll::Ready(Ok(callee)) => QueryPoll::Ready(Ok(Some(callee))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
