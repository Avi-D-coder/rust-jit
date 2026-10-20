//! Current admitted owning body. Names, members and constructors consume this
//! source receipt; it performs no namespace selection or guest operation.
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
    ir::pc::{NodeId, PcKey, Phase},
    source::changes::PreparedSourceView,
    syntax::{
        body_index::{BodyIndex,NodeRole},
        outline::{BodyHandle, DeclarationContext, HeaderRecipe, ScopeKey},
        parse::ParsedFile,
        token_wire::{Edition, SpanRecord},
    },
};
use std::sync::Arc;
#[path = "body_import_occurrences.rs"]
mod import_occurrences;
#[path = "local_body_context.rs"]
mod local_body;
pub(crate) use local_body::capture_local;
#[path = "body_type_occurrences.rs"]
pub(crate) mod type_occurrences;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
pub(crate) struct AdmittedBodyContext {
    parsed: Arc<ParsedFile>,
    index: Arc<BodyIndex>,
    context: Arc<InputValue>,
    header: Arc<InputValue>,
    body: Arc<InputValue>,
    source: Arc<InputValue>,
    cfg: Arc<InputValue>,
    source_receipt: Option<Arc<InputValue>>,
    original: Option<PreparedSourceView>,
    generated_boundary: Option<Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>>,
    local_body: Option<local_body::LocalBodyContext>,
    identity: Digest,
    input_stamps: [Digest; 6],
    exclusive_bytes: usize,
}
impl AdmittedBodyContext {
    /// Exact immutable indexed Type-use location. Current source/query guards
    /// are supplied by the owning preparation result, not by this projection.
    pub(crate) fn require_type_site(
        &self,index:&BodyIndex,site:&PcKey,range:crate::syntax::spans::SourceRange,
    )->Result<(),Diagnostic>{
        let error=|message:&str|Diagnostic::new("LRR-E-TYPE-CONTEXT",message.into());
        if site.phase!=0 || index.body()!=&site.body || self.body().key()!=site.body
            || self.index().semantic_identity()!=index.semantic_identity()
            || !self.body().range.contains(range) {
            return Err(error("prepared Type use source/body/range differs"));
        }
        let node=index.nodes().get(site.node as usize)
            .filter(|node|node.id==NodeId(site.node)&&node.role==NodeRole::Syntax)
            .ok_or_else(||error("prepared Type use indexed occurrence differs"))?;
        let start=self.body().range.start;
        if start.checked_add(node.range.start)!=Some(range.start)
            || start.checked_add(node.range.end)!=Some(range.end)
            || range.start.checked_sub(start)!=Some(node.range.start)
            || range.end.checked_sub(start)!=Some(node.range.end) {
            return Err(error("prepared Type use indexed range differs"));
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub(crate) fn retained_inputs(&self) -> impl Iterator<Item = &Arc<InputValue>> {
        [
            &self.source,
            &self.body,
            &self.header,
            &self.context,
            &self.cfg,
        ]
        .into_iter()
        .chain(self.source_receipt.iter())
        .chain(self.local_body.iter().map(|local| local.owner_input()))
    }
    pub(crate) fn local_parent(
        &self,
    ) -> Option<&Arc<super::source_locals::PreparedSourceLocalPlan>> {
        self.local_body.as_ref().map(|local| local.parent())
    }
    /// Execution ownership for an actual local declaration. Immutable child
    /// closure evidence is minted at capture; this consumes that evidence and
    /// existing source currentness without parsing or source planning.
    pub(crate) fn require_execution_readiness(
        &self,
        db: &mut Database,
        body: &crate::source::manifest::BodyKey,
        parse: &crate::syntax::parse::ParseKey,
    ) -> QueryPoll<()> {
        let checked = (|| {
            if self.body().key() != *body
                || self.index.body() != body
                || self.parsed.key() != *parse
                || self.local_body.is_none()
            {
                return Err(fault("local execution body/index/source differs"));
            }
            let mut current = self;
            while let Some(local) = &current.local_body {
                let parent = local.parent().context().source();
                let same_receipt = match (&current.source_receipt, &parent.source_receipt) {
                    (None, None) => true,
                    (Some(child), Some(owner)) => Arc::ptr_eq(child, owner),
                    _ => false,
                };
                if parent.parsed.key() != *parse
                    || !same_receipt
                    || !parent.body().range.contains(current.body().range)
                    || parent.body().range == current.body().range
                {
                    return Err(fault("local execution parent source chain differs"));
                }
                if matches!(
                    current.source_receipt.as_deref(),
                    Some(InputValue::RetainedSyntax(_))
                ) && !local.has_retained_readiness()
                {
                    return Err(fault("local execution lacks child structural readiness"));
                }
                current = parent;
            }
            if let Some(InputValue::RetainedSyntax(unit)) = current.source_receipt.as_deref()
                && (unit.key() != parse || !unit.prepared().permits_body(current.body().def_key))
            {
                return Err(fault("local execution root readiness differs"));
            }
            Ok(())
        })();
        if let Err(error) = checked {
            return QueryPoll::Ready(Err(error));
        }
        // One recursive check covers local owner, source/cfg/epoch and ancestor
        // dependencies. Do not repeat it for every edge of the walk above.
        self.require_current(db)
    }
    pub(crate) fn generated_boundary(
        &self,
    ) -> Option<&Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>> {
        self.generated_boundary.as_ref()
    }
    pub(crate) fn generated_source(
        &self,
    ) -> Option<&Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>> {
        if self.local_body.is_some() {
            return None;
        }
        match self.source_receipt.as_deref() {
            Some(InputValue::GeneratedSyntax(unit)) => Some(unit),
            // A retained admission of a generated function keeps its unit's
            // module macro boundary: only its body's calls were attached.
            Some(InputValue::RetainedSyntax(unit)) => unit.generated_unit(),
            _ => None,
        }
    }
    /// This body's declaration context in its generated unit's own output
    /// coordinates (the insertion boundary's anchor), for either locator.
    fn generated_unit_context(&self) -> Result<DeclarationContext, Diagnostic> {
        match self.source_receipt.as_deref() {
            Some(InputValue::GeneratedSyntax(_)) => Ok(self.context().clone()),
            Some(InputValue::RetainedSyntax(unit)) => {
                let anchor = unit
                    .prepared()
                    .selected_generated()
                    .filter(|anchor| anchor.definition() == self.body().def_key)
                    .ok_or_else(|| fault("retained generated body anchor absent"))?;
                Ok(DeclarationContext {
                    scope: self.context().scope.clone(),
                    range: anchor.output_range(),
                })
            }
            _ => Err(fault(
                "generated boundary supplied for another owning source",
            )),
        }
    }
    fn refresh_identity(&mut self) {
        let mut e = crate::core::digest::StableEncoder::new("lrr/v2/admitted-owning-body-context");
        e.record(&self.parsed.key());
        e.record(self.index.body());
        e.digest(self.index.fingerprint());
        for stamp in self.input_stamps {
            e.digest(stamp);
        }
        if let Some(original) = &self.original {
            e.bool(true);
            e.string(original.selection().path.as_str());
            e.record(&original.selection().outline);
        } else {
            e.bool(false);
        }
        e.option(&self.generated_boundary, |e, boundary| {
            e.digest(boundary.fingerprint())
        });
        e.option(&self.local_body, |e, local| e.digest(local.identity()));
        self.identity = e.finish();
    }
    pub(crate) fn attach_generated_boundary(
        self: Arc<Self>,
        db: &mut Database,
        boundary: Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>,
    ) -> QueryPoll<Arc<Self>> {
        if self.local_body.is_some() {
            return QueryPoll::Ready(Err(fault(
                "local body uses its actual parent lexical boundary",
            )));
        }
        match self.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let Some(unit) = self.generated_source() else {
            return QueryPoll::Ready(Err(fault(
                "generated boundary supplied for another owning source",
            )));
        };
        match boundary.require_current(db, unit) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        if let Err(error) = self
            .generated_unit_context()
            .and_then(|context| boundary.declaration_outer(self.body().def_key, &context))
        {
            return QueryPoll::Ready(Err(error));
        }
        if let Some(current) = &self.generated_boundary {
            if current.fingerprint() != boundary.fingerprint() {
                return QueryPoll::Ready(Err(fault(
                    "owning body already has a different macro boundary",
                )));
            }
            return QueryPoll::Ready(Ok(self));
        }
        let mut admitted = Self {
            parsed: Arc::clone(&self.parsed),
            index: Arc::clone(&self.index),
            context: Arc::clone(&self.context),
            header: Arc::clone(&self.header),
            body: Arc::clone(&self.body),
            source: Arc::clone(&self.source),
            cfg: Arc::clone(&self.cfg),
            source_receipt: self.source_receipt.clone(),
            original: self.original.as_ref().map(PreparedSourceView::share),
            generated_boundary: Some(boundary),
            local_body: None,
            identity: Digest([0; 32]),
            input_stamps: self.input_stamps,
            exclusive_bytes: self.exclusive_bytes,
        };
        admitted.refresh_identity();
        QueryPoll::Ready(Ok(Arc::new(admitted)))
    }
    pub(crate) fn parsed(&self) -> &Arc<ParsedFile> {
        &self.parsed
    }
    pub(crate) fn index(&self) -> &Arc<BodyIndex> {
        &self.index
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        let InputValue::DefinitionBody(body) = self.body.as_ref() else {
            unreachable!("private admitted body input")
        };
        body
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("private admitted declaration context")
        };
        context
    }
    pub(crate) fn header(&self) -> &HeaderRecipe {
        let InputValue::DefinitionHeader(header) = self.header.as_ref() else {
            unreachable!("private admitted header input")
        };
        header
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.context().scope
    }
    pub(crate) fn original_source(&self) -> Option<&PreparedSourceView> {
        self.original.as_ref()
    }
    pub(crate) fn module_source(&self) -> Option<&PreparedSourceView> {
        if let Some(parent) = self.local_parent() {
            return parent.context().module_source();
        }
        self.generated_boundary()
            .and_then(|boundary| boundary.source_roots().first())
            .or_else(|| self.original_source())
    }
    pub(crate) fn identifier_context(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Option<crate::syntax::token_wire::Key>, Diagnostic> {
        if !self.context().range.contains(range) {
            return Err(fault("identifier escapes admitted declaration"));
        }
        self.parsed.identifier_context(range)
    }
    /// The context of a declaration owner's identifier (an impl or trait
    /// generic parameter), which lies outside the admitted declaration. Only
    /// plain original source, which carries no expansion marks, supplies it:
    /// its original root. Generated or expanded owners stay without one.
    pub(super) fn owner_identifier_context(
        &self,
        range: crate::syntax::spans::SourceRange,
        original: crate::syntax::token_wire::Key,
    ) -> Result<Option<crate::syntax::token_wire::Key>, Diagnostic> {
        if self.context().range.contains(range) {
            return self
                .identifier_context_or_original(range, original)
                .map(Some);
        }
        let plain = match self.source_receipt.as_deref() {
            None => true,
            Some(InputValue::RetainedSyntax(unit)) => unit.prepared().outputs().is_empty(),
            _ => false,
        };
        Ok((plain
            && self.local_parent().is_none()
            && self
                .original_source()
                .is_some_and(|source| source.selection().outline.parse == self.parsed.key()))
        .then_some(original))
    }
    pub(super) fn identifier_context_or_original(
        &self,
        range: crate::syntax::spans::SourceRange,
        original: crate::syntax::token_wire::Key,
    ) -> Result<crate::syntax::token_wire::Key, Diagnostic> {
        match self.identifier_context(range)? {
            Some(context) => Ok(context),
            None if self
                .original_source()
                .is_some_and(|source| source.selection().outline.parse == self.parsed.key()) =>
            {
                Ok(original)
            }
            None => Err(fault("admitted identifier lacks its actual syntax context")),
        }
    }
    pub(crate) fn require_identifier_origin(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        range: crate::syntax::spans::SourceRange,
        written: &str,
    ) -> Result<(), Diagnostic> {
        if !self.body().range.contains(range) {
            return Err(fault("identifier escapes admitted body"));
        }
        let root = self.parsed.item_name_context(range)?;
        match self.source_receipt.as_deref() {
            Some(InputValue::GeneratedSyntax(unit)) => require_single_origin(
                db,
                sources,
                &self.parsed,
                range,
                written,
                root,
                unit.items.project_source_spans(range)?.into_iter(),
            ),
            Some(InputValue::RetainedSyntax(_)) => {
                let origins = self
                    .parsed
                    .retained_source_spans(range)?
                    .ok_or_else(|| fault("admitted identifier origins absent"))?;
                require_single_origin(
                    db,
                    sources,
                    &self.parsed,
                    range,
                    written,
                    root,
                    origins.iter(),
                )
            }
            None => {
                if root.is_some() {
                    return Err(fault("original identifier has generated context"));
                }
                Ok(())
            }
            _ => Err(fault("admitted source receipt differs")),
        }
    }
}

fn require_single_origin<'a>(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    parsed: &ParsedFile,
    range: crate::syntax::spans::SourceRange,
    written: &str,
    root: Option<crate::syntax::token_wire::Key>,
    mut origins: impl Iterator<Item = &'a SpanRecord>,
) -> Result<(), Diagnostic> {
    let origin = origins
        .next()
        .ok_or_else(|| fault("identifier requires one actual token origin"))?;
    if origins.next().is_some() {
        return Err(fault("identifier requires one actual token origin"));
    }
    require_origin(db, sources, parsed, range, written, root, origin)
}

fn edition(edition: Edition) -> u16 {
    match edition {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    }
}
/// Source roots remain authority-bearing identities. The pinned item context
/// only proves that no opaque entered source adjustment is required; it never supplies
/// the caller module or grants access to a same-spelled private member.
fn require_origin(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    parsed: &ParsedFile,
    range: crate::syntax::spans::SourceRange,
    written: &str,
    root_context: Option<crate::syntax::token_wire::Key>,
    origin: &SpanRecord,
) -> Result<(), Diagnostic> {
    let facts = parsed
        .hygiene_facts()
        .ok_or_else(|| fault("entered source origin has no pinned hygiene facts"))?;
    let root = facts.item(origin.context)?;
    let context = facts.context(root)?;
    if Some(root) != root_context
        || context.parent.is_some()
        || context.defining_unit != origin.source.unit
    {
        return Err(fault(
            "entered source selector source root differs from its actual token",
        ));
    }
    let mut candidates = sources.iter().filter(|source| {
        let selection = source.selection();
        selection.outline.crate_key.0 == origin.source.unit.0
            && selection.outline.parse.content.0 == origin.source.content.0
            && selection.path.as_str() == origin.source.path
    });
    let source = candidates
        .next()
        .ok_or_else(|| fault("entered source token original source owner absent"))?;
    if candidates.next().is_some() {
        return Err(fault(
            "entered source token original source owner ambiguous",
        ));
    }
    source.require_original_hygiene()?;
    let selection = source.selection();
    if edition(origin.edition) != selection.outline.parse.profile.edition
        || edition(context.edition) != selection.outline.parse.profile.edition
        || !matches!(db.input(&InputKey::CapturedSource(selection.location_identity())).as_deref(),Some(InputValue::CapturedSource(actual)) if actual==selection)
        || !matches!(db.input(&InputKey::CrateCfg(selection.outline.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==selection.outline.cfg)
    {
        return Err(fault(
            "entered source token original source publication differs",
        ));
    }
    let current = db
        .prepared_source(selection)?
        .ok_or_else(|| fault("entered source token original source lease absent"))?;
    if !source.same_publication(&current) {
        return Err(fault(
            "entered source token belongs to another source publication",
        ));
    }
    let blob = db
        .input(&InputKey::SourceBlob(Digest(origin.source.content.0)))
        .ok_or_else(|| fault("entered source token original source bytes absent"))?;
    let InputValue::SourceBlob(blob) = blob.as_ref() else {
        return Err(fault("entered source token source bytes variant differs"));
    };
    let text = blob
        .text()?
        .get(origin.start as usize..origin.end as usize)
        .ok_or_else(|| fault("entered source token original source range differs"))?;
    if crate::frontend::names::owned_name(text, edition(origin.edition))?.as_str()
        != crate::frontend::names::owned_name(written, parsed.token_edition(range)?)?.as_str()
    {
        return Err(fault(
            "entered source selector differs from its actual original token",
        ));
    }
    Ok(())
}
/// The admitted context of an executing frame's body. A body whose owning
/// environment is published (an entered body, a local function or a member of
/// a block-level impl) reuses that environment's one admitted context; any
/// other body is captured as a module body.
pub(crate) fn frame_context(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Arc<AdmittedBodyContext>> {
    if let Some(input) = db.input(&InputKey::BodyEnvironment(index.body().clone())) {
        let InputValue::BodyEnvironment(environment) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("frame body environment variant differs")));
        };
        if environment.body() == index.body() && environment.source().index().body() == index.body()
        {
            match environment.require_current(db) {
                QueryPoll::Ready(Ok(())) => {
                    return QueryPoll::Ready(Ok(Arc::clone(environment.source())));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
    }
    match capture(db, sources, index, pc) {
        QueryPoll::Ready(Ok(owner)) => QueryPoll::Ready(Ok(Arc::new(owner))),
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}
pub(crate) fn capture(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<AdmittedBodyContext> {
    match capture_inner(db, sources, index, pc, None) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
fn capture_inner(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    index: &BodyIndex,
    pc: &PcKey,
    local: Option<(
        Arc<super::source_locals::PreparedSourceLocalPlan>,
        &super::callables::SourceCallable,
    )>,
) -> Result<QueryPoll<AdmittedBodyContext>, Diagnostic> {
    if index.body() != &pc.body || index.pc(NodeId(pc.node), Phase::from_code(pc.phase))? != *pc {
        return Err(fault("entered source source entered PC differs"));
    }
    let definition = pc.body.definition;
    let locator = db
        .input(&InputKey::DefinitionSource(definition))
        .ok_or_else(|| fault("entered source declaration source absent"))?;
    let body = db
        .input(&InputKey::DefinitionBody(definition))
        .ok_or_else(|| fault("entered source declaration body absent"))?;
    let context = db
        .input(&InputKey::DeclarationContext(definition))
        .ok_or_else(|| fault("entered source declaration context absent"))?;
    let header_input = db
        .input(&InputKey::DefinitionHeader(definition))
        .ok_or_else(|| fault("entered source declaration header absent"))?;
    let (
        InputValue::DefinitionSource(parse),
        InputValue::DefinitionBody(handle),
        InputValue::DeclarationContext(declaration),
        InputValue::DefinitionHeader(header),
    ) = (
        locator.as_ref(),
        body.as_ref(),
        context.as_ref(),
        header_input.as_ref(),
    )
    else {
        return Err(fault("entered source declaration input variants differ"));
    };
    let cfg = db
        .input(&InputKey::CrateCfg(declaration.scope.crate_key))
        .ok_or_else(|| fault("entered body owning cfg absent"))?;
    if !matches!(cfg.as_ref(), InputValue::CrateCfg(_)) {
        return Err(fault("entered body owning cfg differs"));
    }
    if handle.key() != pc.body
        || header.profile != parse.profile
        || index.profile() != &parse.profile
        || !declaration.range.contains(handle.range)
    {
        return Err(fault(
            "entered source caller source revision differs from database",
        ));
    }
    let current_index = match db.fetch(&SyntaxProvider, QueryKey::BodyStructure(pc.body.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
    };
    let QueryValue::BodyStructure(current_index) = current_index.value.as_ref() else {
        return Err(fault("entered source current body index outcome differs"));
    };
    if current_index.body() != index.body()
        || current_index.profile() != index.profile()
        || current_index.nodes().get(pc.node as usize) != index.nodes().get(pc.node as usize)
    {
        return Err(fault(
            "entered source selected index node differs from current source",
        ));
    }
    let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
    };
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return Err(fault("entered source selected parse outcome differs"));
    };
    if parsed.key() != *parse {
        return Err(fault("entered source selected parse key differs"));
    }
    let mut source_receipt = None;
    let mut original = None;
    let local_body = if let Some((parent, callee)) = local {
        let admitted = match local_body::LocalBodyContext::capture(
            db,
            parent,
            callee,
            parse,
            handle,
            declaration,
            header,
        ) {
            QueryPoll::Ready(Ok(admitted)) => admitted,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        let owner = admitted.parent().context().source();
        original = owner.original_source().map(PreparedSourceView::share);
        source_receipt = owner.source_receipt.clone();
        Some(admitted)
    } else {
        None
    };
    if local_body.is_some() {
        // The exact selected local declaration and source receipt were joined
        // above. Module declaration maps are not local-function authority.
    } else if let Some(input) = db.input(&InputKey::RetainedSyntax(parse.content)) {
        let InputValue::RetainedSyntax(unit) = input.as_ref() else {
            return Err(fault("entered source retained source input differs"));
        };
        if unit.key() != parse || !unit.prepared().permits_body(definition) {
            return Err(fault(
                "entered source retained source belongs to another entered body",
            ));
        }
        match unit.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        }
        let mut owners = sources
            .iter()
            .filter(|source| source.selection() == unit.prepared().source());
        let owner = owners
            .next()
            .ok_or_else(|| fault("entered source retained original source owner absent"))?;
        if owners.next().is_some() {
            return Err(fault(
                "entered source retained original source owner ambiguous",
            ));
        }
        if let Some(generated) = unit.generated_unit() {
            // A retained admission of a macro-generated function: its exact
            // structural anchor, owning unit and current signature join here,
            // the same checks the unit's own publication receives below.
            let anchor = unit
                .prepared()
                .selected_generated()
                .filter(|anchor| anchor.definition() == definition)
                .ok_or_else(|| fault("entered source retained generated anchor absent"))?;
            let selected = generated
                .definitions
                .get(&definition)
                .ok_or_else(|| fault("entered source retained generated definition absent"))?;
            let current = db
                .prepared_source(owner.selection())?
                .ok_or_else(|| fault("entered source retained original source lease absent"))?;
            if !owner.same_publication(&current)
                || anchor.declaration_range() != declaration.range
                || anchor.body_range() != handle.range
                || selected.crate_key != declaration.scope.crate_key
                || selected.module_path != declaration.scope.module_path
                || declaration.scope.crate_key != generated.scope.crate_key
                || !declaration
                    .scope
                    .module_path
                    .starts_with(&generated.scope.module_path)
                || selected.item_kind != crate::source::manifest::ItemKind::Function
            {
                return Err(fault(
                    "entered source retained generated declaration rows differ",
                ));
            }
            let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition))
            {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
            };
            if !matches!(signature.value.as_ref(), QueryValue::FunctionSignature(signature)
                if signature.definition == definition && signature.header == *header && signature.scope == declaration.scope)
            {
                return Err(fault(
                    "entered source retained generated signature proof differs",
                ));
            }
            // Like its unit's own publication, the generated declaration has
            // no original module owner; its macro boundary is the unit's.
            source_receipt = Some(input.clone());
        } else if let Some(associated) = unit.generated_associated() {
            // A macro-generated associated function retained inside its
            // original impl/trait: the anchored declaration, its actual owner
            // scope and the current signature join here.
            let anchor = unit
                .prepared()
                .selected_generated()
                .filter(|anchor| anchor.definition() == definition)
                .ok_or_else(|| fault("entered source retained associated anchor absent"))?;
            let current = db
                .prepared_source(owner.selection())?
                .ok_or_else(|| fault("entered source retained original source lease absent"))?;
            if !owner.same_publication(&current)
                || anchor.declaration_range() != declaration.range
                || anchor.body_range() != handle.range
                || associated.scope != declaration.scope
            {
                return Err(fault(
                    "entered source retained associated declaration rows differ",
                ));
            }
            match associated.def_key.item_kind {
                crate::source::manifest::ItemKind::Function => {
                    let signature = match db
                        .fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition))
                    {
                        QueryPoll::Ready(Ok(value)) => value,
                        QueryPoll::Ready(Err(error)) => return Err(error),
                        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
                    };
                    if !matches!(signature.value.as_ref(), QueryValue::FunctionSignature(signature)
                        if signature.definition == definition && signature.header == *header && signature.scope == declaration.scope)
                    {
                        return Err(fault(
                            "entered source retained associated signature proof differs",
                        ));
                    }
                }
                crate::source::manifest::ItemKind::Constant => {
                    parsed.initializer_parameters(header, declaration, handle)?;
                }
                _ => return Err(fault("admitted declaration has no executable body proof")),
            }
            source_receipt = Some(input.clone());
        } else {
            // The admitted immutable assembly anchors and already-published rows
            // certify this original member. Do not rebuild its complete outline at
            // every reached entered source operation merely to rediscover the same anchor.
            let original_item = owner
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.fingerprint() == definition)
                .ok_or_else(|| fault("entered source retained original declaration absent"))?;
            let mut anchors = unit
                .prepared()
                .original_declarations()
                .iter()
                .filter(|anchor| anchor.original_definition() == definition);
            let anchor = anchors.next().ok_or_else(|| {
                fault("entered source retained original declaration anchor absent")
            })?;
            let current = db
                .prepared_source(owner.selection())?
                .ok_or_else(|| fault("entered source retained original source lease absent"))?;
            if anchors.next().is_some()
                || !owner.same_publication(&current)
                || anchor.original_range() != original_item.declaration_span
                || anchor.declaration_range() != declaration.range
                || anchor.body_range() != Some(handle.range)
                || original_item.scope != declaration.scope
                || original_item.header.fingerprint() != header.fingerprint()
                || !declaration.range.contains(anchor.header_range())
            {
                return Err(fault(
                    "entered source retained original declaration rows differ",
                ));
            }
            original = Some((*owner).share());
            source_receipt = Some(input.clone());
        }
    } else if let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) {
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            return Err(fault("entered source generated source input differs"));
        };
        let selected = unit
            .definitions
            .get(&definition)
            .ok_or_else(|| fault("entered source generated definition absent"))?;
        if unit.key != *parse
            || selected.crate_key != declaration.scope.crate_key
            || selected.module_path != declaration.scope.module_path
            || declaration.scope.crate_key != unit.scope.crate_key
            || !declaration
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
            || !unit.items.items().iter().any(|item| {
                item.key.fingerprint() == definition
                    && item.start == declaration.range.start
                    && item.end == declaration.range.end
            })
            || !matches!(db.input(&InputKey::CrateCfg(unit.scope.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==unit.cfg.fingerprint())
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==unit.seal.epoch && unit.items.epoch()==*epoch)
        {
            return Err(fault(
                "entered source generated declaration/source epoch differs",
            ));
        }
        // Canonical identity and the ordinary declaration parser prove the
        // actual header/body pair. Initializers never masquerade as functions.
        match selected.item_kind {
            crate::source::manifest::ItemKind::Function => {
                let signature =
                    match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
                        QueryPoll::Ready(Ok(value)) => value,
                        QueryPoll::Ready(Err(error)) => return Err(error),
                        QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
                    };
                if !matches!(signature.value.as_ref(), QueryValue::FunctionSignature(signature)
            if signature.definition == definition && signature.header == *header && signature.scope == declaration.scope)
                {
                    return Err(fault(
                        "entered source generated current signature proof differs",
                    ));
                }
            }
            crate::source::manifest::ItemKind::Constant
            | crate::source::manifest::ItemKind::Static => {
                parsed.initializer_parameters(header, declaration, handle)?;
            }
            _ => return Err(fault("admitted declaration has no executable body proof")),
        }
        match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(unit.seal.clone())) {
            QueryPoll::Ready(Ok(value)) if matches!(value.value.as_ref(),QueryValue::DiscoverySeal(seal) if seal.key()==unit.seal) =>
                {}
            QueryPoll::Ready(Ok(_)) => {
                return Err(fault("entered source generated discovery outcome differs"));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        }
        let publication = db
            .input(&InputKey::DiscoverySeal(unit.seal.clone()))
            .ok_or_else(|| fault("entered source generated discovery publication absent"))?;
        let InputValue::DiscoverySeal(publication) = publication.as_ref() else {
            return Err(fault(
                "entered source generated discovery publication differs",
            ));
        };
        let record = unit.items.record();
        if publication.seal.key() != unit.seal
            || !publication
                .records
                .iter()
                .any(|row| row.fingerprint() == record.descriptor().fingerprint())
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),Some(InputValue::ExpansionRecord(actual)) if actual.same_record(record))
        {
            return Err(fault(
                "entered source generated actual expansion record differs",
            ));
        }
        source_receipt = Some(input.clone());
    } else {
        let mut owners = sources.iter().filter(|source| {
            source.selection().outline.parse == *parse
                && source
                    .outline()
                    .declaration_rows()
                    .any(|item| item.def_key.fingerprint() == definition)
        });
        let owner = owners
            .next()
            .ok_or_else(|| fault("entered source original source owner absent"))?;
        if owners.next().is_some() {
            return Err(fault("entered source original source owner ambiguous"));
        }
        owner.require_original_hygiene()?;
        let selection = owner.selection();
        let current = db
            .prepared_source(selection)?
            .ok_or_else(|| fault("entered source original source lease absent"))?;
        if !owner.same_publication(&current)
            || !owner.outline().declaration_rows().any(|item| {
                item.def_key.fingerprint() == definition
                    && item.header == *header
                    && item.scope == declaration.scope
                    && item.declaration_span == declaration.range
                    && item.body.as_ref() == Some(handle)
            })
            || !matches!(db.input(&InputKey::CrateCfg(selection.outline.crate_key)).as_deref(),Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint()==selection.outline.cfg)
        {
            return Err(fault(
                "entered source caller source revision differs from database",
            ));
        }
        original = Some((*owner).share());
    }
    let exclusive_bytes = std::mem::size_of::<AdmittedBodyContext>()
        .checked_add(
            original
                .as_ref()
                .map(|source| {
                    source
                        .selection()
                        .metadata_bytes()
                        .and_then(|n| n.checked_mul(2).ok_or_else(crate::runtime::arena::resource))
                })
                .transpose()?
                .unwrap_or(0),
        )
        .ok_or_else(crate::runtime::arena::resource)?;
    let mut admitted = AdmittedBodyContext {
        parsed: Arc::clone(parsed),
        index: Arc::clone(current_index),
        context,
        header: header_input,
        body,
        source: locator,
        cfg,
        source_receipt,
        original,
        generated_boundary: None,
        local_body,
        identity: Digest([0; 32]),
        input_stamps: [Digest([0; 32]); 6],
        exclusive_bytes,
    };
    let mut stamps = [Digest([0; 32]); 6];
    for (slot, input) in stamps.iter_mut().zip(admitted.retained_inputs()) {
        *slot = input.fingerprint();
    }
    admitted.input_stamps = stamps;
    admitted.refresh_identity();
    Ok(QueryPoll::Ready(Ok(admitted)))
}

/// Fully retained ordinary lexical facts for the actual admitted body. This
/// does not invent the module macro-rib insertion boundary for generated items.
/// The body item's local identity and the expansion producer's identity name
/// one actual declaration occurrence. Keep that join in cold source metadata;
/// syntax-context marks never manufacture a declaration identity.
struct BodyMacroDefinition {
    event: Digest,
    definition: crate::expand::scopes::MacroDefinition,
    name_range: crate::syntax::spans::SourceRange,
}
fn prepare_body_macro_definitions(
    source: &AdmittedBodyContext,
    facts: &crate::syntax::entered_macros::EnteredMacroFacts,
) -> Result<Box<[BodyMacroDefinition]>, Diagnostic> {
    use crate::{
        expand::scopes::{MacroDefinition, MacroName},
        syntax::entered_macros::BodyMacroEventKind,
    };
    let count = facts
        .events
        .iter()
        .filter(|event| matches!(event.kind, BodyMacroEventKind::Definition { .. }))
        .count();
    let mut definitions = Vec::new();
    definitions
        .try_reserve_exact(count)
        .map_err(|_| crate::runtime::arena::resource())?;
    for event in &facts.events {
        let BodyMacroEventKind::Definition { identity, name, .. } = &event.kind else {
            continue;
        };
        if !source.body().range.contains(event.range) {
            return Err(fault("local macro declaration escapes actual body"));
        }
        let mut items = facts.items.iter().filter(|item| {
            item.scope == event.scope
                && item.item.def_key.item_kind == crate::source::manifest::ItemKind::Macro
                && item.item.def_key.fingerprint() == *identity
                && item.item.declaration_span == event.range
        });
        let item = items
            .next()
            .ok_or_else(|| fault("local macro declaration header absent"))?;
        if items.next().is_some()
            || item.item.header.profile != source.parsed().profile
            || item.item.scope.crate_key != source.scope().crate_key
            || item.item.scope.module_path != source.scope().module_path
        {
            return Err(fault("local macro declaration actual header/scope differs"));
        }
        let shape = source.parsed().macro_definition_shape_at(event.range)?;
        let name_range = shape.name_range();
        let edition = source.parsed().token_edition(name_range)?;
        let actual_name = MacroName::parse(shape.name(), edition)?;
        if actual_name != MacroName::parse(name, edition)?
            || item.item.def_key.declared_name != actual_name.as_str()
        {
            return Err(fault("local macro declaration actual name differs"));
        }
        // Contexts are still consumed by the existing common name selector.
        // Checking this occurrence here never substitutes an origin for one.
        source.parsed().identifier_context(name_range)?;
        let definition = match source.source_receipt.as_deref() {
            Some(InputValue::RetainedSyntax(unit)) => {
                if unit.key() != &source.parsed().key() {
                    return Err(fault("local macro declaration retained parser differs"));
                }
                let anchors = unit.prepared().macro_definitions();
                let anchor = anchors
                    .binary_search_by_key(&event.range.start, |anchor| anchor.range().start)
                    .ok()
                    .and_then(|index| anchors.get(index))
                    .filter(|anchor| anchor.range() == event.range)
                    .ok_or_else(|| fault("local macro declaration retained occurrence absent"))?;
                let actual = anchor.definition();
                if actual.defining_unit != source.scope().crate_key
                    || actual.definition_edition != edition
                {
                    return Err(fault("local macro declaration retained authority differs"));
                }
                actual.clone()
            }
            // These paths retain their existing admitted declaration identity.
            // A retained assembly alone supplies the producer occurrence join.
            Some(InputValue::GeneratedSyntax(_)) | None => MacroDefinition {
                identity: *identity,
                defining_unit: source.scope().crate_key,
                definition_edition: edition,
            },
            _ => return Err(fault("local macro declaration source receipt differs")),
        };
        definitions.push(BodyMacroDefinition {
            event: *identity,
            definition,
            name_range,
        });
    }
    definitions.sort_unstable_by_key(|definition| definition.event);
    if definitions
        .windows(2)
        .any(|pair| pair[0].event == pair[1].event)
    {
        return Err(fault("local macro declaration event identity is ambiguous"));
    }
    Ok(definitions.into_boxed_slice())
}
pub(crate) struct PreparedBodyContext {
    source: Arc<AdmittedBodyContext>,
    facts: Arc<crate::syntax::entered_macros::EnteredMacroFacts>,
    retained_lease: Option<crate::db::memo::RetainedSourceLease>,
    macro_outers: Vec<super::macro_boundary::RetainedMacroScope>,
    macro_hygiene: Option<Arc<super::hygiene::HygieneFacts>>,
    macro_ancestry: Option<Arc<super::macro_origins::ExpansionAncestry>>,
    macro_imports: import_occurrences::PreparedMacroImportOccurrences,
    body_types: type_occurrences::PreparedBodyTypeOccurrences,
    macro_definitions: Box<[BodyMacroDefinition]>,
    exclusive_bytes: usize,
}
#[path = "declaration_name_completions.rs"]
mod declaration_names;
pub(crate) use declaration_names::{PreparedDeclarationNameCompletions, EnteredDeclarationAdmission};
impl PreparedBodyContext {
    pub(crate) fn source(&self) -> &Arc<AdmittedBodyContext> {
        &self.source
    }
    pub(crate) fn facts(&self) -> &Arc<crate::syntax::entered_macros::EnteredMacroFacts> {
        &self.facts
    }
    pub(crate) fn local_macro_definition(
        &self,
        event: Digest,
    ) -> Option<(
        &crate::expand::scopes::MacroDefinition,
        crate::syntax::spans::SourceRange,
    )> {
        self.macro_definitions
            .binary_search_by_key(&event, |definition| definition.event)
            .ok()
            .map(|index| {
                let definition = &self.macro_definitions[index];
                (&definition.definition, definition.name_range)
            })
    }
    pub(crate) fn generated_boundary(
        &self,
    ) -> Option<&Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>> {
        self.source.generated_boundary()
    }
    pub(crate) fn macro_outers(&self) -> &[super::macro_boundary::RetainedMacroScope] {
        &self.macro_outers
    }
    pub(crate) fn macro_hygiene(&self) -> Option<&Arc<super::hygiene::HygieneFacts>> {
        self.macro_hygiene.as_ref()
    }
    pub(crate) fn macro_ancestry(&self) -> Option<&Arc<super::macro_origins::ExpansionAncestry>> {
        self.macro_ancestry.as_ref()
    }
    pub(crate) fn module_source(&self) -> Option<&PreparedSourceView> {
        self.source.module_source()
    }
    /// Actual expansion order belongs to this structural name occurrence,
    /// independently of copied diagnostic spans and syntax-context marks.
    pub(crate) fn reference_origin(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Option<crate::syntax::token_wire::Key>, Diagnostic> {
        use crate::expand::source_declarative::assembly::SourceOccurrence;
        if range.is_empty() || !self.source.context().range.contains(range) {
            return Err(fault(
                "macro reference escapes its actual owning declaration",
            ));
        }
        match self.source.source_receipt.as_deref() {
            Some(InputValue::GeneratedSyntax(unit)) if self.source.generated_boundary.is_some() => {
                Ok(Some(crate::syntax::token_wire::Key(
                    unit.items.record().key().fingerprint().0,
                )))
            }
            Some(InputValue::GeneratedSyntax(_)) => Err(fault(
                "generated reference requires its actual macro boundary",
            )),
            Some(InputValue::RetainedSyntax(unit)) => {
                if unit.key() != &self.source.parsed().key() {
                    return Err(fault("retained macro reference parser differs"));
                }
                match unit.prepared().occurrence(range)? {
                    SourceOccurrence::Original { .. } => Ok(None),
                    SourceOccurrence::Generated {
                        record,
                        descriptor,
                        parent_inventory,
                        invocation,
                        ..
                    } => {
                        if !unit.prepared().outputs().iter().any(|output| {
                            output.record().key().fingerprint() == record
                                && output.record().descriptor().fingerprint() == descriptor
                                && output.parent_inventory() == parent_inventory
                                && output.parent_invocation() == invocation
                                && output.epoch() == unit.prepared().epoch()
                        }) {
                            return Err(fault("retained macro occurrence output differs"));
                        }
                        Ok(Some(crate::syntax::token_wire::Key(record.0)))
                    }
                    SourceOccurrence::Mixed | SourceOccurrence::Pending => Err(fault(
                        "retained macro reference requires one actual structural occurrence",
                    )),
                }
            }
            None => Ok(None),
            _ => Err(fault("owning macro source receipt differs")),
        }
    }
    /// Lookup positions remain scope cursors. Join the actual invocation name
    /// (which need not start at that cursor for qualified calls), or an actual
    /// identifier token for an ordinary source-name demand.
    pub(crate) fn reference_origin_at(
        &self,
        at: u64,
        name: &str,
    ) -> Result<Option<crate::syntax::token_wire::Key>, Diagnostic> {
        if self.source.source_receipt.is_none() {
            return Ok(None);
        }
        let parsed = self.source.parsed();
        let at = u32::try_from(at).map_err(|_| fault("macro reference position differs"))?;
        let token = parsed
            .parsed
            .syntax_node()
            .token_at_offset(at.into())
            .right_biased()
            .ok_or_else(|| fault("macro reference identifier absent"))?;
        let range = crate::syntax::ast_adapter::range(token.text_range());
        let edition = parsed.token_edition(range)?;
        if range.start != at
            || crate::expand::scopes::MacroName::parse(token.text(), edition)?
                != crate::expand::scopes::MacroName::parse(name, edition)?
        {
            return Err(fault(
                "macro reference requires its actual identifier occurrence",
            ));
        }
        self.reference_origin(range)
    }
    /// Cold source preparation only. Preserve the published module ancestry and
    /// add the actual retained attachment topology, whose parents are inventory
    /// identities, never diagnostic token contexts.
    pub(crate) fn prepare_macro_ancestry(
        &self,
        published: &Arc<super::macro_origins::ExpansionAncestry>,
    ) -> Result<Arc<super::macro_origins::ExpansionAncestry>, Diagnostic> {
        let mut ancestry = Arc::clone(published);
        if let Some(boundary) = self.macro_ancestry() {
            if !Arc::ptr_eq(boundary, &ancestry) {
                Arc::make_mut(&mut ancestry).extend(boundary)?;
            }
        }
        let Some(InputValue::RetainedSyntax(unit)) = self.source.source_receipt.as_deref() else {
            return Ok(ancestry);
        };
        if unit.prepared().outputs().is_empty() {
            return Ok(ancestry);
        }
        if unit.key() != &self.source.parsed().key() {
            return Err(fault("retained macro ancestry owning source differs"));
        }
        // finish_scope stores outputs in canonical record-key order, not
        // expansion order. Walk the real inventory-parent edges from the
        // retained initial root; a missing parent must never become a root.
        let mut children = std::collections::BTreeMap::<_, Vec<_>>::new();
        for output in unit.prepared().outputs() {
            if output.epoch() != unit.prepared().epoch() {
                return Err(fault("retained macro ancestry output epoch differs"));
            }
            let siblings = children.entry(output.parent_inventory()).or_default();
            siblings
                .try_reserve(1)
                .map_err(|_| crate::runtime::arena::resource())?;
            siblings.push(output);
        }
        let root_inventory = unit.prepared().initial_inventory();
        let mut seen = std::collections::BTreeSet::from([root_inventory]);
        let mut work = Vec::new();
        work.try_reserve_exact(
            unit.prepared()
                .outputs()
                .len()
                .checked_add(1)
                .ok_or_else(crate::runtime::arena::resource)?,
        )
        .map_err(|_| crate::runtime::arena::resource())?;
        work.push((root_inventory, None));
        while let Some((inventory, parent)) = work.pop() {
            let Some(outputs) = children.remove(&inventory) else {
                continue;
            };
            for output in outputs {
                let origin = crate::syntax::token_wire::Key(output.record().key().fingerprint().0);
                Arc::make_mut(&mut ancestry).register(origin, parent)?;
                let child = output.scan().fingerprint();
                if !seen.insert(child) {
                    return Err(fault("retained macro ancestry inventory owner differs"));
                }
                work.push((child, Some(origin)));
            }
        }
        if !children.is_empty() {
            return Err(fault("retained macro ancestry parent inventory absent"));
        }
        Ok(ancestry)
    }
    pub(crate) fn body_type_occurrences(&self) -> &type_occurrences::PreparedBodyTypeOccurrences {
        &self.body_types
    }
    /// Cold query selection records the actual retained source dependencies.
    /// Returning occurrence data does not grant a local/generic/module answer.
    pub(crate) fn body_type_occurrence(
        &self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<type_occurrences::EnteredBodyTypeOccurrence<'_>>> {
        match self.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let occurrence = match self.body_types.select(&self.source, node) {
            Ok(Some(occurrence)) => occurrence,
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match type_occurrences::EnteredBodyTypeOccurrence::prepare(&self.source, db, occurrence) {
            QueryPoll::Ready(Ok(occurrence)) => QueryPoll::Ready(Ok(Some(occurrence))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    pub(crate) fn macro_import_occurrence(
        &self,
        row: &crate::syntax::entered_macros::BodyImport,
        name: &str,
    ) -> Result<&import_occurrences::MacroImportOccurrence, Diagnostic> {
        self.macro_imports.select(row, name)
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match self.source.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        self.require_retained_lease(db)
    }
    /// The opaque environment already owns and checks this exact source. Do
    /// not recursively walk a retained local parent twice at every depth.
    pub(crate) fn require_current_in_environment(
        &self,
        db: &mut Database,
        environment: &crate::db::BodyEnvironmentInput,
    ) -> QueryPoll<()> {
        if environment.source().identity() != self.source.identity()
            || environment.body() != self.source.index().body()
            || !matches!(db.input(&InputKey::BodyEnvironment(environment.body().clone())).as_deref(),
                Some(InputValue::BodyEnvironment(current)) if current.identity() == environment.identity())
        {
            return QueryPoll::Ready(Err(fault("prepared context current environment differs")));
        }
        match environment.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        self.require_retained_lease(db)
    }
    fn require_retained_lease(&self, db: &mut Database) -> QueryPoll<()> {
        match &self.retained_lease {
            Some(lease) => match lease.require_current(db) {
                QueryPoll::Ready(Ok(())) => {}
                other => return other,
            },
            None => {}
        }
        QueryPoll::Ready(Ok(()))
    }
}
impl AdmittedBodyContext {
    /// Coordinator reuse test only. A true answer does not grant body/name
    /// authority: reuse must still consume require_current and the actual seal.
    pub(crate) fn has_current_inputs(&self, db: &mut Database) -> Result<bool, Diagnostic> {
        if let Some(local) = &self.local_body {
            if !local.has_current_inputs(db)? {
                return Ok(false);
            }
        }
        let definition = self.body().def_key;
        for (ordinal, key) in [
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionBody(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DeclarationContext(definition),
            InputKey::CrateCfg(self.scope().crate_key),
        ]
        .into_iter()
        .enumerate()
        {
            if db.input(&key).is_none()
                || db
                    .published_input(&key)
                    .is_none_or(|(_, stamp)| stamp != self.input_stamps[ordinal])
            {
                return Ok(false);
            }
        }
        let receipt_key = match self.source_receipt.as_deref() {
            Some(InputValue::GeneratedSyntax(unit)) => {
                Some(InputKey::GeneratedSyntax(unit.key.content))
            }
            Some(InputValue::RetainedSyntax(unit)) => {
                Some(InputKey::RetainedSyntax(unit.key().content))
            }
            None => None,
            _ => return Err(fault("admitted body receipt variant changed")),
        };
        if let Some(key) = receipt_key {
            if db.input(&key).is_none()
                || db
                    .published_input(&key)
                    .is_none_or(|(_, stamp)| stamp != self.input_stamps[5])
            {
                return Ok(false);
            }
        } else {
            let owner = self
                .original_source()
                .ok_or_else(|| fault("admitted original owner absent"))?;
            if !db
                .prepared_source(owner.selection())?
                .is_some_and(|current| owner.same_publication(&current))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        if let Some(local) = &self.local_body {
            match local.require_current(db) {
                QueryPoll::Ready(Ok(())) => {}
                other => return other,
            }
        }
        let definition = self.body().def_key;
        for (ordinal, key) in [
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionBody(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DeclarationContext(definition),
            InputKey::CrateCfg(self.scope().crate_key),
        ]
        .into_iter()
        .enumerate()
        {
            // input records the real dependency; the publication already owns
            // its checked stable stamp, so currentness does not rehash headers.
            if db.input(&key).is_none()
                || db
                    .published_input(&key)
                    .is_none_or(|(_, stamp)| stamp != self.input_stamps[ordinal])
            {
                return QueryPoll::Ready(Err(fault("admitted owning declaration was replaced")));
            }
        }
        match self.source_receipt.as_deref() {
            Some(InputValue::RetainedSyntax(unit)) => {
                let key = InputKey::RetainedSyntax(unit.key().content);
                if db.input(&key).is_none()
                    || db
                        .published_input(&key)
                        .is_none_or(|(_, stamp)| stamp != self.input_stamps[5])
                {
                    return QueryPoll::Ready(Err(fault("admitted retained input was replaced")));
                }
                unit.require_current(db)
            }
            Some(InputValue::GeneratedSyntax(unit)) => {
                if !matches!(db.input(&InputKey::GeneratedSyntax(unit.key.content)).as_deref(),Some(InputValue::GeneratedSyntax(current)) if current.same_unit(unit))
                    || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==unit.seal.epoch)
                {
                    return QueryPoll::Ready(Err(fault("admitted generated body receipt changed")));
                }
                match db.fetch(&SyntaxProvider, QueryKey::ParseFile(unit.key.clone())) {
                    QueryPoll::Ready(Ok(value)) if matches!(value.value.as_ref(),QueryValue::ParseFile(parsed) if parsed.key()==self.parsed.key()) => {
                        match &self.generated_boundary {
                            Some(boundary) => boundary.require_current(db, unit),
                            None => QueryPoll::Ready(Ok(())),
                        }
                    }
                    QueryPoll::Ready(Ok(_)) => {
                        QueryPoll::Ready(Err(fault("admitted generated parse proof changed")))
                    }
                    QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                }
            }
            None => {
                let Some(owner) = self.original_source() else {
                    return QueryPoll::Ready(Err(fault("admitted original source owner absent")));
                };
                match db.prepared_source(owner.selection()) {
                    Ok(Some(current)) if owner.same_publication(&current) => {
                        QueryPoll::Ready(Ok(()))
                    }
                    Ok(_) => {
                        QueryPoll::Ready(Err(fault("admitted original source publication changed")))
                    }
                    Err(error) => QueryPoll::Ready(Err(error)),
                }
            }
            _ => QueryPoll::Ready(Err(fault("admitted body receipt variant changed"))),
        }
    }
    /// Explicit cold preparation. A pure query owner must retain this complete
    /// immutable result; source operations only select from that retained plan.
    pub(crate) fn prepare(
        self: Arc<Self>,
        db: &mut Database,
    ) -> QueryPoll<Arc<PreparedBodyContext>> {
        let boundary = self.generated_boundary.clone();
        self.prepare_with_boundary(db, boundary)
    }
    pub(crate) fn prepare_with_generated_boundary(
        self: Arc<Self>,
        db: &mut Database,
        boundary: Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>,
    ) -> QueryPoll<Arc<PreparedBodyContext>> {
        match self.attach_generated_boundary(db, boundary) {
            QueryPoll::Ready(Ok(admitted)) => admitted.prepare(db),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn prepare_with_boundary(
        self: Arc<Self>,
        db: &mut Database,
        generated_boundary: Option<Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>>,
    ) -> QueryPoll<Arc<PreparedBodyContext>> {
        match self.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let mut retained_lease = None;
        if self.local_body.is_none()
            && matches!(
                self.source_receipt.as_deref(),
                Some(InputValue::RetainedSyntax(_))
            )
        {
            let generated = matches!(self.source_receipt.as_deref(),
                Some(InputValue::RetainedSyntax(unit))
                    if unit.generated_unit().is_some() || unit.generated_associated().is_some());
            let proof = match if generated {
                db.retained_generated_declaration(self.body().def_key)
            } else {
                let Some(owner) = self.original_source() else {
                    return QueryPoll::Ready(Err(fault(
                        "retained body original publication absent",
                    )));
                };
                db.retained_original_declaration(owner, self.body().def_key)
            } {
                QueryPoll::Ready(Ok(Some(proof))) => proof,
                QueryPoll::Ready(Ok(None)) => {
                    return QueryPoll::Ready(Err(fault("retained body declaration bridge absent")));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if proof.parse() != &self.parsed.key()
                || proof.body() != self.body()
                || proof.header() != self.header()
                || proof.context() != self.context()
            {
                return QueryPoll::Ready(Err(fault("retained body declaration bridge differs")));
            }
            retained_lease = Some(proof.into_lease());
        }
        let InputValue::CrateCfg(cfg) = self.cfg.as_ref() else {
            return QueryPoll::Ready(Err(fault("admitted body cfg differs")));
        };
        let mut macro_outers = Vec::new();
        let mut macro_hygiene = self.parsed.hygiene_facts().map(Arc::clone);
        let mut macro_ancestry = None;
        if let Some(parent) = self.local_parent() {
            macro_hygiene = parent.context().macro_hygiene().cloned();
            macro_ancestry = parent.context().macro_ancestry().cloned();
            if macro_outers
                .try_reserve_exact(parent.context().macro_outers().len())
                .is_err()
            {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            macro_outers.extend(parent.context().macro_outers().iter().cloned());
        }
        if let Some(boundary) = &generated_boundary {
            let Some(unit) = self.generated_source() else {
                return QueryPoll::Ready(Err(fault(
                    "generated macro boundary supplied for another source kind",
                )));
            };
            match boundary.require_current(db, unit) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            let selected = match self
                .generated_unit_context()
                .and_then(|context| boundary.declaration_outer(self.body().def_key, &context))
            {
                Ok(selected) => selected,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let count = boundary
                .scopes()
                .len()
                .checked_add(usize::from(selected.is_some()));
            let Some(count) = count else {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            };
            if macro_outers.try_reserve_exact(count).is_err() {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            if let Some(selected) = selected {
                macro_outers.push(selected);
            }
            macro_outers.extend(boundary.scopes().iter().cloned());
            let Some(hygiene) = &mut macro_hygiene else {
                return QueryPoll::Ready(Err(fault("generated macro body hygiene facts absent")));
            };
            let mut ancestry = super::macro_origins::ExpansionAncestry::default();
            for root in &macro_outers {
                if let Err(error) = Arc::make_mut(hygiene)
                    .extend(root.facts())
                    .and_then(|_| ancestry.extend(root.ancestry()))
                {
                    return QueryPoll::Ready(Err(error));
                }
            }
            // Only the completed-empty receipt certifies this original-source
            // item insertion has no generated parent catalogue.
            if boundary.selected_root().is_none() {
                if let Err(error) = ancestry.register(
                    crate::syntax::token_wire::Key(unit.items.record().key().fingerprint().0),
                    None,
                ) {
                    return QueryPoll::Ready(Err(error));
                }
            }
            macro_ancestry = Some(Arc::new(ancestry));
        }
        let facts = match self
            .parsed
            .entered_macro_facts(self.body(), cfg, self.scope())
        {
            Ok(facts) => facts,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let macro_definitions = match prepare_body_macro_definitions(&self, &facts) {
            Ok(definitions) => definitions,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let macro_imports =
            match import_occurrences::PreparedMacroImportOccurrences::prepare(&self, &facts) {
                Ok(prepared) => prepared,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let body_types = match type_occurrences::PreparedBodyTypeOccurrences::prepare(&self) {
            Ok(prepared) => prepared,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let exclusive_bytes =
            match facts.metadata_bytes().and_then(|bytes| {
                bytes
                    .checked_add(std::mem::size_of::<PreparedBodyContext>())
                    .and_then(|bytes| bytes.checked_add(macro_imports.extra_metadata_bytes()))
                    .and_then(|bytes| bytes.checked_add(body_types.extra_metadata_bytes()))
                    .and_then(|bytes| {
                        bytes.checked_add(
                            macro_definitions
                                .len()
                                .checked_mul(std::mem::size_of::<BodyMacroDefinition>())?,
                        )
                    })
                    .and_then(|bytes| {
                        bytes.checked_add(macro_outers.capacity().checked_mul(
                            std::mem::size_of::<super::macro_boundary::RetainedMacroScope>(),
                        )?)
                    })
                    .ok_or_else(crate::runtime::arena::resource)
            }) {
                Ok(bytes) => bytes,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        QueryPoll::Ready(Ok(Arc::new(PreparedBodyContext {
            source: self,
            facts: Arc::new(facts),
            retained_lease,
            macro_outers,
            macro_hygiene,
            macro_ancestry,
            macro_imports,
            body_types,
            macro_definitions,
            exclusive_bytes,
        })))
    }
}

#[cfg(test)]
#[path = "source_body_context_tests.rs"]
mod tests;
