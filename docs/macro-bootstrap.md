# Macro bootstrap evidence

M001 uses rust-analyzer commit
`bcf0514497034e1156f3cc2bb72b99cc9634a949`, recovered from the published
`ra_ap_syntax 0.0.356` package's `.cargo_vcs_info.json`. That metadata reports a
dirty publishing tree; the adapter deliberately uses the exact recorded upstream
commit, while the runtime retains its separately pinned published parser. No
upstream parser/interner identity crosses the adapter's owned records.

The retained archive has SHA-256
`fa50adddc735fc46b4a5e98f2650469b72f6cfe13615a78d918582e84de2f43d`.
The authoritative spelling is in `vendor/UPSTREAM.toml`; validate it and all 2,345
file/symlink entries with `python3 tools/bootstrap_vendor.py`. The complete source
workspace, member paths, original lockfile, and MIT/Apache licenses are preserved.
Cargo builds only the adapter dependency closure. The root workspace excludes the
vendored workspace. No upstream files have been patched.

The local adapter probe exercises the pinned lexer/token bridge and macro-rule
parser for editions 2015, 2018, 2021, and 2024. The resolved closure audit rejects
inference/HIR/IDE dependencies, compiler-private feature activation, local unsafe,
runtime Salsa usage, and inference function/method imports/calls. An intentionally
injected `infer_body` call fails the source audit. This does not establish macro
execution, hygiene wire remapping, or a matched server protocol.

M001 local component evidence is in `artifacts/gates/m001-components.json`,
`artifacts/m001-workspace-local-tests.log`, and `artifacts/m001-clippy-local.log`.
The ARM64 macOS host uses debug information disabled and incremental compilation
disabled for host validation because default debug artifacts exhausted the disk.
These settings do not change guest/native preparation flags or release settings.
The failed build and lint outputs are retained separately. Historical Linux bootstrap remains
unverified after Docker storage errors. Before the macOS-first amendment, gate 00
failed the Linux-only host check; that prerequisite is now deferred.

M002's controlled probe is `tools/probe_compile_time_deps.py`. It requires explicit
`--allow-build-tools`, absolute pinned tool paths, a new evidence directory, and a
separate target directory. It uses nightly-2026-10-01 Cargo with the explicitly
selected Rust 1.99 compiler, a forwarding compiler recorder, and the fixture at
`fixtures/macros/tool-workspace`. It never forges compiler output or skips a compiler
invocation. The fixture has an invalid ordinary cold body, a proc macro, a build
script with an execution counter, generated source/cfg/env, a dev-feature witness,
and an integration-test selector.

`artifacts/compile-time-probe/attempt-002/report.json` records 37 passing controlled
checks. Both selectors prepare tools without compiling live source. Macro artifacts
are selected from Cargo messages. Warm preparations report fresh macro artifacts,
run no non-information compiler invocation, and leave script counters unchanged.
Ordinary checks actually invoke the compiler and reject the deliberately missing
function with E0425. The lib-test control records `--test`, verifying that the pinned
planner's `--lib --profile test` selection has test intent. Attempt 001 is retained:
its preparation behavior was correct, but the probe incorrectly expected oracle
diagnostics exclusively in stdout JSON. The corrected check requires a real failed
live compiler invocation and the exact missing-function diagnostic, accepting
Cargo's rendered stderr as evidence too; the wrapper also preserves jobserver FDs.

`xtask compile-time-probe` now feeds the actual captured graphs into the production
`project/cargo_units.rs` decoder and publishes owned normalized unit records.
Stable identities include roles, exact target/profile/test intent/features/cfg,
dependency edges, and compiler identity. Unit-array reordering preserves identities;
unknown required fields, unsupported profile formats, cycles, and unselected units
fail publication. Host macro/build units retain separate contexts. Preparation
captures the Cargo-reported macro artifact hash before the server probe consumes it.

M003 captures the server from the selected Rust 1.99 sysroot's owning component
manifest. The external server reports protocol 6 and compiler commit b940084d.
The pinned client loads only the recorded Cargo artifact after hash, compiler,
protocol, and separate macro-authority checks. Actual `identity!(3_i64)` and an
attribute echo preserve tokens and spans, including raw identifiers/literals,
Unicode, punctuation spacing, suffixes, and lifetimes. Raw protocol responses,
hashes, actual helper PIDs, and mismatch diagnostics are recorded in
`artifacts/m003-server-compatibility.json`. Wrong server hash, compiler tuple,
artifact hash, protocol, and absent authority are refused before library loading.

`xtask compile-time-tools-gate` runs fresh M001–M003 component probes and workspace
acceptance tests. Earlier `artifacts/gates/compile-time-tools.json` results passed capability and
acceptance checks but failed the former Linux-only host check. The macOS-first
amendment selects the actual macOS target for new preparation and execution.
Linux-host validation is deferred; the full revision-4 implementation remains
incomplete. Broader process
timeouts, cancellation, and containment are later tasks, not established by this
controlled bootstrap probe.

`tools/linux-bootstrap.py --allow-build-tools --evidence-dir NEW_DIRECTORY`
provides an isolated Linux replay using the exact Rust image digest documented in
dependency-lock.md. The input checkout is mounted read-only and copied without its
derived build caches; container evidence is exported to the new directory. The
runner exercises H004 reuse, gate 00, the compile-time-tools gate, and strict
Clippy, with actual exit codes and raw output. It never restarts Docker. A daemon
readiness timeout fails the run before any container or compiler is launched.
This runner does not establish Linux acceptance until its actual reports pass.
`artifacts/linux-bootstrap/attempt-001/report.json` records the actual 20-second
daemon-readiness timeout. No Linux container or compiler started. The runner's
authority regression and the ignored-fixture gate regression pass locally. Gate 00
now requires every registered fixture to have a real passing test event, in
addition to a successful Cargo exit. The ignored-fixture witness runs a separate
tiny host Cargo test package and observes its disabled test; it is not a skipped
release fixture.

The October 4 macOS-first amendment makes local macOS gates authoritative for
current task progression. The selected target participates in the tool cache key,
compiler cfg capture, Cargo unit graph, and reported guest target. Linux-first
Docker failures are historical evidence, not current blockers. Docker restart
authorization is no longer needed to continue macOS implementation.

Initial macOS-first evidence passed all checks with 71 host/doc tests and
`artifacts/macos-first/clippy.log` passed strict Clippy at that checkpoint.
The live gate artifacts are updated by later tasks; current counts belong to
their named checkpoint artifacts rather than this historical bootstrap count.
The former gate failure caused by historical Linux-target fixture decoding is
retained in `artifacts/macos-first/first-gate-target-fixture-failure.json`. The
pure decoder recognizes explicit macOS/Linux preparation targets independently
of the current host; that recognition is not a Linux execution/support claim.

M004 generated capture now opens only an explicitly selected build-script
OUT_DIR capability. Two bounded tree captures compare file bytes, directory
membership, canonical paths, and inode/ctime/mtime metadata. Escaping symlinks,
directory cycles, special files, oversized trees, and replaced OUT_DIR roots
are refused. Immutable generated blobs retain their exact producer unit,
normalized relative path, original virtual path, content digest, and bytes.
Persisted trees validate producer/path/content and parent membership before use.

The same-name unit-isolation and racing-write tests currently exercise controlled
filesystem roots with explicit unit keys. They do not yet establish the full
MX005/MX006 preparation contract. No build tools or macro execution are invoked
by generated-tree capture or by query providers.

M004 now has schema4 toolchain, graph, artifact, build-output, compile-environment,
and source/external-input inventory records. Stable encodings preserve absence
distinct from empty values. Pure decoding bounds wire size/depth and rejects
duplicate object keys/set members, unknown fields, stale schemas, noncanonical
source-path wire forms, inconsistent unit/dependency identities, and mismatched
OUT_DIR/consumer associations. Only host-world tool artifacts may appear in this
manifest; native/runtime code is never loaded by these APIs. The JSON contract in
compat/preparation-v4.json documents shapes; Rust validation additionally checks
identity and semantic constraints. The field-contract test does not claim an
independent JSON Schema validator ran.

ToolStore writes a bounded complete staging file and syncs it before linking it
under its content identity. A create-only hard link atomically exposes the final
name without replacing an existing generation. Concurrent/idempotent publication,
stale observed inputs, malformed records, and tampered store files have acceptance
tests. Reads validate the full record and filename/content identity. These tests
establish namespace atomicity, not power-loss durability or arbitrary hostile
filesystem/process containment.

M004 input capture now consumes explicitly opened source and external directory
capabilities, selected files/subdirectories or whole roots, exclusions, and declared
environment names. Two bounded passes record file contents and absence, directory
membership, canonical routes and file/link metadata. The pre/post witness retains
identity changes even when replacement files restore the original bytes. Capture
has a 128 MiB aggregate byte limit, 100,000-member limit and bounded traversal.
These are preparation resource limits; later source parsing has its own limits.
No build tool or macro execution is a capture or adoption-comparison side effect.

Publication with capture witnesses refuses changed inputs before creating a stage
file and preserves the previous record. The initial pure adoption comparison verified caller-supplied toolchain and input
records. The prepared-file checkpoint below strengthens that API by requiring a
separately captured witness for current tool/artifact binaries. Real Cargo message
association, verified current binary capture, and complete MX005/MX006 preparation
integration remain M004 work. The schema/store fixtures use declared test toolchain
tuples and do not pretend to be compiled tool evidence. The input checkpoint passes
macOS Gate 01 with 112 host/doc tests and strict Clippy. Clippy caught one redundant
closure in a test; its correction and successful rerun are retained in the logs.

M004 build-script records now have a bounded strict decoder for the pinned Cargo
message shape. Duplicate object keys, cfgs or environment names, reserved OUT_DIR
overrides, malformed paths and unknown fields fail. Association indexes graph
producers by package and consumers by exact dependency edge; it never selects the
last message. A package record with multiple runnable unit contexts fails and
requires separately selected planner invocations. Duplicate unit reports and
shared output directories also fail. The normalized graph and generated records
retain the exact producer/consumer identities and link directives.

The two-build-unit fixture uses actual Rust 1.99.0 and the pinned nightly Cargo
planner on the selected host. Each of two packages emits generated.rs with distinct
bytes, environment and cfg. The test captures both OUT_DIRs through the production
association/capture APIs, verifies consumer edges, checks independence from message
order, and refuses a deliberately shared OUT_DIR. A declared forwarding compiler
wrapper logs every real invocation and always runs the compiler. The assertions
require both build scripts to compile and reject any compilation of a live source.
The source-input capture is unchanged across preparation. Raw graph, messages,
compiler argv/status, tool versions and captured output records are retained under
artifacts/compile-time-probe/two-output-*.

This establishes actual Cargo output association and immutable generated-byte
isolation. It does not yet establish interpreter execution of the include!/env!
expressions, a complete schema4 manifest assembled from independently verified
current tools/artifacts, build-script dependency-metadata capture, or the whole
MX005/MX006 transaction. Those requirements remain open rather than counting these
component fixtures as the full named release fixtures.


M004 adoption now requires PreparedFiles, a private witness bound to the validated
manifest identity. Capture groups only declared file names under explicitly opened
directory capabilities and uses the bounded two-pass input reader. It verifies
planner, compiler, matched macro server, component manifest and every compiled
artifact against recorded contents. Missing/tampered bytes, escaping symlinks and
inconsistent expected hashes fail. Invalid live/native artifact roles fail schema
validation before any file capability is opened. This operation never executes a
program or loads a library; matched-server protocol execution retains its separate
M003 authority and checks.

The stronger publication API checks both source-input and prepared-file witnesses.
Replacing a compiled file with identical bytes still changes the pre/post identity
witness and refuses publication while preserving the previous generation. The
pure adoption decision checks caller-captured current toolchain and source inputs
and the file witness for this exact immutable manifest. Callers must capture those
current witnesses at the preparation/adoption boundary; these are snapshot checks,
not a guarantee that an arbitrary external filesystem cannot change afterward.

The actual two-output Cargo fixture now assembles, publishes and reloads schema4
records with installed tool paths/content hashes, the sysroot-owned server and
component manifest, two Cargo-reported compiled build programs, per-unit cfg,
captured outputs and source-input witnesses. At the prepared-file checkpoint its artifact association used a strict fixture
adapter. The production decoder described below now replaces that adapter;
production orchestration remains open. It records OUT_DIR/rustc-env
and captured compiler cfg but does not yet claim complete Cargo package environment
capture. Full live include evaluation, build-script dependency metadata and the
complete MX005/MX006 transaction remain pending. Macro protocol compatibility is
proved by the current M003 gate, not by executing macros in this fixture.


M004 now decodes selected compiler-artifact records in production code with bounded
wire size, duplicate checks and the pinned target/profile shape. ArtifactProfiles
captures private profile and full target facts from the same validated unit graph.
Association uses deterministic indexes and checks exact package, target name/kind,
source, features, profile, target capabilities/edition and unit identity. It refuses
ambiguous contexts, mismatches, duplicate reports, shared filenames and live/native
runtime units before opening any artifact file. Reading captures files only through
explicit parent-directory grants; it never executes or loads them.

ToolArtifact now retains an auxiliary filename-to-content map alongside its primary
file. All Cargo-reported files are hashed, stable-encoded, schema-checked and included
in PreparedFiles adoption/publication witnesses. The macro primary must match the
recorded host's library suffix; host libraries retain the reported Rust library and
metadata. Earlier development schema4 records with nonempty artifacts lacking the
required auxiliary field must be prepared again; the decoder does not infer hashes
for unrecorded files. The field-contract fixture now covers ToolArtifact explicitly.

The actual build-program fixture uses this production decoder and checks profile,
target-edition, duplicate-report and live-unit refusals. The controlled macro probe
now includes a real build-host library used by build.rs. Both lib-test and integration
preparation contexts capture HostLibrary (including its reported metadata file),
BuildProgram and ProcMacroLibrary on initial and warm preparation. Freshness flags
remain Cargo evidence rather than executable-run claims; the existing controlled
counter and forwarded compiler logs prove warm behavior. Raw records and captured
artifacts are retained under each compile-time probe directory. At the artifact checkpoint, environment assembly still used fixture code. The
compile-environment checkpoint below replaces that code for package variables and
cfg facts; orchestration, additional target-specific variables, dependency metadata
and generated include/transaction integration remain pending M004 work.


M004 package environments now come from a pure bounded Cargo metadata-schema1
adapter. Duplicate JSON keys/package IDs, malformed required package fields and
invalid versions fail. Unused metadata stays opaque, including legitimate repeated
feature entries and user metadata numbers; these do not become unit feature facts.
The adapter emits per-package Cargo variables, empty defaults, version components,
author lists, manifest paths and per-target crate/binary/primary-package values.
These rules follow [Cargo's environment-variable contract](https://doc.rust-lang.org/cargo/reference/environment-variables.html).

CompileEnv assembly uses the exact normalized unit and private graph profile facts.
It rebuilds test/debug-assertion/feature facts for that unit and applies only its
captured build output. It checks output identity, dependency membership and reserved
OUT_DIR ownership, rather than unioning inherited feature/test flags. Captured cfg
atoms and key/value facts use the pinned Rust syntax parser's typed CfgMeta/CfgAtom
nodes and Rust string unescaping; no alternative Rust frontend or cfg expression
evaluator is introduced. Invalid captured facts leave the input cfg snapshot intact.
Lookup gives rustc-env precedence over Cargo-provided values and declared inputs;
explicit empty values are preserved and absent inputs never consult the host process.

The actual selected two-build-unit fixture now compares the production assembly
against the forwarding compiler's recorded package/crate environment. A rich package
covers prerelease/build versions, authors, description, home/repository, license-file,
README and minimum Rust version; empty fields in the other package remain empty.
The test seeds unrelated test/feature facts and verifies removal for other contexts,
and refuses applying another producer's output. Cargo metadata, compiler environment
records and the published schema4 manifest are retained in its evidence directory.
Earlier fixture-only cfg parsing was replaced with the pinned syntax adapter after
reading its generated AST API; initial failure logs remain recorded. This validates
package/unit values for this selected fixture, not all target-specific Cargo values
such as integration-test binary executable paths or temporary directories.

The compile-environment checkpoint still leaves production preparation orchestration,
build-script dependency metadata and generated include/full MX005/MX006 transaction
integration open. No live source was compiled to obtain these environment values.

M004 generated-source lookup now builds a deterministic consumer-to-producer index
from one validated immutable manifest. Membership fingerprints are computed once per
producer and contain original OUT_DIR, directories and member names, independently
of file bytes. Lookup accepts a normalized virtual member or routes a captured
original path through the exact unit's CompileEnv. Another unit's OUT_DIR fails;
there is no filesystem fallback. The lookup retains exact unit, producer, path,
virtual origin, explicit file absence and a stable output fingerprint.

The lookup borrows immutable generated bytes from the retained manifest. Rust/text
source conversion applies the existing size/UTF-8 checks only on demand, while byte
includes can read binary data. A changed or deleted current OUT_DIR cannot change
this generation. A newly prepared generation can add/remove names and update the
membership/lookup witnesses. Membership stays unchanged for content-only edits;
positive file fingerprints carry the changed content. Query-input publication and
macro builtin routing will consume these witnesses in later integration work; this
checkpoint does not claim those providers or full include-expression execution.

The real two-build-unit fixture now uses production original-path lookup for each
consumer's CompileEnv OUT_DIR and converts the correctly scoped generated bytes to
source. It refuses the other producer's original path and records origins, producer
IDs, source text, lookup fingerprints and membership under generated-includes.json.
Controlled tests prove old snapshots survive disk edits/deletion, explicit absence
survives an unprepared file addition, a refreshed generation changes the witnesses,
and invalid UTF-8 remains available as bytes but fails when demanded as source.


M004 transaction finalization now binds an input inventory captured before
explicit authorized tool work. Commit revalidates the candidate, source/external/
environment metadata, every generated OUT_DIR tree, and the planner/compiler/
matched-server/component-manifest/compiled-artifact file witnesses before the
immutable store publication. It invokes no build tool or macro. A controlled
barrier write during final OUT_DIR capture refuses publication without staging a
new record or replacing the old one; a stable retry publishes the changed bytes.
Changed source or generated membership also refuses publication before staging.
The real two-build-unit Cargo fixture now uses this finalizer.

Gate 01 passes with 124 host/doc tests and strict Clippy. The initial gate attempt
caught a fixture-registration name mismatch; that failed report is retained beside
the corrected gate report. Section 57 defines M004 as schema/capture/publication:
its component acceptance is now complete. Production include/env/option_env/concat
expansion and query-input routing belong to M011; Cargo CLI orchestration and
dependency-metadata collection belong to T090. Earlier checkpoint statements that
listed those later integrations as pending still describe unimplemented behavior,
but do not prevent advancing to M005. Full macro expression evaluation, Linux
execution acceptance, batch-B completion and release completion are not claimed.


M005 now has an owned, versioned token/context/span model with explicit stable
encodings and canonical content-keyed tables. Invisible and empty groups retain
separate open/close spans. Lifetimes, raw identifiers, literal spelling and all
three upstream punctuation spacing states survive structural transport. Context
records carry parent/expansion, transparency, defining unit and edition. Span
records independently carry immutable source identity/content/range, context,
edition and origin. Generated item identity uses expansion plus structural path
and header; ExpansionKey includes every Section 49.6 identity input and requires
an epoch for procedural expansion.

The pinned source conversion and storage are confined to the adapter. The runtime
shares only the pure owned schema source, serializes records across that boundary,
and has no frontend/Salsa dependency. An initial integration incorrectly imported
the whole adapter into lrr; the architecture audit refused it, and the dependency
was removed. The actual adapter/runtime fixture decodes a captured-source token
forest into runtime records, verifies the identical encoding/fingerprint and
attaches the Unicode identifier to the retained immutable snapshot after dropping
adapter-side records. Wrong content/unit/revision and invalid UTF8 ranges refuse
attachment. A separate pinned test changes temporary file slots and observes
identical owned identities. No upstream arena ID becomes a semantic cache key.

The raw converter's doc-comment panic and same-pin API correction are documented
in docs/errata/m005-doc-comment-converter.md. Original failures remain evidence.
Current Gate 01 and strict Clippy pass; M005 remains in_progress because the
nested JSON transport has provisional limits. Finish bounded allocation/transport
and align generated-token and output limits with Section 50.6 before task
completion. Structural context transport does not claim expansion hygiene name
resolution, procedural span operations or macro execution.


M005 component acceptance is now complete on macOS: Gate 01 passes with 137
host/doc tests and strict Clippy. Development token JSON schema1 is replaced by
`LRRT` binary schema2 and rejected on decode. Fixed tags, checked little-endian
lengths, content-sorted context/span table references and preorder child counts
encode the owned tree; no upstream arena/symbol/file ID is serialized. Tool
preparation schema4 and matched upstream server protocol6 are unaffected.

The writer checks its 32 MiB limit before reserve/append and bounds capacity
requests. Decode checks declared lengths against remaining bytes, table/token
limits before reserves, scalar/tag/reference validity, canonical table ordering,
UTF8 and complete consumption. Pending sibling tokens count before nested
reservation, preventing repeated large child allocations from exceeding the
2,000,000-token budget. Tests exercise the actual token and wire-byte boundaries,
then refuse token/byte+1. Deep groups round-trip beyond the former JSON recursion
limit. Context and transport nesting are checked against the 256-frame bound;
request-wide invocation/cumulative output/deadline accounting remains M012.

Span source-location edition and resolution-context edition are independent.
`resolved_at` changes only resolution context, agreeing with the pinned server's
`Span { ctx: at.ctx, ..span }`; `located_at` changes source/range/location edition
and keeps resolution context and the original generating origin. Cross-edition
records pass encode/decode. These are owned-record operations, not a claim that
all matched external-server span operations have been exercised; that remains
M009 integration. Source_text/start/end attachment uses retained snapshots.
Continue with M006; actual hygiene/name resolution remains M007, and batch-B,
release and Linux acceptance remain unclaimed.


M006 now wires the pinned mbe parser/matcher/transcriber through owned forests.
A private span-only Salsa database remains inside the adapter; runtime dependency
closure remains isolated. Definition demand retains the parsed rules and syntax
failure. Matching never resolves unknown function names or type-checks argument
expressions. Results distinguish definition syntax, no-rule, leftover, unexpected
input, binding, unresolved binding and backend limit failures. Output returns only
used owned spans/contexts and checks complete record size before adoption.

Current scope accepts root opaque contexts. Marked/non-opaque contexts fail
explicitly until actual private mark registration and scope integration in M007;
marks are never silently erased. Template marking/name resolution is not claimed
by this single-invocation component. Nested/recursive invocation orchestration and
reduced-frame-limit acceptance remain M006 work, as does further backend resource
coverage. M006 is in_progress and no batch-B/release completion is claimed.

The cardinality regression exposed actual upstream truncation: with a two-element
binding and a one-element binding the original backend emitted only the first
pair. Rust 1.99.0 rejects that input. E-M006-001 records the exact witness,
regression and minimal transcriber correction. Repetition axes now reject unequal
lengths, including zero/nonzero cases, while valid ragged inner rows retain their
independent lengths. The original archive/inventory stay unchanged; patch audit
reconstructs the current formatted source from the archive and checks the declared
pre/postimage. Tests reject changed preimage contexts, wrong targets and forged
patch/preimage/postimage receipts. The formatting mismatch caught during this
integration remains in the historical log; the exact formatted receipt passes.

A standalone native oracle builds an edition-2021 macro library and edition-2024
caller with Rust 1.99.0, executes it, and compares actual rule selection and literal
binding order against pinned expansion. Native cardinality rejection is also
observed. This compiler fixture is separate from selected live Cargo runtime
units; it does not establish guest VM execution or hygiene name resolution.

The integrated M006 foundation passes Gate 01 with 144 host/doc tests and
strict Clippy; evidence is in artifacts/macos-first/m006-foundation-checkpoint.json.

M006 output admission now uses the shared schema2 counting writer. The writer
executes the same field/tag/count/table traversal as encoding with no output
buffer, checks each addition against the requested reduced byte limit, and
returns an exact byte count. Randomized marked contexts/raw tokens and the real
32 MiB boundary compare measurement with encoding. Declarative request adoption
checks the smaller of the single-output limit and remaining cumulative bytes
before publishing totals; failed requests stay poisoned. Export checks avoid
allocating a wire buffer merely for measurement. This is candidate admission,
not proof of preallocation limits inside the pinned transcriber. M006 remains
in progress until that backend requirement passes.

M006 token admission now runs count-only mode of the same pinned transcription
algorithm before creating its output builder. The output facade preserves group
flattening, restore-point rollback and fragment emission. LRR uses the bounded
entry point by default, with a fixed ceiling of 2,000,000 tokens and explicit
positive reductions. An oversized compact repetition records zero output
materializations; exact counts match actual owned empty/ragged/group/expression
output. A resource failure returns immediately rather than trying another macro
arm. Token preflight does not execute the caller's hygiene marker a second time.
The combined transcriber and API/dispatch/restore-type patches retain exact
original preimages and independently checked receipts. Backend wire-byte and
speculative scratch limits remain pending; component completion is not claimed.

M006 exact final wire bytes now join token admission before the main output
builder is created. The private mapper supplies captured span/context costs
from shared schema2 field writers. The same pinned algorithm counts spelling,
groups, punctuation and distinct metadata, with a checked first-use undo log
for rollback. Returned owned bytes must equal the admitted count. The real
32 MiB boundary succeeds and byte+1 fails with zero materializations, as does
an 18 MiB literal duplicated into oversized output. Current scope is root
contexts; marked-context preview remains M007. Speculative discarded-iteration
scratch remains pending before M006 completion.

M006 final macOS component acceptance passes with 167 host/doc tests, Gate 01
and strict Clippy. Count-only transcription records accepted iteration recipes;
replay skips empty/after-end trial bodies and trailing separators, checks recipe
exhaustion/final counts and guards temporary raw slots before appending them.
Depth-80 replay matches the actual native oracle and materializes exactly the
final leaf count. Ordinary stable definition demand now rejects pinned nightly
metavariable syntax that native Rust 1.99.0 rejects, retaining the same attributed
error without invoking preflight or materialization. Positive literal/nested
variable/body-syntax cases remain syntax-only. All four patched files reconstruct
from original archived preimages and reject forged receipts. Continue M007;
actual marked hygiene/namespaces, M010 generated scheduling, M011 certified
builtins, M012 deadlines/cancellation and guest/release integration remain pending.


M007 now has a private owned hygiene store using the actual pinned mark and
normalization algorithm. The fifth audited patch supplies explicit call-site
context to the existing algorithm through a span-only Salsa interface; it
does not add HIR source/type queries. A source-proof regression checks the
complete archived algorithm suffix. Typed context/mark tables preserve owned
unit/edition/root identity and reject conflicting imports before publication;
worklist/input sizes are checked before reserves. Marked token transcription,
exact marked-span preflight and final context-aware namespace selection remain
pending. The E0659 native witness is not yet verified against LRR resolution.


M007 marked transcription now connects the actual hygiene store to private
span import/export and matcher/parser edition lookup. An explicit hygienic entry
receives source-index expansion/definition authority; token locations never
choose a generated definition owner. Preflight reads a closed marked-span map
and counts the complete ancestor closure with shared schema2 field writers.
Materialization uses the same spans, substitutions retain original argument
records, and speculative metadata rollback removes unused ancestors. Seven
component regressions pass, including the actual 32 MiB boundary and byte+1
refusal before materialization. Legacy unmarked matcher fixtures are component
checks only. Source-index wiring, final macro/local context resolution and full
MX011/MX012 remain pending. The two combined MBE patches now also carry
E-M007-002; all five archived preimages and historical evidence are preserved.

### M009 production adapter boundary

The service now calls the private macro adapter through owned, checked token records. The custom LRR query database remains unchanged and pure-provider imports of macro/service execution remain forbidden. The resolved dependency audit stops the runtime query dependency walk at the unique `lrr-expand-adapter` package and separately walks the adapter’s complete closure, retaining all inference/compiler-private prohibitions. Direct runtime Salsa dependencies and dependencies through any other helper still fail the audit. The former whole-package walk incorrectly counted the permitted private adapter interner as runtime query Salsa; the new regression exercises adapter-only, direct, and alternate-helper paths. This implements the existing D08/Section48.7 boundary and changes neither the query framework nor the source pin.
