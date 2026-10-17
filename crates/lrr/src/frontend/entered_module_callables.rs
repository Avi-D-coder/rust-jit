//! Canonical module facts and cold admission from an already selected Value.
//! No name lookup, source catalogue census, body lowering or guest effects.
use super::{SourceCallable, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{OutlineKey, QueryKey},
        memo::{Database, PreparedInputGuard, RetainedSourceLease},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    expand::source_discovery::SourceScopeOwner,
    frontend::{
        names::NameDefinition,
        source_locals::PreparedSourceLocalPlan,
        source_value_goals::{SelectedValueDeclaration, ValueGoalDemand},
    },
    source::manifest::{DefKey, ItemKind, Namespace},
    syntax::{
        outline::{BodyHandle, DeclarationContext, HeaderRecipe},
        parse::ParseKey,
        signatures::FunctionSignature,
    },
};
use std::sync::Arc;

const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();
macro_rules! ready {
    ($v:expr) => {
        match $v {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
macro_rules! checked {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
    };
}
fn add(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .ok_or_else(crate::runtime::arena::resource)
}
fn definition_bytes(definition: &DefKey) -> Result<usize, Diagnostic> {
    let mut bytes = add(
        std::mem::size_of::<DefKey>(),
        definition.declared_name.capacity(),
    )?;
    bytes = add(
        bytes,
        definition
            .module_path
            .capacity()
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    for part in &definition.module_path {
        bytes = add(bytes, part.capacity())?;
    }
    Ok(bytes)
}

pub(crate) enum CanonicalModulePreparation {
    Ready(CanonicalModuleDeclaration),
    Pending(ValueGoalDemand),
}
/// Header/source facts only. Non-function kinds remain facts, never callable
/// authority. Actual input Arcs stay in existing input pin accounting.
pub(crate) struct CanonicalModuleDeclaration {
    identity: Digest,
    definition: DefKey,
    source: Arc<InputValue>,
    header: Arc<InputValue>,
    context: Arc<InputValue>,
    row_stamps: [Digest; 3],
    body: Option<BodyHandle>,
    // The actual syntax input/lease owns admitted generated or retained trees.
    // Its complete relevant closure is captured cold in guards below.
    _syntax: Option<Arc<InputValue>>,
    _retained: Option<RetainedSourceLease>,
    guards: Vec<Arc<PreparedInputGuard>>,
    bytes: usize,
    fingerprint: Digest,
}
impl std::fmt::Debug for CanonicalModuleDeclaration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanonicalModuleDeclaration")
            .field("identity", &self.identity)
            .field("kind", &self.definition.item_kind)
            .finish_non_exhaustive()
    }
}
impl CanonicalModuleDeclaration {
    pub(crate) fn definition(&self) -> &DefKey {
        &self.definition
    }
    pub(crate) fn source(&self) -> &ParseKey {
        let InputValue::DefinitionSource(source) = self.source.as_ref() else {
            unreachable!()
        };
        source
    }
    pub(crate) fn header(&self) -> &HeaderRecipe {
        let InputValue::DefinitionHeader(header) = self.header.as_ref() else {
            unreachable!()
        };
        header
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!()
        };
        context
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn revalidate(&self, db: &Database) -> Result<(), Diagnostic> {
        for guard in &self.guards {
            guard.require_current(db)?;
        }
        if let Some(expected) = &self._syntax {
            let (key, same) = match expected.as_ref() {
                InputValue::GeneratedSyntax(expected) => {
                    let key = InputKey::GeneratedSyntax(expected.key.content);
                    let same = matches!(db.published_input(&key),
                        Some((InputValue::GeneratedSyntax(actual), _)) if Arc::ptr_eq(actual, expected));
                    (key, same)
                }
                InputValue::RetainedSyntax(expected) => {
                    let key = InputKey::RetainedSyntax(expected.key().content);
                    let same = matches!(db.published_input(&key),
                        Some((InputValue::RetainedSyntax(actual), _)) if Arc::ptr_eq(actual, expected));
                    (key, same)
                }
                _ => return Err(fault("selected module syntax authority variant differs")),
            };
            if !same || db.published_input(&key).is_none() {
                return Err(fault("selected module syntax publication changed"));
            }
        }
        for ((key, expected), expected_stamp) in [
            (InputKey::DefinitionSource(self.identity), &self.source),
            (InputKey::DefinitionHeader(self.identity), &self.header),
            (InputKey::DeclarationContext(self.identity), &self.context),
        ]
        .into_iter()
        .zip(self.row_stamps)
        {
            let Some((actual, stamp)) = db.published_input(&key) else {
                return Err(fault("selected module declaration row disappeared"));
            };
            let same = match (actual, expected.as_ref()) {
                (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => a == b,
                (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => a == b,
                (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => a == b,
                _ => false,
            };
            if !same || stamp != expected_stamp {
                return Err(fault("selected module declaration row changed"));
            }
        }
        Ok(())
    }
}
impl StableEncode for CanonicalModuleDeclaration {
    const DOMAIN: &'static str = "lrr/v1/canonical-selected-module-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.definition);
        e.record(self.source());
        e.digest(self.header().fingerprint());
        e.record(self.context());
        e.option(&self.body, |e, body| e.record(body));
        e.sequence(&self.guards, |e, guard| e.digest(guard.fingerprint()));
    }
}
fn pending(demand: ValueGoalDemand) -> QueryPoll<CanonicalModulePreparation> {
    QueryPoll::Ready(Ok(CanonicalModulePreparation::Pending(demand)))
}

/// Shared canonical fact projection for binding classification and module
/// admission. Input arguments must match the actual published rows. This never
/// selects a spelling, publishes a body locator or prepares executable code.
pub(crate) fn canonical_module_declaration(
    db: &mut Database,
    parent: &PreparedSourceLocalPlan,
    selected: &NameDefinition,
    source: &ParseKey,
    header: &HeaderRecipe,
    context: &DeclarationContext,
) -> QueryPoll<CanonicalModulePreparation> {
    ready!(parent.require_current(db));
    let id = selected.identity;
    if selected.namespace != Namespace::Value
        || context.scope.owner.is_some()
        || selected.defining_unit != context.scope.crate_key
        || header.profile != source.profile
    {
        return QueryPoll::Ready(Err(fault("selected module source/namespace/owner differs")));
    }
    let Some(source_input) = db.input(&InputKey::DefinitionSource(id)) else {
        return pending(ValueGoalDemand::DeclarationPublication(id));
    };
    let Some(header_input) = db.input(&InputKey::DefinitionHeader(id)) else {
        return pending(ValueGoalDemand::DeclarationPublication(id));
    };
    let Some(context_input) = db.input(&InputKey::DeclarationContext(id)) else {
        return pending(ValueGoalDemand::DeclarationPublication(id));
    };
    if !matches!(source_input.as_ref(), InputValue::DefinitionSource(actual) if actual == source)
        || !matches!(header_input.as_ref(), InputValue::DefinitionHeader(actual) if actual == header)
        || !matches!(context_input.as_ref(), InputValue::DeclarationContext(actual) if actual == context)
    {
        return QueryPoll::Ready(Err(fault("selected module actual declaration rows differ")));
    }
    let mut guards = Vec::new();
    checked!(
        guards
            .try_reserve_exact(2)
            .map_err(|_| crate::runtime::arena::resource())
    );
    let mut syntax = None;
    let mut retained = None;
    let (definition, body) = if let Some(input) =
        db.input(&InputKey::GeneratedSyntax(source.content))
    {
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "selected module generated source variant differs",
            )));
        };
        let Some(definition) = unit.definitions.get(&id) else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        let mut generated_headers = unit
            .items
            .items()
            .iter()
            .filter(|item| item.key.fingerprint() == id);
        let Some(generated_header) = generated_headers.next() else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        if unit.key != *source
            || generated_headers.next().is_some()
            || definition.crate_key != selected.defining_unit
            || definition.module_path != context.scope.module_path
            || context.scope.crate_key != unit.scope.crate_key
            || !context
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
            || generated_header.start != context.range.start
            || generated_header.end != context.range.end
        {
            return QueryPoll::Ready(Err(fault("selected module generated attachment differs")));
        }
        let Some(environment) = parent.environment() else {
            return pending(ValueGoalDemand::ModulePublication);
        };
        let Some(declared) = environment
            .macros()
            .source_environment()
            .names()
            .declared_item_name(Namespace::Value, id)
        else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        let Some(name) = &generated_header.name else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        if declared.definition() != selected
            || declared.scope() != &context.scope
            || declared.range().start != name.start
            || declared.range().end != name.end
            || name.text != definition.declared_name
        {
            return QueryPoll::Ready(Err(fault(
                "selected module declared name attachment differs",
            )));
        }
        checked!(declared.require_generated_source(unit));
        checked!(definition_bytes(definition));
        let key = QueryKey::Outline(OutlineKey {
            parse: source.clone(),
            crate_key: unit.scope.crate_key,
            module_path: unit.scope.module_path.clone(),
            cfg: unit.cfg.fingerprint(),
        });
        let outlined = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::Outline(outline) = outlined.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected generated outline variant differs")));
        };
        let mut rows = outline
            .items
            .iter()
            .filter(|item| item.declaration_span == context.range);
        let Some(item) = rows.next() else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        checked!(definition_bytes(&item.def_key));
        if rows.next().is_some()
            || item.def_key != *definition
            || item.scope != context.scope
            || item.header != *header
        {
            return QueryPoll::Ready(Err(fault("selected generated canonical row differs")));
        }
        let mut body = item.body.clone();
        if let Some(body) = &mut body {
            body.def_key = id;
        }
        guards.push(Arc::new(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            outlined.fingerprint
        ))));
        syntax = Some(Arc::clone(&input));
        (definition.clone(), body)
    } else {
        let Some(environment) = parent.environment() else {
            return pending(ValueGoalDemand::ModulePublication);
        };
        let environment = environment.macros().source_environment();
        let SourceScopeOwner::Selected(owner) = environment.scope_owner(&context.scope) else {
            return pending(ValueGoalDemand::SourceOwner(id));
        };
        let key = QueryKey::Outline(owner.selection().outline.clone());
        let outlined = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::Outline(outline) = outlined.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected module outline variant differs")));
        };
        let mut rows = outline
            .items
            .iter()
            .filter(|item| item.def_key.fingerprint() == id);
        let Some(item) = rows.next() else {
            return pending(ValueGoalDemand::DeclarationOccurrence(id));
        };
        checked!(definition_bytes(&item.def_key));
        let value_member = outline.scopes.get(&context.scope).is_some_and(|scope| {
            scope
                .candidates
                .get(&(Namespace::Value, item.def_key.declared_name.clone()))
                .is_some_and(|candidates| candidates.contains(&id))
        });
        if rows.next().is_some()
            || item.scope != context.scope
            || item.header.profile.edition != selected.definition_edition
            || !(item.def_key.namespace == Namespace::Value
                || item.def_key.item_kind == ItemKind::Struct && value_member)
        {
            return QueryPoll::Ready(Err(fault("selected module canonical scope differs")));
        }
        guards.push(Arc::new(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            outlined.fingerprint
        ))));
        let body = if *source == owner.selection().outline.parse {
            if item.declaration_span != context.range || item.header != *header {
                return QueryPoll::Ready(Err(fault("selected original module attachment differs")));
            }
            item.body.clone()
        } else {
            let Some(input) = db.input(&InputKey::RetainedSyntax(source.content)) else {
                return pending(ValueGoalDemand::DeclarationOccurrence(id));
            };
            let InputValue::RetainedSyntax(unit) = input.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "selected retained module source variant differs",
                )));
            };
            ready!(unit.require_current(db));
            let anchor = unit
                .prepared()
                .original_declarations()
                .iter()
                .find(|anchor| anchor.original_definition() == id);
            if unit.key() != source
                || unit.prepared().source() != owner.selection()
                || !anchor.is_some_and(|anchor| {
                    anchor.original_range() == item.declaration_span
                        && anchor.declaration_range() == context.range
                })
                || !item.header.same_semantics(header)
            {
                return QueryPoll::Ready(Err(fault("selected retained module attachment differs")));
            }
            let mut retained_key = owner.selection().outline.clone();
            retained_key.parse = source.clone();
            let retained_key = QueryKey::Outline(retained_key);
            let current = ready!(db.fetch(&SyntaxProvider, retained_key.clone()));
            let QueryValue::Outline(current_outline) = current.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("selected retained outline variant differs")));
            };
            let mut actual_rows = current_outline
                .items
                .iter()
                .filter(|actual| actual.declaration_span == context.range);
            let Some(actual) = actual_rows.next() else {
                return pending(ValueGoalDemand::DeclarationOccurrence(id));
            };
            if actual_rows.next().is_some()
                || actual.scope != context.scope
                || actual.header != *header
                || actual.def_key.item_kind != item.def_key.item_kind
                || actual.def_key.declared_name != item.def_key.declared_name
            {
                return QueryPoll::Ready(Err(fault("selected retained canonical row differs")));
            }
            let mut body = actual.body.clone();
            if let Some(body) = &mut body {
                body.def_key = id;
            }
            if item.def_key.item_kind == ItemKind::Function && body.is_some() {
                let Some(current) = ready!(db.retained_original_declaration(owner, id)) else {
                    return QueryPoll::Ready(Err(fault("retained module function lease absent")));
                };
                if current.parse() != source
                    || current.header() != header
                    || current.context() != context
                    || Some(current.body()) != body.as_ref()
                {
                    return QueryPoll::Ready(Err(fault("retained module function lease differs")));
                }
                retained = Some(current.into_lease());
            }
            guards.push(Arc::new(ready!(db.prepare_input_guard(
                &SyntaxProvider,
                &retained_key,
                current.fingerprint
            ))));
            syntax = Some(Arc::clone(&input));
            body
        };
        (item.def_key.clone(), body)
    };
    let mut bytes = checked!(add(
        std::mem::size_of::<CanonicalModuleDeclaration>(),
        checked!(definition_bytes(&definition).and_then(|bytes| {
            bytes
                .checked_sub(std::mem::size_of::<DefKey>())
                .ok_or_else(crate::runtime::arena::resource)
        }))
    ));
    bytes = checked!(add(
        bytes,
        checked!(
            guards
                .capacity()
                .checked_mul(std::mem::size_of::<Arc<PreparedInputGuard>>())
                .ok_or_else(crate::runtime::arena::resource)
        )
    ));
    for guard in &guards {
        bytes = checked!(add(
            bytes,
            checked!(add(guard.metadata_bytes(), ARC_HEADER))
        ));
    }
    let mut row_stamps = [Digest([0; 32]); 3];
    for (ordinal, (key, input)) in [
        (InputKey::DefinitionSource(id), &source_input),
        (InputKey::DefinitionHeader(id), &header_input),
        (InputKey::DeclarationContext(id), &context_input),
    ]
    .into_iter()
    .enumerate()
    {
        let Some((_, stamp)) = db.published_input(&key) else {
            return QueryPoll::Ready(Err(fault("selected module row stamp absent")));
        };
        if stamp != input.fingerprint() {
            return QueryPoll::Ready(Err(fault("selected module actual row stamp differs")));
        }
        row_stamps[ordinal] = stamp;
    }
    let mut canonical = CanonicalModuleDeclaration {
        identity: id,
        definition,
        source: source_input,
        header: header_input,
        context: context_input,
        body,
        row_stamps,
        _syntax: syntax,
        _retained: retained,
        guards,
        bytes,
        fingerprint: Digest([0; 32]),
    };
    canonical.fingerprint = canonical.fingerprint();
    checked!(canonical.revalidate(db));
    QueryPoll::Ready(Ok(CanonicalModulePreparation::Ready(canonical)))
}

pub(crate) enum ModuleFunctionPreparation {
    Ready(Arc<EnteredModuleAdmission>),
    Pending(ValueGoalDemand),
}
#[derive(Debug)]
pub(crate) struct EnteredModuleAdmission {
    parent: Arc<PreparedSourceLocalPlan>,
    declaration: SelectedValueDeclaration,
    canonical: CanonicalModuleDeclaration,
    signature: Box<FunctionSignature>,
    signature_identity: Digest,
    selection_guard: Arc<PreparedInputGuard>,
    signature_guard: Arc<PreparedInputGuard>,
    body_stamp: Option<Digest>,
    bytes: usize,
}
impl EnteredModuleAdmission {
    pub(crate) fn parent(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.parent
    }
    pub(crate) fn declaration(&self) -> &SelectedValueDeclaration {
        &self.declaration
    }
    pub(crate) fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn selection_guard(&self) -> &Arc<PreparedInputGuard> {
        &self.selection_guard
    }
    pub(crate) fn revalidate(&self, db: &Database) -> Result<(), Diagnostic> {
        self.selection_guard.require_current(db)?;
        self.signature_guard.require_current(db)?;
        self.canonical.revalidate(db)?;
        let key = InputKey::DefinitionBody(self.canonical.identity);
        match (
            self.canonical.body.as_ref(),
            db.published_input(&key),
            self.body_stamp,
        ) {
            (None, None, None) => {}
            (
                Some(expected),
                Some((InputValue::DefinitionBody(actual), stamp)),
                Some(expected_stamp),
            ) if actual == expected && stamp == expected_stamp => {}
            _ => return Err(fault("selected module callable body locator changed")),
        }
        Ok(())
    }
    /// Projection of retained facts only: no source query/parse/name lookup.
    pub(crate) fn callable_shared(
        self: &Arc<Self>,
        db: &mut Database,
    ) -> Result<Box<SourceCallable>, Diagnostic> {
        self.revalidate(db)?;
        let callee = SourceCallable {
            identity: self.canonical.identity,
            definition: self.canonical.definition.clone(),
            signature: self.signature.clone(),
            body: self.canonical.body.clone(),
            source: self.canonical.source().clone(),
            revision: db.source_revision(),
            context: self.canonical.context().clone(),
            lexical: None,
            ordinary_local: None,
            ordinary_module: Some(Arc::clone(self)),
            ordinary_owner: Some(Arc::clone(&self.parent)),
            retained: None,
        };
        callee.revalidate(db)?;
        Ok(Box::new(callee))
    }
}
impl StableEncode for EnteredModuleAdmission {
    const DOMAIN: &'static str = "lrr/v1/entered-selected-module-function";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.parent.source_identity());
        e.record(self.declaration.definition());
        e.record(self.declaration.source());
        e.digest(self.declaration.header());
        e.record(self.declaration.context());
        e.digest(self.canonical.fingerprint);
        e.digest(self.signature_identity);
        e.digest(self.selection_guard.fingerprint());
        e.digest(self.signature_guard.fingerprint());
    }
}
/// Called by the cold entered-call goal with its actual selected declaration
/// and private current Value input guard. No new namespace answer is obtained.
pub(crate) fn prepare_entered_module_function(
    db: &mut Database,
    parent: Arc<PreparedSourceLocalPlan>,
    declaration: &SelectedValueDeclaration,
    selection_guard: Arc<PreparedInputGuard>,
) -> QueryPoll<ModuleFunctionPreparation> {
    checked!(selection_guard.require_current(db));
    if declaration.owner().is_some() {
        return QueryPoll::Ready(Ok(ModuleFunctionPreparation::Pending(
            ValueGoalDemand::DeclarationOwner(declaration.identity()),
        )));
    }
    let Some(header) = db.input(&InputKey::DefinitionHeader(declaration.identity())) else {
        return QueryPoll::Ready(Ok(ModuleFunctionPreparation::Pending(
            ValueGoalDemand::DeclarationPublication(declaration.identity()),
        )));
    };
    let InputValue::DefinitionHeader(header) = header.as_ref() else {
        return QueryPoll::Ready(Err(fault("selected module header variant differs")));
    };
    if header.fingerprint() != declaration.header() {
        return QueryPoll::Ready(Err(fault("selected module header identity differs")));
    }
    let canonical = match ready!(canonical_module_declaration(
        db,
        &parent,
        declaration.definition(),
        declaration.source(),
        header,
        declaration.context()
    )) {
        CanonicalModulePreparation::Ready(canonical) => canonical,
        CanonicalModulePreparation::Pending(demand) => {
            return QueryPoll::Ready(Ok(ModuleFunctionPreparation::Pending(demand)));
        }
    };
    if canonical.definition.item_kind != ItemKind::Function {
        return QueryPoll::Ready(Ok(ModuleFunctionPreparation::Pending(
            ValueGoalDemand::BindingClassification(declaration.identity()),
        )));
    }
    let body_key = InputKey::DefinitionBody(declaration.identity());
    // Admission observes the real body locator, without preparing executable
    // syntax. Record that dependency in the entered call's query closure too;
    // published_input alone deliberately grants no dependency observation.
    let body_input = db.input(&body_key);
    let body_stamp = match (body_input.as_deref(), db.published_input(&body_key)) {
        (
            Some(InputValue::DefinitionBody(observed)),
            Some((InputValue::DefinitionBody(actual), stamp)),
        ) if observed == actual
            && canonical.body.as_ref() == Some(actual)
            && stamp == InputValue::DefinitionBody(actual.clone()).fingerprint() =>
        {
            Some(stamp)
        }
        (None, None) => {
            return QueryPoll::Ready(Ok(ModuleFunctionPreparation::Pending(
                ValueGoalDemand::DeclarationPublication(declaration.identity()),
            )));
        }
        _ => return QueryPoll::Ready(Err(fault("selected module canonical body locator differs"))),
    };
    let key = QueryKey::FunctionSignature(declaration.identity());
    let signature = ready!(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::FunctionSignature(actual) = signature.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("selected module signature variant differs")));
    };
    if !signature.cacheable
        || actual.definition != declaration.identity()
        || actual.scope != canonical.context().scope
        || !actual.header.same_semantics(canonical.header())
    {
        return QueryPoll::Ready(Err(fault("selected module signature declaration differs")));
    }
    let signature_guard = Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &key,
        signature.fingerprint
    )));
    let bytes = checked!(add(
        std::mem::size_of::<EnteredModuleAdmission>(),
        ARC_HEADER
    ));
    let canonical_heap = checked!(
        canonical
            .metadata_bytes()
            .checked_sub(std::mem::size_of::<CanonicalModuleDeclaration>())
            .ok_or_else(crate::runtime::arena::resource)
    );
    let bytes = checked!(add(bytes, canonical_heap));
    let bytes = checked!(add(bytes, actual.estimated_bytes()));
    let bytes = checked!(add(
        bytes,
        checked!(add(selection_guard.metadata_bytes(), ARC_HEADER))
    ));
    let bytes = checked!(add(
        bytes,
        checked!(add(signature_guard.metadata_bytes(), ARC_HEADER))
    ));
    let bytes = checked!(add(
        bytes,
        declaration.source().profile.semantics.capacity()
    ));
    let bytes = checked!(add(bytes, declaration.context().scope.heap_bytes()));
    let mut owned_signature = actual.clone();
    owned_signature.header = canonical.header().clone();
    if let Some(body) = &canonical.body {
        checked!(body.require_entry());
    }
    let admission = Arc::new(EnteredModuleAdmission {
        parent,
        declaration: declaration.clone(),
        canonical,
        signature: owned_signature,
        signature_identity: signature.fingerprint,
        selection_guard,
        signature_guard,
        body_stamp,
        bytes,
    });
    checked!(admission.revalidate(db));
    QueryPoll::Ready(Ok(ModuleFunctionPreparation::Ready(admission)))
}

#[cfg(test)]
#[path = "entered_module_callable_tests.rs"]
mod tests;
