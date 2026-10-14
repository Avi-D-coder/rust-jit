# T034 original-source test harness — in progress

Current implemented scope: original-source discovery/filtering with fresh guest
heaps, compiler-identity-resolved literal, bare zero-argument call, local and
parameter assertion predicates, explicit static panic, lazy failure-message
preparation, and sequential no-item certificates. General operands, formatted
messages, generated catalog integration and complete Section 28 execution remain
pending. Later sections record incremental component evidence; their historical
pending statements describe those earlier checkpoints, not the current scope.

`lrr test FILE.rs [FILTER]` captures the standalone source with `cfg(test)` and
builds a deterministic owned catalog. Filtering uses qualified names: substring
by default, `--exact` for equality. `--list` lists selected names without querying
signatures or bodies. Ignored cases are excluded by default; `--ignored` selects
only ignored cases, and `--include-ignored` includes both. These two options
conflict. Zero selected cases produce `LRR-E-TEST-NO-MATCH` and exit 5.

Each executed case uses a fresh `RunMachine` and guest store. Immutable captured
source and database memo facts may be shared; guest heaps, fuel and continuations
are independent. Malformed builtin control attributes fail only when their case
is selected. Active catalog-producing attributes require expansion and report
`LRR-E-CATALOG-INCOMPLETE` before filtering; this component cannot claim generated
tests absent merely because they have not been expanded.

Bare `should_panic` accepts a guest panic. Its `expected` string matches the actual
numeric panic description. Unsupported-feature faults, missing input, and normal
completion cannot satisfy expected panic. Selected async cases remain visible in
the catalog and fail as unsupported. CLI reports use the explicitly provisional
`lrr/v2/development-tests` schema and `original-source-component` scope.

Eight registered host acceptance tests cover cold unselected broken bodies,
listing/filtering/ignore/no-match, malformed attributes, expected panic versus
fault, cfg selection, no-query discovery, and repeated independent executions.
They do not replace the Section 28 fixture format. Builtin assertions and explicit
panic macros, that fixture harness, generated catalog integration and Gate 03
remain pending. T034 and the full release remain incomplete. macOS is the primary
validation platform; Linux acceptance is deferred and has not been claimed.

## Explicit panic LRIR primitive

The verified `Panic` operation uses stable opcode tag 25 and owns a UTF-8 message
bounded to 65,536 bytes, including retained capacity. Admission requires a macro
expression origin. The VM charges fuel before starting the operation, then returns
`GuestPanic` with kind `Explicit`, the owned message, exact PC and source origin.
It never executes the recipe successor. Numeric panics retain their existing
static descriptions; the test harness matches a panic's actual description.

This primitive requires an identity-resolved frontend producer. It does not
recognize macros by their spelling and does not yet enable `panic!` in live source.
Builtin identity resolution, argument parsing/expansion, message formatting,
assertion lowering and generated-origin integration remain pending. These tests
exercise verified recipes and physical guest frames, not native compilation or
successful macro expansion. Existing guest cleanup retires an actual panic scope
once; resource-fault refusal preserves that scope and starts no panic operation.

## Conditional assertion LRIR primitive

The verified `Assert` operation uses stable opcode tag 26, distinct condition and
unit-result slots, a macro expression origin and the same bounded owned-message
contract as `Panic`. Immutable preflight requires a concrete bool before charging
fuel. A true condition validates the unit destination and writes unit after the
charge. A false condition starts a guest panic with kind `Assertion` and the actual
message, leaving the result slot unallocated and the condition intact. A type or
fuel fault starts no assertion operation and preserves the operand and active
scope. The existing dispatcher cleanup path handles this guest panic.

Three registered tests exercise true/false outcomes, concrete-type/fuel refusal,
actual cleanup, macro-origin/slot/message admission and stable message identity.
This is a VM primitive; source macro identity selection, argument expansion and
ordered lazy message evaluation still require frontend integration. It does not
yet claim successful live `assert!`, `assert_eq!` or `assert_ne!` execution.

## Runtime macro definition facets

`frontend::runtime_macro_catalog::RuntimeMacroCatalog` constructs the compiler
facet for `panic`, `assert`, `assert_eq` and `assert_ne` in one exact compilation
unit and edition. Explicit stable semantic tags 0–3 contribute to the
`lrr/v2/runtime-macro-definition` identity encoding. The facade holds only the
owned semantic kinds. Selection accepts an exact resolved definition capsule;
matching identity with changed unit/edition is refused, and unrelated user
identities remain nonintrinsic. This catalog neither expands tokens nor executes
guest/native code.

Its ordinary `SourceMacroRoutes` contributions use the existing namespace and
visibility resolver. Actual parsed source checks cover builtin aliases, a public
reexport chain, raw identifier spelling, user macros named `assert` and `panic`,
an inaccessible private-alias reexport, ambiguity and absence. Catalog rebuilding
is deterministic; different units/editions produce distinct identities.

Separate Rust 1.99 macOS oracle preparation retained
`t034-public-macro-alias-oracle.rs` (compile exit 0 and run exit 0, empty stdout and
stderr) and `t034-private-macro-alias-oracle.rs` (compile exit 1, E0364). These
validate native macro routing/visibility behavior; they do not establish guest
macro execution. No selected live runtime unit was compiled for these checks.
Entered invocation selection, source argument parsing/expansion, lazy message
execution and the connection to the tested LRIR operations remain pending.

## Section 28 fixture contracts

`engine::fixture` reads schema-1 fixture manifests with unknown fields denied at
every table. IDs, paths, entry names, modes, text sizes and retained metadata are
checked. `FixtureCatalog` indexes a supplied batch deterministically and rejects
duplicate IDs and aggregate metadata above 32 MiB. Relative source paths are
checked lexically; capture/loading and symlink permissions remain the source
loader's responsibility. Native-pass oracles require root entry `case` and cannot
be requested by relaxed/profile-fault fixtures.

Expectations retain the exact status text supplied in the manifest; no status
normalization is performed. `expected_drop_log` belongs in `[expect]`; repeated
events are meaningful and ordered. Observation records carry a typed completed,
guest-panic or fault outcome and optional measured fields. Missing output, mode,
panic, cleanup, drop, body or marker evidence fails when required. Marker checks
require separate thunk-execution, installed-operation and native-operation counts.
Force mode requires both native counters even for default zero minima. Runtime
faults cannot satisfy a passed or guest-panic expectation. No collector substitutes
an inferred zero for an absent observation.

Five host tests verify this contract with explicitly supplied model observations,
including missing/corrupt records. They are checker acceptance, not executed guest
fixture passes. `compat/fixtures/T034-FORMAT` contains the schema-1 on-disk example
and source; its runtime acceptance is **not run**. The test confirms that merely
reading this contract cannot satisfy its required observations.

The execution runner, captured broker transcripts, actual marker-to-expression
full-span binding, query/install/native observation collection, native-oracle
wrapper and required mode matrix remain pending. Source macro argument/lowering
integration and Gate 03 also remain pending. T034 and release remain incomplete.

## Original-source fixture marker bindings

`ParsedFile::fixture_markers` uses pinned syntax to associate a marker comment
with the complete following expression, preserving UTF-8 byte ranges and skipping
trivia. When multiple expression nodes begin at the following token, the outermost
full span is retained. String literals that look like marker comments are ignored.
Duplicate/malformed/orphan directives fail. Indexed parse-error ranges reject an
unreliable marked expression without rejecting an unrelated cold syntax error.
The owned result records its exact content/profile parse key; stale use is refused.
Stable encoding uses explicit expression/demand tags and deterministic name order.

A marker inside an opaque macro argument returns `NeedsMacroExpansion` with the
actual invocation range. It is not a guessed expression or a zero preparation
count. Generated/source argument binding still needs the expansion/lowering path.
The scanner has checked expression/token/marker count and charged metadata limits;
no parser node, query execution, macro execution or guest state escapes it.

Four host source tests verify ordinary full spans, UTF-8/trivia, source staleness,
parse boundaries, opaque macro demand and the actual on-disk source marker.
`T034-CONTRACT-ON-DISK` still has **no executed runtime acceptance**. Binding its
`unknown()` span alone supplies none of the required query/install/native/output
observations. The execution runner and those collectors remain pending.

## Actual guest preparation observation

`FixturePreparationObserver` captures a detailed query-trace window and immutable
source/body atlas before execution. It reads actual completed trace events and
installed verified recipe origins afterwards. Body counts include provider and
cache-served structural demands; marker thunk counts count provider runs only.
Installed-operation counts remain separate and include warm memo-backed recipes.
Collection neither fetches queries nor changes guest fuel/state.

Missing/ambiguous bodies, missing or macro-pending markers, unknown/evicted body
mappings, expanded origins needing integration, stale publications, disabled or
truncated traces fail collection. Full byte spans attach through exact body keys
and source ranges. The installed table exposes immutable inspection only.

The actual `T034-CONTRACT-ON-DISK` guest case now completes with unit on macOS.
Its `broken` body has zero measured structural demands; `cold_branch` has zero
measured thunk provider runs and installed operations. A reached-branch companion
fault has positive counts in both marker channels. A warm literal companion has
zero provider runs but positive installed-operation preparation, proving why
provider counts alone are insufficient.

These are partial execution observations, not complete fixture acceptance.
Output, raw outcome grading, cleanup/drop and native channels remain absent in
this observer. The contract checker still rejects its partial record. No zero
native count or empty output is inferred. Complete runtime/broker/native/oracle
collection and source macro lowering remain pending; T034 and release remain
incomplete.

## Entered runtime macro argument capture

`ParsedFile::runtime_macro_arguments` captures one exact original invocation
using a semantic kind already selected by the frontend. It does not infer builtin
identity from the invocation name. A private pinned-parser expression-list wrapper
handles nested generics, blocks, closure commas and all three delimiters; only
owned original byte ranges leave the syntax boundary. The wrapper is never run or
compiled. Exact source/profile affinity and bounded input/argument counts are
checked, with explicit stable encoding.

Assertions retain their required operand ranges in written order (one for assert,
two for equality/inequality). The remaining formatting token tail is retained
separately, including named formatting syntax and trailing comma. Capture performs
no operand evaluation, name/type resolution, thunk preparation or formatting.
Empty panic and trailing assertion commas are admitted; missing operands,
incomplete expression lists and oversized inputs fail.

Four source tests cover these shapes, UTF-8 range mapping, aliases with supplied
semantic kind and stale source keys. This is capture acceptance, not live assertion
or panic execution. Identity selection wiring, argument-body publication/ordered
execution, lazy formatting, generated/hygienic arguments and complete fixture
output/native/oracle collection remain pending. T034 and release are incomplete.

## Identity-resolved runtime macro preparation

`frontend::runtime_macro_plan` now connects the existing resolved namespace
outcome, compiler catalog and pinned source argument capture. Only a `Defined`
compiler capsule accepted by the catalog prepares an intrinsic. User definitions
return `SourceExpansion` with their original invocation; absent, ambiguous,
inaccessible and missing-input results return the unchanged typed resolution
outcome. Builtin argument syntax is never imposed on these other cases.

An owned intrinsic plan retains the exact definition/invocation/argument source
facts. Assert uses predicate intent; equality/inequality use shared-comparison
intent. Panic messages are always needed; assertion messages are needed only on
assertion failure. These are lowering contracts, not implemented guest leases,
trait comparison, formatting or opcode execution. Preparation rejects stale
source/profile, forged builtin provenance and malformed builtin operands.

Four preparation tests and the extended actual-source import test verify the
connection. A public `confirm!` alias prepares the compiler assertion; a source
`panic` definition remains a source-expansion outcome even with tokens outside
builtin expression-list grammar. No user expansion or operand/message execution
is claimed. Argument-body publication, guest lowering, borrow/trait integration,
lazy formatting and complete runtime fixture acceptance remain pending.

## Static resolved panic lowering

Runtime argument capture now retains the message-argument count and an optional
pinned decoded first string literal. These are structural facts, not expression
execution. `RuntimeIntrinsicPlan::lower_static_panic` accepts only a resolved
panic plan and emits the existing owned `Panic` operation for no arguments
(`explicit panic`) or one brace-free string literal. Empty strings, escapes and
raw strings preserve their actual decoded message. Payloads above 65,536 bytes
fail with a metadata-limit diagnostic.

Any brace-containing template, implicit capture, dynamic expression or additional
format argument returns `NeedsFormatting`; no template is substituted as the
actual panic message. Even escaped braces retain this demand until formatting is
integrated. Assertions cannot be misused as terminal panic plans.

Three tests lower prepared source plans, verify the operation against the actual
original macro body node, execute guest frames, compare message and origin, and
perform actual panic cleanup. The four default/escaped/raw/empty messages agree
with the separately prepared Rust 1.99 macOS oracle (compile/run exit 0, empty
stderr). The source wrapper used for structural capture is not compiled or run.

This proves resolved-plan-to-verified-VM lowering for the static cases. Automatic
RunMachine source macro dispatch, argument-body publication, conditional assertion
lowering, formatting and complete fixture output/native/oracle observation remain
pending. No complete live-source macro or release acceptance is claimed.

## Source-affine static panic recipe emission

`RuntimeIntrinsicPlan::lower_static_panic_recipe` now emits a verified recipe only
after matching the immutable source parse/profile key, owned body key, entry
phase, macro expression node and exact absolute invocation range. The body handle
must be present in the retained source publication with reliable entry boundaries.
The normal LRIR verifier checks the emitted origin, operation and successor.
Formatting demand remains a typed result without recipe emission.

Three tests execute the emitted recipe with its actual original-node guest origin
and cleanup, refuse another macro/phase/body/stale source, and retain formatting
demand. The caller no longer needs to construct the panic recipe or choose its
source origin manually. This is emission plus verified VM execution, not automatic
RunMachine lookup/installation. Source dispatch, conditional operand lowering,
formatting and complete fixture acceptance remain pending. T034 stays in progress.

## Lazy dispatcher integration for supplied static panic plans

`RunMachine::queue_static_panic_plan` accepts a resolved plan from the frontend
only before execution and only for an exact retained source publication and root
macro node. Duplicate, excessive, foreign, stale and late plans refuse. Owned
argument/path/module/string capacities contribute to a checked queue budget.
Queuing syntax facts fetches no thunk and installs no executable recipe.

When execution reaches that macro entry PC, the dispatcher invokes the checked
source-affine emitter and installs the verified recipe through the existing
bounded installed-PC table. Prepared installation shares the normal table charge
and refuses replacement/stale publication. A reached static message produces the
actual guest panic; a cold queued branch produces no thunk, installed recipe or
panic opcode. A formatter-dependent plan faults as unsupported when reached and
never satisfies expected guest panic.

Three real RunMachine tests verify reached/cold behavior and queue refusal, in
addition to the earlier emission/VM tests. This API consumes supplied resolved
facts; automatic source namespace preparation is not implemented yet. Only root
static panic plans are wired here. Callee/generated plans, assertion operand and
lease lowering, formatting, source CLI preparation and complete fixture acceptance
remain mandatory and pending. T034 and release remain incomplete.

## Called-function static panic dispatch

Supplied static panic facts are now keyed by exact owned body identity and relative
invocation range. Queue attachment uses immutable source body metadata rather
than callee body indexing or PC construction. The active called body's macro PC
selects the plan only when reached; the source-affine emitter still validates its
actual node/range before installation. Absent/ambiguous body attachments refuse.
The earlier root-only index is removed; source attachment consistency is retained.

Three added RunMachine tests verify a leaf panic unwinds two suspended callers
with two published/consumed actions and three cleaned scopes; an uncalled leaf has
no BodyStructure memo or installed origin; and recursion reaches 202 guest frames
with 201 published/consumed actions, one panic opcode and complete active-frame
cleanup. These use the existing explicit dispatcher frames, fuel and cleanup path.

This completes supplied original-function static panic plan dispatch across calls.
It does not automate source namespace preparation or claim generated/callee
argument expansion, assertions, formatting, Drop/reference integration or complete
fixture grading. Those integrations and Gate03 remain pending; T034 and release
stay incomplete.


T034 entered source resolution checkpoint: `EnteredMacroIndex::prepare_runtime_macro`
resolves the retained invocation with actual textual/path/hygiene lookup and captures
arguments from the exact immutable source publication. A compiler `panic` imported
as `stop` reaches the actual dispatcher through the existing bounded static plan
queue. Body-local `macro_rules! stop` retains its user definition and custom grammar.
An earlier unexpanded producer retains `NeedInput`; a missing name retains `Absent`.
Stale selection, foreign body and partial invocation ranges fail before preparation.
This remains explicit plan attachment; automatic CLI preparation, generated-source
expansion, dynamic formatting and assertion operand evaluation remain pending.


T034 reached runtime macro context checkpoint: `RuntimeMacroContext` borrows an
exact publication-backed source index, its macro namespace and the compiler facet.
`RunMachine::poll_with_runtime_macros` resolves/captures only reached original macro
PCs and installs checked static panic recipes through the normal budgeted installer.
The compiler facet and its disclosed extern aliases also enter the ordinary import
engine, allowing a macro-only import to resolve without blocking an unrelated source
callee. No type/value definition is fabricated. A reached callee panic unwinds actual
callers without prequeued plans. Malformed arguments in a cold branch and an uncalled
body remain untouched; installed PCs skip resolution on reuse.

`run_original_with_runtime_macros` supplies this context to a fresh guest test run.
Actual expected panic passes; missing/ambiguous names, a private strong import,
user expansion and dynamic formatting faults cannot satisfy `should_panic`. Private
strong imports fail in the existing resolver with `LRR-E-MACRO-SCOPE`. Distinct
unresolved `PathLookup` variants retain distinct dispatcher diagnostics. Foreign
publication and duplicate contexts refuse before execution, leaving fuel/install
counts unchanged. This is explicit supplied-context integration; automatic compiler
prelude/CLI wiring, generated discovery and expansion suspension, formatting,
assertion operand evaluation, full fixture transcripts and Gate03 remain pending.


T034 literal assertion integration checkpoint: opaque macro nodes now reserve two
stable value temporaries (four typed slot positions) during deterministic body
indexing. This is metadata reservation, not argument syntax/thunk preparation or
cell allocation. The index budget checks the extra slots and includes their stable
identities; guest frames attach them to the macro's lexical scope.

After actual definition resolution, the pinned argument parser records a boolean
literal predicate. The entry recipe emits the ordinary VM `Literal` operation with
its original operand byte span and branches to success/failure phases. Success
emits unit and uses the original source continuation. It never installs the failure
phase or prepares executable message code. Failure alone lowers a static owned
message into `Assert`, producing an actual assertion guest panic and normal cleanup.
Dynamic formatting remains a typed unsupported demand on failure; it does not
block literal-true success. Nonliteral operands and comparison assertions still
require operand expansion/evaluation integration.

Acceptance covers a cold broken message function, actual default/escaped UTF8/raw/
empty failure messages and macro origins, exactly one unfinished panic operation,
nested-scope cleanup, fresh test expected-panic grading, missing operand/formatting
fault refusal, stable bounded temporary metadata, and three actual loop iterations
using one installed predicate recipe with retired scoped cells. Source/body/phase/
node/kind guards reject invalid assertion recipe preparation. A separately compiled
Rust 1.99.0 macOS native oracle records the same four messages and lazy successful
message behavior; it compiles no selected live runtime unit. Automatic prelude/CLI
wiring, general operand expressions, sequential macro producer retirement, generated
source, formatting, complete fixture transcripts and Gate03 remain pending.


T034 source predicate-call checkpoint: the pinned argument parser captures a bare
zero-argument source function call after actual assertion definition resolution.
Original-source lexical guards reject local binding, parameter, import or hoisted
item contexts that the module-call dispatcher cannot yet represent. The entry
recipe uses the normal call seal and exit, with a strict boolean result hint and
an exact scoped predicate temporary. Recipe admission accepts this macro call
form only with the exact two-operation hint/seal layout, empty arguments/generics,
matching temporary and reserved operand-return phase. Other macro call forms
remain rejected.

The dispatcher reads the result slot from the checked call exit and uses its
prepared primitive hint before guest mutation. Named primitive return types are
resolved in the declaration's source scope, not recognized by spelling. A known
non-boolean return rejects before callee body preparation or call publication,
even when that body would otherwise panic. Valid predicates use ordinary source
call frames, shared fuel, argument actions, return contracts and panic unwinding.
The predicate-return branch uses the existing scoped phase-unit pattern; assertion
result storage remains unallocated on failure. Success keeps failure formatting
cold. Missing predicate names now use the ordinary `LRR-E-NAME-MISSING` diagnostic.

Acceptance records one actual predicate call publication/consumption, two guest
frames, cold broken message code, false assertion origin, normalized default
messages for comments/raw identifiers/Unicode, lexical/parameter refusal, strict
admission, known return mismatch and original callee panic cleanup. A separate
Rust 1.99.0 macOS native oracle confirms those three default messages. Calls with
arguments, general expressions, comparison assertions, automatic prelude/CLI,
sequential producer retirement, generated-source/formatting/full-fixture integration
and Gate03 remain pending. No selected live runtime unit is compiled.


T034 automatic standalone macro namespace checkpoint: `StandaloneRuntimeMacros`
constructs deterministic synthetic core/std macro facets and original-source
hygiene facts through the pinned adapter. Std exports preserve core definition
identity. The existing source namespace implements an explicit macro prelude
bucket, after textual and named lookup; qualified `self`/`crate` paths receive no
prelude fallback. The CLI's run/test paths supply this context automatically.
Listing remains metadata-only and does not construct/capture entered arguments.
Selected cases still use separate guest machines and heaps.

Cfg-aware original crate/module policy respects no_std and inherited outer/inner
no_implicit_prelude. Both macro and automatic extern lookup are disabled in those
scopes, including source import route planning. Active invalid imports fail typed
namespace preparation before test execution, while inactive scopes stay excluded.
Prelude metadata has deterministic indexes, per-scope and aggregate bounds.
Original user macro shadowing retains a source-expansion demand; it never becomes
an intrinsic or expected panic. The synthetic facet currently exposes the four
implemented compiler macro kinds, not the full later facade inventory.

Actual CLI acceptance exercises selected assertion success with a broken sibling,
prelude/std/core guest panics, std macro aliases and source predicate calls, shadow
and qualified-absence faults, no_std, cfg-aware disabled preludes, inactive modules,
invalid argument listing, run outcomes and typed invalid-header refusal. Separate
Rust 1.99.0 macOS oracles confirm the three panic messages/lazy success and E0433/E0432
for disabled automatic std lookup and imports. Explicit extern declarations in
disabled scopes, broader compiler macro exports, calls with arguments/general
operands/comparisons, sequential producer retirement, generated source, formatting,
complete fixture transcripts and Gate03 remain pending. No selected live unit is
compiled and no macro execution authority is conveyed by namespace preparation.

The sequential original compiler macro component admits a bounded no-item certificate only after exact compiler identity resolution and pinned-AST inspection of every argument. Nested macros, declarations and attributes withhold the certificate, including cold formatting operands. Admission checks source publication, body, invocation and compiler catalog, and retires only the exact entered producer. Certificates are run metadata; predicate calls and panics continue through the ordinary guest dispatcher. This does not satisfy general expansion, formatting or generated-source acceptance.

Bare local and parameter assertion predicates use the same pinned original-source lexical binding helpers as ordinary path expressions. Lowering emits normal VM ReadLocal or ReadParameter into the scoped macro predicate temporary, then branches on the actual guest bool. Shadowing, repeated reads and mutation remain guest operations; no predicate value is cached. Failure alone prepares the default source name or explicit message. Non-local and qualified operands retain typed pending resolution, and nonboolean values remain faults. Owned predicate-name capacity is included in the checked argument metadata budget. General expression operands still require generated-source integration.

The fixture observer now collects terminal facts retained privately by the actual
dispatcher. First terminal completion, panic description or fault code remains
immutable after public report mutation and repeated polling. Before termination
there is no outcome. Entry BodyKey and observation start must match; older runs
and another entry on the same publication cannot supply fixture evidence.
Cleanup observations use actual return/panic scope counters. A retained panic
message has a separate checked 64KiB budget. Output, executed tier mode, diagnostic
phase, destructor transcript and native counts remain unmeasured; partial outcome
collection still cannot pass the full on-disk fixture contract.
