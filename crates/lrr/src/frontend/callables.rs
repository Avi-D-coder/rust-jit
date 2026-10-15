//! Direct module-function identity before argument evaluation; no body entry.
use super::names::{NameLookup, NameNamespaces, NamePath};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, RetainedOriginalDeclaration, RetainedSourceLease},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind, Namespace},
    },
    syntax::{
        outline::{BodyHandle, DeclarationContext, ScopeKey},
        parse::ParseKey,
        signatures::FunctionSignature,
    },
};
#[path = "lexical_callables.rs"]
pub(crate) mod lexical;
pub(crate) use lexical::admit_lexical_function;

#[derive(Debug, Clone)]
pub struct SourceCallable {
    definition: DefKey,
    identity: Digest,
    signature: Box<FunctionSignature>,
    body: Option<BodyHandle>,
    source: ParseKey,
    revision: u64,
    context: DeclarationContext,
    lexical: Option<Box<lexical::LexicalAdmission>>,
    retained: Option<RetainedSourceLease>,
}
impl SourceCallable {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn definition(&self) -> &DefKey {
        &self.definition
    }
    pub fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    pub fn body(&self) -> Option<&BodyHandle> {
        self.body.as_ref()
    }
    pub(crate) fn lexical_contract(&self) -> Option<&lexical::LexicalAdmission> {
        self.lexical.as_deref()
    }
    pub(crate) fn retained_lexical_metadata_bytes(&self) -> usize {
        self.lexical
            .as_ref()
            .map_or(0, |proof| proof.metadata_bytes())
            .saturating_add(std::mem::size_of::<Option<RetainedSourceLease>>())
    }
    pub fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        if let Some(proof) = &self.lexical {
            proof.revalidate(db)?;
        }
        if db.source_revision() != self.revision {
            return Err(fault("selected callable source revision changed"));
        }
        if let Some(lease) = &self.retained {
            match lease.require_current(db) {
                QueryPoll::Ready(result) => result?,
                QueryPoll::NeedInput(_) => {
                    return Err(fault(
                        "selected callable retained discovery is no longer available",
                    ));
                }
            }
        }
        let definition = self.identity;
        if !matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),Some(InputValue::DefinitionSource(key))if *key==self.source)
        {
            return Err(fault("selected callable source locator changed"));
        }
        if !matches!(db.input(&InputKey::DefinitionHeader(definition)).as_deref(),Some(InputValue::DefinitionHeader(header))if header==&self.signature.header)
        {
            return Err(fault("selected callable header changed"));
        }
        if !matches!(db.input(&InputKey::DeclarationContext(definition)).as_deref(),Some(InputValue::DeclarationContext(context))if context==&self.context)
        {
            return Err(fault("selected callable declaration context changed"));
        }
        let body = db.input(&InputKey::DefinitionBody(definition));
        match (self.body.as_ref(), body.as_deref()) {
            (None, None) => {}
            (Some(expected), Some(InputValue::DefinitionBody(current))) if current == expected => {}
            _ => return Err(fault("selected callable body handle changed")),
        }
        Ok(())
    }
}
/// Fixed-size, owned proof that a reached call selects an ordinary function.
/// Constructed only after exact caller/callee input revalidation.
#[derive(Debug, Clone)]
pub(crate) struct OrdinaryCallWitness {
    caller: crate::source::manifest::BodyKey,
    site: crate::ir::pc::NodeId,
    definition: Digest,
}
impl OrdinaryCallWitness {
    pub(crate) fn require(
        &self,
        caller: &crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
    ) -> Result<Digest, Diagnostic> {
        if &self.caller != caller || self.site != site {
            return Err(fault(
                "ordinary call scope witness belongs to another body/site",
            ));
        }
        Ok(self.definition)
    }
}
impl SourceCallable {
    pub(crate) fn ordinary_call_witness(
        &self,
        db: &mut Database,
        caller: crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
    ) -> Result<OrdinaryCallWitness, Diagnostic> {
        self.revalidate(db)?;
        if self.definition.item_kind != ItemKind::Function {
            return Err(fault(
                "temporary-scope barrier requires a selected source function",
            ));
        }
        if !matches!(db.input(&InputKey::DefinitionBody(caller.definition)).as_deref(),
            Some(InputValue::DefinitionBody(body)) if body.key() == caller)
        {
            return Err(fault(
                "ordinary call scope witness requires the exact captured caller body",
            ));
        }
        Ok(OrdinaryCallWitness {
            caller,
            site,
            definition: self.identity,
        })
    }
}

#[derive(Debug)]
pub enum CallSelection {
    Source(Box<SourceCallable>),
    NeedProducer(Digest),
    NeedOwnerContext(ScopeKey),
    NeedValue(DefKey),
    Unresolved(NameLookup),
}
pub struct CallResolver<'a> {
    graph: &'a NameNamespaces,
    sources: &'a [&'a PreparedSourceView],
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
impl<'a> CallResolver<'a> {
    pub fn new(
        graph: &'a NameNamespaces,
        sources: &'a [&'a PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        if sources.len() > 100_000 {
            return Err(fault("call source catalogue limit exceeded"));
        }
        for source in sources {
            graph.require_source(source.selection(), source.outline())?;
        }
        Ok(Self { graph, sources })
    }
    /// Module lookup only. Executed local ribs/callee values route before this API.
    pub fn module_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        let found =
            match self
                .graph
                .lookup_unqualified(Namespace::Value, name, &scope, edition, None)
            {
                Ok(found) => found,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
        self.select(db, found)
    }
    /// Owned source spelling from the entered call recipe; no AST or runtime effects.
    pub fn source_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        if !name.contains("::") {
            return self.module_function(db, caller, name);
        }
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        if name.len() > 4096 {
            return QueryPoll::Ready(Err(fault("source call path text limit exceeded")));
        }
        let absolute = name.starts_with("::");
        let text = if absolute { &name[2..] } else { name };
        let mut segments = Vec::new();
        for part in text.split("::") {
            if segments.len() >= 256 {
                return QueryPoll::Ready(Err(fault("source call path depth exceeded")));
            }
            use crate::syntax::imports::ImportSegment;
            let segment = match part {
                "crate" => ImportSegment::Crate,
                "self" => ImportSegment::SelfModule,
                "super" => ImportSegment::Super,
                _ => {
                    if let Err(e) = crate::expand::scopes::MacroName::parse(part, edition) {
                        return QueryPoll::Ready(Err(e));
                    }
                    ImportSegment::Name(part.into())
                }
            };
            segments.push(segment);
        }
        match self
            .graph
            .lookup_source_path(Namespace::Value, &segments, absolute, &scope, edition)
        {
            Ok(found) => self.select(db, found),
            Err(e) => QueryPoll::Ready(Err(e)),
        }
    }
    pub fn qualified(
        &self,
        db: &mut Database,
        caller: Digest,
        path: &NamePath,
    ) -> QueryPoll<CallSelection> {
        let (scope, _) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        let found = match self.graph.lookup(Namespace::Value, path, &scope) {
            Ok(found) => found,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        self.select(db, found)
    }
    fn caller_context(&self, db: &mut Database, caller: Digest) -> QueryPoll<(ScopeKey, u16)> {
        if let Some((source, item)) = self.item(caller) {
            match self.current(db, source, caller) {
                QueryPoll::Ready(Ok(Some(current))) => {
                    return QueryPoll::Ready(Ok((
                        current.context().scope.clone(),
                        current.header().profile.edition,
                    )));
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            }
            return QueryPoll::Ready(Ok((item.scope.clone(), item.header.profile.edition)));
        }
        let input = match db.input(&InputKey::DefinitionSource(caller)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated caller source absent"))),
        };
        let InputValue::DefinitionSource(parse) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated caller source variant differs")));
        };
        let generated = match db.input(&InputKey::GeneratedSyntax(parse.content)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated caller syntax absent"))),
        };
        let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated caller syntax variant differs")));
        };
        let Some(definition) = unit.definitions.get(&caller) else {
            return QueryPoll::Ready(Err(fault("generated caller identity absent")));
        };
        let context = match db.input(&InputKey::DeclarationContext(caller)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated caller context absent"))),
        };
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated caller context variant differs")));
        };
        if context.scope.owner.is_some()
            || context.scope.crate_key != unit.scope.crate_key
            || context.scope.crate_key != definition.crate_key
            || context.scope.module_path != definition.module_path
            || !context
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
        {
            return QueryPoll::Ready(Err(fault("generated caller owner differs")));
        }
        match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(_)) => {
                QueryPoll::Ready(Ok((context.scope.clone(), unit.key.profile.edition)))
            }
            QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
        }
    }
    fn select_generated(&self, db: &mut Database, identity: Digest) -> QueryPoll<CallSelection> {
        let source = match db.input(&InputKey::DefinitionSource(identity)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("selected generated source absent"))),
        };
        let InputValue::DefinitionSource(parse) = source.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected generated source variant differs")));
        };
        let generated = match db.input(&InputKey::GeneratedSyntax(parse.content)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("selected generated syntax absent"))),
        };
        let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected generated syntax variant differs")));
        };
        let Some(definition) = unit.definitions.get(&identity) else {
            return QueryPoll::Ready(Err(fault("selected generated declaration identity absent")));
        };
        if definition.item_kind != ItemKind::Function {
            return QueryPoll::Ready(Ok(CallSelection::NeedValue(definition.clone())));
        }
        let outcome = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(identity)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated signature variant differs")));
        };
        let context = match db.input(&InputKey::DeclarationContext(identity)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated declaration context absent"))),
        };
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated declaration context variant differs")));
        };
        if signature.scope != context.scope
            || context.scope.owner.is_some()
            || context.scope.crate_key != unit.scope.crate_key
            || context.scope.crate_key != definition.crate_key
            || context.scope.module_path != definition.module_path
            || !context
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
        {
            return QueryPoll::Ready(Err(fault("generated callable owner differs")));
        }
        let body = match db.input(&InputKey::DefinitionBody(identity)).as_deref() {
            Some(InputValue::DefinitionBody(body)) => Some(body.clone()),
            None => None,
            _ => return QueryPoll::Ready(Err(fault("generated body variant differs"))),
        };
        let callee = SourceCallable {
            identity,
            definition: definition.clone(),
            signature: signature.clone(),
            body,
            source: parse.clone(),
            revision: db.source_revision(),
            context: context.clone(),
            lexical: None,
            retained: None,
        };
        if let Err(e) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(e));
        }
        QueryPoll::Ready(Ok(CallSelection::Source(Box::new(callee))))
    }
    fn item(
        &self,
        definition: Digest,
    ) -> Option<(&PreparedSourceView, &crate::syntax::outline::ItemOutline)> {
        self.sources.iter().find_map(|source| {
            source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.fingerprint() == definition)
                .map(|item| (*source, item))
        })
    }
    fn current(
        &self,
        db: &mut Database,
        source: &PreparedSourceView,
        definition: Digest,
    ) -> QueryPoll<Option<RetainedOriginalDeclaration>> {
        db.retained_original_declaration(source, definition)
    }
    pub(crate) fn select(&self, db: &mut Database, found: NameLookup) -> QueryPoll<CallSelection> {
        let selected = match found {
            NameLookup::Defined(selected) => selected,
            NameLookup::Absent => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    "reached direct function identity is absent before argument evaluation".into(),
                )));
            }
            NameLookup::NeedInput(producer) => {
                return QueryPoll::Ready(Ok(CallSelection::NeedProducer(producer)));
            }
            other => return QueryPoll::Ready(Ok(CallSelection::Unresolved(other))),
        };
        let Some((source, item)) = self.item(selected.identity) else {
            return self.select_generated(db, selected.identity);
        };
        let retained = match self.current(db, source, selected.identity) {
            QueryPoll::Ready(Ok(current)) => current,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        if item.scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(item.scope.clone())));
        }
        if item.def_key.item_kind != ItemKind::Function {
            return QueryPoll::Ready(Ok(CallSelection::NeedValue(item.def_key.clone())));
        }
        let outcome = match db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(selected.identity),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "selected function signature query result mismatch",
            )));
        };
        let header = retained
            .as_ref()
            .map_or(&item.header, |current| current.header());
        let context = retained.as_ref().map_or_else(
            || DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            |current| current.context().clone(),
        );
        if signature.scope != context.scope
            || signature.header.fingerprint() != header.fingerprint()
            || retained.is_none() && signature.header != *header
        {
            return QueryPoll::Ready(Err(fault("selected signature source declaration mismatch")));
        }
        // Cached signatures carry semantic header identity. Reattach the actual
        // current header locations without changing any inferred declaration.
        let mut signature = signature.clone();
        signature.header = header.clone();
        let body = retained
            .as_ref()
            .map_or_else(|| item.body.clone(), |current| Some(current.body().clone()));
        let parse = retained.as_ref().map_or_else(
            || source.selection().outline.parse.clone(),
            |current| current.parse().clone(),
        );
        let callee = SourceCallable {
            definition: item.def_key.clone(),
            identity: selected.identity,
            signature,
            body,
            source: parse,
            revision: db.source_revision(),
            context,
            lexical: None,
            retained: retained.map(RetainedOriginalDeclaration::into_lease),
        };
        if let Err(error) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        QueryPoll::Ready(Ok(CallSelection::Source(Box::new(callee))))
    }
}
