//! Frozen demand scheduling and reply admission, outside the pure query database.
//! Starting a scheduling ticket neither executes a helper nor grants authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{key::QueryKey, poll::InputRequest},
    source::manifest::BodyKey,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DemandContext {
    pub request: Digest,
    pub session: Digest,
    pub unit: Digest,
    pub revision: u64,
    pub epoch: u64,
    pub source_generation: Digest,
    pub native_generation: Option<Digest>,
    pub compile_environment: Digest,
}
impl StableEncode for DemandContext {
    const DOMAIN: &'static str = "lrr/v2/demand-context";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.request);
        e.digest(self.session);
        e.digest(self.unit);
        e.u64(self.revision);
        e.u64(self.epoch);
        e.digest(self.source_generation);
        e.option(&self.native_generation, |e, generation| {
            e.digest(*generation)
        });
        e.digest(self.compile_environment)
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PhaseIdentity {
    pub frame: Digest,
    pub body: BodyKey,
    pub index: u32,
}
impl StableEncode for PhaseIdentity {
    const DOMAIN: &'static str = "lrr/v2/demand-phase";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.frame);
        e.digest(self.body.definition);
        e.digest(self.body.exact_body_bytes);
        e.u32(self.index)
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DemandToken {
    context: DemandContext,
    query: QueryKey,
    phase: PhaseIdentity,
    input: InputRequest,
    sequence: u64,
}
impl StableEncode for DemandToken {
    const DOMAIN: &'static str = "lrr/v2/demand-token";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.context);
        e.record(&self.query);
        e.record(&self.phase);
        e.record(&self.input);
        e.u64(self.sequence)
    }
}
impl DemandToken {
    pub fn context(&self) -> &DemandContext {
        &self.context
    }
    pub fn phase(&self) -> &PhaseIdentity {
        &self.phase
    }
    pub fn input(&self) -> &InputRequest {
        &self.input
    }
}
#[derive(Debug, Clone)]
pub struct Reply {
    pub context: DemandContext,
    pub token: Digest,
    pub input: Digest,
    pub output: Result<Digest, Diagnostic>,
}
impl Reply {
    pub fn success(token: &DemandToken, output: Digest) -> Self {
        Self {
            context: token.context.clone(),
            token: token.fingerprint(),
            input: token.input.fingerprint(),
            output: Ok(output),
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StartDecision {
    Start,
    InFlight,
    Reuse(Digest),
}
enum Operation {
    Queued,
    Running(Digest),
    Complete {
        result: Result<Digest, Diagnostic>,
        adopted: bool,
    },
}
struct Waiter {
    token: DemandToken,
    consumed: bool,
}
pub struct ExpansionCoordinator {
    context: DemandContext,
    closed: bool,
    next_sequence: u64,
    metadata: usize,
    demands: BTreeMap<Digest, Waiter>,
    operations: BTreeMap<Digest, Operation>,
}
fn waiter_key(token: &DemandToken) -> Digest {
    let mut key = StableEncoder::new("lrr/v2/demand-waiter");
    key.record(&token.context);
    key.record(&token.query);
    key.record(&token.phase);
    key.record(&token.input);
    key.finish()
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-DEMAND", message.into())
}
fn add(bytes: usize, extra: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(extra)
        .ok_or_else(|| error("demand metadata limit exceeded"))
}
fn query_bytes(query: &QueryKey) -> Result<usize, Diagnostic> {
    let mut bytes = std::mem::size_of::<QueryKey>();
    let mut scope = |modules: &[String], capacity: usize| -> Result<(), Diagnostic> {
        bytes = add(
            bytes,
            capacity
                .checked_mul(std::mem::size_of::<String>())
                .ok_or_else(|| error("scope size overflow"))?,
        )?;
        for module in modules {
            bytes = add(bytes, module.capacity())?
        }
        Ok(())
    };
    match query {
        QueryKey::NominalMember(_) => {
            bytes = add(bytes, query.heap_bytes())?;
        }
        QueryKey::PreparedBodySyntax(_key) => {
            bytes = add(bytes, query.heap_bytes())?;
        }
        QueryKey::CurrentModuleName(_key) => {
            bytes = add(bytes, query.heap_bytes())?;
        }
        QueryKey::ParseFile(k) => bytes = add(bytes, k.profile.semantics.capacity())?,
        QueryKey::Outline(k) => {
            scope(&k.module_path, k.module_path.capacity())?;
            bytes = add(bytes, k.parse.profile.semantics.capacity())?
        }
        QueryKey::ScopeBucket(k) => {
            scope(&k.scope.module_path, k.scope.module_path.capacity())?;
            bytes = add(bytes, k.name.capacity())?
        }
        QueryKey::SealedName(key) => {
            scope(
                &key.bucket.scope.module_path,
                key.bucket.scope.module_path.capacity(),
            )?;
            bytes = add(bytes, std::mem::size_of_val(key.as_ref()))?;
            bytes = add(bytes, key.bucket.name.capacity())?;
        }
        QueryKey::DiscoverySeal(_) | QueryKey::ScopeDeclarationInventory(_) => {}
        QueryKey::ConstructorPlan(_)
        | QueryKey::BoundRecordConstructor(_)
        | QueryKey::QualifiedBodyType(_)
        | QueryKey::SourceLocalPlan(_)
        | QueryKey::TypeSchema(_)
        | QueryKey::TypeAlias(_)
        | QueryKey::FunctionSignature(_)
        | QueryKey::ReferenceHeader(_)
        | QueryKey::PatternPlan(_)
        | QueryKey::Signature(_)
        | QueryKey::BodyStructure(_)
        | QueryKey::PreparedExplicitTypeArguments(_)
        | QueryKey::EnteredCall(_)
        | QueryKey::DeclaredReturnGoal(_)
        | QueryKey::PreparedReturnPlan(_)
        | QueryKey::EnteredType(_)
        | QueryKey::PreparedTypePlan(_)
        | QueryKey::PreparedBindingPlan(_)
        | QueryKey::BindingConstraints(_)
        | QueryKey::EnteredValue(_)
        | QueryKey::DeclaredParameterGoal { .. }
        | QueryKey::SourceConstraints(_)
        | QueryKey::Thunk(_) => {}
        QueryKey::ExpansionInput(k) | QueryKey::CurrentExpansionInput(k) => {
            bytes = add(
                bytes,
                std::mem::size_of_val(k.as_ref()) + crate::expand::record::key_heap_bytes(k),
            )?
        }
        QueryKey::Constant(k) => bytes = add(bytes, k.heap_bytes()?)?,
    }
    Ok(bytes)
}
fn input_bytes(input: &InputRequest) -> Result<usize, Diagnostic> {
    match input {
        InputRequest::Discover { .. } => Ok(std::mem::size_of::<crate::db::seals::SealKey>()),
        InputRequest::CaptureCompileEnvironment { .. } => Ok(std::mem::size_of::<Digest>()),

        InputRequest::CaptureModule { request, .. } => add(
            std::mem::size_of::<InputRequest>(),
            request.metadata_bytes()?,
        ),
        InputRequest::CaptureInclude { request, .. } => request
            .metadata_bytes()
            .ok_or_else(|| error("include request metadata overflow"))
            .and_then(|n| add(std::mem::size_of::<InputRequest>(), n)),
        InputRequest::Expand(r) => add(
            std::mem::size_of_val(&r.key),
            match &r.key.macro_identity {
                crate::expand::identity::MacroIdentity::Procedural { export, .. } => {
                    export.capacity()
                }
                _ => 0,
            },
        ),
        InputRequest::CaptureGeneratedSource(r) => {
            add(std::mem::size_of_val(r), r.path.as_str().len())
        }
    }
}

impl ExpansionCoordinator {
    pub fn new(context: DemandContext) -> Self {
        Self {
            context,
            closed: false,
            next_sequence: 0,
            metadata: 0,
            demands: BTreeMap::new(),
            operations: BTreeMap::new(),
        }
    }
    pub fn issue(
        &mut self,
        query: QueryKey,
        phase: PhaseIdentity,
        input: InputRequest,
    ) -> Result<DemandToken, Diagnostic> {
        if self.closed {
            return Err(error("request is closed"));
        }
        input.validate()?;
        match &input {
            InputRequest::Discover { .. } => {
                return Err(error("discovery demand requires the catalog coordinator"));
            }
            InputRequest::CaptureCompileEnvironment { unit } => {
                if *unit != self.context.unit {
                    return Err(error("compile environment demand unit mismatch"));
                }
            }

            InputRequest::CaptureModule {
                request,
                compile_environment,
            } => {
                if request.candidates().iter().any(|c| {
                    c.unit() != self.context.unit
                        || c.generation() != self.context.source_generation
                }) || *compile_environment != self.context.compile_environment
                {
                    return Err(error(
                        "module consumer unit/generation/environment mismatch",
                    ));
                }
            }
            InputRequest::CaptureInclude {
                request,
                compile_environment,
            } => {
                if request.unit() != self.context.unit
                    || request.generation() != self.context.source_generation
                    || *compile_environment != self.context.compile_environment
                {
                    return Err(error(
                        "include consumer unit/generation/environment mismatch",
                    ));
                }
            }
            InputRequest::Expand(r) => {
                if r.key.consumer_compile_env != self.context.compile_environment
                    || r.key
                        .procedural_epoch
                        .is_some_and(|epoch| epoch != self.context.epoch)
                {
                    return Err(error("expansion consumer environment/epoch mismatch"));
                }
            }
            InputRequest::CaptureGeneratedSource(r) => {
                if r.consumer_unit != self.context.unit
                    || r.compile_environment != self.context.compile_environment
                {
                    return Err(error("capture consumer unit/environment mismatch"));
                }
            }
        }
        let bytes = add(query_bytes(&query)?, input_bytes(&input)?)?;
        let mut key = StableEncoder::new("lrr/v2/demand-waiter");
        key.record(&self.context);
        key.record(&query);
        key.record(&phase);
        key.record(&input);
        let key = key.finish();
        if let Some(token) = self.demands.get(&key) {
            return Ok(token.token.clone());
        }
        let sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| error("demand sequence exhausted"))?;
        let metadata = add(
            self.metadata,
            add(
                bytes,
                std::mem::size_of::<Waiter>() + std::mem::size_of::<Operation>() + 128,
            )?,
        )?;
        let operation = input.fingerprint();
        let token = DemandToken {
            context: self.context.clone(),
            query,
            phase,
            input,
            sequence,
        };
        self.demands.insert(
            key,
            Waiter {
                token: token.clone(),
                consumed: false,
            },
        );
        self.operations
            .entry(operation)
            .or_insert(Operation::Queued);
        self.next_sequence = sequence;
        self.metadata = metadata;
        Ok(token)
    }
    fn check(&self, current: &DemandContext, token: &DemandToken) -> Result<Digest, Diagnostic> {
        if self.closed {
            return Err(error("request is closed"));
        }
        if current != &self.context || token.context != self.context {
            return Err(error("stale demand context"));
        }
        let mut key = StableEncoder::new("lrr/v2/demand-waiter");
        key.record(&token.context);
        key.record(&token.query);
        key.record(&token.phase);
        key.record(&token.input);
        let key = key.finish();
        if self.demands.get(&key).map(|w| &w.token) != Some(token) {
            return Err(error("unknown or superseded demand token"));
        }
        Ok(token.input.fingerprint())
    }
    pub fn start(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
    ) -> Result<StartDecision, Diagnostic> {
        let key = self.check(current, token)?;
        let operation = self
            .operations
            .get_mut(&key)
            .ok_or_else(|| error("operation missing"))?;
        match operation {
            Operation::Queued => {
                *operation = Operation::Running(token.fingerprint());
                Ok(StartDecision::Start)
            }
            Operation::Running(_) => Ok(StartDecision::InFlight),
            Operation::Complete {
                result: Ok(output), ..
            } => Ok(StartDecision::Reuse(*output)),
            Operation::Complete {
                result: Err(error), ..
            } => Err(error.clone()),
        }
    }
    pub fn accept(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        reply: Reply,
    ) -> Result<(), Diagnostic> {
        let key = self.check(current, token)?;
        if reply.context != self.context || reply.token != token.fingerprint() || reply.input != key
        {
            return Err(error("reply context/token/input mismatch"));
        }
        if !matches!(self.operations.get(&key), Some(Operation::Running(started)) if *started == token.fingerprint())
        {
            return Err(error("operation is not awaiting a reply"));
        }
        let extra = reply
            .output
            .as_ref()
            .err()
            .map_or(0, |e| e.heap_bytes().unwrap_or(usize::MAX));
        let metadata = add(self.metadata, extra)?;
        self.operations.insert(
            key,
            Operation::Complete {
                result: reply.output,
                adopted: false,
            },
        );
        self.metadata = metadata;
        Ok(())
    }
    /// Admit immutable captured bytes only for the exact authority-bound request.
    /// Source publication is separate; this does not make a phase resumable.
    pub fn accept_include(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        captured: &crate::service::source_capture::CapturedInclude,
    ) -> Result<(), Diagnostic> {
        self.check(current, token)?;
        let InputRequest::CaptureInclude { request, .. } = token.input() else {
            return Err(error("token is not an include capture request"));
        };
        if captured.request() != request.identity() {
            return Err(error("captured include request mismatch"));
        }
        self.accept(current, token, Reply::success(token, captured.identity()))
    }
    /// Publish the exact admitted include record before any saved phase resumes.
    pub fn adopt_include(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        db: &mut crate::db::memo::Database,
        captured: crate::service::source_capture::CapturedInclude,
    ) -> Result<(), Diagnostic> {
        let operation = self.check(current, token)?;
        if db.source_revision() != self.context.revision {
            return Err(error("include target source revision mismatch"));
        }
        let InputRequest::CaptureInclude { request, .. } = token.input() else {
            return Err(error("token is not an include capture request"));
        };
        if captured.request() != request.identity()
            || !matches!(self.ready(current, token)?, Some(Ok(output)) if output == captured.identity())
        {
            return Err(error("include record is not the admitted output"));
        }
        db.publish_include(operation, captured)?;
        let Some(Operation::Complete { adopted, .. }) = self.operations.get_mut(&operation) else {
            return Err(error("operation missing"));
        };
        *adopted = true;
        Ok(())
    }
    pub fn accept_module(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        captured: &crate::source::module_capture::CapturedModule,
    ) -> Result<(), Diagnostic> {
        self.check(current, token)?;
        let InputRequest::CaptureModule { request, .. } = token.input() else {
            return Err(error("token is not a module capture request"));
        };
        captured.validate(request)?;
        self.accept(current, token, Reply::success(token, captured.identity()))
    }
    pub fn adopt_module(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        db: &mut crate::db::memo::Database,
        captured: crate::source::module_capture::CapturedModule,
    ) -> Result<(), Diagnostic> {
        let operation = self.check(current, token)?;
        if db.source_revision() != self.context.revision {
            return Err(error("module target source revision mismatch"));
        }
        let InputRequest::CaptureModule { request, .. } = token.input() else {
            return Err(error("token is not a module capture request"));
        };
        captured.validate(request)?;
        if !matches!(self.ready(current,token)?,Some(Ok(output)) if output==captured.identity()) {
            return Err(error("module record is not the admitted output"));
        }
        db.publish_module(operation, captured)?;
        let Some(Operation::Complete { adopted, .. }) = self.operations.get_mut(&operation) else {
            return Err(error("operation missing"));
        };
        *adopted = true;
        Ok(())
    }
    pub fn ready(
        &self,
        current: &DemandContext,
        token: &DemandToken,
    ) -> Result<Option<Result<Digest, Diagnostic>>, Diagnostic> {
        let key = self.check(current, token)?;
        match self.operations.get(&key) {
            Some(Operation::Complete { result, .. }) => Ok(Some(result.clone())),
            Some(_) => Ok(None),
            None => Err(error("operation missing")),
        }
    }
    /// Install checked tokens only after matching reply admission. This does
    /// not execute the macro and does not advance a saved guest phase.
    pub fn adopt(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
        db: &mut crate::db::memo::Database,
        record: crate::expand::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        self.check(current, token)?;
        if db.source_revision() != self.context.revision {
            return Err(error("expansion target source revision mismatch"));
        }
        let InputRequest::Expand(request) = token.input() else {
            return Err(error("token is not an expansion request"));
        };
        if request.key != *record.key() {
            return Err(error("expansion record key mismatch"));
        }
        match self.ready(current, token)? {
            Some(Ok(output)) if output == record.output() => {
                db.publish_expansion(record)?;
                let key = token.input.fingerprint();
                let Some(Operation::Complete { adopted, .. }) = self.operations.get_mut(&key)
                else {
                    return Err(error("operation missing"));
                };
                *adopted = true;
                Ok(())
            }
            _ => Err(error("expansion record is not the admitted output")),
        }
    }
    /// Advance an item transform only from the exact reply already adopted into the DB.
    /// This schedules no native execution and cannot bypass record publication.
    pub fn admit_transform(
        &self,
        current: &DemandContext,
        token: &DemandToken,
        db: &crate::db::memo::Database,
        worklist: &mut crate::expand::worklist::ItemTransformWorklist,
        record: crate::expand::record::ExpansionRecord,
    ) -> Result<(), Diagnostic> {
        let operation = self.check(current, token)?;
        if db.source_revision() != self.context.revision {
            return Err(error("transform source revision changed after adoption"));
        }
        let InputRequest::Expand(request) = token.input() else {
            return Err(error("transform token is not an expansion request"));
        };
        if request.key != *record.key()
            || !matches!(self.operations.get(&operation),
                Some(Operation::Complete { result: Ok(output), adopted: true }) if *output == record.output())
        {
            return Err(error("transform output is not the exact adopted expansion"));
        }
        worklist.admit(record)
    }
    /// Claim one successful phase resumption. Shared input remains available
    /// to other query/phase waiters without replaying the operation.
    pub fn take_ready(
        &mut self,
        current: &DemandContext,
        token: &DemandToken,
    ) -> Result<Option<Result<Digest, Diagnostic>>, Diagnostic> {
        self.check(current, token)?;
        let key = waiter_key(token);
        if self.demands.get(&key).is_some_and(|w| w.consumed) {
            return Err(error("phase demand token is consumed"));
        }
        let operation = token.input.fingerprint();
        if matches!(
            self.operations.get(&operation),
            Some(Operation::Complete {
                result: Ok(_),
                adopted: false
            })
        ) {
            return Ok(None);
        }
        let result = self.ready(current, token)?;
        if matches!(result, Some(Ok(_))) {
            self.demands
                .get_mut(&key)
                .expect("validated waiter")
                .consumed = true;
        }
        Ok(result)
    }
    pub fn cancel(&mut self) {
        self.closed = true;
        self.demands.clear();
        self.operations.clear();
        self.metadata = 0;
    }
}
