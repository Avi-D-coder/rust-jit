# Reached local callee source preparation — LOCAL-024 (pre-code)

M017/M018, T024/T030, October16. An entered local function needs the same
BodyEnvironment/SourceLocalPlan as main. The current cold APIs correctly refuse
active-run use; calling them after a lexical callee is selected is not valid.
The unchanged upstream integer-literal-suffix-inference local functions and
local nominal field bodies are primary witnesses. Child metadata must not be
prepared while merely querying a declared signature or an unentered call.

Extend only actual RunMachine::prepare_source_call after selected callee and
all reached argument types are available, before contracts/normalized-return
proofs/GenericEnv capture their revision. Fetch the actual child's existing
BodyStructure once and reuse that Arc at frame entry. A lexical call uses the
private coordinator-published LocalDeclarationOwner -> parent SourceLocalPlan,
real selected SourceCallable/lexical proof, exact indexed Call site and actual
source rows. The source worker's capture_local admits the exact child through
parent source/hygiene/facts; it does not select a name or run code.

The engine mints a private reached preparation receipt containing only stable
source/call identities and immutable admitted source handles. A separate derived
DB entry requires an active run, empty query stack and current receipt before
reusing the common body-environment publication implementation and ordinary pure
SourceLocalPlan query. Bulk child header metadata uses the same exact per-item
publication; it grants neither DefinitionBody nor a Type name seal. Cold public
entry points and source-I/O quiescence guards remain unchanged. There is no
filesystem read, compiler invocation, macro execution, VM callback, guest value
or effect in the receipt/publication/provider. NeedInput returns before arguments
are transferred or call_ready/action consumption and resumes this same demand.

BodyEnvironment is derived source metadata, like exact Definition* rows and
LocalDeclarationOwner; exclude it from lexical namespace census only while
its own exact input, real discovery-seal and parent/child source dependencies
remain guarded. Do not relax normalized return or GenericEnv revision checks;
finish new derived publication before minting either proof. Reuse an already
current child environment; stale source errors never become fallback success.

Account parent-plan/environment DAG roots in the existing retained source pool.
No new registry, full-file node search, synthetic body, mutable namespace graph
or deep BodyIndex copy. Existing per-item cold header validation and global
legacy lexical census remain explicit cost/architecture debt; no performance
claim. All eight recurring failure modes and MA001-MA008 require separate
source-aware review on exact frozen bytes. This change adds no macro-specific
expression, method, numeric or pattern rule. Ordinary module-call environment
selection and recursive lexical member selection remain integration obligations.

Evidence needs original/retained/generated/wrapper local calls, exact-once
argument/callee effects, suspension/resume before transfer, signature-only and
cold child absence, stale owner/epoch/input refusal, active-query/cold API
refusals, query/source quotas and retired parent roots. Component source alone
cannot close those requirements or the genuine Drop/whole-main failure.
