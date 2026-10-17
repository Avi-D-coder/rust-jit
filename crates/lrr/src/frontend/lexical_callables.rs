//! Narrow coordinator admission of exact original-source lexical numeric functions.
use super::{SourceCallable, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        names::{NameLookup, NameNamespaces},
        source_locals::OrdinaryNameSite,
    },
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::{lexical_numeric_callees::LexicalFunctionPlan, outline::DeclarationContext},
};
use std::collections::BTreeMap;

/// Fields are private: only exact source/namespace validation can mint this.
/// The database receives four closed derived facts, never arbitrary key/value rows.
#[derive(Debug, Clone)]
pub(crate) struct LexicalAdmission {
    plan: LexicalFunctionPlan,
    namespace_stamps: BTreeMap<InputKey, Digest>,
    metadata_bytes: usize,
}
impl LexicalAdmission {
    pub(crate) fn plan(&self) -> &LexicalFunctionPlan {
        &self.plan
    }
    pub(crate) fn namespace_dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.namespace_stamps
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn derived_inputs(&self) -> [(InputKey, InputValue); 4] {
        let item = self.plan.item();
        let identity = item.def_key.fingerprint();
        [
            (
                InputKey::DefinitionSource(identity),
                InputValue::DefinitionSource(self.plan.source().clone()),
            ),
            (
                InputKey::DefinitionHeader(identity),
                InputValue::DefinitionHeader(item.header.clone()),
            ),
            (
                InputKey::DeclarationContext(identity),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                }),
            ),
            (
                InputKey::DefinitionBody(identity),
                InputValue::DefinitionBody(
                    item.body
                        .as_ref()
                        .expect("private plan requires local body")
                        .clone(),
                ),
            ),
        ]
    }
    pub(crate) fn revalidate(&self, db: &Database) -> Result<(), Diagnostic> {
        if db.source_revision() != self.plan.source_revision()
            || db.lexical_namespace_stamps()? != self.namespace_stamps
        {
            return Err(fault(
                "lexical callable namespace/source/epoch certificate changed",
            ));
        }
        let definition = self.plan.caller().def_key;
        for (key, expected) in [
            (
                InputKey::DefinitionSource(definition),
                InputValue::DefinitionSource(self.plan.source().clone()),
            ),
            (
                InputKey::DefinitionHeader(definition),
                InputValue::DefinitionHeader(self.plan.caller_header().clone()),
            ),
            (
                InputKey::DeclarationContext(definition),
                InputValue::DeclarationContext(self.plan.caller_context().clone()),
            ),
            (
                InputKey::DefinitionBody(definition),
                InputValue::DefinitionBody(self.plan.caller().clone()),
            ),
        ] {
            if db
                .published_input(&key)
                .is_none_or(|(_, fingerprint)| fingerprint != expected.fingerprint())
            {
                return Err(fault("lexical callable owning caller input changed"));
            }
        }
        Ok(())
    }
    fn validate(
        db: &Database,
        source: &PreparedSourceView,
        plan: &LexicalFunctionPlan,
    ) -> Result<Self, Diagnostic> {
        db.require_coordinator()?;
        if db.source_revision() != plan.source_revision()
            || db.revision() != plan.query_revision()
            || &source.selection().outline.parse != plan.source()
        {
            return Err(fault(
                "lexical plan belongs to a stale or different captured source",
            ));
        }
        source.require_original_hygiene()?;
        source.require_declaration_syntax()?;
        let owner = source
            .outline()
            .items
            .iter()
            .find(|item| item.body.as_ref() == Some(plan.caller()))
            .ok_or_else(|| fault("lexical owning original caller absent"))?;
        if owner.scope.owner.is_some()
            || owner.header != *plan.caller_header()
            || (DeclarationContext {
                scope: owner.scope.clone(),
                range: owner.declaration_span,
            }) != *plan.caller_context()
        {
            return Err(fault(
                "lexical caller header or declaration ownership differs",
            ));
        }
        // Every captured base fact must still match. Derived facts cannot replace
        // a captured namespace/cfg/world under the same prepared source lease.
        for (key, expected) in source.prepared_inputs() {
            if db
                .published_input(key)
                .is_none_or(|(_, fingerprint)| fingerprint != expected.fingerprint())
            {
                return Err(fault("lexical source base publication differs"));
            }
        }
        let seal = db.lexical_scope_certificate(source.selection(), &owner.scope)?;
        let InputValue::DiscoverySeal(sealed) = seal.as_ref() else {
            return Err(fault("lexical namespace certificate variant differs"));
        };
        // Reuse the pinned original-source hygiene adapter and existing entered
        // local namespace. No expansion, native helper, child body or query runs.
        let provider = crate::expand::source_declarative::SourceDeclarative::new(source.share())?;
        let published = provider.completed_published_index()?;
        let mut entered = published.enter_body(source.selection(), &plan.caller().key())?;
        for (_, proof) in sealed
            .runtime_no_item_proofs
            .iter()
            .filter(|(_, proof)| proof.body() == &plan.caller().key())
        {
            let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_definition(&plan.source().profile, proof.definition())?;
            entered.accept_no_item_proof(
                source.selection(),
                &plan.caller().key(),
                proof,
                &catalog,
            )?;
        }
        let locals = entered.local_index(
            source.selection(),
            &plan.caller().key(),
            &Default::default(),
            &[],
        )?;
        let graph = NameNamespaces::from_outlines(&[source.outline()])?;
        let lookup = |at, name, namespace| {
            locals.lookup_ordinary(
                OrdinaryNameSite {
                    source: source.selection(),
                    body: &plan.site().body,
                    at,
                    name,
                    context: locals.context(),
                    namespace,
                },
                &graph,
                None,
            )
        };
        let selected = lookup(
            u64::from(plan.path_range().start),
            &plan.item().def_key.declared_name,
            Namespace::Value,
        )?;
        if !matches!(selected, NameLookup::Defined(definition) if definition.identity == plan.item().def_key.fingerprint())
            || !entered
                .facts()
                .items
                .iter()
                .any(|row| row.scope == plan.scope() && row.item.def_key == plan.item().def_key)
        {
            return Err(fault(
                "lexical reached Value namespace selects another identity",
            ));
        }
        let declaration_scope = entered
            .facts()
            .scopes
            .iter()
            .find(|scope| scope.ordinal == plan.scope())
            .ok_or_else(|| fault("lexical declaration parent scope absent"))?;
        // Block-scoped type items/imports occupy the whole declaring scope. Its
        // exact retained start is outside the child's opaque function syntax.
        // Own generic binders are excluded by this closed numeric plan.
        let type_site = u64::from(declaration_scope.range.start);
        for name in plan.primitive_names() {
            if lookup(type_site, name, Namespace::Type)? != NameLookup::Absent {
                return Err(fault(
                    "lexical numeric header has a source Type namespace identity",
                ));
            }
        }
        let index =
            source.index_body(source.selection(), &plan.caller().key(), Default::default())?;
        let parsed = db
            .prepared_parse(plan.source())
            .ok_or_else(|| fault("lexical exact original parser lease absent"))?;
        let rebuilt = parsed.lexical_numeric_function_plan(
            plan.caller(),
            &index,
            plan.site(),
            plan.caller_header(),
            plan.caller_context(),
            plan.item().def_key.fingerprint(),
            &plan.item().def_key.declared_name,
            plan.source_revision(),
            plan.query_revision(),
        )?;
        if rebuilt.identity() != plan.identity() {
            return Err(fault(
                "lexical declaration/signature/source PC plan differs",
            ));
        }
        let stamps = db.lexical_namespace_stamps()?;
        let mut metadata_bytes = plan
            .metadata_bytes()
            .checked_add(std::mem::size_of::<Self>())
            .ok_or_else(|| fault("lexical certificate metadata overflow"))?;
        for key in stamps.keys() {
            metadata_bytes = metadata_bytes
                .checked_add(std::mem::size_of::<(InputKey, Digest)>())
                .and_then(|n| n.checked_add(128))
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("lexical certificate metadata limit"))?;
        }
        let proof = Self {
            plan: plan.clone(),
            namespace_stamps: stamps,
            metadata_bytes,
        };
        proof.revalidate(db)?;
        Ok(proof)
    }
}

pub(crate) fn admit_lexical_function(
    db: &mut Database,
    source: &PreparedSourceView,
    plan: &LexicalFunctionPlan,
) -> QueryPoll<Box<SourceCallable>> {
    let admission = match LexicalAdmission::validate(db, source, plan) {
        Ok(proof) => proof,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if let Err(error) = db.publish_lexical_function(&admission) {
        return QueryPoll::Ready(Err(error));
    }
    let identity = plan.item().def_key.fingerprint();
    let outcome = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(identity)) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("lexical function signature outcome differs")));
    };
    if signature.as_ref() != plan.signature() {
        return QueryPoll::Ready(Err(fault(
            "lexical admitted signature differs from captured plan",
        )));
    }
    let callee = SourceCallable {
        definition: plan.item().def_key.clone(),
        identity,
        signature: signature.clone(),
        body: plan.item().body.clone(),
        source: plan.source().clone(),
        revision: db.source_revision(),
        context: DeclarationContext {
            scope: plan.item().scope.clone(),
            range: plan.item().declaration_span,
        },
        lexical: Some(Box::new(admission)),
        ordinary_local: None,
        ordinary_module: None,
        ordinary_owner: None,
        retained: None,
    };
    if let Err(error) = callee.revalidate(db) {
        return QueryPoll::Ready(Err(error));
    }
    QueryPoll::Ready(Ok(Box::new(callee)))
}
