# LRR: Rust 2015/2018/2021/2024 lazy interpreter/JIT — macros, generated code, traits, and Cargo-native reuse

**Document revision:** 5.0  
**Specification date:** October 13, 2026  
**Encoding amendment:** direct/native-resident default, binary internal bulk transport, measured interpreter invoker specialization and copy reduction; requirements only  
**Runtime release to build:** `lrr 0.2.0`  
**Guest semantics profile:** `lrr-2024-v2`  
**Execution modes:** `managed` (default) and explicit `hybrid-native` using the same interpreter/JIT  
**Status:** implementation contract; not an implemented or benchmarked runtime  
**Platform priority (October 4, 2026):** implement and test on macOS first; Linux is a later port. Docker is optional supplementary tooling and is never a prerequisite for macOS progress.  
**Supersedes:** revisions 1–4. Revision 5 adopts compiler-described direct Rust ABI bindings and optimizing execution with Cranelift and LLVM. Existing macro, trait, edition, capability and interpreter requirements remain mandatory. Earlier implementation evidence retains its original scope and is not revision-5 acceptance.

**Interpreter-first runtime-semantics priority (October 13, 2026):** Before guest JIT implementation, require complete interpreter execution of every applicable official rustc runtime contract, including currently unsupported runtime language features, plus applicable interpreter correctness gates. Preserve a complete upstream inventory; compile-time diagnostics, static type/borrow checking and compiler/tooling outputs have separate acceptance reports and do not block guest JIT. LRR does not implement a borrow checker or full static type checker; users run cargo check separately. Report runtime-contract passes, full-fixture passes, failures, unsupported and unexecuted cases distinctly; imported files and component checks are not full stock passes. Section 1.2.1 defines this user-authorized runtime-semantics prerequisite.

**Edition support amendment (October 13, 2026):** Support all released official Rust editions: 2015, 2018, 2021 and 2024 for selected roots, dependencies and macro origins. This user-authorized amendment supersedes the former 2024-only root restriction. Preserve owning editions throughout parsing, lookup, macro spans, patterns, temporary lifetimes, preparation identities and caches. The pinned parser, host toolchain, query framework and runtime architecture remain unchanged. The compiler's experimental future edition is not a released edition. Acceptance requires edition-paired upstream interpreter cases and effective-edition regressions; admission alone is not full language compatibility.

**Execution route:** Read Sections 1–5, 34–36, 47, 61 and 62 first. Follow the cross-ledger order in Section 57, including the new G2 optimization batch. T tasks build the common engine and optimizing tiers; H tasks build Cargo-native reuse; M tasks implement expansion/resolution and integration. Sections 61–62 are normative implementation contracts, not optional proposals. The independent frontend, custom query DB, Cranelift first tier, LLVM optimizing tier, native preparation adapter and execution contracts below are the selected architecture.

**Revision 5 contract:** Compile-time tools, expansion, execution-directed resolution, native preparation and execution are distinct phases. Live execution retains the independent frontend and demand-driven trait engine without a whole-body Rust checker or rustc invocation. Authorized preparation produces concrete native specializations, compiler-described Rust function-pointer ABIs and optional admitted LLVM IR. Cranelift compiles reached code first; optimized loops retain typed values and reconstruct interpreter state at exits; LLVM additionally optimizes eligible prepared native bodies together with hot live units. Compilation workers and code caches consume immutable verified inputs, never live query pointers. The default execution contract is `precise`; `throughput` is separately selected and reported.

**Meaning of laziness:** Unentered bodies are not executable-lowered or JIT-compiled. Declaration discovery may inspect their syntax and expand macros that can add items or implementations; associated-item selection may inspect relevant headers. This is necessary to avoid choosing an implementation from an incomplete universe. Every such request carries an auditable demand reason. Arbitrary cold expansion failures can therefore block a reached lookup whose correctness depends on them; ordinary cold body type errors still do not block it.

**Compatibility boundary:** The stable language forms below remain the supported live profile; this is not arbitrary-crate compatibility. Native generic specializations accept concrete prepared native types, not arbitrary live types with matching fields or derives. Direct ABI bindings and optional LLVM IR remove execution overhead only within their admitted contracts. Unsupported macro-generated executable forms still fail when reached. Native ownership, exact-once effects and type/world separation apply in every tier and execution contract.

## Contents

1. [Mandate and interpretation of this document](#section-1)
2. [Locked design decisions and accepted costs](#section-2)
3. [Build target, dependency lock, and bootstrap](#section-3)
4. [Repository layout and module ownership](#section-4)
5. [Closed release scope](#section-5)
6. [Identities, fingerprints, and serialization](#section-6)
7. [Source capture and immutable runs](#section-7)
8. [Syntax adapter, declaration completeness, and lookup](#section-8)
9. [Query database: exact implementation](#section-9)
10. [Lazy program counters and LRIR preparation](#section-10)
11. [Runtime types, expected context, and primitive operations](#section-11)
12. [Value layout, arenas, and guest storage](#section-12)
13. [Places, references, ownership, and destruction](#section-13)
14. [Function calls, generics, and method selection](#section-14)
15. [LRIR specification and syntax lowering](#section-15)
16. [Interpreter, dispatcher, and interruption](#section-16)
17. [Closed standard-library facade](#section-17)
18. [Built-in macros, derives, constants, and temporary annotations](#section-18)
19. [Native ABI and helper contracts](#section-19)
20. [Observation, region formation, and promotion](#section-20)
21. [Cranelift lowering and closed symbol resolution](#section-21)
22. [Artifact cache, edits, and code-memory lifetime](#section-22)
23. [Permission broker and security boundary](#section-23)
24. [Cargo-compatible project preparation](#section-24)
25. [CLI, test runner, and agent service](#section-25)
26. [Diagnostics and resource limits](#section-26)
27. [Ordered implementation task ledger](#section-27)
28. [Fixture format and mandatory concrete cases](#section-28)
29. [Property testing, model checking, and regression reduction](#section-29)
30. [Performance evaluation and acceptable claims](#section-30)
31. [Release runbook and completion evidence](#section-31)
32. [Instructions to the implementation agent](#section-32)
33. [Primary sources and dependency provenance](#section-33)
34. [All official editions: required semantics and migration gates](#section-34)
35. [Native integration: locked design and exact reuse boundary](#section-35)
36. [Binding manifest, native types, and native closure selection](#section-36)
37. [Cargo bridge preparation and existing artifact adoption](#section-37)
38. [Input fingerprints, immutable native generations, and freshness](#section-38)
39. [Native control interface and explicit codec transport](#section-39)
40. [Boundary execution machine, aliases, panic, and semantics](#section-40)
41. [Query/JIT integration, process lifecycle, and warm reuse](#section-41)
42. [CLI, service messages, permissions, and result reporting](#section-42)
43. [Required hybrid evidence, diagnostics, and performance gates](#section-43)
44. [Ordered edition/hybrid implementation tasks and gates](#section-44)
45. [Standalone Cargo reuse probe and first implementation experiment](#section-45)
46. [Additional primary sources for Rust 2024 and native reuse](#section-46)
47. [Revision 5 contract and demand boundaries](#section-47)
48. [Exact Cargo units, tool preparation, and macro ABI compatibility](#section-48)
49. [Hygienic token and macro expansion implementation](#section-49)
50. [Discovery fixed point, nested impls, and generated tests](#section-50)
51. [Effectful expansion, epochs, immutable records, and trust](#section-51)
52. [Source-aware type terms, call-local elaboration, and member identity](#section-52)
53. [Tabled demand-driven trait and projection engine](#section-53)
54. [Trait-aware runtime operations and managed object views](#section-54)
55. [Live, native, macro-export, and compile-time worlds](#section-55)
56. [Interfaces, query keys, diagnostics, and service contract](#section-56)
57. [Ordered implementation tasks and cross-ledger integration](#section-57)
58. [Mandatory macro, trait, native, and cache fixtures](#section-58)
59. [Release, migration, and implementation audit](#section-59)
60. [Primary sources for the expansion and trait revision](#section-60)
61. [Compiler-described direct Rust ABI bindings](#section-61)
62. [Optimizing execution and throughput contracts](#section-62)

<a id="section-1"></a>

## 1. Mandate and interpretation of this document

Build one execution system for the Rust 2015, 2018, 2021 and 2024 editions under the execution profile specified here. Interpret first, prepare executable operations when reached, retain dependency-tracked source work between edits, and promote observed code through Cranelift and the LLVM optimizing tier described in Sections 20–22 and 62. Live execution has no borrow checker, whole-body inference pass, global trait validation, or coherence scan. It does perform the declaration expansion and local trait-goal resolution required to identify a reached operation correctly. Do not confuse these bounded semantic demands with validating all source. Explicit tool preparation builds macro libraries/build scripts; explicit native preparation checks and links the native closure and bridge. Neither operation compiles the selected live runtime unit. Telemetry separates all four phases.

The implementation language is Rust. Host builds use rustc and Cargo. Explicit project preparation uses Cargo metadata, a pinned Cargo compile-time-dependency operation, and, for hybrid execution, the native bridge build. No live run/test launches Cargo, rustc, a linker, or a build script. A previously compiled procedural-macro library may execute in its authorized expansion helper during a run; this is expansion, not compilation, and is separately reported. Arbitrary authorized native programs can themselves launch processes, so the no-compiler claim describes LRR's own orchestration and is not hostile-code containment.

The language remains eager: function arguments, arithmetic operands, and effects execute in the specified order. “Lazy” describes preparation, resolution, and code generation, not call-by-need evaluation. Never postpone an effect or arithmetic error to obtain type information from a later statement.

This is an execution profile, not an alternative implementation of every Rust language rule. Passing a test means that this test passed under `lrr-2024-v2`; it does not mean rustc would accept the source. In particular, programs with invalid Rust aliasing can have defined, checked behavior here. Never optimize using static Rust guarantees that this runtime has not established.

### 1.1 How an implementation agent must use the specification

Read Sections 1–5, 34–36, 47, 61 and 62 before creating files. Use Section 57’s exact cross-ledger dependency order, with H001–H003 applied to core tasks and H004 run after bootstrap. Treat each task's acceptance tests as requirements, not illustrations. Definitions in the main specification govern implementation details; fixtures govern observable behavior. A later correction to a discovered contradiction must be recorded in `docs/spec-errata.md`, accompanied by a regression test, and must preserve the mandate above.

Do not substitute a different frontend, query framework or ownership model. Implement the two native tiers selected here: Cranelift for initial/reached code and optimized loops, and LLVM for profitable hot units and admitted prepared-IR inlining. Native rustc-driver integration is confined to explicit preparation; it is not a live-source frontend. Asynchronous optimization, bounded specialization, reconstruction maps and relocatable caches are required by Section 62. Wasm, a generic native plugin system, automatic compilation of live Cargo units, speculative execution and unchecked numeric semantics remain excluded. The macro server, native preparation adapter, C control/codec surfaces, Rust ABI entries and guest JIT helper ABI are distinct versioned interfaces.

An implementation obstacle is handled by reducing the failing case to a test and fixing the assigned component. It is not permission to silently change guest semantics. API spelling errors in this document may be repaired against the pinned dependency source without changing architecture. A missing external package or incompatible toolchain is a bootstrap failure with recorded evidence; do not silently select `latest`.

### 1.2 What “complete” means here

The interpreter-first prerequisite is complete applicable upstream runtime-semantics acceptance under Section 1.2.1, independently of historical component milestones. Current runtime-feature exclusions are capability boundaries and required compatibility backlog entries; they cannot narrow that prerequisite. Static diagnostics and compiler/tooling contracts remain separately inventoried and reported outside that gate.

Completion means all specified supported features, task gates, and release checks work. It does not mean accepting arbitrary crates.io packages. Unsupported features have a prescribed error and prescribed point of failure. They must not become silent no-ops, implicit native fallbacks, or indefinitely postponed research tasks.

The document includes Rust-shaped interface definitions and algorithms. Except for explicitly labeled bootstrap snippets and guest fixtures, these are normative interface specifications, not a precompiled codebase. No claim is made that dependency combinations, generated native code, latency targets, or security properties have already passed the specified tests. The implementation must produce that evidence.

#### 1.2.1 Complete upstream inventory and runtime-semantics prerequisite

This user-authorized policy supersedes the former requirement to satisfy every rustc category before guest JIT. LRR remains an execution-directed interpreter with reached type/shape/trait checks. It does not implement whole-program static type checking, borrow checking or cold-body diagnostic validation. Users may run `cargo check` as a separate explicit external workflow; LRR never invokes it implicitly during live execution. External rustc/Cargo results identify their producer and authority and cannot count as interpreter execution. This policy does not introduce a rustc-backed LRR validation mode.

Produce a deterministic inventory from the official rustc/rust checkout's complete `tests/` tree at its recorded commit. Record every file/hash, original path, auxiliary/expected-output relationships, driver/category, directives, revisions, editions and target/tool requirements. A case identity is `(upstream commit, test path, revision, effective configuration)`; support files are inventory members, not invented test passes. Configurations come from declared directives/revisions and a frozen upstream harness run matrix, not an arbitrary product of flags. Continue unchanged incremental fixture imports with licenses and provenance. Record inventory/harness/schema hashes and explicit denominator changes when updating the pin.

Classify asserted contracts within each case as `runtime`, `diagnostic`, or `compiler_tooling`, using directives and expected observations rather than directory names alone. Runtime includes run-pass, run-fail expecting a runtime panic/failure, and execution assertions in mixed/run-make cases. Diagnostic includes compile-time acceptance/rejection, exact errors/warnings, static type and borrow checking, including unentered bodies. Compiler/tooling includes MIR/IR/assembly dumps, compiler optimization assertions, rustdoc and other tool outputs. Check/build-only cases carry their actual acceptance/tool contract; executing an empty main cannot manufacture a runtime requirement or a diagnostic pass. Mixed cases preserve all assertions and dependencies: a runtime-contract pass may coexist with outstanding diagnostic/tooling assertions, but cannot be labeled a full-fixture pass.

Host/target applicability derives from unchanged upstream directives and the pinned run matrix. The macOS prerequisite includes every applicable runtime contract on the recorded host/toolchain. Linux-only configurations remain platform-deferred under the later-port policy. Unsupported LRR features, missing required tools, unsupported runtime harness drivers, ignored applicable cases, and missing auxiliary support do not make runtime cases inapplicable. Runtime cases requiring currently unsupported closures, async, traits, library operations or edition semantics remain required blockers. Nightly/compiler-internal cases stay in the inventory with their upstream requirements: the inventory grants no unstable-feature or compiler-invocation authority. Applicable runtime cases whose requirements conflict with the pinned architecture/toolchain retain an explicit unresolved runtime-contract blocker until resolved by an authorized specification decision.

Every asserted contract has one primary outcome: `passed`, `failed`, `unsupported`, `unexecuted`, `contract_blocked`, or `platform_deferred`. Partial/imported cases remain unexecuted with progress fields. A runtime pass requires complete execution of its asserted behavior through LRR, including necessary auxiliaries, macros, preparation, editions/configurations, output/effects and expected runtime failures. Replacing required behavior with an unsupported diagnostic, skipping it, or running it only through rustc cannot pass. A full-fixture pass requires all original assertions and supporting behavior; separate external compiler validation must never be presented as an LRR full-fixture pass. Static/tooling requirements outside LRR's product scope retain explicit `outside_lrr_scope` classification alongside their outcome and optional external evidence; scope is not a passing outcome.

For example, `fn f() -> isize { true } fn main() {}` expecting E0308 has a diagnostic contract. LRR may successfully run main without checking f, as reached execution requires. This is not a pass of that upstream diagnostic test and does not block the runtime prerequisite. Borrow-checking, MIR/LLVM-output and rustdoc-only cases follow their separate contracts. A runtime failure in a mixed test remains a blocker even when that same fixture also requires an out-of-scope compiler artifact.

Publish `upstream_runtime_acceptance`, `diagnostic_compatibility`, `compiler_tooling_compatibility`, and `full_suite_compatibility` separately, plus applicable interpreter correctness gates. Reconcile case/configuration totals with the complete inventory and reconcile each category's contract counts separately; mixed fixtures must not double-count cases as full passes. Unknown or incomplete classification that could hide a runtime requirement blocks the runtime gate; missing execution/enumeration for a known compiler-only category does not. Runtime unsupported/unexecuted/failed/contract-blocked cases carry required backlog entries with task association, witness and next action. Diagnostic/tooling exclusions need accurate reporting, not mandatory implementation of a static checker.

Guest JIT implementation may begin only after every applicable upstream runtime contract passes and the applicable interpreter ownership, cleanup, fuel/resource, interruption, capability and exact-once gates pass. No in-profile subset or historical component receipt satisfies that requirement. Outstanding diagnostic/compiler-tooling contracts do not prevent guest JIT work; their reports and full_suite_compatibility remain incomplete when not satisfied. Platform-deferred cases prevent cross-platform claims but do not block authorized macOS progression. This prerequisite overlays Section 57 without changing cross-ledger task order. CA037/T034/M047 enforce complete accounting, correct classification and the runtime gate; passing their harness controls alone is not passing the upstream runtime suite.

### 1.3 One architecture

```text
lrr CLI / stdio service / cargo-lrr shim
                  |
       coordinator and effect broker
   source capture, Cargo metadata, permissions
                  |
       immutable snapshot + run request
                  |
       persistent managed worker
    +--------------------------------------+
    | syntax adapter and declaration index |
    | custom dependency-tracked query DB   |
    | lazy LRIR thunks                     |
    | register interpreter                |
    | Cranelift + LLVM optimizing tiers   |
    | managed guest heap and cleanup       |
    +--------------------------------------+
                  |
       brokered, bounded effect messages
                  |
      explicitly granted host resources
```

The diagram above shows managed mode; hybrid mode uses the same core in a fresh worker per run request plus the immutable prepared native generation. One interpreter, logical value/ownership model, dispatcher, cleanup mechanism and managed effect protocol serve both native tiers. Typed SSA and native resident storage are proven execution representations of that machine, with complete reconstruction at required exits; they do not define a different language. A compiler worker receives frozen compilation inputs and cannot query or mutate the execution worker.

<a id="section-2"></a>

## 2. Locked design decisions and accepted costs

This section records the trade-off evaluation as decisions. It is not a menu.

| ID | Determined choice | Why this choice wins for this project | Cost deliberately accepted |
|---|---|---|---|
| D01 | Independent execution frontend over `ra_ap_syntax` | Controls exactly when executable meaning is requested | We implement resolution and runtime semantics ourselves |
| D02 | No rustc HIR, THIR, or MIR on the guest execution path | Typed compiler IR is downstream of analysis we are intentionally omitting | No automatic compatibility with the native Rust standard library |
| D03 | Selected roots support Rust 2015/2018/2021/2024; dependency and macro-origin editions are preserved | Preserves the owning edition of roots, imported source and macros | Edition-specific lookup/pattern/temporary rules and oracle fixtures are required |
| D04 | macOS first: aarch64-apple-darwin on the current machine; x86_64-apple-darwin when separately exercised. Linux x86-64 is a later port | Each execution uses one explicit matching host/guest/native target and ABI | No Windows or other targets; no untested platform support claims |
| D05 | Parse loaded source files; semantically prepare only reached executable operations | Reuses a maintained parser without confusing parsing with checking | Cold startup reads and parses more bytes than it executes |
| D06 | Index declaration headers outside the execution trace when lookup needs them | Correct method/name selection requires a complete candidate set | “Untouched” means no body semantics, not no inspection of declarations |
| D07 | Custom single-threaded dependency-tracked memo database | The required query set is small and its invalidation rules can be explicit | We implement dependency validation, cycles, and cache accounting |
| D08 | LRR uses its custom query database; upstream macro adapters may have a private Salsa interner | Reuses macro infrastructure without introducing whole-body analyzer queries | Audit that upstream inference/HIR execution is unreachable from the adapter |
| D09 | Immutable snapshots; one active run per worker | Prevents mixed-revision execution and simplifies memory/code lifetime | No live patching of active frames and no parallel tests in one worker |
| D10 | Fresh managed state per test; fresh hybrid worker per run request | Resets native globals while reusing immutable source and relocatable code artifacts | No guest state or absolute pointers survive worker restart |
| D11 | Tagged canonical values, checked handles, typed SSA and validated native resident storage | Unproven paths stay checked; proven hot paths avoid tagging and conversion | Reconstruction maps and native validity/ownership contracts are required |
| D12 | Explicit moves, initialization state, and logical drops | Ownership has observable execution consequences even without a borrow checker | A real cleanup subsystem is required |
| D13 | Native borrow guards with proof-based lease reuse | Preserves native reference validity without a global borrow checker | A lease can span an optimized interval only while its ownership/alias facts remain valid |
| D14 | Concrete call contracts plus operation guards | Supplies enough information to execute safely without analyzing cold expressions | Some otherwise valid Rust needs annotations |
| D15 | Rust-compatible source constraints and numeric fallback | Preserve Rust inference while executing concrete primitive arithmetic | Resolve relevant source constraints before defaulting unconstrained integer/float variables |
| D16 | Preserve symbolic source types; use a tabled goal engine and lazy implementation dictionaries | Correct generic method identity, conditional impls, and associated items without global checking | Some reached operations require header predicates or explicit type annotations |
| D17 | Closed runtime facade for selected `core`, `alloc`, and `std` APIs | Produces useful tests without interpreting unsafe standard-library internals | Every supported API must be implemented and tested explicitly |
| D18 | General hygienic macro_rules plus all three stable procedural-macro forms; generated code enters the common frontend | Existing generated declarations and tests are visible to the interpreter | Declaration-sensitive expansion can require cold syntax; proc macros need native tool authority |
| D19 | No guest closures in this release | Capture inference and closure ownership are independent language projects | Tests must use named functions and explicit data |
| D20 | One lazy LRIR machine from the beginning | Prevents divergence between a temporary AST interpreter and production semantics | The first scalar interpreter must already use explicit PCs and continuations |
| D21 | Compile reached regions and hot loop graphs with native backedges | Avoids dispatcher round trips while leaving unentered live paths unprepared | Every side exit needs an exact state map; compilation work is bounded |
| D22 | Typed optimized IR, Cranelift speed optimization and LLVM hot-unit optimization | Removes helper traffic and enables prepared native IR inlining | Two backend lowerings, exact producer compatibility and expanded acceptance |
| D23 | Dispatcher-owned guest recursion with accepted reached-call inlining; direct native calls | Preserves logical frames and lazy semantics while reducing hot calls | Inlined frames require source/ownership reconstruction; uninlined recursion remains on explicit frames |
| D24 | Synchronous bounded first-tier installation and asynchronous optimizing jobs | Balances first-use latency and steady execution | Immutable compile inputs, stale-result rejection and bounded compiler resources |
| D25 | Up to four concrete optimized variants per reached unit plus baseline fallback | Avoids repeated generic shape misses | Per-unit/global code budgets and exact specialization/invalidation identities |
| D26 | Content-validated relocatable code cache separate from portable query checkpoints | Reuses compilation across fresh workers | Closed relocation admission, ABI/CPU/profile matching and no persisted addresses |
| D27 | Retire code only at safe run boundaries after all active references drain | Prevents freed native entries during tier replacement | Retired versions occupy budget until safe reclamation |
| D28 | Capability broker from the first output operation | Keeps host effects behind a replaceable boundary | Some operations require an IPC round trip |
| D29 | Capability-mediated execution plus crash-isolated worker, not a hardened hostile-code sandbox | Delivers the execution system without pretending a JIT is an isolation mechanism | Do not run adversarial code or claim containment against JIT/runtime bugs |
| D30 | Read/write directory grants; injected environment; no networking or subprocess support | A finite, testable permission system covers initial test workloads | Network/process APIs remain unsupported even when users request a grant |
| D31 | Explicit Cargo metadata/unit-graph and compile-time-only preparation; native bridge remains a workspace cdylib | Cargo owns features, compile-time dependencies, and artifact freshness | A pinned nightly Cargo planner is required for tool preparation; the project rustc stays explicit |
| D32 | Separate build-tool, macro-execution, managed-effect, and native-runtime authorization | Build outputs and compiled macro libraries can serve live code without opening the guest native ABI | Proc-macro host execution is unsandboxed and expansion output is volatile between runs by default |
| D33 | Deterministic serial test scheduling and structured results | Makes replay, debugging, and agent integration straightforward | Lower total throughput for independent large test suites |
| D34 | Native Rust is an oracle only for well-typed supported fixtures | Separates intended relaxed behavior from implementation bugs | Two categories of semantic tests must be maintained |
| D35 | Correctness, laziness, and measured JIT use are release gates | Prevents a fast but incorrect interpreter-only demonstration | Throughput targets cannot be met by silently skipping work or tests |
| D36 | Prepared concrete Rust ABI entries with C control and explicitly selected codecs | Near ordinary native-call cost without guessing Rust layouts | Exact producer ABI export, native storage and unsupported-signature diagnostics |
| D37 | Explicit precise and throughput execution contracts | Makes metering/cancellation tradeoffs visible | Results and caches include the contract; safety/effects never become optional |
| D38 | Optional per-binding prepared LLVM IR, required optimizing infrastructure | Enables cross-boundary optimization where bodies are available | No assumed bitcode in arbitrary Cargo artifacts; native state identity remains singular |

The external premise for D02 is specific: rustc describes THIR as typed and MIR construction as operating on THIR. The decision is architectural, not a claim that borrow-checking flags are unavailable.[^R01] Rust-analyzer documents a syntax layer distinct from semantic analysis; LRR reuses that syntax boundary, not its inference engine.[^R02] Query memoization and input/derived-fact separation are established compiler techniques; the small implementation below is our design, not a claim to reproduce all of rustc or Salsa.[^R03]

### 2.1 Non-negotiable invariants

Assign these identifiers to tests and review comments:

| Invariant | Requirement |
|---|---|
| I-LAZY | No executable recipe/JIT body for an unentered live path; prepared native bodies are compiled only under explicit native preparation authority. Permit syntax/expansion/header work only under recorded declaration, representation, selected-call, or selected-projection demands; never use these exceptions to run whole-body inference |
| I-ORDER | Every guest-visible operation happens once and in the prescribed order |
| I-SNAPSHOT | Source/build/configuration inputs are frozen. Expansion records are append-only within one epoch; a sealed scope cannot gain declarations later. Every frame/query/entry carries that epoch and validated semantic dependencies |
| I-MEMORY | Guest bytes, tags, references, and callable identities never become unchecked host pointers |
| I-OWNER | Every initialized non-Copy owned value has exactly one logical owner or one explicitly tracked transfer ticket |
| I-DROP | Normal completion and guest panic perform specified cleanup once; fatal runtime faults do not execute more guest code |
| I-EFFECT | Every managed effect is broker-authorized for that run; hybrid native effects require explicit whole-island authority and invalidate any whole-run sandbox claim |
| I-CACHE | Reuse is conditional on all positive and negative semantic dependencies |
| I-JIT | A pre-operation guard miss reconstructs that operation; a completed native call resumes after it. No tier exit replays committed effects, ownership transfers or drops |
| I-REPORT | Report actual backend entries, validation status, limitations, and effective security strength |
| I-HYGIENE | Preserve token identity, expansion context, defining edition, crate origin, and diagnostic ancestry; never execute pretty-printed expansion text |
| I-METHOD | Resolve the source-level associated-item identity before choosing its concrete implementation; runtime type specialization cannot change that identity |
| I-COMPLETE | Absence, uniqueness, ambiguity, and cached success require appropriate sealed declaration/impl indexes, including generated and nested items |
| I-TOOLS | Only explicit preparation may compile tools/run build scripts; previously compiled macro execution is separately authorized and never a query-provider side effect |
| I-WORLDS | Live, compile-time-host, and native-runtime unit/type identities never unify solely by names or source paths |

<a id="section-3"></a>

## 3. Build target, dependency lock, and bootstrap

### 3.1 macOS-first host and guest environment

Build, implement, and run acceptance tests on macOS first, using `aarch64-apple-darwin` on the current development machine. `x86_64-apple-darwin` is an additional macOS target only when exercised separately. Use Rust **1.99.0** for the host runtime and native oracle. The release announcement identifies this version; the syntax package manifest examined for this plan requires Rust 1.98, so the selected host toolchain is above that declared minimum.[^R04][^R05]

Guest pointer width is 64 bits. Guest integer overflow checks are always enabled. Guest floating point uses ordinary IEEE operations without fast-math. Guest debug_assertions follows the captured Cargo unit configuration; standalone roots default to true. Guest target cfg must describe the selected execution target: on the current macOS machine, `target_os="macos"`, `target_arch="aarch64"`, `target_endian="little"`, `target_pointer_width="64"`, and `unix` are true. Never label macOS execution with Linux cfg or reuse artifacts across platform/architecture identities. Guest target-feature cfg comes from the recorded project compiler/configuration when preparing Cargo units; standalone uses the explicit baseline target feature set. Intrinsics outside the runtime feature matrix remain unsupported even when cfg enables their source; native code generation may use detected host instructions, and relocatable artifacts may be reused only under exact validated CPU-feature and target identities.

Platform sequencing is normative: complete the T/H/M implementation and its named gates on macOS in Section 57 order. Port and validate `x86_64-unknown-linux-gnu` later as a separate platform phase; unavailable Docker or Linux-only evidence must not block macOS task completion or progression. Docker may be used for supplementary experiments, but is not the default test environment. macOS acceptance is mandatory now; Linux acceptance remains mandatory before claiming Linux support. Preserve historical Linux failures and label them deferred, not passed. Report platform coverage separately, and do not claim a full multi-platform release before both phases pass. The architecture, capability rules, pins, and feature requirements remain unchanged. Dynamic-library examples use `.dylib` for macOS; the later Linux equivalents use `.so`. Always discover exact artifact paths from Cargo messages rather than synthesizing a filename.

Host source and generated bridge source use edition2024. Selected live roots support Rust 2015, 2018, 2021 and 2024 and retain their declared edition. Dependency units and macro-origin tokens retain their declared editions, with supported-operation compatibility under Section 34.7. The host runtime stays pinned below; the native build uses the explicitly recorded project toolchain under Section 37.3. Codec-only generations may use an independently supported producer; any direct binding requires the exact accepted Rust 1.99.0 producer/exporter and matched ABI lowering. The C control interface does not relax the direct Rust ABI match.

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.99.0"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

### 3.2 Exact direct dependencies

Retain the exact direct versions below and commit the resolved transitive `Cargo.lock`. All later builds use `--locked`. These are inherited bootstrap pins, not a claim that this revision rebuilt them. M001 obtains the exact rust-analyzer source revision embedded in the pinned syntax package and vendors the macro component dependency closure from that same revision; no unpinned master dependency is permitted. Keep the published host syntax package and the coherent vendored adapter syntax stack separate behind owned wire records; do not force incompatible upstream parser/interner types to become one Rust type. The combined build and macro-server protocol probes are mandatory bootstrap gates.[^R05][^R06][^R07]

```toml
[workspace]
members = ["crates/lrr", "crates/lrr-expand-adapter", "xtask"]
resolver = "3"

[workspace.package]
version = "0.2.0"
edition = "2024"
rust-version = "1.99"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
ra_ap_syntax = "=0.0.356"
cranelift-codegen = "=0.136.2"
cranelift-frontend = "=0.136.2"
cranelift-module = "=0.136.2"
cranelift-jit = "=0.136.2"
serde = { version = "=1.0.229", features = ["derive", "rc"] }
serde_json = "=1.0.151"
blake3 = "=1.8.7"
clap = { version = "=4.6.7", features = ["derive"] }
toml = "=1.1.6"
cap-std = "=4.0.3"
proptest = "=1.11.0"
libloading = "=0.8.9"
toml_edit = "=0.23.7"
unicode-normalization = "=0.1.25"
rustix = { version = "=1.1.5", features = ["fs", "process"] }

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"

[profile.dev]
panic = "abort"
```

The TOML package's published version includes build metadata; the exact `=1.1.6` requirement selects that base version, and `Cargo.lock` records the complete resolution. Do not add Tokio, Wasmtime, `hir_ty`, a GC library, a general-purpose plugin framework, or a filesystem-watcher dependency. LLVM 23.1.1 and an exact-Rust-1.99.0 rustc-driver adapter are required only for the interfaces in Sections 61–62. The adapter links compiler development libraries matching the actual native producer and runs only in explicit preparation. Pin the LLVM build/source and adapter binary hashes in dependency provenance before integrating T087; a mismatched installed source checkout is not a valid pin. Use an audited LLVM C API/ORC binding in the allowed JIT unsafe modules; do not silently choose an unrelated LLVM build or latest Rust wrapper crate. LRR itself does not use Salsa; an upstream macro adapter may retain its required private interner dependency, with no body inference entry point exposed. `libloading` is confined to the explicit hybrid loader (macro libraries load only in the external macro server), and `toml_edit` to transactional bridge-workspace setup; their versioned APIs were checked separately.[^H14][^H15]

Use the standard library for process management, channels, deterministic `BTreeMap` indexes, timers, and file polling. M009 supervision exception E-M009-001: retain standard-library child ownership, channels and timers, and use the already-locked Rustix 1.1.5 safe APIs solely for nonblocking owned helper pipes, owned process-group signals, and controlled liveness probes. The standard-library-only rule cannot provide bounded cleanup when a descendant retains a helper pipe; the minimal witness and regressions are recorded in [the supervision erratum](docs/errata/m009-owned-helper-supervision.md). This adds no local production unsafe and changes no frontend, query framework, JIT/native backend, language profile, capability policy, or sandbox backend. Native macros still have no hardened containment against deliberate process-group escape. Use `proptest` only in host tests. Host macros/derives remain ordinary host compilation. Live macro expansion uses the separately specified upstream adapter and pinned external server, not host Rust macro expansion of guest text.

### 3.3 First commands

These commands build the future repository; they do not refer to a preinstalled `lrr` executable:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
cargo +1.99.0 generate-lockfile
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- bootstrap-audit
cargo +1.99.0 build --release --locked -p lrr
```

`bootstrap-audit` must print the toolchain, direct dependency versions, pointer width, OS, parser smoke result, and native JIT smoke result. It fails on a target outside the current supported platform profile; macOS ARM64 is supported for the primary development phase. It also writes `artifacts/bootstrap.json`. The actual runtime must reject unsupported host targets at startup with `LRR-E-HOST-TARGET`; do not accidentally ship interpreter-only behavior as a completed native release.

### 3.4 Parser smoke test

Use the pinned syntax API through one local adapter. Its documented parse operation accepts text and an edition.[^R05]

```rust
use ra_ap_syntax::{ast, AstNode, Edition};

pub fn parser_smoke() -> Result<(), String> {
    let text = "fn live() -> i64 { 4_i64 } fn cold() { absent(); }";
    let parsed = ast::SourceFile::parse(text, Edition::Edition2024);
    let root = parsed.tree();
    let count = root.syntax().descendants()
        .filter_map(ast::Fn::cast)
        .count();
    if count != 2 { return Err(format!("expected 2 functions, got {count}")); }
    Ok(())
}
```

This smoke test must not resolve `absent`. The production adapter stores parse errors by span; the smoke test merely confirms the reuse boundary and API.

### 3.5 Native smoke test contract

Create one Cranelift function with signature `extern "C" fn(u64) -> u64`, returning its input plus seven, and invoke it with 35. Then create a second function that calls a registered host helper which performs the same operation. Verify both return 42. Keep the `JITModule` alive until both calls complete. Free its memory only after clearing every saved function pointer. Cranelift's documented finalization and code-memory lifetimes are the constraints being tested.[^R06]

This smoke test is infrastructure only. It may compile unobserved synthetic CLIF because it is not guest execution. The guest JIT must obey I-LAZY.

<a id="section-4"></a>

## 4. Repository layout and module ownership

Use one runtime library crate with three binary entry points and one isolated macro-adapter library crate. The adapter is the sole additional frontend crate; do not begin with fifteen mutually dependent runtime crates. Rust module visibility enforces boundaries; `xtask architecture-audit` enforces forbidden imports. This removes package wiring work without weakening ownership boundaries.

```text
lrr/
  Cargo.toml
  Cargo.lock
  vendor/cranelift-jit/          # Exact upstream release plus the specified lookup patch.
  vendor/UPSTREAM.toml
  rust-toolchain.toml
  AGENTS.md
  crates/lrr/
    Cargo.toml
    src/lib.rs
    src/bin/lrr.rs
    src/bin/cargo-lrr.rs
    src/bin/lrr-worker.rs
    src/core/{mod,ids,digest,diagnostic,profile,limits}.rs
    src/syntax/{mod,parse,outline,body_index,ast_adapter,spans}.rs
    src/source/{mod,snapshot,loader,changes,manifest}.rs
    src/project/{mod,cargo,units,cfg,prepare,native_workspace,native_build,native_inputs}.rs
    src/db/{mod,key,value,memo,provider,dependency,trace}.rs
    src/frontend/{mod,names,types,signatures,dispatch,thunks,constants,source_view}.rs
    src/expand/{mod,tokens,hygiene,attributes,driver,completeness,generated,records}.rs
    src/traits/{mod,terms,env,canonical,unify,table,candidates,normalize,dictionary}.rs
    src/project/{tool_prepare,tool_units,build_output,toolchain,tool_inputs}.rs
    src/service/{macro_host,expansion_requests}.rs
    src/runtime/{dyn_object,boxed}.rs
    src/vm/{trait_ops,format_frame,iterator_frame}.rs
    src/ir/{mod,pc,op,recipe,validate}.rs
    src/runtime/{mod,value,arena,cell,object,place,reference,shape}.rs
    src/vm/{mod,frame,step,call,control,pattern,cleanup,numeric}.rs
    src/facade/{mod,registry,collections,text,option_result,macros,derive}.rs
    src/effects/{mod,protocol,policy,broker,files,output}.rs
    src/jit/{mod,abi,record,plan,lower,helpers,module_cache,verify}.rs
    src/engine/{mod,worker,dispatch,install,run,telemetry}.rs
    src/native/{mod,manifest,binding,codec,staging,call,handles,ffi,loader,generation}.rs
    src/cache/{mod,portable,validate,checkpoint}.rs
    templates/native_bridge/{lib.rs.in,abi.rs.in,store.rs.in,codec.rs.in}
    compat/native-abi-v1.h
    src/service/{mod,coordinator,wire,requests,watch}.rs
    src/cli/{mod,args,render,exit_code}.rs
    tests/{bootstrap,fixtures,properties,protocol,security,invalidation,jit}.rs
  xtask/{Cargo.toml,src/main.rs,src/tasks.rs}
  compat/{features.toml,facade.toml,diagnostics.toml}
  fixtures/{native,lazy,runtime,cargo,edits,permissions,edition2024,hybrid}/
  docs/{semantics,abi,query-keys,security,dependency-lock,spec-errata}.md
  crates/lrr-expand-adapter/{Cargo.toml,src/lib.rs}
  tools/{bootstrap_vendor.py,probe_compile_time_deps.py}
  vendor/rust-analyzer/          # Source commit recovered from the pinned package, not master.
  compat/{macro-tools.lock.json,traits.toml,frontend-features.toml}
  fixtures/frontend/{macro_rules,proc_macro,generated,traits,hybrid,cache}/
  artifacts/.gitkeep
```

Files are created when their numbered task begins. Do not fill the tree with `todo!()` stubs. Every public function returns the explicitly defined result/error type; do not use panics to represent unsupported guest behavior.

### 4.1 Import rules

`core` imports only standard library, Serde, and hashing support. `syntax` and the isolated `lrr-expand-adapter` are the only modules importing upstream syntax/token types. All syntax nodes stay on the worker's owner thread; expose owned IDs, ranges, and summaries elsewhere. The upstream AST type is not `Send`/`Sync` in the inspected API, and LRR has no reason to force it across threads.[^R05]

`db` and `frontend` do not import `effects::broker`, process creation, networking, filesystem reads, or the native JIT. `runtime` has no syntax or compiler dependency. `vm` consumes a small `Resolver` interface and LRIR; it does not create Cranelift objects. `jit` may import Cranelift, the pinned LLVM interface and runtime ABI types; code generation consumes immutable reached plans and verified prepared IR, not the live source database. `engine` composes these modules. `service::coordinator` owns host filesystem authority and subprocess invocation. The worker's only intended host communication is its framed protocol.

Only the existing JIT ABI/memory modules and the explicitly enumerated native-FFI modules in Section 35 may contain local runtime `unsafe`. Upstream macro loading remains outside the runtime process; preserve the audited upstream helper rather than embedding its loader. Macro transport parsing, trait solving, and guest values contain no new unsafe blocks. The architecture audit rejects other local unsafe code. The runtime does not use unsafe Rust to store guest aggregates or references.

### 4.2 Stable internal interfaces

Expose these top-level operations:

```rust
// Result types and IDs are LRR-owned, not upstream parser types.
trait Resolver {
    fn thunk(&mut self, key: PcKey) -> QueryPoll<RecipeId>;
    fn call_plan(&mut self, request: CallRequest) -> QueryPoll<CallPlan>;
    fn descriptor(&mut self, key: TypeKey) -> QueryPoll<TypeId>;
    fn drop_plan(&mut self, ty: TypeId) -> QueryPoll<DropPlan>;
}

trait BrokerClient {
    fn request(&mut self, request: EffectRequest) -> Result<EffectReply, Diagnostic>;
}

// QueryPoll is Ready(Result<T, Diagnostic>) or NeedInput(ExpansionRequest).
// A suspension never unwinds/replays guest execution; Section 56 defines resumption.
// These interfaces are owned by a single execution thread.
fn run_test(engine: &mut Engine, request: RunRequest) -> RunResult;
fn interpret_one(engine: &mut Engine) -> StepOutcome;
fn compile_region(plan: &RegionPlan, cache: &mut NativeCache)
    -> Result<InstalledRegion, CompileFailure>;
```

Do not use interior mutability to make mutually recursive ownership compile. `Engine` owns disjoint fields and lends short-lived borrows. When dispatch needs a query and a VM mutation, finish the query first, own its returned plan, then mutate the VM. A compiler borrow error here is usually a sign that a borrow is held across a phase boundary.

<a id="section-5"></a>

## 5. Closed release scope

The following matrix is normative. `Supported` means implemented before release, not merely parsed. `Reached error` means a cold occurrence does not block an unrelated execution. `Preparation error` applies only when declaration completeness or selected target preparation actually requires the feature.

| Area | Required supported forms | Fixed exclusions and failure point |
|---|---|---|
| Guest edition | Selected roots and dependency/expansion origins retain 2015/2018/2021/2024 as declared | Unknown or unreleased editions are rejected; never reinterpret older root, dependency or macro tokens as 2024 |
| Scalars | `()`, `bool`, `char`, `i8/i16/i32/i64/isize`, `u8/u16/u32/u64/usize`, `f32/f64` | `i128/u128`, SIMD: reached type/operation error |
| Literals | Integer bases, suffixes, decimal floats, Unicode chars, UTF-8 strings, byte strings | C strings and unsupported literal forms: reached error |
| Functions | Named functions, simple identifier/`_` parameters, explicit parameter types, recursion, function items/pointers with concrete signatures | Variadics, foreign ABI, async, generators: reached call error |
| Control flow | Blocks, `if`, `if let`, `while`, `while let`, top-level `&&` let chains in if/while, `loop`, labels, `break`, `continue`, `return`, `&&`, `||` | Pattern guards/let chains in match guards and async suspension: reached error |
| Local bindings | `let`, `mut`, annotations, tuple/struct destructuring, `ref`/`ref mut`, `let-else` | Complex unsupported patterns: error when tested/bound |
| Aggregates | Structs, tuple structs, enums, tuples, arrays, slices through references | Unions, arbitrary layout attributes, DST struct tails: reached schema error |
| Ownership | Runtime Copy/move, partial moves, replacement, guest references, logical Drop | No borrowing/lifetime proof, no raw pointers, no transmute |
| Generics | Type/lifetime/const parameters, structural substitution, call-local hints, associated projections/GATs, higher-ranked binders, parameter and return `impl Trait`, guest `dyn Trait` | No whole-body inference or borrow proof; unresolved necessary inference reports an annotation request; unstable specialization/negative bounds/trait aliases remain excluded |
| Methods | Source-aware inherent/trait/default/supertrait lookup, conditional and blanket impls, associated functions/types/constants, custom Deref/Index/operator dispatch | Reached ambiguity, ill-formed required binder, unsupported native-world interchange, or resource limit is a precise fault, never a guessed method |
| Constants | Pure primitive/tuple/array constant expressions and pure named calls under a restricted constant VM | Static variables, effects in constants, heap-owning constants: reached error |
| Patterns | Wildcards, bindings, literals, tuple/struct/enum patterns, `ref`, `&`, or-patterns with consistent bindings, guards | Range/slice-rest patterns and complex binding ambiguity: reached error |
| Macros | Stable macro_rules in all legal fragment positions; function-like, attribute, and derive proc macros; cfg/cfg_attr; nested expansion; include/env built-ins | Macro tool unavailable, unauthorized, crashing, invalid output, or unsealed required scope: attributed frontend error; no text-only expansion fallback |
| Derives | All custom derives through the macro server; built-in Copy/Clone/Debug/Default/PartialEq/Eq/PartialOrd/Ord/Hash through the pinned compiler-compatible expander | Generated executable operations use this same runtime feature matrix; unsupported operations remain lazy, not ignored |
| Closures | None | Creation of a closure expression: reached unsupported error; cold closures remain untouched |
| Collections | Vec, String, slices, range iterator, Option, Result, Box and PhantomData facade; owned/borrowed guest trait objects | HashMap, Rc, Arc, custom allocators and unlisted APIs remain reached errors; this revision does not promise the entire standard library |
| Iteration | For loops and supported adapters over facade or user Iterator/IntoIterator; Item projection and named-function-based adapters | Closures/async remain outside the executable profile; expansion may generate them, but a selected unsupported operation is an attributed error |
| Error flow | Guest panic/assertions; Option propagation; Result propagation with reached From error conversion; expected-panic tests | Guest catch_unwind and unstable custom Try remain excluded |
| Effects | Managed: captured output, granted files, injected environment. Hybrid: native calls additionally have ambient worker authority | Managed network/process/clock/random/native APIs remain unsupported; native effects are not mediated |
| Cargo | Cargo-resolved feature/unit graph, proc-macro/build-dependency preparation, OUT_DIR source capture, and hybrid native reuse | Offline misses or unsupported Cargo protocol/target configuration fail preparation; generated files are never guessed from target-directory names |
| JIT | Observed baseline regions; typed optimized loop graphs; bounded specialization; reached-call inlining; Cranelift and LLVM tiers; admitted native IR imports | No speculative live body preparation, arbitrary symbol resolution or LLVM-to-CLIF translation. Internal vectorization requires semantic/target proof and does not admit guest SIMD types |
| Security (managed) | Capability-mediated guest effects; separately authorized and supervised compile-time native tools | Proc-macro helpers have host authority, so a run executing them reports tool-phase noncontainment; no hardened whole-run sandbox claim |

A syntactically valid `unsafe { ... }` block itself is a transparent lexical block. It grants no authority. Executed supported safe operations inside it behave normally; raw pointer/FFI/assembly operations still fail. An `unsafe fn` may be called under the same checked runtime conventions; its signature does not enable native access.

### 5.1 Compatibility reporting

Every result contains:

```json
{
  "runtime_version": "0.2.0",
  "semantics_profile": "lrr-2024-v2",
  "rust_validation": "live_not_performed",
  "sandbox_strength": "capability_mediation",
  "macro_execution": "none",
  "trait_resolution": "demand",
  "guest_target": "aarch64-apple-darwin",
  "execution_status": "passed"
}
```

A runtime fault, unsupported operation, incomplete discovery, timeout, or worker crash cannot be converted into `passed`, `ignored`, or an expected guest panic. A test that intentionally handles an ordinary I/O permission error may pass; its audit log still records the denial.

Hybrid results additionally identify the native generation, native build profile, and `sandbox_strength="none_native"`; see Section 42. A supported source feature in a native dependency does not become a supported live frontend feature.

### 5.2 Required product demonstration

The release demonstration uses a real two-package fixture containing an invalid unused function, an invalid untaken branch, a generic named function, a loop that enters native code, a small Vec/String transformation, and a capability-controlled file read. It edits one called body and reruns a selected test in the same service. The new result must appear while unrelated body/query/code artifacts remain reusable. A second run without the file permission must observe the defined denial. The additional hybrid demonstration in Section 44 must also pass. These are release gates, not illustrative aspirations.
<a id="section-6"></a>

## 6. Identities, fingerprints, and serialization

### 6.1 Three identity lifetimes

Never use one integer identifier for all lifetimes. Define the following families:

| Family | Lifetime | Representation | May appear in persistent input? |
|---|---|---|---|
| Content identity | Across processes | 32-byte BLAKE3 digest | Yes |
| Worker identity | One worker process | Checked monotonically allocated `u32` | No |
| Guest handle | One test/run, checked against worker run epoch | `u64`: high 32 bits run epoch, low 32 bits arena index | No |

Use newtypes, not type aliases, for `DefId`, `BodyId`, `TypeId`, `RecipeId`, `OpId`, `PcId`, `ScopeId`, `SlotId`, `CellId`, `ObjectId`, `RefId`, and `FrameId`. Accidental use of a field index as a cell handle must fail host compilation.

Define `Digest([u8; 32])`, encoded in JSON as exactly 64 lowercase hexadecimal characters. Define a `StableEncode` trait for hashed structures. Its encoding is: a versioned domain string; then each field in declared order; unsigned integers little-endian; strings as `u64` byte length followed by UTF-8 bytes; vectors as element count followed by elements; enum variants as explicit `u32` tags; maps as key-sorted vectors. Never hash `Debug` output, host pointer values, randomized `HashMap` iteration, Serde's incidental field order, or source offsets as semantic identity.

Each digest domain starts with `lrr/v2/`, for example `lrr/v2/body`. Length-prefix each component so concatenation cannot confuse `ab,c` with `a,bc`. For input content, use the exact bytes. For normalized declaration summaries, encode token kinds and text without nonsemantic whitespace; retain literal text, attributes, visibility, generic syntax, and supported cfg information.

### 6.2 Stable keys

```rust
struct PackageKey {
    source_identity: String,  // Cargo package ID or approved standalone root identity
    package_name: String,
    version: String,
}

struct CrateKey {
    package: PackageKey,
    target_kind: TargetKind,
    target_name: String,
    guest_edition: u16,       // declared edition for roots, dependencies and expansion origins
    cfg_fingerprint: Digest,
    dependency_aliases: Digest,
}

struct DefKey {
    crate_key: Digest,
    module_path: Vec<String>,
    namespace: Namespace,
    item_kind: ItemKind,
    declared_name: String,
    enclosing_impl: Option<Digest>,
    duplicate_ordinal: u32,
}

struct BodyKey { definition: Digest, exact_body_bytes: Digest }
struct PcKey { body: BodyKey, node: u32, phase: u16 }
```

A unique named function keeps its `DefKey` when only its body changes. Duplicate declarations get source-order ordinals and always create an ambiguity bucket; never select one because its ordinal is smaller. A changed impl header produces a new impl identity. A rename produces a new definition identity. Do not attempt fuzzy AST matching.

Within one `BodyKey`, assign syntax node IDs in deterministic preorder. A changed body invalidates all its node IDs and executable recipes. Unchanged function bodies retain their keys even when another body earlier in the file changes byte length. This is the chosen incremental granularity.

### 6.3 Source locations are separate from semantic identity

Store executable locations as `BodySpan { body_key, relative_start, relative_end }`. Store declaration locations as `HeaderSpan { def_key, relative_start, relative_end }`. The active snapshot supplies the current file and base offset. Never bake whole-file line numbers into reusable native artifacts.

This separation is required: inserting ten lines in another function must not make a reused diagnostic point ten lines too early. Cache reusable semantic recipes independently from the snapshot's `SourceMap`. Macro built-ins use synthetic child spans with the original invocation as parent; generated assertion paths remain attributable to that invocation.

A missing mapping is an internal diagnostic error, not permission to report a misleading location. Diagnostic rendering may fall back to a definition name and relative byte span, but the result records that source mapping failed.

### 6.4 Worker interning

Worker tables are append-only between full worker resets. Index zero is invalid. Allocation checks `checked_add`; exhaustion retires the worker between runs. Never wrap an ID. Primitive `TypeId` values are fixed by Section 11. Nominal `TypeId` values are interned from immutable `TypeKey` values and their schema fingerprints. A nominal schema edit creates a new type descriptor, even when the source definition name is unchanged.

No worker-local ID or raw executable memory image is loaded from disk. Section 62 permits validated relocatable objects whose symbolic IDs and relocations are rebound to the current worker. Stored snapshots and portable query records contain stable keys and explicitly encoded data; intern/remap those keys in the current worker. A Cargo-native generation is a separately authorized, validated linked library, not a deserialized JIT/query memory image (Sections 38–41).

<a id="section-7"></a>

## 7. Source capture and immutable runs

### 7.1 Coordinator responsibilities

The coordinator is the only LRR runtime component that discovers or reads host project files; explicitly authorized external build/macro tools are separate ambient-authority processes. It captures source bytes, manifests, the lockfile, explicit environment inputs, and the supported crate graph. It gives the worker an immutable bundle. A worker query cannot open a path or read ambient environment. It may return a typed NeedInput request for a source-provenanced include/expansion; the coordinator validates it against the captured source/build roots and never accepts an arbitrary file request.

For standalone execution, the source root is the directory containing the root `.rs` file. Discover external `mod` declarations using syntax. Resolve `name.rs` and `name/mod.rs` according to supported module rules; both existing is `LRR-E-MODULE-AMBIGUOUS`. `#[path]` is accepted only when its literal relative path stays within the approved root. Source symlinks escaping the approved root are refused. Runtime file-read permission is distinct from permission to load source or execute compile-time tools. A generated OUT_DIR root is approved only by the matching immutable build-output record.

For Cargo execution, explicit preparation establishes the approved package source roots. A cached registry dependency's source directory is source authority, not guest runtime filesystem authority. Only source paths listed in the captured manifest may be referenced by a worker.

### 7.2 Snapshot shape

```rust
struct Snapshot {
    id: Digest,
    parent: Option<Digest>,
    project: ProjectManifest,
    files: BTreeMap<SourcePath, SourceBlob>,
    source_map: SourceMap,
    cfg_by_crate: BTreeMap<Digest, CfgSet>,
    explicit_compile_env: BTreeMap<String, String>,
    facade_version: u32,       // 1
    semantics_version: u32,    // 1
}

struct SourceBlob { content: Digest, bytes: Arc<[u8]> }
```

Snapshot ID is the hash of the sorted source manifest, project/target configuration, explicit compile-time environment, facade version, and semantics version. Parent is metadata and does not affect the identity of otherwise identical snapshots. Normalize source paths into root-relative UTF-8 paths with `/` separators. Reject non-UTF-8 source paths in this release with a preparation diagnostic.

Source must be valid UTF-8 before parsing. A file may have recoverable syntax errors; that is different from invalid encoding. Exact source size and token limits apply before semantic execution.

### 7.3 Capture algorithm

1. Enumerate the approved manifest and module graph without following escapes.
2. Read each file using a bounded read. Record file metadata before and after reading.
3. If a file changes during its read, discard that capture attempt.
4. After the graph is captured, recheck metadata for every captured file. Retry the whole capture at most three times if it changed.
5. Hash bytes, construct source maps, and publish the immutable snapshot only after all checks succeed.
6. Failure after three attempts is `LRR-E-SNAPSHOT-RACE`.

These checks reduce races but do not turn a normal filesystem into an atomic multi-file transaction. The snapshot is the exact captured bundle. For truly atomic multi-file edits, the stdio protocol accepts a batch of supplied bytes and applies that batch as one revision. Never claim that a filesystem scan reflects a stronger transactional guarantee.

### 7.4 Revision publication

The worker has one active source revision. It never switches revisions while a test is executing. A new snapshot received during a run is queued. After completion or cancellation, publish the next revision, update input records, and allow memo validation against it. Keep no more than two queued snapshots; coalesce queued edits to the newest snapshot and report superseded request IDs.

Test requests explicitly name a snapshot ID. A request for a superseded, unavailable snapshot returns `LRR-E-SNAPSHOT-NOT-AVAILABLE`; it must not run newer source silently. A run result echoes the snapshot ID actually used.

### 7.5 Persistence policy

Persist prepared project metadata and source manifests in a coordinator-owned private cache. Never persist raw query memory objects, pointers, guest heaps, live resources, or Cranelift executable memory. Managed warm reuse lives in its persistent worker. Hybrid mode additionally retains the explicitly encoded portable pure-query whitelist and immutable linked Cargo-native generation described in Sections 38 and 41; those formats never restore runtime state or worker-local IDs.

A private cache directory is created with owner-only access where supported. Cache input is still parsed and bounds-checked. Its hashes establish integrity against accidental corruption, not authenticity against the same user or a compromised machine. A cache mismatch causes a clean miss and recapture. Do not deserialize Rust structs directly from memory images.

### 7.6 Watch mode

`lrr watch` polls known source/manifest paths every 100 milliseconds, using metadata to identify candidates and content hashing before publication. A changed Cargo configuration file, package manifest, or lockfile invalidates preparation and stops automatic running with `LRR-E-PREPARE-STALE`. It does not run Cargo automatically. Source-body edits publish a new snapshot and rerun the filter only when tool/native input records remain valid; attribute-macro outputs and generated declaration fingerprints are refreshed as required. A body edit observed by a build script can instead require explicit preparation.

Only one test request is active. When another source edit arrives, request cancellation and queue the newest snapshot. The supervisor may terminate a stuck worker. This is application behavior being implemented; it does not require the implementation agent to work asynchronously.

<a id="section-8"></a>

## 8. Syntax, configuration, namespaces, and expansion completeness

### 8.1 Parse before execution; keep syntax separate from meaning

Parse every captured file with its owning unit's edition. Selected live roots support Rust 2015, 2018, 2021 and 2024; every source unit and macro definition keeps its declared edition. Parse errors in a file required for discovery are preparation diagnostics. A syntactically valid unknown expression receives an opaque node ID; do not lower or reject its executable meaning while scanning an unentered body. Token trees inside macro inputs are not ordinary expressions until the macro's grammar consumes them.

Source, generated source, and expansion tokens have separate identities. Expansion tokens carry edition, syntax context, origin chain, punctuation spacing, and delimiters through the private adapter in Section 49. A printed expansion is a diagnostic view, never a semantic input. Retain the original file snapshot for source_text/span operations. No provider reads ambient files.

### 8.2 Configuration and attributes

Apply the selected Cargo unit's cfg set, including feature flags, target values, test intent, and build-script cfg. Handle cfg_attr in source order, recursively; disabled items do not enter any namespace, impl catalog, or test catalog. cfg! is a Boolean expression, not source exclusion. Treat registered inert helper attributes as inert only within their valid scope. Preserve them in the token stream delivered to the relevant derive/attribute macro.

Recognize compiler attributes whose semantics this profile implements: cfg, cfg_attr, derive, test, ignore, should_panic, path, macro_export, macro_use, repr for the supported semantic forms, and lint/tool attributes on the documented inert allowlist. Attribute macros are transformations, not inert annotations. A replacing attribute executes before the next transformation of its output according to the pinned expansion scheduler. Derive output can introduce arbitrary items; a derive is never summarized as affecting only its annotated type. An unrecognized attribute that might be a macro is resolved in the macro namespace; failure blocks the affected discovery request. Do not ignore it to run a test.

Layout-sensitive repr forms remain a reached layout-operation restriction where the managed representation does not support them. Merely parsing repr(C) does not confer a native ABI or enable transmute. Unsafe/FFI/runtime exclusions remain separate from expansion support.

### 8.3 Namespace and identity model

Maintain value, type, macro, trait-associated-item, and impl indexes. Distinguish package identity from Cargo unit identity and live/native/macro-export worlds. Use hygienic identifier identity rather than text alone for expansion names. Values and types with the same spelling are not the same namespace entry. Imports and reexports preserve original DefKey and visibility rather than creating a new nominal type.

Inside a block, item declarations can be visible independently of statement execution; structural discovery collects their headers. Local let bindings activate only after initializer evaluation and pattern binding. A local shadow creates a distinct LocalId. Record the lexical statement point for lookup. Macro_rules textual scope follows definition position; path-based macro imports use their own resolution rules. Never approximate both with a single module-wide name map.

A name bucket has candidates, a membership digest, and a completeness certificate. An empty complete bucket is a dependency. An incomplete bucket is neither empty nor ambiguous: it yields NeedInput or a discovery diagnostic. Adding, removing, or changing a macro-generated declaration invalidates relevant positive and negative lookups.

### 8.4 Path lookup

For a reached path, determine namespace from the syntax role. Resolve crate/self/super and dependency aliases using the captured unit graph. For an unqualified path consult active locals, nested item scopes, direct declarations/imports, and the edition's supported prelude according to the language namespace rules. Resolve imports to a fixed point; a glob needs the source scope's export seal. Preserve hygiene when following $crate, generated identifiers, and reexports. Apply visibility to the chosen identity; do not substitute a similarly named accessible declaration after an inaccessible resolution.

A lookup returns one identity, a genuine missing-name result, ambiguity with sorted candidates, or NeedInput. Associated paths are handed to ResolveMember/NormalizeProjection, not guessed from a spelling. Method lookup uses Section 14, not this ordinary path search. The algorithm for worklist ordering, glob cycles, and seal dependencies is specified in Section 50.

### 8.5 Complete declarations before relying on absence or uniqueness

Create ScopeSeal, ImplCatalogSeal, and TestCatalogSeal as separate records. The requested seal determines what discovery is necessary. The conservative impl-catalog discovery scans active nested blocks/functions for impl items and macros capable of emitting them, without preparing executable operations. An impl does not become relevant only when its enclosing function executes. An expression macro may expand to a block containing impls; unknown output therefore cannot automatically be deferred just because its invocation is in a cold body.

Expand unresolved declaration-producing macros in the required discovery domain, integrate their output, resolve new imports/macros, and repeat until no frontier remains. Known runtime-only intrinsic macros have audited no-item summaries; these may remain cold. Unknown derives and attributes keep both surrounding scope membership and affected impl catalogs incomplete. Generated includes participate identically to source files. Apply cfg before exploring excluded output.

This is a correctness boundary: an unused function may contain an unresolved ordinary call and still permit unrelated tests, but an unresolved macro capable of changing the implementation being selected can block that selection. Diagnostics state the blocked demand and macro origin. Never pick a candidate from the files visited so far. Section 50 gives the exact finite-work limits and fixed-point failure states.

### 8.6 Called-body structural index

On first call, index the final expanded body structure into node roles, scopes, binding locations, source views, loop labels, and temporary slots. Expansion bodies remain separate from named callees and closure/async bodies. This scan may traverse untaken syntax but does not resolve executable names, prove traits, evaluate ordinary constants, or create machine code for those paths.

Allocate stable structural node ordinals within a body-content identity. Each expression gets logical value/place slots; guest cells are allocated only when executed. Preserve each node's expansion ancestry and source-view recipe. A missing effectful expansion is serviced through QueryPoll::NeedInput; resume the saved phase, not the whole expression. Generated item changes that would invalidate an already-issued namespace seal are a frontend consistency failure, never a mid-run silent namespace mutation.

<a id="section-9"></a>

## 9. Query database: exact implementation

### 9.1 Database ownership and purity

`SourceDb` lives in the worker and is accessed only by its execution thread. It owns parsed source, declaration indexes, body structures, and pure recipes. It does not own mutable guest state or native entry pointers. Runtime executions, profile counters, permissions, and native publication are not queries.

Query providers read only explicit input records or queries. They cannot call std::fs, std::env, SystemTime, process creation, the effect broker, the macro server, or the ordinary VM. Missing effectful expansion input returns QueryPoll::NeedInput; no partial or blocked result is memoized as a completed absence. Section 56 defines the dependency-stack suspension protocol. Constant evaluation is a small pure evaluator owned by `frontend::constants`, not a call back into the effectful guest engine.

### 9.2 Input records

Each input record is identified by `InputKey` and contains an immutable value plus an output fingerprint. Inputs are SourceBlob, DefinitionHeader, DefinitionBody, ScopeMembers, ImportEdges, ImplHeaders, CrateCfg, DependencyAliases, FacadeSchema, CompileEnv, ToolUnit, BuildOutput, MacroLibrary, ExpansionRecord, GeneratedSource, and ExpansionEpoch. Scope/impl membership is publishable as complete only with the completeness certificate in Section 50. Missing input is represented explicitly and fingerprinted. Revision publication replaces changed input values and increments a database revision counter.

A file reparse updates derived outlines. The coordinator does not invent semantic input records. A preliminary outline refresh in the worker produces fine-grained header/body records for the new revision. Exact old/new summary comparison determines which inputs changed. Thus a body edit changes `DefinitionBody` but need not change `DefinitionHeader` or a name bucket.

### 9.3 Query key catalog

| Query | Key | Value and dependency expectations |
|---|---|---|
| `ParseFile` | Content digest, edition | Private parse tree and syntax diagnostics |
| `Outline` | File content, crate/module identity | Headers and body handles; no executable semantics |
| `ScopeBucket` | Scope identity, namespace, name | Complete candidate set including empty/ambiguous result |
| `LookupPath` | Lexical scope, active local-scope shape, namespace, path | Symbolic target and all searched bucket fingerprints |
| `Signature` | Definition key | Parameter/return recipes and unresolved generic/bound syntax |
| `BodyStructure` | Body key | Nodes, scopes, labels, slots, spans; also depends on the owning signature header for parameter/binding layout |
| `Thunk` | PC key | One small LRIR recipe for the entered node phase |
| `TypeSchema` | Nominal definition + concrete argument keys + header digest | Field names, deferred field contracts, policies |
| `ResolveMember` | SourceView, ParamEnv, trait imports, member syntax, hygiene, relevant sealed headers | Symbolic associated-item identity and adjustment recipe |
| `SelectImpl` | Canonical trait goal, ParamEnv, world/unit identity, sealed impl catalog | Unique witness, no-solution, ambiguity, or resource failure |
| `NormalizeProjection` | Associated-item identity and substitutions, ParamEnv, sealed catalog | Concrete/symbolic type plus constraints and dependencies |
| `Dispatch` | Resolved member identity, runtime substitution, witness dependencies | Target plus checked receiver/call adaptation, never a name-only runtime search |
| `Constant` | Constant definition, concrete arguments, cfg/profile | Serialized pure constant value or deterministic diagnostic |
| `DropRecipe` | Concrete descriptor key, impl-header fingerprint | Destructor identity and ordered child recipe |

The lexical-point component of LookupPath is structural: body/scope/statement position and pattern-binding layout. It never hashes live guest local values or reads VM state. A pure lookup returns a stable binding identity; execution separately checks whether that binding's cell is initialized.

Do not add a query for every execution of an instruction. `Thunk` is fetched once per new PC and cached in an installed PC table. Hot execution uses that table directly.

### 9.4 Memo entry

```rust
struct MemoEntry {
    key: QueryKey,
    value: Arc<QueryValue>,
    fingerprint: Digest,
    dependencies: Vec<DependencyStamp>,
    verified_revision: u64,
    last_used_counter: u64,
    estimated_bytes: u64,
}

enum DependencyStamp {
    Input { key: InputKey, fingerprint: Digest },
    Query { key: QueryKey, fingerprint: Digest },
}
```

`QueryValue` is an explicit enum. Do not use `Any`, unchecked downcasts, or opaque pointer casts to store heterogeneous query values. Every query kind checks its corresponding result variant. A mismatch is an internal bug.

Keep `active_queries: Vec<QueryKey>` and a stack of dependency collectors. Query recursion depth is limited to 256. The ordinary guest call stack is unrelated to this stack.

### 9.5 Fetch algorithm

```text
fetch(key):
    if memo[key].verified_revision == current_revision:
        record key + memo fingerprint in the caller's dependency collector
        return the memoized result

    if key is already on active_queries or query depth exceeds the limit:
        mark this outcome and all collecting ancestors non-cacheable
        return the query-kind-specific cycle/depth diagnostic

    push key onto active_queries
    if memo[key] exists:
        copy its dependency stamps out of the map
        push an isolated validation collector
        recursively fetch current values for those dependencies
        compare every fingerprint; discard the validation collector afterward
        if all match and all outcomes are cacheable:
            set verified_revision to current_revision
            pop key from active_queries
            record key + memo fingerprint in caller
            return old value without rerunning provider

    push an empty provider dependency collector
    execute the pure provider for key
    pop, sort, and deduplicate dependency stamps
    pop key from active_queries
    fingerprint the provider's value, including cacheable deterministic errors
    if provider and every dependency outcome are cacheable:
        replace memo entry atomically
        record key + output fingerprint in caller
    else:
        remove any stale memo for key
        mark caller outcome non-cacheable
    return value
```

Cycle, active-stack-depth, transient allocation/resource, and cancellation outcomes are not memoized, and non-cacheability propagates to their callers. Otherwise a cached cycle error could survive an edit that breaks the cycle, or a depth failure from a deep call chain could poison a later shallow lookup. Pure syntax/name/type failures whose full input dependencies are known are cacheable. The stored memo dependency graph is acyclic. Add tests that break a previously observed alias/constant cycle and immediately succeed.

Cleanup of active-query state must occur on ordinary returned errors as well as success. Providers return errors as values; they do not panic. Copy dependency metadata before recursive fetching so a mutable borrow of the memo map is not held during recursion. Deduplicate a dependency key only if its recorded fingerprint agrees; disagreement within a supposedly immutable revision is `LRR-E-INTERNAL-SNAPSHOT`.

### 9.6 Cycles and recursion

Function recursion does not recurse through `Thunk` queries: a call recipe stores a target identity and returns to the dispatcher. Its body is requested when a new guest frame begins. Mutually recursive function headers are legal.

Constant cycles are `LRR-E-CONST-CYCLE`, with the cycle path. Alias cycles are `LRR-E-TYPE-ALIAS-CYCLE`. Import cycles use the worklist described above. Recursive nominal field types are rejected only when constructing a finite by-value schema requires infinitely expanding them. A nominal reference field does not expand its referent layout. Query dependency cycles must never be “fixed” by returning an empty result or unit value.

### 9.7 Negative dependencies and completeness

A successful path lookup records every searched bucket up to and including the winning scope. A failed lookup records all searched buckets. Method/goal queries record the complete candidate bucket, including candidates rejected structurally or by failed child goals and candidates still ambiguous. Selected implementation bodies are not dependencies of candidate selection; their headers are. The fingerprint includes the fact that scope expansion is complete.

Adding a competing method must invalidate an old successful dispatch even when the old method body did not change. Removing a local declaration must invalidate a shadowed import decision. Resolving a previously blocked scope must invalidate the former error. These are direct tests of I-CACHE.

### 9.8 Query cache limits

Budget source trees, query values, and installed recipes separately. Query memo budget is 128 MiB; parsed-source budget is 128 MiB; installed LRIR budget is 64 MiB. Eviction happens between runs, oldest `last_used_counter` first, with key-order tie breaking. Active snapshot input bytes are pinned. Memo eviction means recomputation on demand, not an error.

If pinned active input exceeds its cap, refuse the run before executing guest code. Do not evict a syntax tree still referenced by a body handle; use owned content-based handles and release all dependent private nodes together. Track actual allocated vector/string capacities where practical and document the remaining allocator overhead separately.

### 9.9 Query tracing

Each query event contains revision, key, action (`provider_run`, `memo_hit`, `validated_reuse`, `invalidated`, `cycle`, `evicted`), reason, and dependency fingerprints. Default output includes counts; detailed event recording is enabled by `--trace-queries` and is bounded to 100,000 events per run. A truncation field reports lost events.

The laziness tests assert absence of `Thunk`, `Dispatch`, `Constant`, and native-plan events for designated cold spans. File parsing and body structure counts are reported separately so the runtime cannot pass a “lazy” test by relabeling semantic work as parsing.

<a id="section-10"></a>

## 10. Lazy program counters and LRIR preparation

### 10.1 Static structure, dynamic preparation

Each called function gets a structural tree, but executable recipes are materialized one phase at a time. A PC is `(body, node, phase)`. The phase is from a closed per-node table. A parent relationship supplies the syntactic continuation. No native Rust stack frame is used for expression recursion.

For example, a binary expression has phases `Enter`, `AfterLeft`, `AfterRight`, and `Apply`. `Enter` schedules/evaluates the left operand. `AfterLeft` schedules the right. `AfterRight` reaches the operator. `Apply` commits the result and follows the parent's continuation. These phases may be combined into fewer LRIR operations, but no phase may resolve a child before entering that child.

Every syntactic child has a deterministic return PC determined by its parent's node kind and child role. Function recursion works because each guest frame has its own slots for those same PC identities. Loops revisit the same PCs and reinitialize their iteration scopes.

### 10.2 Structural continuation rules

Implement `continuation_after(node)` as a pure structural operation:

| Child role | Next phase |
|---|---|
| Binary left | Parent `AfterLeft` |
| Binary right | Parent `Apply` |
| Unary operand | Parent `Apply` |
| Call receiver/callee | Parent `ResolveHeader` |
| Call argument at index i | Next argument entry, or parent `Invoke` |
| If condition | Parent `SelectBranch` |
| Selected if arm | Parent `Join` |
| While condition | Parent `SelectBody` |
| While body | Parent loop poll, then condition entry |
| Block statement | Block `StatementCleanup(i)`, then next statement |
| Block tail expression | Block `Exit` |
| Function body | Function `ReturnCheck`, then cleanup/return |
| Match scrutinee | Match `TestArm(0)` |
| Match guard | Match `GuardResult(i)` |
| Selected match arm body | Match `Join` |
| Let initializer | Let `BindPattern` |
| Assignment RHS | Assignment `EvaluateDestination` |
| Return operand | Return `BeginExitCleanup` |

Labels are resolved structurally within the current function only when a `break`/`continue` is reached. A missing label is a reached error. A syntactic item boundary cannot be crossed by a label.

### 10.3 Deferred semantics rule

`Thunk` produces instructions with symbolic operands such as `NameRecipe`, `TypeRecipe`, `PatternRecipe`, and `CallSiteRecipe`. It may identify that an entered node is unsupported, but its diagnostic must be executed at that PC. It must not inspect subsequent statements and return their error while an earlier statement is still pending.

Unknown branches are represented by `PcKey`, not by precompiled blocks. A branch instruction records both successor identities but requests neither successor's thunk until the condition chooses one. `&&`, `||`, guards, and `let-else` use the same mechanism.

### 10.4 Hot-path installation

When a thunk is fetched, validate its structural LRIR and install an immutable `BoundRecipe` in a worker table. Bind stable PC keys and literal pool references to worker-local IDs. The current frame PC points directly to this table on repeat visits. A missing entry invokes the resolver slow path; a hit does not enter the query database.

Each recipe contains at most eight primitive machine operations. A long syntactic phase is split into linked recipe fragments with deterministic fragment indices. A recipe may still contain a checked helper that performs a bounded aggregate operation; such helpers enforce element/fuel limits internally.

### 10.5 No speculative execution

Preparation never calls a guest function, evaluates a test, reads a fixture file, executes a destructor, or tries both sides of a branch. Recording for the JIT occurs during the real execution and never replays an operation to collect observations. A skipped expression produces no execution record. Structural knowledge that a branch exists is not permission to prepare its executable contents.
<a id="section-11"></a>

## 11. Runtime types, expected context, and primitive operations

### 11.1 Closed type universe

A runtime `TypeId` is a worker-local interned identifier. It is not Rust's `std::any::TypeId`. Reserve these IDs permanently: 1 unit, 2 bool, 3 char, 4 i8, 5 i16, 6 i32, 7 i64, 8 isize, 9 u8, 10 u16, 11 u32, 12 u64, 13 usize, 14 f32, 15 f64, 16 str. Zero means “no concrete descriptor” and is legal only in an unresolved literal or internal empty slot. Reference, tuple, array, nominal, function, and facade-container types are interned above this range.

```rust
pub enum TypeKey {
    Primitive(Primitive),
    Ref { mutable: bool, target: TypeId },
    Slice { element: TypeId },
    Array { element: TypeId, len: u32 },
    Tuple(Box<[TypeId]>),
    Nominal { definition: DefKey, schema: Digest, args: Box<[TypeId]> },
    Function { params: Box<[TypeId]>, result: TypeId },
    Facade { family: FacadeType, args: Box<[TypeId]> },
}
```

`TypeDescriptor` contains the key, shape category, an optional deferred field recipe, Copy status, an optional destructor identity, and a representation version. Lifetimes are erased from value layout, not from source-view/solver binder identity. A reference to a nominal type does not force its field schema. A value construction, field operation, or reached drop may force it. An alias expands on demand with an active-alias stack; a cycle reports `LRR-E-TYPE-ALIAS-CYCLE`.

Each frame carries a symbolic ParamEnv, a runtime type/const substitution, and lazy dictionary slots. Keep these separate. Reading a signature captures its bound syntax but does not prove unused bounds. SourceView records retain abstract type parameters, Self, opaque identities, projections, and higher-ranked binders even after a concrete TypeId is available. Required projections and implementation predicates are resolved on demand under Sections 52–54. Runtime layout may erase lifetimes only after the symbolic binder structure needed for resolution is preserved.

### 11.2 Expected-context propagation

Use `Hint = Unknown | Concrete(TypeId) | Tuple(Box<[Hint]>) | Array(Box<Hint>, u32)`. Hints are frame state attached to expression slots, not global runtime inference variables. Ordinary hint transfer travels from a reached parent into the child about to execute. Numeric widths may additionally reflect source constraints collected under this section, including later textual uses and constraints in other arms. Collecting those source facts never executes the uses or arms, prepares their guest thunks, or compiles a live runtime unit; it preserves the separate source inference and reached runtime-hint layers.

Provide context in these exact cases:

| Construct | Context passed into entered child |
|---|---|
| `let x: T = e` | Resolve `T`, pass concrete T to `e`. |
| Reached function argument | The corresponding parameter after currently available generic substitution. |
| Function tail or `return e` | The declared return contract after current substitution. |
| Struct field initializer | The field schema after constructor type arguments bind. |
| Array/tuple construction | Expected element/member hints if present. |
| Assignment to an already known typed local | Its established storage contract, without evaluating the LHS. |
| `if`/`match` selected arm | The parent expression's hint, without querying other arms. |
| Primitive arithmetic | A concrete result hint, or the concrete type of an already evaluated operand. |
| Comparison | A concrete operand's type; a bool result hint does not determine operand types. |
| Index or capacity argument | `usize`, because it is a reached built-in operation contract. |

Do not evaluate an effectful assignment destination early to obtain a hint. Only an already resolved local contract may hint the RHS without evaluating the LHS. Otherwise evaluate RHS first, resolve the destination afterward, then materialize an unresolved literal at replacement; width-sensitive RHS arithmetic without context still requests an annotation.

An unannotated local accepts the descriptor of its first initialized value as its storage contract. Reassignment must match that contract. An unresolved literal local retains an unresolved numeric contract until materialized by a reached contextual use. Once materialized, the local and subsequent reads share its fixed type. This is a local runtime invariant, not static analysis of its uses. `mut` is required for assignment to an already initialized local. Initializing an uninitialized `let x;` is supported once; reading it earlier fails.

### 11.3 Unsuffixed literals: Rust-compatible inference and fallback

Decode integer sign, radix, magnitude and written suffix once when preparing the reached literal recipe. Preserve spelling/origin for diagnostics and negative-minimum handling. Repeated execution reads this immutable decoded payload without rescanning suffixes, normalizing strings or parsing digits. Range checks, source-context inference, fallback and mutable binding remain reached runtime semantics under the rules below. A malformed or unsupported literal retains an attributed reached diagnostic; decoding cannot demand a cold successor or branch. Decoded payloads use explicit stable encodings and participate in recipe/checkpoint schema identity; reject incompatible old records as cache misses. CA027 is interpreter-first acceptance, not a JIT prerequisite implementation shortcut.

Represent an unsuffixed integer as `NumericLiteral { sign, magnitude: u128, spelling, origin }`; the parser rejects magnitudes beyond this internal range. This internal u128 is a literal parser representation, not a supported guest integer type. An unsuffixed float stores its decimal spelling until a reached f32/f64 context exists. Do not execute arithmetic in a “universal number” representation and narrow later.

Materialization rules are deterministic:

1. A suffix supplies the type immediately. Validate representability; negative minimum values receive the unary-negation treatment below.
2. A concrete numeric hint supplies the type, with a checked literal conversion.
3. A concrete other operand supplies the type when an operation requires matching primitive operands.
4. A reached typed argument, field, return, index, or assignment supplies context.
5. Resolve the relevant source type constraints before an arithmetic/comparison/cast-sensitive operation fixes a numeric variable. This includes constraints from later textual uses, typed calls, assignments, returns and applicable trait goals; inspecting source constraints must not execute those uses or prepare their guest thunks. After constraint closure, apply Rust1.99.0 fallback to an unconstrained integer variable (`i32`) or float variable (`f64`, including the pinned compiler's applicable `f32: From<{float}>` exception). Validate representability before primitive execution. A not-yet-supported constraint path remains an explicit incomplete diagnostic; it is not a successful stock fixture. Do not substitute fallback for an unresolved nonnumeric type, projection, missing input or ambiguity.

An unresolved literal payload is a checked LiteralId in a run-owned arena. Its record contains the numeric spelling/magnitude and a one-time optional concrete TypeId binding. Copies of that unresolved literal share the record, so materializing a later copy fixes the same numeric variable for prior aliases; conflicting later context fails rather than reinterpreting it. Source recipes store immutable spelling/origin and decoded lexical payloads, never mutable inference bindings or run-owned handles. Each evaluation requiring an unresolved literal allocates a fresh literal record; cached decoding does not share bindings across evaluations. Numeric materialization validates range and reserves its result before setting the binding.

A literal may cross a local binding while unresolved. Numeric inference variables remain distinct from rigid generic parameters and ordinary type inference variables. Constrain them through source-aware substitution and unification; after constraint closure an unconstrained numeric argument to `identity(1)` falls back to i32. Container inference may use later source operations such as a push to determine `Vec<T>` without executing that operation. A genuinely unconstrained nonnumeric container element type still reports ambiguity. This user-authorized compatibility correction supersedes the former no-future-use/never-default policy; see [E-NUM-001](docs/errata/numeric-rust-compatibility.md).

These examples are normative:

```rust
fn use64(x: u64) -> u64 { x }
fn works() -> u64 { let x = 3; use64(x) }
fn works2() -> u64 { 3 + 4 }          // Return context reaches arithmetic.
fn works3() { let x = 3; assert_eq!(x, 3_u64); }
fn works4() { let x = 3 + 4; use64(x); } // Later source call constrains operands to u64.
fn fallback() { assert!(1 == 2); }    // i32 comparison then ordinary assertion panic.
```

`works3` is valid because the other operand constrains the numeric variable. Ordinary Display/Debug dispatch uses the resolved concrete type after source constraint solving and fallback. Diagnostic rendering may display the raw spelling of an unresolved literal, but that rendering must not bind a variable or influence dispatch/arithmetic.

### 11.4 Arithmetic contracts

Implement all arithmetic in `vm::numeric` before native lowering. The interpreter and JIT must agree on these rules. The Rust Reference documents several non-obvious overflow cases, including signed division overflow and shift counts; use it as the oracle for supported concrete primitive operations.[^R08]

- Integer `+`, `-`, `*`, unary `-`, `/`, `%`, shifts, and compound assignments are checked. Overflow produces a guest panic, not a host panic. Use `checked_*` and explicit shift count checks. Signed minimum divided or remaindered by -1 is overflow. Division/remainder by zero panics.
- For a signed, explicitly suffixed minimum literal such as `-128_i8`, recognize the unary-negation-plus-literal syntactic pair and construct the minimum directly. Do not first reject `128_i8` as an independently evaluated positive literal. Negating an already materialized minimum value still overflows.
- Shift operands need not have the same integer type. Require an integer right operand, reject a negative count, and require the count to be less than the left width. Unconstrained unsuffixed shift counts follow Rust integer fallback (`i32`); source constraints may select another integer type. The former forced-u32 profile rule is superseded by E-NUM-001 and requires interpreter/native parity before acceptance.
- Bitwise operators operate on concrete same-type integers, or bool/bool where Rust permits it. `&&`/`||` operate only on bool and short-circuit. User operators use exact language-item trait identities through Section 54; && and || remain non-overloadable short-circuit operators.
- Ordering and equality are implemented for primitive same-type values, strings/slices under the facade rules, and supported derived comparisons. Float comparisons preserve NaN behavior. No fast-math, reassociation, or fused multiply-add substitution is allowed.
- Integer `as` conversions implement Rust's truncation/sign-extension rules. Float-to-integer conversion saturates according to Rust's primitive cast semantics. Float-to-float and integer-to-float use the host primitive cast with matched source/destination types. `char` casts are limited to supported valid primitive casts; integer-to-char is only from u8. Ref-to-pointer, pointer-to-int, and representation casts fail.
- `usize`/`isize` are 64-bit guest types but retain separate identities from u64/i64. No implicit numeric conversion between them.
- f32 calculations round at each guest primitive operation, not after a chain calculated in f64. Test raw bit patterns where meaningful, with explicit treatment of unspecified NaN payloads.

The operation dispatch table must enumerate every `(operator, left kind, right kind)` implementation. A missing entry produces `LRR-E-OP-TYPE`, never an `unreachable!()` reached through guest input.

### 11.5 Standard coercions

The implicit coercion set is literal materialization, reborrowing, &mut T to &T, array reference to slice reference, function item to compatible function pointer, reached user Deref coercions, and supported guest reference/Box unsizing to slices or dyn trait views (Section 54). No integer widening or implicit string allocation occurs. Result residual conversion is an explicit From trait call, not a coercion. No native Rust vtable or reference is synthesized from a guest descriptor.

Check a coercion after argument evaluation but before moving the argument into its final parameter cell. A coercion can create a new reference handle without copying the referent. Error reporting names the expected and actual runtime types and explains that LRR did not perform Rust's full inference.

### 11.6 Defined runtime failure classes

Arithmetic overflow, division/remainder by zero, invalid shift count, executed indexing out of bounds, invalid UTF-8 slicing boundaries, failed assertion, and failed unwrap/expect produce GuestPanic and ordinary guest cleanup. A get/get_mut out-of-range result is None, not a panic. Wrong operand descriptors, uninitialized/moved/stale locations, missing names, unsupported constructs, and exhausted execution quotas produce RuntimeFault with no further guest cleanup. Supported filesystem failures, including permission denial, return a guest IO Result. An absent injected environment key returns NotPresent. These distinctions apply in both tiers.

<a id="section-12"></a>

## 12. Value layout, arenas, and guest storage

### 12.1 Wire-independent value representation

All interpreter and native helpers share this representation:

```rust
#[repr(C)]
#[derive(Clone, Copy)] // Host bit-copy only; NOT guest Copy permission.
pub struct ValueRepr {
    pub kind: u32,
    pub type_id: u32,
    pub payload: u64,
}
```

Assert size 16 and alignment 8 in tests. `kind` discriminants are: 0 empty/internal, 1 unit, 2 bool, 3 char, 4 signed integer, 5 unsigned integer, 6 f32 bits, 7 f64 bits, 8 object handle, 9 reference handle, 10 function item, 11 unresolved integer, 12 unresolved float, 13 built-in iterator, 14 opaque native object handle (hybrid-only), 15 managed Box handle, and 16 managed dyn-view handle. Kinds15/16 use their typed guest arenas under Section 54 and do not change size/alignment. Kind 14 payload is a checked run-local NativeObjectRecord arena ID, never an address or foreign handle accepted from the public protocol; its descriptor is opaque and non-Copy (Section 39). Store sign-extended signed integers in payload; validate the declared width at construction. Bool payload is 0 or 1. Char payload is a Unicode scalar value. Validate these invariants at all host entry points and debug-check internal producers.

Do not serialize `ValueRepr` to the public protocol. Results use a separate printable/value-summary schema. Worker-local type IDs and guest handles must never be accepted as executable values from a client.

### 12.2 Arena implementation

Use append-only vectors with checked handle decoding:

```rust
pub struct GuestArena<T> {
    epoch: u32,
    entries: Vec<Option<T>>,
}
```

Index zero is permanently empty. A handle's upper 32 bits are the current run epoch and lower 32 bits are the index. Allocating appends. Freeing takes the entry, leaves a tombstone, and releases its owned host memory. Never reuse an index during a run. The coordinator increments run epochs between test executions; reaching u32 exhaustion restarts the worker before another run.

Every access checks epoch, nonzero index, bounds, and occupied entry. Return `LRR-E-STALE-HANDLE` on failure. Handle arithmetic is never available to the guest. Charge logical allocations/handles under Section 62.6 even when optimized storage is virtual. Cap cumulative handle allocations as well as live bytes; a loop that allocates and drops cannot force unlimited tombstones. All quota failures are explicit runtime faults.

Keep separate typed arenas for cells, objects, references, frames, iterators, unresolved-literal records, transfer tickets, hybrid NativeObjectRecords, ManagedBoxRecords, and DynViewRecords. Optimized virtual allocations use the separate logical identity contract in Section 62.6; a logical ordinal is never decoded as an arena index. Only actual materialization appends a physical arena entry. Typed Rust wrappers prevent passing a cell handle to an object lookup even if the bit pattern happens to exist in both arenas.

### 12.3 Cells and contracts

```rust
pub struct Cell {
    pub state: CellState,
    pub declared: StorageContract,
    pub writable: bool,
    pub location_epoch: u32,
    pub owner: CellOwner,
}
pub enum CellState {
    Uninit,
    Live(ValueRepr),
    Moved,
    Dropping,
    Dropped,
}
pub enum StorageContract { Unbound, NumericLiteral, Exact(TypeId) }
```

A cell's owner is a frame-local slot, an aggregate field, a facade buffer element, or a transfer ticket. It cannot have two owners. References do not own referent cells. A `ValueRepr` containing an owning object handle is valid in exactly one owning live cell or transfer ticket at a time.

The physical cell ID denotes a storage location. Reassigning a same-type live scalar at that location does not invalidate references to the location. Moving its value out, dropping the location, or ending its storage lifetime increments `location_epoch`. Reinitialization after a move is allowed only for writable cells and does not resurrect older references. Epoch overflow invalidates the cell permanently and allocates a replacement logical location before any further initialization.

A field location belongs to its enclosing aggregate object. Destroying or replacing the enclosing object retires the old field cells, so references into the old object fail even if another object has identically named fields.

### 12.4 Aggregate representation

`Object` is a tagged enum: tuple fields, array elements, nominal struct fields, enum variant fields, String buffer, Vec buffer, and facade wrapper where needed. Each contains its concrete `TypeId`. Nominal field order is declaration order; lookup translates a name to an index only after obtaining the reached schema.

Tuple/array/struct/enum owning fields are `CellId`s. A moved field remains a moved cell; do not collapse the aggregate into a single initialized bit. Maintain each object's initialized-child bitset and a cached fully-initialized summary. Field state changes propagate that summary through the unique owning-parent chain, bounded by the type/object depth limit. A whole-object ordinary copy/move requires the summary to be complete; cleanup may traverse an incomplete object and visits only live children. This avoids rescanning an entire object graph at every call boundary. An enum records its current variant and only that variant's cells. No guest object contains a direct Rust reference to another arena entry.

String holds UTF-8 bytes in a host Vec owned by the guest object, with allocation quota accounting. Vec holds element type, logical length/capacity, and a separately identified buffer containing element cells. Logical capacity is deterministic: new is zero, first growth is four, then double with checked overflow. This is LRR capacity policy, not a promise to match Rust's allocator capacity choices. A request for explicit capacity uses that value subject to quotas. Capacity is not exposed by the initial facade, so callers cannot rely on allocator-specific values.

A guest immutable string literal is a run-owned read-only UTF-8 allocation, instantiated from an interned source constant. It is not an unchecked pointer into source memory. An `&str` reference stores byte-range bounds and validates UTF-8 boundaries. String indexing by integer is rejected as in Rust; slicing uses the supported range forms and checks boundaries.

### 12.5 Checked mutation API

All mutations go through these methods; VM/JIT code cannot write arena entries directly:

```text
new_cell(contract, writable, owner)
initialize(cell, transfer_ticket)
copy_from(place) -> transfer_ticket
move_from(place) -> transfer_ticket
borrow(place, access) -> reference_handle
replace(place, transfer_ticket) -> old_transfer_ticket
read_scalar(place) -> scalar_copy
project(place, field_or_index) -> place
begin_drop(cell) -> cleanup_work
retire_cell(cell)
```

`copy_from` checks runtime Copy policy. Copying a Copy tuple/array/nominal aggregate allocates a fresh aggregate and fresh field cells, recursively copying values under a construction guard; it never duplicates an owning ObjectId into a second owner. Copying a shared reference may reuse its immutable RefId because that handle owns no referent. Mutable references cannot use this copying path. `move_from` transfers ownership and invalidates that location's previous reference epoch. `replace` first validates destination, incoming descriptor, and ownership. It then extracts the old value and installs the new value as one runtime transaction; the old value becomes an explicit drop obligation. No guest callback executes while this mutation transaction holds a host arena borrow.

Use `try_reserve` for host vectors/strings whose size depends on guest input, convert reservation failure to the resource outcome, and account for it before committing ownership.

Every operation must follow “validate, reserve resources, commit.” Reserve all necessary cells/tickets before consuming source ownership. A quota fault during reservation leaves existing guest ownership unchanged. Guest panics from a subsequent destructor happen after the replacement has committed; never undo the new value.

### 12.6 Host memory accounting

Track live guest payload bytes, cumulative allocated handles, frame count, and temporary output buffers. Charge String/Vec capacity and object/cell overhead using documented constants rather than pretending to report exact host allocator RSS. Separately, the coordinator monitors worker RSS and kills it at its process limit. Guest accounting is deterministic; RSS is a containment backstop.

At test end, physically free the entire guest arena regardless of leaked guest owners or errors. Physical reclamation does not run guest Drop. It must also release every broker resource associated with that run ID. Persistent source, query, LRIR, and native caches remain worker-owned and contain no guest handle.

<a id="section-13"></a>

## 13. Places, references, ownership, and destruction

### 13.1 Place resolution

A `Place` is an internal checked address recipe, not a guest pointer:

```rust
pub struct Place {
    pub root: PlaceRoot,
    pub projections: Box<[Projection]>,
}
pub enum PlaceRoot { Cell(CellId), Deref(RefId) }
pub enum Projection { Field(u32), Tuple(u32), Index(u32) }
```

Evaluate dynamic index expressions in the specified operand order, obtain a checked usize, and then freeze that index in the place. `project` validates active enum variant, object identity, bounds, and field existence. Dereferencing a guest reference resolves its lease each time; it does not cache a host pointer across guest operations.

### 13.2 Reference leases

A reference holds access (`Shared` or `Mutable`), target `TypeId`, and a lease. A location lease records root cell ID/epoch, every traversed enclosing object ID, and final cell ID/epoch. A buffer lease records buffer identity/version, element/range bounds, and affected cell epochs. Reading/writing resolves and validates the full lease.

The lease deliberately does not count aliases. Two mutable guest references may coexist. Writes through them serialize in the single-threaded VM. This is a relaxed execution behavior, not evidence that the source would pass Rust's borrow checker. LRR still rejects writes through shared refs and stale locations because these are execution and host-safety invariants.

Reference rules:

- Moving a stack aggregate invalidates references anchored in the source cell or its inline fields, even if its logical field object is reused in the destination.
- Moving a Vec or String preserves references into its heap buffer while that buffer remains alive. This matches the distinction between moving a container handle and reallocating its buffer.
- Vec growth that reallocates increments the buffer version and replaces the storage identity; all old buffer leases fail. A push within existing capacity preserves existing element references. Pop invalidates the removed element; it does not invalidate earlier elements.
- String mutation conservatively invalidates all outstanding string-slice leases, even when capacity would permit in-place mutation. Valid borrow-checked Rust cannot depend on mutating a String while such shared borrows remain in use; this conservative rule is explicit relaxed-profile behavior.
- Moving through a shared reference is forbidden for non-Copy values. Reading a Copy value is allowed. Moving a non-Copy value through mutable reference syntax is also rejected; use `mem::replace` or a consuming container operation so the referent is not silently left invalid.
- Creating `&mut` requires a writable location or mutable reference. Creating `&` from `&mut` is supported. Neither operation creates host aliasing Rust references.

Do not model references as `Rc<RefCell<T>>`: that would conflate guest aliasing errors with host borrow panics and silently change lifetime semantics.

### 13.3 Copy and move policy

Sized primitives, supported function pointers/items, and shared references are Copy. Unsized str/slice descriptors cannot be read as standalone owning scalar values. Mutable references are not Copy. Tuples and arrays are Copy only if every element type is Copy. A nominal type is Copy only with a selected Copy witness (manual or generated) and a representation-safe copy policy for its actual fields. Consult required header predicates and independently reject an owning noncopyable representation claimed Copy by invalid relaxed source. `Clone` is not Copy. Vec and String are not Copy.

Computing Copy policy is a pure demand-driven descriptor/trait query, returning NeedInput if declaration completion requires missing expansion. It may examine necessary impl predicates and used field schemas; it never executes Clone/Drop bodies or validates unrelated types. Recursive owning aggregates are invalid without indirection; references terminate descriptor recursion.

Read operands use `copy_from` for Copy types and `move_from` otherwise, except in borrow/display/comparison contexts explicitly defined to peek or reborrow. Never infer that a bitwise duplicated `ValueRepr` is a legal guest copy.

A type with a user Drop method cannot have an owning non-Copy field moved out by ordinary projection. Report `LRR-E-PARTIAL-MOVE-DROP`. This prevents executing its Drop body over an incompletely initialized self without implementing additional Rust restrictions. Types without user Drop support partial moves and drop only their remaining live fields.

### 13.4 Assignment order and replacement

Simple assignment evaluates the right-hand expression before evaluating the destination place, matching Rust's specified assignment operand order.[^R08] Store the RHS in a transfer ticket. Resolve the LHS and validate the destination. Replace atomically, then schedule dropping the replaced old value before proceeding to the next source operation.

Determine compound-assignment kind from symbolic source views before executing either operand. If both operands are known primitive in the definition's pre-substitution context, evaluate RHS, then destination place, then mutate with checked arithmetic. Otherwise evaluate the left receiver then RHS and call the exact AddAssign/SubAssign/etc. trait member. Instantiating a generic T with i32 does not change that order. If a necessary source view is unavailable, report an annotation requirement before either operand; never evaluate once to discover the order and then replay. A primitive arithmetic failure leaves the destination unchanged.[^M12]

### 13.5 Lexical cleanup and temporary scopes

Each frame has a stack of `ScopeRecord`s containing local bindings in declaration order, their current initialization state, temporary cells, and a parent scope ID. A scope exit schedules live locals in reverse declaration order; late initialization and reinitialization do not reorder bindings. For patterns, the first occurrence of each primary binding determines declaration order. Fields of an aggregate are scheduled in declaration order. Ordering erratum E-T043-001 records the unchanged rustc witness and pinned native confirmation; see [the witness and regression](docs/errata/t043-local-declaration-drop-order.md). Function parameters are registered in parameter order and therefore cleaned up in reverse order. Array and Vec elements are cleaned up in increasing index order under the LRR profile.

For nodes whose effective edition is2024, implement the scope rules in Section 34; root/dependency/expansion nodes with earlier editions use Section 34.7. A full expression statement's non-extended temporaries end with that statement. Non-extended block-tail temporaries end after evaluation of the tail, before the block's locals are destroyed. A failed if-let/if-let-chain cleans its pattern-condition temporaries and bindings before entering else; successful pattern-condition storage lives through the consequent. Ordinary boolean operands have their own shorter temporary scopes. Direct syntactically extending let borrows still extend their operands to the containing block; propagation is specified in Section 18.4. A match scrutinee is not itself a temporary boundary. Do not implement the edition change by naive match desugaring.[^R09][^H01][^H02]

Create dedicated native differential fixtures for each supported extension form and for a borrowed temporary passed through a function. If a syntactic temporary-extension form is not implemented, reject that reached borrow with `LRR-E-TEMPORARY-SCOPE`; do not silently choose a shorter or longer lifetime. The release feature manifest must enumerate the forms above, and all must pass before release.

### 13.6 Drop machine

Destruction is a VM state machine, never Rust host stack recursion:

1. Mark a live owner `Dropping`, preventing duplicate guest drop.
2. If its concrete type has a supported user `Drop::drop`, create a mutable self reference valid for this destructor activation and push a normal guest call frame.
3. After that call returns, traverse still-live fields in the specified order and push their drop obligations.
4. Release facade-owned buffers after their live elements finish dropping.
5. Mark the owner dropped, retire its storage, and continue the pending control transfer.

For a user Drop signature, require exactly fn drop(&mut self) returning unit. Select its exact trait witness and the necessary impl predicates under Section 53 when cleanup reaches the type. Ambiguity or no solution is a trait diagnostic, not a blanket bounded-impl rejection. A cold Drop body is not prepared until called.

DropRecipe queries store immediate child descriptor/field references without recursively expanding every child DropRecipe. Recursive container types therefore produce finite recipes; the cleanup machine follows only the finite live object graph.

Cleanup tasks store concrete owner IDs and a next-field index. A yield, effect, or native exit in a destructor preserves this state. Do not invoke one destructor twice on resumption. Locals not successfully initialized are not registered for cleanup; partially initialized aggregate construction holds each completed field in a construction guard until the final aggregate commits.

### 13.7 Panic, fault, and leak distinctions

`GuestPanic` is explicit guest control flow carrying a bounded message and source origin. It unwinds guest frames and executes the cleanup machine. A second guest panic while already unwinding terminates the test immediately with `LRR-E-DOUBLE-PANIC`. `#[should_panic]` recognizes only the first ordinary guest panic after successful cleanup; double panic fails.

`RuntimeFault` means execution cannot proceed under this profile: missing name, unsupported construct, stale reference, resource exhaustion, incompatible call shape, or denied operation classified as fatal. It stops guest execution immediately and does not run guest destructors. The run is marked failed, all physical arenas/resources are reclaimed, and already completed output/effects remain recorded. This avoids executing arbitrary extra guest code after an invariant failure. Clearly distinguish this from a Rust panic in diagnostics.

`mem::forget` transfers a managed owner to a run-lifetime leak list; it does not execute guest Drop. End-of-run physical reclamation still frees its managed representation. An opaque native value instead follows the explicit process-lifetime forgotten-store policy in Section 44.7; never physically free it through the worker allocator or invoke native Drop as a substitute for forgetting. No forgotten value can be recovered through an ownership path, though still-valid guest references may read its intentionally live storage until the run ends.

Host code must not panic for guest input. Release host panic aborts the worker. The coordinator reports `LRR-E-WORKER-CRASH` and invalidates all worker-local cache telemetry for that process.

<a id="section-14"></a>

## 14. Calls, generic source context, and demand-driven member selection

### 14.1 Call packet and exact-once boundary

```rust
// Normative interfaces: IDs are LRR-owned, not rustc or rust-analyzer IDs.
struct CallPacket {
    member: ResolvedCallable,
    signature: SignatureId,
    args: Vec<TransferId>,
    param_env: ParamEnvId,
    substitution: RuntimeSubstId,
    dictionaries: DictionaryEnvId,
    result_hint: Hint,
    continuation: ReturnContinuation,
    origin: Origin,
}

enum ResolvedCallable {
    SourceFunction(DefKey),
    InherentMember { impl_key: DefKey, item: DefKey },
    TraitMember { trait_ref: TraitRef, item: AssocItemKey, witness: WitnessRequest },
    FunctionValue(FunctionValueId),
    Intrinsic(IntrinsicId),
    NativeBinding(NativeBindingKey),
}
```

Evaluate a callee or method receiver first and retain it as a place/value without prematurely moving it. Resolve the source-level member identity under the definition's symbolic ParamEnv, then request any concrete implementation needed for this call. Prepare parameter/result hints from explicit arguments and already available local context. Perform the selected receiver adjustment chain once. Evaluate written arguments left-to-right into transfer tickets. Excess arguments still evaluate before arity failure, as in the relaxed profile's existing rule. Bind remaining generic variables using actual descriptors and source views, apply supported coercions, and check concrete call contracts. Do not prove unrelated signature bounds.

Resolution may need argument shapes after argument evaluation; store a pending identity/candidate request in that case. Never evaluate operands for a speculative candidate and then retry another candidate. A missing direct function identity fails before its arguments; a concrete call mismatch fails at its prescribed boundary. A NeedInput/NeedGoal pause retains completed transfers and the phase index. Memory limits and cancellation are checked before publishing a callee frame. NativeBinding enters Section 40's boundary machine instead of a source frame.

A source frame is created only after the call is accepted. It retains the callee's definition ParamEnv, the caller's concrete substitution, and unforced trait dictionaries. Do not replace the definition's T or Self by its runtime type in the source-view environment. On return, materialize the required output descriptor, apply the explicit coercion list, check the returned value, clean scopes, then transfer it once. A cold incompatible return is not checked. A reached mismatch is LRR-E-CALL-RETURN-TYPE.

### 14.2 Signature substitution is not whole-body inference

Signature recipes contain rigid definition parameters, local inference variables, lifetimes/binders, const arguments, associated projections, function signatures, opaque identities, and nominal applications. Bind explicit generic arguments first, then known receiver/call-result context, then actual arguments. Use structural unification with an occurs check. Two nominal identities do not unify because their fields have equal layout. Existing concrete integer types do not widen implicitly. An unsuffixed literal waits for relevant source constraints, then applies Rust numeric fallback; rigid generic parameters and genuinely unconstrained nonnumeric inference variables never default to i32.

Capture bounds as symbolic facts and lazy witness requirements. A bound on an ordinary called function is not automatically proved at entry; force it when an executed operation requires its dictionary, associated item, or relevant selection predicate. An impl's where-clause is different: if the clause determines whether this impl is the applicable candidate for a reached selection, solve those necessary goals. Record the distinction in telemetry.

Lifetimes are erased in guest value layout, but higher-ranked binders, region equalities, and universe identity remain in resolution terms. No borrow/outlives analysis of executed storage is introduced. An unresolved region condition whose answer would change member selection yields an explicit unresolved-goal diagnostic; it is not silently true. Section 53 defines supported region facts and higher-ranked matching.

### 14.3 SourceView and definition context

Every expression result/place carries a SourceView in addition to its concrete runtime descriptor. SourceView is derived from the declared type, generic syntax, associated-item signature, and the already-entered expression's local constraints. A value originating as T stays symbolically T through aliases and reborrows, even when its actual value is Widget. A field of Wrapper<T> has its declared substituted symbolic field type. A trait default body sees Self under the trait's bounds; an inherent impl body sees the impl's declared self type. Return-position impl Trait exposes its opaque identity and declared bounds, not incidental inherent methods of the hidden runtime type.

The source view is used for name/member identity and source-dependent evaluation rules. The runtime descriptor is used for actual layout, checked operations, and concrete dictionary fulfillment. Losing this distinction is a correctness bug, not a performance shortcut. Section 52 defines propagation for every supported expression and mutation.

### 14.4 Member identity before concrete implementation

For a reached method call, build ordered receiver candidates from the symbolic source view: the receiver, its allowed autoderef targets, and the supported terminal unsized view; interleave each candidate with shared and mutable borrowed candidates. User Deref target lookup uses headers and associated-type normalization; no deref method body runs during search. Search the inherent namespace appropriate to that symbolic candidate and the language-ordered trait candidates, prioritizing declared bounds for a type parameter. Preserve an exact trait/member ID when resolution selects a bound method. The Rust Reference's candidate ordering is the native oracle for these supported rules.[^M08]

Do not filter a candidate out merely because the current place is not writable: Rust member lookup does not use that filter to find a lower-ranked method. After identity selection, an impossible receiver adjustment or visibility/contract violation fails; do not fall back. At an equal rank, unresolved applicability retains ambiguity until the necessary header goals resolve. Distinct applicable members remain ambiguous. Explicit qualification narrows identity but does not waive contracts.

After member identity is fixed, substitute concrete runtime parameters into its TraitRef and request SelectImpl. A lazy dictionary memo stores the unique witness for that exact TraitRef/item context, not a method name. With `fn f<T: Describe>(x: &T) { x.describe(); }`, this means Describe::describe even when actual T has an inherent describe. Native dispatch is only through a declared NativeBinding, never through this trait catalog.

### 14.5 Conditional impls, associated items, and defaults

SelectImpl indexes all relevant sealed implementation headers, structurally matches generic arguments, and recursively resolves only the candidate predicates needed to establish applicability. It returns Unique, NoSolution, Ambiguous, Overflow, or NeedInput. It never globally validates all impls for overlap or orphan rules. Overlap affecting a reached selection is an ambiguity error. No candidate is preferred merely for appearing first or looking more specific; unstable specialization is not implemented.

Normalize associated types and consts through the selected trait witness and associated-item arguments. GAT arguments have their own binder scope. Selected associated constants run the pure constant evaluator; unused constants remain uncomputed. A selected missing method may use the trait's default body. Keep that body's symbolic Self and trait item identity when it recursively calls another method. Do not inspect all required method bodies or reject an otherwise unused missing method.

Built-in language traits, derived impls, manual impls, and ordinary library traits share this selection service. Only their selected executable bodies may be intrinsic recipes. User Drop, Copy, Clone, operators, formatting, iterator conversion, and dyn dispatch use Sections 53–54. A no-solution result is valid only after required macro-generated impls have been integrated into a complete catalog.

### 14.6 Native and foreign boundary

A live `extern` declaration grants no native access. Native binding preparation checks concrete Rust target calls using rustc; the managed solver cannot prove a live type into a native Rust ABI. Raw addresses, arbitrary native symbols, native trait-object vtables, and callbacks remain excluded from the closed boundary. A live signature requiring such a conversion fails before native entry with LRR-E-NATIVE-WORLD-MISMATCH. Preserve already-completed argument effects; never compile an adapter or restart a test to recover from that failure.

<a id="section-15"></a>

## 15. LRIR specification and syntax lowering

### 15.1 Recipe and opcode schema

LRIR means Lazy Runtime Intermediate Representation. It is a register/cell-slot execution language with explicit continuations. A recipe is immutable and contains no source parser objects, guest handles, native pointers, or mutable profile observations.

```rust
pub struct Recipe {
    pub key: PcKey,
    pub origin: Origin,
    pub operations: Box<[Op]>, // Length 1..=8.
    pub exit: RecipeExit,
    pub dependencies: Box<[FactDependency]>,
}
pub enum RecipeExit {
    Next(PcKey),
    Branch { condition: SlotId, yes: PcKey, no: PcKey },
    Call { site: CallSiteId, resume: PcKey, result: SlotId },
    Control(ControlRecipe),
    Effect { site: EffectSiteId, resume: PcKey, result: SlotId },
    Fault(DiagnosticTemplate),
}
```

The following is the common opcode inventory. The revision-4 trait continuation records below are required extensions, not optional implementations. Every variant must have interpreter, verifier, effect classification, recorder, native lowering, and fixture coverage. An operation capable of calling guest code must never remain classified as a pure primitive solely because an older implementation only handled scalars.

| Opcode | Inputs / output | Required behavior |
|---|---|---|
| `Literal` | literal recipe, hint → slot | Allocate/materialize literal, or make a shared unresolved-literal record. |
| `Unit` | → slot | Initialize unit without allocation of an owning object. |
| `ResolveValue` | name recipe → value/place slot | Resolve only this reached name in the correct namespace. |
| `MakePlace` | local or temporary identity → place slot | Produce checked location, not value read. |
| `ProjectField` | place/value, field recipe → place | Force only the used schema; validate field. |
| `ProjectIndex` | base, index slot → place | Intrinsic bounds-checked projection or selected Index/IndexMut call followed by checked reference-to-place commitment. |
| `Deref` | reference/receiver → place | Validate an intrinsic lease or execute a selected Deref/DerefMut adjustment once and validate its returned lease. |
| `Read` | place, read mode → value slot | Copy/move/peek according to the selected context. |
| `Borrow` | place, access → reference slot | Make checked lease, no alias counting. |
| `SetHint` | child slot, hint recipe | Set context before entering the child. |
| `Unary` | operator, value slot → slot | Certified intrinsic operation or suspended selected trait-member call under Section 54.3. |
| `Binary` | operator, two value slots → slot | Certified intrinsic operation or selected operator/comparison member with correct ownership and source context. |
| `Cast` | source, type recipe → slot | Resolve reached cast target and apply supported conversion. |
| `ConstructStart` | type/variant recipe → guard | Reserve aggregate construction state. |
| `ConstructField` | guard, field index, value | Transfer one completed field to the guard. |
| `ConstructFinish` | guard → aggregate slot | Verify all required fields, commit aggregate ownership. |
| `BindPattern` | pattern recipe, source, mode | Run the pattern engine and commit bindings if selected. |
| `Replace` | place, RHS transfer | Validate/commit replacement; schedule old-value cleanup. |
| `ScopeEnter` | scope ID | Activate a new scope incarnation. |
| `ScopeLeave` | scope ID | Enter cleanup; never synchronously recurse through guest drops. |
| `CallPrepare` | callee/receiver, site → packet | Select header and currently available argument hints. |
| `CallArgument` | packet, ordinal, value | Append exactly one evaluated argument. |
| `CallSeal` | packet | Check contracts and reserve a sealed call packet; the Call exit publishes it. |
| `ResultTransfer` | completed child value → parent slot | Transfer ownership, no bit-copy of owning handles. |
| `IteratorStart` | supported source → iterator slot | Move or borrow source according to source form. |
| `IteratorNext` | iterator → item/present slots | Execute one bounded iteration step. |
| `TryUnwrap` | Option/Result value → value/control | Success payload or suspended early-return conversion through the required From witness; never replay operand evaluation. |
| `FacadePure` | fixed facade ID, arguments → slot | Bounded no-IO primitive/container operation. |
| `EffectPrepare` | fixed effect ID, arguments | Validate guest arguments and create pending broker request. |
| `PanicPrepare` | message recipe | Prepare an explicit guest panic record; the control exit activates it. |
| `Poll` | no guest value | Check fuel/cancel/deadline epoch. |
| `DropStart` | owner | Schedule explicit cleanup work. |
| `Unsupported` | diagnostic template | Reached profile fault. |

Revision 4 adds explicit phase records rather than hiding calls inside primitive helpers:

| Required record/op | Persisted inputs | Execution contract |
|---|---|---|
| MemberRequest | source-view ID, ParamEnv, member syntax, generic hints | Resolve symbolic identity; may return NeedInput without committing guest effects. |
| ApplyAdjustment | selected adjustment list, next index, receiver place/value | Run each selected Deref/autoref/unsize adjustment once; retain completed intermediates. |
| AwaitTraitCall | exact member/witness, transfer IDs, destination, resume PC | Publish a normal dispatcher-owned guest call and resume after it once. |
| CommitProjection | completed checked reference, expected output type, use mode | Validate lease and descriptor before creating an index/deref place; never call the method again. |
| SourceAssignKind | PrimitiveKnown or TraitDirected, resolved operator | Immutable recipe discriminator that fixes RHS-first versus LHS-first evaluation before operands run. |

SourceAssignKind is recipe metadata, not an executable action. The other four records become explicit Phase variants; their recipes use ResolveValue/CallPrepare/CallSeal/ResultTransfer and the checked projection helper rather than adding a hidden callback to a pure opcode. The opcode implementation returns a pending-action exit when such a phase is required. End a Cranelift region before an unresolved request or guest call; a committed projection can then be used by a later region.

Place slots, value slots, packet slots, and guard slots are distinct `SlotKind`s. A structural slot can be reused on the next loop iteration only after its previous scope incarnation has retired or transferred all ownership. The verifier catches a recipe addressing the wrong slot kind; runtime checks catch dynamic lifecycle mistakes.

### 15.2 Recipe verifier

Validate immediately after pure construction and before installation. Check: nonempty bounded operation list; valid node/phase keys; all slot IDs within the body's structural slot count; allowed operand/output kinds; each output written at most once per recipe unless the opcode explicitly mutates a place; terminal exit exactly once; valid `Origin`; no direct native address; no host filesystem path; no unknown facade ID; no call or effect hidden in an opcode marked pure.

The verifier does not type-check guest expressions, prove branch conditions, inspect successor recipes, or resolve a missing name. An `Unsupported` operation is a valid recipe that produces a guest-visible fault only when executed. Structural verifier failure is `LRR-E-INTERNAL-IR`, a runtime implementation defect.

### 15.3 Expression phase machines

Use the following phase templates. Every named phase is a deterministic `Phase` enum value or indexed value. Child enumeration reads syntax only; asking for a phase recipe does not ask for a child's recipe.

**Literal/path.** `Enter` creates a literal or resolves one value, then `Finish` transfers the result to the parent's structurally determined continuation. Paths in place context return a place without prematurely copying/moving the value.

**Ordinary binary.** `Enter` sets left hint and enters left. `AfterLeft` stores the left result, derives right hint where possible, and enters right. `Apply` executes the certified intrinsic or enters the selected trait member after both have completed; retain the operand transfers across suspension. `Finish` transfers the result. For a left unresolved literal and right concrete value, materialize the left only at Apply. The left and right expressions are never rerun.

**Short circuit.** `Enter` evaluates the left under bool context. `Select` validates bool. For `false && rhs` or `true || rhs`, it produces the known bool and finishes. Otherwise it enters RHS under bool context. No RHS semantic recipe exists until selected.

**Block.** `Enter` activates its scope. For each statement i, `Statement(i)` enters only that statement; `StatementCleanup(i)` cleans that statement's temporary scope then advances. `Tail` enters the actual tail under the block's result hint or produces unit. `Exit` transfers the tail result out before leaving the block's local scope. Block-tail temporary ownership follows its separately annotated temporary scope.

**Let.** `Enter` prepares only the written annotation and pattern header needed for binding hints, without looking at later statements. `AfterInit` stores the initializer; `Bind` matches the irrefutable pattern. A failed ordinary let pattern reports `LRR-E-PATTERN-REFUTABLE` when reached. A let-else failure enters the else block; normal completion of that else block is `LRR-E-LET-ELSE-DIVERGE` because the profile requires a control transfer. Register successful bindings in left-to-right pattern order.

**If.** `Enter` evaluates condition under bool. `Select` chooses one arm or unit if no else. `Join` transfers selected arm result. Do not unify arm result types. The surrounding reached storage/call contract checks the selected result.

**If let.** Evaluate the scrutinee once; `Test` runs non-consuming pattern matching. On success commit bindings into a condition scope and enter the consequent. On failure destroy that condition scope before entering else. On success destroy consequent-local state, then condition bindings/remaining scrutinee temporaries, before Join. The owned result is transferred out first. Let chains generalize this state machine in Section 34. This paragraph governs effective-2024 nodes; earlier-edition dependency or macro-origin nodes retain their Section 34.7 temporary scopes.

**While/while let.** `Enter` activates loop control record. Each `PollCondition` creates fresh condition temporaries; evaluate condition/scrutinee; `SelectBody` chooses body or exit. `AfterBody` cleans iteration scope and returns to PollCondition. A while-let scrutinee lives through that iteration body. Normal loop result is unit.

**Loop.** `Enter` activates a loop record and enters body. `AfterBody` cleans iteration locals, polls, and starts again. A reached `break value` transfers value to the loop's result destination after cleaning intervening scopes. Value-bearing breaks from while/for are rejected. `continue` never skips cleanup.

**Assignment.** `Enter` executes RHS first. `EvaluateDestination` resolves/evaluates LHS place. `Commit` replaces and enters old-value cleanup. `AfterDrop` returns unit. A fault resolving LHS leaves RHS ownership in a temporary until physical fault cleanup.

**Function/method call.** `Enter` evaluates target/receiver. `ResolveHeader` performs CallPrepare. For each argument i, `Argument(i)` enters that argument with available hint; `AfterArgument(i)` transfers it into the packet. `Invoke` seals the packet and returns a call exit to the dispatcher. `Resume` consumes the completed result. Method receiver adjustment is frozen before argument evaluation; do not re-evaluate an effectful receiver.

**Struct/tuple/array constructor.** Begin construction guard, evaluate written initializers left-to-right in source order, transfer each successful value into its field slot, then commit. Struct field declaration order controls storage/drop; written order controls evaluation. Struct update `..base` is excluded. Repeated array `[e; N]` evaluates e once, then requires Copy or a supported constant scalar recipe; it does not clone non-Copy values. N is a reached bounded pure constant. Empty arrays require element context.

**Match.** `Enter` evaluates scrutinee once. `TestArm(i)` prepares only that arm's pattern. A mismatch advances to the next arm. A match creates pending bindings, evaluates guard if present, and commits/enters the body only on success. A false guard discards pending bindings and advances according to the or-pattern rule below. `Join` transfers selected result and cleans scrutinee at its enclosing temporary scope. No exhaustiveness analysis. Falling off all arms is `LRR-E-MATCH-NO-ARM`.

**For.** Evaluate source once; IteratorStart resolves IntoIterator::into_iter and the required associated types through Section 54.4. Each iteration invokes Iterator::next once through IteratorNext, checks Option<Item>, matches the loop pattern, enters the body, and cleans iteration scope. Drop the iterator on break/return/panic according to the cleanup machine. Intrinsic owning array/Vec iterators drop remaining elements in increasing index order; user iterator cleanup follows its actual Drop and fields.

**Question mark.** Evaluate the operand once. An Ok/Some extracts and transfers its payload. An Err begins a pending early return, selects From<SourceError> for the enclosing Result error type, and invokes that conversion once through a guest call when needed; validate its concrete result before constructing Err and unwinding the function. A None propagates through the supported enclosing Option contract. Success never forces an error conversion. Do not implement unstable custom Try/FromResidual. Preserve the pending payload and continuation across queries, macro input requests, JIT exits, and conversion calls.

**Borrow and dereference.** Evaluate the place components once, select the annotated temporary scope if a temporary is needed, then construct/validate the lease. Ordinary `*r` in value context subsequently uses Read, so non-Copy extraction through a reference remains controlled.

### 15.4 Pattern engine

Use a work stack and a list of `PendingBinding { name, mode, place, hint, order }`. Match without changing ownership. Supported patterns are `_`, an identifier, `mut name`, `ref name`, `ref mut name`, same-type primitive/string literal, tuple, tuple struct/variant, named struct/variant fields with `..`, array with exact length, reference pattern, and `p1 | p2`. Slice rest bindings, ranges in patterns, `@`, const-generic patterns, and arbitrary constant expressions as patterns are excluded. Named primitive constants are permitted through the pure constant query.

Matching an aggregate checks descriptor and active variant before traversing fields. Match reference ergonomics by peeling references for non-reference aggregate patterns and changing default binding mode to shared ref, or mutable ref when the actual reference is mutable and no shared peel has occurred. In the 2024 profile, explicit `mut`/`ref`/`ref mut` binding modifiers and explicit `&`/`&mut` pattern prefixes require the current default binding mode to be Move. Otherwise report the reached `LRR-E-EDITION-PATTERN` diagnostic. Explicit reference patterns then consume the matching reference layer structurally. Do not create owning copies while testing a pattern. Section 34 specifies the reached-pattern procedure and tests.

For or-patterns, enumerate alternatives in written order. Each alternative must expose the same binding names and modes; validate that small pattern header when the or-pattern is reached. Binding cleanup order follows the first alternative. If a guard is false, try the next matching alternative in the same arm before later arms; a guard can therefore run more than once for overlapping alternatives. No guard expression is prepared for an arm that never matches.

During a guard, pending owning bindings are exposed as read-only views. Copy reads are allowed; borrowing is allowed subject to mutability; a non-Copy move or mutation of the pending binding faults with `LRR-E-GUARD-MOVE`. This avoids rollback of ownership and matches the practical guard restriction the profile supports. Effects executed by a false guard are not rolled back.

After guard success, first validate and reserve every destination binding. Then perform transfers in binding order. A validation failure before commit changes no ownership. A pattern with conflicting overlapping owning moves is `LRR-E-PATTERN-OVERLAP`. Ordinary pattern mismatch is not a fault in match/if-let/while-let, but is a fault in an irrefutable let/for binding.

### 15.5 Bound-op PCs and explicit exit operations

A canonical BoundPc is `(BoundRecipeId, operation_index)`, not merely the syntactic node or region start. Installing a recipe appends one synthetic `BoundOpKind::Exit(RecipeExit)` after its 1–8 ordinary operations. The installed list therefore has 2–9 bound operations. The exit is an executed, fuel-charged operation with its own stable ID; it is included in observation and native lowering. `BoundOpKind::Body(Op)` and `BoundOpKind::Exit(RecipeExit)` share the common step interface.

An ordinary successful opcode commits its output and increments operation_index. The synthetic exit executes Next/Branch/Call/Control/Effect/Fault, sets the next canonical PC or pending action, and emits its completion/boundary record. This makes branches, call publication, and effect publication real observed operations instead of hidden work a JIT could forget to perform.

CallSeal checks and reserves without publishing VM.pending. Exit::Call publishes the sealed packet and returns Call. EffectPrepare validates/prepares without submitting IO; Exit::Effect publishes and returns Effect. PanicPrepare creates a frame-local panic record; its Control exit activates panic/cleanup. ScopeLeave and DropStart may schedule cleanup and return Dispatch after advancing their own op PC; the dispatcher executes that cleanup before the already recorded continuation. A native step cannot continue past such a boundary.

Treat zero-op semantic phases as one Unit body operation followed by an exit, so verifier assumptions remain uniform. The completion hook records the exact before/after bound PC for both ordinary and exit ops. A guard miss resumes at that exact operation_index. The 64-op baseline region limit counts synthetic exits too; optimizing units use Section 62 budgets and exit maps instead; a recipe boundary alone does not end a region if its observed exit is an unconditional Next.

<a id="section-16"></a>

## 16. Interpreter, dispatcher, and interruption

### 16.1 VM state

```rust
pub struct Vm {
    pub run: RunId,
    pub snapshot: SnapshotId,
    pub memory: GuestMemory,
    pub frames: Vec<FrameId>,
    pub control: ControlState,
    pub pending: Option<PendingAction>,
    pub limits: RunLimits,
    pub counters: RunCounters,
}
pub struct Frame {
    pub body: BodyKey,
    pub pc: BoundPc,
    pub slots: Vec<SlotState>,
    pub scopes: Vec<ScopeRecord>,
    pub generic_env: GenericEnv,
    pub return_to: ReturnContinuation,
    pub cleanup: Vec<CleanupTask>,
}
```

Frames live in a checked arena. The explicit `frames` vector is guest call order; helper recursion never mirrors it. Slot vectors contain logical handles/states, not preallocated guest objects for every node. Only reached values allocate guest storage. The body index's total slot count still incurs a bounded host vector cost on frame creation; report it under frame preparation rather than claiming literally zero work for cold syntax.

### 16.2 Authoritative interpreter loop

```text
while run is active:
    check pending cancellation/deadline at dispatch boundary
    if pending broker reply exists: install it once, clear pending request
    if cleanup is active: execute one cleanup transition, then continue
    obtain current frame and PC
    install missing recipe through Resolver, with no guest effects
    poll ready compiler completion without waiting; validate/publish only at a safe boundary
    select a matching entry under the shape/cooldown policy
    if native entry is installed and this entry is not suppressed:
        invoke region; process its explicit exit; continue
    execute exactly one recipe through interpret_one
    record completed operation observations from that real execution
    process recipe exit: next, branch, call, return, effect, panic, fault, yield
    consider one eligible native promotion at this safe boundary
```

The authoritative order inside one opcode is: validate non-mutating preconditions, check/charge its fuel unit in precise (omit that charge in throughput), reserve needed resources, perform the guest operation, commit its result/next-op PC, append a completion observation. An error event is recorded with its opcode identity but is not a successful completion. A guest panic may occur after an earlier committed side effect, which remains visible.

### 16.3 Execution contracts and fuel

`precise` is the default execution contract. Charge one unit per LRIR opcode and per processed aggregate element. Queries/recipe installation consume preparation budgets, not guest fuel. The default is 10,000,000 units per test. Every tier produces the same executed-prefix charges and fault location. A proven fixed segment may check sufficient fuel once, but earlier fault/panic exits account only for executed work; insufficient fuel uses the exact checked path. Do not advance an effect before its charge.

`throughput` is explicitly selected and has no exact guest instruction fuel. Service fuel and result fuel_used are null in this mode; an explicit numeric --fuel together with throughput is a configuration error. Const evaluation, query, allocation, handle and compiler budgets remain enforced. Ownership, validity, overflow semantics and exact-once effects do not change. The contract is a cache/result identity, never implied by opt_level.

Precise cancellation polling occurs at least every 256 guest charges and at every call/effect/backedge/cleanup boundary. Throughput polls before and after external/native calls, effects and cleanup, at region entry/exit, and at most every 1,024 loop backedges or 4,096 straight-line guest operations, whichever occurs first. These are bounds in guest work, not time. Native internals remain outside guest fuel and rely on the supervisor deadline. Optimized exits reconstruct current state before returning; a stuck compiler/helper/native call is handled by the coordinator's deadline and termination policy.

### 16.4 Pending actions and effects

CallSeal and EffectPrepare retain sealed/prepared data in frame slots but do not publish an externally active action. The explicit recipe exit publishes that action.

A pending action is one of `CallPacket` (including NativeBinding targets), `ReturnPacket`, `EffectRequest`, `PanicRecord`, `FaultRecord`, or `YieldRecord`. It contains the next continuation and a monotonically increasing per-run action ID. Publishing requires `pending == None`. The dispatcher, or the matched direct-call continuation in compiled code, consumes it and clears it exactly once. A native helper must not overwrite an outstanding pending action.

Before an effect leaves the worker, store its continuation and mark the opcode `Awaiting(action_id)`. A reply matching that action ID commits the result and advances. Duplicate replies are rejected; unknown replies are protocol faults. A retry of a host operation after a broken broker channel is forbidden, especially for writes. Report an indeterminate-effect failure if the channel breaks after submission but before its reply.

### 16.5 Error containment

Convert all guest indexing, allocation, arithmetic, lookup, and shape failures into typed runtime outcomes. Host `.unwrap()`, indexing without prior validation, `unreachable!()`, and `panic!()` are forbidden on data influenced by source, guest values, or protocol input. Test-only assertions are permitted. Internal invariant violations produce `LRR-E-INTERNAL-*` and terminate the run, preserving a bounded diagnostic dump.

Do not catch managed guest panics using Rust `catch_unwind`. The generated native bridge catches native Rust panics inside its own compilation artifact as specified in Section 40; this does not change the managed panic machine. Guest panics never use host unwinding. Do not call guest code from within a host destructor. Do not retain a host mutable arena reference while invoking Resolver, a facade callback, the broker, or another VM step.

### 16.6 Interpreter-only mode is a testable product mode

`--jit=off` runs this exact interpreter and uses the same caches, facade, permissions, and semantics. It is not a separate implementation. Its correctness tests are prerequisites for JIT tasks. The release still requires a functioning native tier; interpreter-only mode exists for diagnosis, startup-sensitive runs, and cross-tier comparison, not as an incomplete release escape hatch.

### 16.7 Resumable aggregate operations

A long clone, formatting, collect/count, array construction, or element cleanup operation owns an `OpActivation` in its frame: bound-op identity, phase, charged-on-entry bit, current element/byte index, construction guard, and pending cleanup continuation. Process at most 256 elements or 64 KiB of bytes before returning Dispatch to the engine. Check per-element fuel and cancellation inside that chunk. The bound PC remains at the same opcode until the operation completes; its initial opcode fuel charge is not repeated on resume.

Partial progress is canonical activation state, not a native SSA temporary. Completed element transfers are never repeated. On final completion, install the result, clear the activation, advance the PC, and emit one successful-op observation. An interrupted partial operation has no successful completion observation and is not speculatively recorded as if it finished. A native generic helper may resume an existing activation; a scalar staging guard rejects one.

If processing an element requires a user Drop, advance the activation to its post-element continuation before scheduling the ordinary cleanup machine. Resume that saved continuation after guest Drop returns. Do not call guest destructors recursively from a host collection loop. On panic, construction guards and owned partial results join ordinary cleanup; on fatal fault, physical teardown reclaims them without more guest code.

<a id="section-17"></a>

## 17. Closed standard-library facade

### 17.1 Identity and resolution

Create synthetic crates `core`, `alloc`, and `std` with stable facade IDs and exports. They are not parsed from the Rust toolchain's source and not linked as native guest libraries. Each owning edition selects its prelude and reservation/name-resolution rules in Section 34; async-related additions are recognized as unsupported identities rather than executed or silently omitted. A user item may shadow a prelude name through normal lookup. Qualified `std::...` uses the synthetic crate unless the project manifest illegally aliases another dependency as std, which preparation rejects.

Every facade entry in `compat/facade.toml` records: full path, facade ID, argument/return recipe, generic binding rules, ownership behavior, effect class, fuel cost, error behavior, and fixture IDs. A call to an unlisted path reports `LRR-E-FACADE-MISSING`; never attempt source fallback into the real standard library.

### 17.2 Exact API inventory

The following is the complete initial inventory. Overloads listed in the same cell are separate manifest entries with distinct IDs.

| Family | Supported operations | Contracts |
|---|---|---|
| `Vec<T>` | `new`, `with_capacity`, `len`, `is_empty`, `push`, `pop`, `get`, `get_mut`, `as_slice`, `as_mut_slice`, `iter`, `iter_mut`, `into_iter`, indexing | T must bind at construction. Push consumes T. Pop returns Option<T>. Get returns Option of checked reference. |
| `String` | `new`, `from(&str)`, `len`, `is_empty`, `push(char)`, `push_str(&str)`, `as_str`, `clear`, `chars`, `bytes`, `clone` | Length is UTF-8 bytes. Mutation invalidates outstanding slice leases. Clone allocates/copies. |
| `str` / `&str` | `len`, `is_empty`, `chars`, `bytes`, `to_owned`, `to_string`, `eq`, `starts_with(&str)`, `ends_with(&str)`, range slicing | No regex, Unicode case mapping, split closures, parsing traits, or integer indexing. |
| Slices / arrays | `len`, `is_empty`, `get`, `get_mut`, `iter`, `iter_mut`, indexing, range slicing | usize indices; range end checked; reference access controlled. |
| `Option<T>` | `Some`, `None`, `is_some`, `is_none`, `unwrap`, `expect(&str)`, `as_ref`, `as_mut`, `take` | Unwrap/expect consume; failure guest-panics. Take leaves None. |
| `Result<T,E>` | `Ok`, `Err`, `is_ok`, `is_err`, `unwrap`, `expect(&str)`, `unwrap_err`, `as_ref` | Error formatting on panic uses supported Debug. Result ? may call the selected From conversion for its error; unwrap/expect do not convert errors. |
| Ranges | `a..b`, `a..=b`, `..b`, `a..`, `..` | Iteration only concrete integer bounded ranges. Other forms only slicing. |
| Iterators | next, enumerate, count, collect::<Vec<T>>, named-function map/filter | Iterator/IntoIterator/FromIterator selection is shared by facade and live user implementations; closure-valued adapters remain outside the executable profile. |
| Scalar methods | `wrapping_add`, `wrapping_sub`, `wrapping_mul`, `checked_add`, `checked_sub`, `checked_mul`, `abs` on signed integers; `min`, `max` on integers | Exact concrete same-type contracts. Abs(min) panics. No floating min/max ambiguity. |
| `mem` | `drop`, `replace`, `forget` | Shared ownership/drop machine; no raw native drop. |
| Clone | exact Clone trait selection on primitives, containers, derived or manual implementations | Recursive clone actions or guest method frames; do not replace a manual Clone body with a bit copy. |
| Comparison | primitive and string equality/order; Vec/slice/array/tuple equality; supported derived PartialEq/Eq | Compare left-to-right, short-circuit; equality does not move operands. |
| Debug / Display | primitives, supported containers, derived and manual implementations | Bounded guest Formatter and resumable trait callbacks; no host fmt trait object wrapping guest memory. |
| `std::fs` | `read_to_string(&str)`, `read(&str)`, `write(&str, bytes-or-str)` | Broker mediated. `read` produces Vec<u8>; errors return Result. |
| `std::env` | `var(&str)` | Reads only explicitly injected map; no ambient host environment. |
| IO error | Error::kind; display/debug; fixed ErrorKind variants | No raw OS handles or arbitrary native error objects. |
| Box / PhantomData | Box::new, supported deref/unsizing/drop; PhantomData construction | Managed stable heap cell; nominal zero-size PhantomData still contributes trait parameters. |
| Trait support | compat/traits.toml inventory in Section 54 | Explicit headers, intrinsic or guest bodies, no global implicit trait oracle. |

The standard trait identities in compat/traits.toml participate in ordinary live resolution. User impls of supported Copy, Clone, Drop, Default, Debug, Display, equality/order, Hash, operator, conversion, and iterator traits are supported through Section 54. Compiler-generated derives publish ordinary candidate headers with Rust-compatible bounds; policies are not selected merely from the spelling of an attribute. Primitive/facade implementations may use intrinsic bodies after selection. Same-spelled user traits remain different nominal identities.

For method-looking intrinsic entries, resolution still respects inherent/trait namespace rules. The registry recognizes the exact facade type identity, not any user type with the same spelling. `std::string::String` and prelude String have the same synthetic nominal identity.

### 17.3 Iterator mechanics

`IteratorState` variants are integer range, shared/mutable slice, owning Vec, UTF-8 chars, bytes, enumerate, and count/collect driver state. A range stores concrete integer type, next value, bound, and inclusive/exhausted bit; check inclusive maximum without overflowing when stepping past the final item. An owning Vec iterator owns the buffer; yielded cells transfer ownership out and become moved. A borrowed iterator stores a lease and validates it on each next.

`enumerate` owns its inner iterator and a checked usize counter. `count` consumes an iterator and checks fuel every item. `collect` requires an explicitly known Vec element descriptor, moves each yielded item into a growing Vec, and preserves cleanup if an item fails. These drivers run as resumable VM facade operations, not uninterruptible host loops. User iterators use the same resumable drivers: IteratorStart selects IntoIterator, obtains the associated iterator descriptor, and next selects Iterator::next with Item normalized on demand. Never assume a user iterator is a built-in IteratorState payload.

### 17.4 Formatting

Support format strings with literal text, `{{`/`}}`, positional `{}`, positional `{:?}`, and indexed `{0}` / `{0:?}`. Support implicit captured identifiers such as `{name}` by resolving each named argument once at the reached macro invocation. Reject alignment, width, precision, hexadecimal, scientific, and custom format specifications with `LRR-E-FORMAT-SPEC` when formatting is reached.

Parse a format string into a cached immutable format recipe keyed by its literal contents and macro origin. Evaluate explicitly written arguments left-to-right once. Formatting reads/borrows values and does not move them. Limit nesting to 64 and output to the run output budget. Container formatting checks fuel per element. Cyclic reference display emits `<cycle>` after encountering the same active reference/object identity; it does not recurse indefinitely.

Supported equality on references compares checked referent values under the same finite equality policy, not reference-handle identity. Mutable references are only read for comparison, never copied as owning values. Cyclic referent comparison remains bounded by fuel/depth checks.

Formatting dispatches the exact Display/Debug member on the argument's symbolic view and instantiated descriptor. User fmt bodies run as guest frames with the bounded Formatter facade in Section 54. Unsupported formatting specifications or methods produce attributed reached diagnostics; no native fmt::Arguments representation crosses into LRR. A non-formattable value fails when formatting is required, not when a cold assertion message is indexed.

<a id="section-18"></a>

## 18. Built-in macros, derives, constants, and temporary annotations

### 18.1 Macro pipeline and compiler-known intrinsics

Support stable macro_rules definitions/invocations and function-like, attribute, and derive procedural macros through the hygienic adapter and expansion scheduler in Sections 48–51. There is no user-macro exclusion. A macro may emit ordinary source containing any parseable Rust form; executable support is determined at the generated operation's reached point. Unknown macro names, expansion errors, denied execution, protocol mismatch, and resource failures have distinct diagnostics. Declaration-sensitive failures block the seal they affect; never masquerade as a missing generated function or no matching tests.

Register compiler-known macro identities with explicit syntax, effect, and item-emission summaries. The required intrinsic inventory is assert/assert_eq/assert_ne and debug variants; panic, unreachable, todo, unimplemented; print/println/eprint/eprintln/format/format_args; write/writeln; vec; cfg, stringify, concat; line/column/file/module_path; env/option_env; include/include_str/include_bytes. Their names can be shadowed; optimization requires resolved identity, never spelling. Intrinsic wrappers from synthetic std/core export the correct defining-crate context. Register compile_error as a declaration/expansion diagnostic, not a guest panic.

For runtime-only macros, retain the exact-once and lazy argument behavior of the profile. Assertion messages and Debug formatting are prepared only on failure. Vec repetition evaluates its value once even at zero length; selected Clone behavior is invoked for repetitions rather than unconditionally bit-copying. Printing and writing use the resumable formatting machine; write's destination is evaluated once. Formatting arguments have explicit captured source views, borrow handles, and lifetime scopes, not native fmt::Arguments.

include! takes a compile-time string expression built from the supported literal/concat/env/include string operations; its result is parsed in the invocation's grammar position with the invocation's lexical context. Resolve relative included paths against the containing source file, and OUT_DIR paths against the prepared unit's captured build output. Generated source is published by the coordinator as immutable bytes with original path and expansion provenance. include_str!/include_bytes! use the same authority path but produce values instead of syntax. Source input grants are distinct from runtime filesystem grants. Missing or denied inputs become attributed errors at the first demand that requires them.

env! and option_env! use the captured compile-time environment for the consuming Cargo unit, including Cargo/build-script settings, not the live worker environment. Values and absence are dependencies. They do not grant guest std::env access. concat accepts the Rust-compatible supported compile-time literal forms; nested builtin expansion is explicit, not ordinary VM execution. Source-position macros depend on the current span/origin mapping. Preserve lexical meaning of stringify tokens through the adapter; comparison fixtures normalize only behavior the reference leaves unspecified.

No formatting or built-in recipe may bypass trait member identity. The supported format-string grammar remains explicit in Section 17 and expands through Section 54's required formatter API. A custom format form not yet in the release inventory is a reached attributed error, not a macro-loading error.

### 18.2 Derives and inert helper attributes

Expand compiler-built-in derives through the adapter's pinned built-in derive implementation, then integrate their ordinary impl headers, predicates, associated items, and method-body tokens. Custom derives invoke their prepared proc macro library. Copy/Clone/Debug/Default/PartialEq/Eq/PartialOrd/Ord/Hash are mandatory built-in derives. Do not synthesize impl eligibility by inspecting only concrete field types: derive-generated generic bounds are part of Rust's semantics. Selected methods may use an audited intrinsic body only after matching the expanded header and proving equivalence in fixtures; the initial implementation executes the generated live method body.

A derive may emit free functions, helper types, modules, imports, or impls for types other than its annotated input. Its unresolved output keeps the enclosing scope and possibly the global live impl catalog incomplete. After expansion, integrate all output items at the correct parent position. A trait impl generated in a nested block is still indexed according to impl visibility rules. Preserve derive declaration order and register its declared helper attributes before forwarding the relevant input tokens. An attribute is not globally inert merely because another derive somewhere declared the same helper name.

An error in an unrelated ordinary body remains cold. An expansion error affecting a required scope/impl/test seal is reported with the complete invocation chain. Built-in and custom derive output shares the same source identity, invalidation, method resolution, and runtime rules as handwritten code.

### 18.3 Pure constant evaluator

Implement a separate small evaluator over immutable `ConstValue`, not the live VM. This is necessary because query results cannot contain guest side effects or guest handles. Supported values: concrete primitive scalars, immutable strings/bytes, tuples, and arrays. Supported syntax: literals, primitive unary/binary/casts, block locals, if, loops with finite fuel, return, named constants, and calls to named `const fn` whose reached operations remain in this subset.

The evaluator uses the same primitive arithmetic functions but its own local stack of immutable/mutable local scalar slots. No nominal Drop, reference mutation, facade allocation, IO, panic unwinding, macros with effects, or ordinary non-const function call is permitted. `panic!` encountered during const evaluation returns `LRR-E-CONST-PANIC`. A cold branch containing an effect remains untouched.

`Constant` query key contains DefKey, expected concrete type, cfg, and explicit pure arguments for a const-call memo only when all arguments have stable ConstValue encodings. Limit evaluation to 100,000 steps, 128 frames, and 1 MiB serialized result. Active constant cycles report `LRR-E-CONST-CYCLE`; ordinary repeated recursive const calls with changing arguments are legal until budget/depth limits.

An array length requires a nonnegative usize-compatible constant within the element/allocation limits. A named `const X: T` gives its initializer T context. On each runtime use, instantiate fresh owning guest tuple/array cells from immutable ConstValue; sharing the memo does not share mutable guest state. A const string's bytes may be physically shared read-only, with fresh checked guest leases.

### 18.4 Exact temporary-scope annotation procedure

The body structural index produces `TempScopeId` and `ExtendedTo` annotations without evaluating expressions. For the supported 2024 profile, choose the nearest temporary boundary from: statement, complete function, loop/if/else body, ordinary bool condition or bool condition-chain operand, ordinary bool match guard, lazy boolean operand, if-pattern-condition-plus-consequent scope, while-pattern-condition-plus-iteration scope, match arm body, and a block's entire tail expression. Pattern condition chains have nested clause scopes; their cleanup is specified in Section 34. A match scrutinee uses its enclosing boundary, not the selected arm's scope. Apply syntactic lifetime-extension annotations after computing these default boundaries; an explicit extension wins over the default narrowed scope.

For each let initializer, mark an extending-expression frontier. Start at the initializer; propagate through parentheses, tuple/array element constructors (not repeat operands), braced struct fields, tuple-struct/variant constructor arguments, supported casts, block tails, selected-when-executed if branch tails, and match arm expressions. Do not propagate through ordinary function/method calls, arithmetic, or a closure. A borrow expression on that frontier extends its operand's temporary to the containing block. Propagate that extension down borrow/deref/field/tuple-index bases and index bases, but not index expressions.

An extending pattern is a by-reference binding, or a tuple/struct/array/or-pattern containing an extending direct subpattern. An explicit reference-pattern wrapper is not itself an extending wrapper. An extending let pattern extends the initializer's temporary; then propagate through the base operations just listed. The rule acts on structural annotations even if only one branch later executes; only executed temporaries are allocated and registered.

Source compiler behavior in this area is edition-sensitive. The conformance suite records native Rust 1.99.0 outputs for each owning edition, with paired 2015/2018/2021/2024 cases under Section 34.7. An observed discrepancy must be fixed in this annotation pass; do not compensate with ad hoc lifetime extension inside the reference helper.
<a id="section-19"></a>

## 19. Native ABI and helper contracts

This is the private **JIT-to-LRR helper ABI**, shared by Cranelift and LLVM, and distinct from the Cargo-native binding ABI. The separately versioned C bridge in Section 39 never accepts ValueRepr, VmContext, frame handles, or these internal helper pointers.

### 19.1 Uniform entry signature

All guest native regions have this one host ABI:

```rust
pub type RegionFn = unsafe extern "C" fn(*mut VmContext, u64) -> u32;
// Arguments: pinned host context address, checked guest FrameId bits.
```

Use the platform-native calling convention selected by the pinned JIT module: the Apple ABI on macOS ARM64/macOS x86-64, and System V for the later Linux x86-64 port. Verify the helper/entry ABI with actual native probes on each claimed platform. The second argument is an integer handle, not a host frame pointer. Baseline helpers receive no direct guest-object address. Section 61 admits validated scalar frame views and leased native storage; Section 62 admits typed SSA and reached-call inlining with reconstruction maps. RegionFn still returns a status, not a Rust object, and no native unwind may cross JIT or host frames.

`VmContext` is pinned for the duration of a region call. It contains a private pointer/reference bridge to the currently borrowed engine components and a magic/version word for debug validation. Construct it inside `engine::dispatch` from disjoint mutable borrows, invoke native code synchronously, and destroy it before processing external requests or installing a new snapshot. Its address never enters a guest ValueRepr or public message.

### 19.2 Exit discriminants

Freeze the following ABI numbers and include them in the helper ABI digest:

| Number | Name | Meaning |
|---:|---|---|
| 0 | `Continue` | A helper completed its opcode; native code may execute the next listed opcode. Not a legal final region return. |
| 1 | `Dispatch` | Region finished or reached a control boundary; canonical PC is ready. |
| 2 | `Call` | One sealed pending CallPacket is ready. |
| 3 | `Return` | One pending return/cleanup continuation is ready. |
| 4 | `Effect` | One pending broker request is ready. |
| 5 | `Panic` | Explicit guest panic and cleanup state are ready. |
| 6 | `Fault` | Fatal runtime fault recorded. |
| 7 | `Yield` | Requested cancellation reached a safe boundary; dispatcher ends the run with cancellation status. |
| 8 | `Deopt` | Resume the uncommitted operation named by the reconstructed continuation once; a completed call is never that operation. |
| 9 | `TierTransfer` | Canonical state is reconstructed at an eligible unexecuted PC for dispatcher tier selection; no pending action and no Deopt suppression. |

Before invoking a canonical-state helper, optimized code materializes all state that helper reads or may expose, using a matching exit/safepoint map. It cannot call baseline step/stage/commit on stale canonical slots. A helper that returns Call/Return/Effect/Panic/Fault/Yield must have filled the corresponding canonical VM state. A region must immediately return any nonzero helper result. Returning zero from a region, returning an unknown number, or returning a status inconsistent with pending state is `LRR-E-INTERNAL-JIT-ABI`.

### 19.3 Fixed helper registry

The baseline helper lowering imports the following symbols. Optimizing/direct-call lowering additionally uses the versioned helpers listed below and registered generation-bound native entries; no ambient lookup is permitted. Their ABI signatures use pointer-width integer for the context, I64 for frame/op/region IDs and payloads, I32 for status/selector values, and pointer-width integer for the one trusted scratch pointer.

```text
lrr_region_enter_v1(ctx, frame, region_id) -> status
lrr_step_v1(ctx, frame, op_id) -> status
lrr_stage_scalar_v1(ctx, frame, op_id, scratch_ptr) -> status
lrr_commit_scalar_v1(ctx, frame, op_id, result_bits, outcome) -> status
lrr_region_leave_v1(ctx, frame, region_id) -> status
```

`region_enter` validates current frame/PC and installed semantic epoch, checks pending state, increments actual native-entry telemetry, and does not pre-execute an opcode. `step` invokes the shared interpreter opcode implementation, not the entire interpreter loop. It charges and commits once, then returns the required status. `region_leave` verifies the completed end PC and returns Dispatch.

`stage_scalar` is a read-only fast-path guard. It checks exact concrete primitive descriptors, initialized scalar slots, expected operation, and region assumptions, then copies scalar bits into trusted native stack scratch. It does not materialize unresolved literals, consume/move values, charge fuel, advance PC, allocate guest storage, or produce guest-visible diagnostics. Any guest-data mismatch returns Deopt so the interpreter handles that same opcode with its normal checks and ordering.

`commit_scalar` charges the opcode, validates the output descriptor/range, writes the result or creates the specified arithmetic panic, advances PC only after success, and records completion. Outcome is 0 success, 1 overflow, or 2 divide-by-zero; all other numbers are internal faults. Only arithmetic operations with a proven staging guard may call it. The descriptor and next PC come from trusted bound-op metadata, not a guest-supplied numeric tag.

### 19.3.1 Direct-call and reconstruction helpers

Add `native_region_enter_v2(ctx, frame, region_id, view_out)`, `native_call_prepare_v2(ctx, frame, op_id, scratch_out)`, `native_call_finish_v2(ctx, frame, op_id, outcome)` and `lrr_deopt_v2(ctx, map_id, values_ptr)`, each returning the existing u32 status. Section 61 defines direct-call transitions. Section 62 defines bounded reconstruction maps and typed spill schemas for deopt. Pointers refer only to compiler-sized/aligned trusted storage, never guest integers. Freeze all field offsets and effect classifications in helper ABI version 2 golden tests. Add the closed helper `lrr_tier_transfer_v2(ctx, map_id, values_ptr)` with the same validated spill-storage contract as lrr_deopt_v2, returning TierTransfer only after successful reconstruction with no pending action. Include discriminant 9, entry/reconstruction map schemas and this helper signature in the ABI2 digest; old artifacts with a different digest are cache misses, never executable compatibility guesses.

`lrr_deopt_v2` validates the map against its active artifact and materializes the exact exit state, including ownership and executed-prefix accounting. Allocation/handle requirements must be reserved or represented so a failed reconstruction cannot replay a prior effect. The helper never invokes a native target. Generated code may call only an entry already installed under its full matched producer ABI and current native generation.

### 19.4 Scratch layout and unsafe boundary

```rust
#[repr(C)]
pub struct ScalarArgs {
    pub left: u64,
    pub right: u64,
    pub type_id: u32,
    pub operator: u32,
    pub reserved: u64,
}
```

Assert size 32, alignment 8, and field offsets. Allocate a 32-byte, 8-byte-aligned Cranelift explicit stack slot for it. This is the baseline scalar scratch contract. Direct calls may additionally access the verified frame views and leased native storage in Section 61. Optimized code may retain typed values under Section 62 reconstruction invariants. No path permits unchecked arena indexing or an address derived from guest integer bits.

The scratch pointer is a trusted compiler-generated stack address. The helper must not accept such a pointer from any public API or guest value. Put its unsafe write in one audited function with the precondition: caller is verified generated LRR code, scratch slot is live and correctly aligned for this helper call, and no native reference escapes. Use `write` on the raw pointer only after the context/frame/op have been validated. Record this exact safety argument in the code comment.

The native pointer-to-RegionFn conversion occurs once in `jit::abi` after successful finalization. Check target pointer width and entry alignment. Store the function pointer only inside an artifact owned by its live module arena. Do not cast arbitrary bytes or a guest function-item handle to RegionFn.

### 19.5 Guard miss and exactly-once execution

On Deopt, the dispatcher sets `suppress_native_once = (frame, current_pc, current_op_index)`, then executes the authoritative interpreter opcode once. Clear suppression after that opcode completes or produces a terminal action. Section 62.5 additionally prevents persistent guard-miss bouncing. After Deopt, do not re-enter the same region before executing the missed operation, and do not restart at the region entry. A TierTransfer exit is distinct from Deopt: it reconstructs the same unexecuted PC for tier selection without setting suppress_native_once, charging an opcode, or manufacturing a completion observation.

In baseline helper lowering, every completed opcode commits canonical values, ownership, fuel and continuation. Optimized lowering may retain physical state in registers, with a complete reconstruction map at every required exit. Logical effects, transfers and fault order remain identical. A guard before an operation resumes that operation; a guard after a native call resumes its post-call continuation and never retries the call. Canonical-per-op stores are not an optimizing-tier requirement.

A native stage guard can inspect values before fuel is charged because staging is pure. If fuel is exhausted, commit reports Fault with the same resource policy as the interpreter before changing guest state. A guard miss consumes no fuel; the one interpreter execution performs the one charge. Dedicated tests compare exact fuel usage across tiers in precise; throughput omits these fuel checks/charges and reports null under Section 16.3. Unqualified runtime fuel requirements elsewhere in this document apply to precise, while query/const/compiler budgets apply to both contracts.

<a id="section-20"></a>

## 20. Observation, compilation units, and promotion

### 20.1 Observation records

Record only actual completed operations and control edges with body/PC/op identities, exact method/witness identity, concrete representation classes, boundary/effect kind, and source/native generation dependencies. Records contain no guest payloads or process addresses. Native execution contributes actual completed edges/traversals as well as interpreter execution; Section 62.8 defines bounded counter publication. Compilation, cache installation, entry attempts and reconstructed/replayed metadata are not completed executions. A successful observation authorizes compiling that reached operation, not assuming future calls are pure or cannot panic. Unentered live paths remain interpreter side exits.

### 20.2 Baseline and optimizing units

The first Cranelift tier may use straight-line regions of at most 64 completed operations, including synthetic recipe exits. It stops at unresolved demands, uninlined guest calls, returns, effects, panic/fault, cleanup and yield. Prepared synchronous native calls may be explicit NativeCallOp operations inside a region under Section 61, with all required failure paths. Empty/failed observations are not compiled.

The optimizing tiers compile reached hot graphs with native loop backedges and observed branches. Section 62 bounds code and compiler work rather than forcing a dispatcher transition every iteration. A new/unprepared branch exits at that branch with a complete map. Reached guest callees may be inlined only with source-stack, ownership/drop, frame-limit and invalidation maps. Uninlined recursion remains on explicit VM frames. No compilation walk may demand cold executable semantics to enlarge a unit.

### 20.3 Tier selection and bounded specialization

`--jit=off` performs no guest JIT compilation or code-cache execution. `--jit=auto` first makes a baseline candidate eligible after 32 completed interpreter traversals, then selects optimizing work by the cost policy in Section 62. `--jit=force` makes the selected tier eligible after one genuine observation; it never replays a one-shot path to obtain native entries. `--jit-tier=auto|cranelift|llvm` selects eligible backends and is independent of execution contract. Force with an explicit tier must report an unavailable/failed required backend rather than quietly substitute another one. Auto may continue in a proven lower tier and must report the reason.

Permit one baseline artifact and at most four optimized concrete variants per unit. Variant keys include concrete types, selected members/witnesses, native entries/generation, source dependencies and execution contract. A fifth unrepresented shape uses the checked fallback; bounded eviction/replacement follows safe code-lifetime rules. Guard failures do not invalidate valid source or replay committed operations.

At most 1,024 candidates are queued, with deterministic count/digest ordering for equal estimated benefit. First-tier construction is bounded synchronous work at a safe boundary. Optimizing jobs run on immutable inputs in one compiler worker; at most one job executes and one additional prepared job is pending. Stale completion is discarded before installation. Resource limits and deterministic force-mode tests are specified in Section 62.

### 20.4 Native execution evidence

Helper-only Cranelift code remains a valid baseline artifact and must be labeled separately from typed direct operations, optimized loops, Rust ABI calls and LLVM/native-IR inlining. Report actual backend entries and calls, not emitted code as execution. Tests deliberately repeat work when they require native entry; one-shot force executions may legitimately have zero entries.

<a id="section-21"></a>

## 21. Cranelift lowering and closed symbol resolution

### 21.1 Small mandatory upstream patch

The pinned JITBuilder normally falls back to platform symbol lookup, and adding a callback that returns None does not remove earlier lookup callbacks.[^R11] Disable that behavior explicitly rather than relying on guest code not finding it.

Vendor only `cranelift-jit` 0.136.2 under `vendor/cranelift-jit`. Preserve its package version, original source files, license notices, and a recorded archive checksum in `vendor/UPSTREAM.toml`. Add this public method to its `impl JITBuilder` in `src/backend.rs`:

```rust
/// Disable all fallback symbol lookup; explicit symbols remain available.
pub fn clear_symbol_lookup_functions(&mut self) -> &mut Self {
    self.lookup_symbols.clear();
    self
}
```

This is the only allowed semantic upstream patch. Add the path override:

```toml
[patch.crates-io]
cranelift-jit = { path = "vendor/cranelift-jit" }
```

Run this method immediately after builder construction and before registering helpers. Do not add any subsequent lookup callback. `xtask vendor-audit` compares the vendored tree with its recorded source archive and permits only the method addition, its test, and necessary nonsemantic local metadata. The runtime does not depend on a separately maintained Cranelift fork.

The helper symbol table contains the exact versioned registry in Section 19.3. Native generation entries are a separate typed registry under Section 61; imported prepared IR uses the closed references allowed by Section 62. Do not register libc, allocator, filesystem, process, random, clock, Rust std symbols, or guest-exported names. Imported names must come from HelperId, NativeEntryId or an admitted prepared-IR reference, never source-string concatenation or ambient symbol lookup.

If the backend inserts an unregistered libcall, compilation/finalization fails closed. Mark the region compile attempt failed and keep executing its interpreter form in auto mode; force mode turns the compile failure into an explicit test failure. Do not restore ambient lookup to make a libcall resolve. The chosen initial fast-path subset avoids operations requiring floating-point or large-stack libcalls.

### 21.2 Builder settings

Create `JITBuilder::with_flags` with `opt_level="speed"` and `enable_verifier="true"`, plus the pinned module's default libcall-name translator. Preserve the constructor's target-specific relocation/PIC settings; do not overwrite them from an old blog example. The inspected 0.136.2 constructor enables x86-64 PIC for its relocation scheme.[^R12] Clear fallback lookup, register the exact helpers, then create JITModule.

Use the module's target pointer type and default C calling convention for RegionFn and C helper signatures only. Rust ABI native entries and catch adapters use their exported MachineCallAbi, including convention, hidden arguments and component rules; never substitute the C default. Guest frame/op IDs remain I64 even though they are not pointers. Use I32 for all statuses. Helper declarations use `Linkage::Import`; region definitions use local linkage and opaque generated names such as `lrr_region_17`. Source names appear only in diagnostic metadata.

### 21.3 Compile algorithm

Steps 2–7 below describe baseline helper lowering. Optimizing lowering replaces that sequence with the verified typed graph, native backedges and exit maps in Section 62; it retains the entry ABI, closed imports, verification and publication requirements.

1. Confirm the baseline RegionRecipe has 1–64 successful observed operations, or verify the optimizing typed graph and limits in Section 62. All semantic inputs must remain valid for the active snapshot.
2. Create `RegionPlan` with per-op generic or eligible guarded-scalar lowering and a complete dependency fingerprint. Pin every referenced BoundOp, descriptor, and recipe for artifact lifetime.
3. Declare the region function with the uniform ABI. Create Cranelift Context and FunctionBuilderContext owned by the compiler; clear/reuse them after each completed compilation.
4. Create an entry block, append function parameters, and emit region_enter. Branch to a return block on any nonzero status.
5. For each generic op, emit step(ctx, frame, op_id). If status is Continue, continue to the next op. Otherwise return that status immediately.
6. For each eligible scalar op, emit stage_scalar with the explicit stack slot. On nonzero status return. Load the staged scalar bits, compute the whitelisted native operation, and emit commit_scalar with result/outcome. On nonzero status return.
7. Emit region_leave after the final successful op. Return its Dispatch status. A terminator may already have returned Call/Effect/etc.; make CFG predecessors explicit and seal every block.
8. Run Cranelift's verifier and the local import/ABI/memory/exit-map audit. Every imported or indirect call matches a closed helper or generation-bound entry signature. Memory access is limited to typed compiler scratch, admitted frame views and leased native storage, or other checked managed access proved by the optimized IR. Reject guest-derived unchecked pointers and missing exit maps before publication.
9. Define the function, finalize the module definitions, retrieve the finalized function address, and convert it in the single audited ABI function.
10. Publish the artifact to the entry table only after all steps succeed. Record code size, compilation wall time, op count, helper count, scalar count, and content keys. A failure publishes no callable address.

Cranelift JITModule requires finalized referenced definitions and imposes explicit code-memory lifetime requirements; the module arena is designed around those constraints.[^R06]

### 21.4 Whitelisted direct scalar lowering

The required native fast paths are deliberately finite: i32/i64/u32/u64 `+`, `-`, bitwise `& | ^`, integer comparisons, bool `!`, and bool equality/inequality. isize/usize reuse 64-bit machine operations but guards retain their distinct guest TypeIds. In baseline helper lowering, multiplication, division, remainder, shifts, floating-point operations, casts, aggregate operations and reference operations use step. Optimizing tiers may lower those reached operations directly once their typed semantic, fault and reconstruction rules have acceptance; this baseline whitelist is not a release-wide ban on optimized operations.

For unsigned add, compute result and detect overflow by checking whether the result is less than the left operand, additionally checking the width bound for u32. For unsigned subtraction, overflow means left is less than right. For signed 32-bit add/sub, compute in I64 and compare against the signed 32-bit min/max. For signed 64-bit add, overflow occurs when the operands have the same sign and the result's sign differs. For signed 64-bit subtraction, overflow occurs when the operands' signs differ and the result's sign differs from the left operand. Implement these with Cranelift integer operations/compares, not host undefined signed arithmetic.

On overflow call commit_scalar with outcome 1, not an unchecked wrapped result. The helper produces the same guest panic origin and fuel charge as the interpreter. Comparisons use signed or unsigned condition codes according to the guarded type and commit a bool value. Bitwise outputs are normalized to the guest width/sign representation before commit.

Every fast-path operation must have boundary fixtures: zero, one, minimum, maximum, negative one where valid, equal values, opposite signs, and overflow. Property tests compare thousands of random bit-pattern pairs to the shared primitive helper. No fast path may be enabled solely because its happy-path sample works.

### 21.5 No native speculative operations

The optimizer uses speed settings, but correctness is established by the input IR and verifier. Preserve fault/overflow/effect/ownership order; guards may be hoisted or removed only when a proof remains valid across the interval. Precise fuel batching must preserve exact low-fuel and early-fault prefixes. Section 62 permits native loops, typed SSA, scalar replacement and admitted inlining under reconstruction maps; none authorize speculative guest effects, raw managed aliases or cold body checking.

### 21.6 Compile failure policy

Classify failures as resource limit, known unsupported backend operation, ABI/import audit failure, or internal codegen failure. A known resource limit stops promotion for that run and continues interpretation in auto mode. An ABI/import audit failure invalidates that region and reports an internal JIT defect; release tests must fail. An internal codegen failure records the CLIF dump and bounded diagnostics, disables that artifact for its recipe key, and continues interpretation only in auto mode with `jit_degraded=true` in the result.

Force-mode fixture runs fail on any attempted compilation failure. Release gates require zero degraded fixtures. Do not hide failures by reporting the interpreter fallback as a successful JIT execution.

<a id="section-22"></a>

## 22. Artifact cache, edits, and code-memory lifetime

This section covers worker-local Cranelift/LLVM artifacts and validated relocatable cache records. Hybrid workers remain fresh per run and transfer no executable pointers or guest state. They may rebind admitted cached object relocations to the current worker and exact native generation under Section 62. Portable query checkpoints remain a separate format.

### 22.1 Artifact key

```rust
pub struct ArtifactKey {
    pub region: Digest,
    pub body: BodyKey,
    pub semantic_dependencies: Digest,
    pub selected_scalar_guards: Digest,
    pub helper_abi: Digest,
    pub value_layout: Digest,
    pub guest_profile: Digest,
    pub host_isa: Digest,
    pub backend_build_and_settings: Digest,
    pub concrete_variant: Digest,
    pub native_generation_and_ir: Digest,
    pub execution_contract: Digest,
    pub reconstruction_schema: Digest,
}
```

Do not key every artifact by the entire snapshot digest: that would throw away unrelated reuse. Do not key only by source text: signature, method-universe, cfg, facade, and ABI changes can alter meaning. The semantic dependency digest includes both positive and negative facts used in bound recipes. A region containing a dynamic call dispatcher does not depend on the callee's body unless an operation actually embedded such a body-derived fact; any inlined callee/body/constant fact is an explicit positive dependency, and selected method/absence assumptions are recorded too.

Literal addresses, guest handles, permissions, and actual input values are not in the artifact. Permissions are checked on every effect. Origin mappings are reattached to the active snapshot; `line!` and related values use active source-map helpers rather than stale embedded positions.

### 22.2 Installation across runs

At a new run boundary, clear the active-PC-to-native-entry table but retain artifact storage. When a reached PC requests installation, revalidate its recipe dependencies through the current query revision. Obtain its currently valid region/semantic key and look up a matching artifact. On hit, install an entry labeled with the active semantic epoch; on miss, run the interpreter and accumulate a new candidate. Do not label old artifacts valid merely because the body bytes match.

A body edit invalidates every region owned by that BodyKey, even when only a cold branch inside the same function changed. Invalidation is body-granular; execution/preparation is path-lazy. This deliberately avoids a second expression-diff identity system. The cold-body reuse guarantee concerns a different unchanged reached body, not unaffected expressions within an edited body.

A method body edit preserves non-inlined callers whose dispatch target/signature are unchanged; it invalidates callers that incorporated that body or derived facts, and the edited body's own BodyKey changes. Adding a competing impl changes the relevant candidate-set fingerprint and invalidates affected dispatch results. Adding a previously missing name invalidates a negative lookup. Editing a comment before a function changes its displayed line locations but not its body key when the function bytes remain identical.

The first reached execution of a previously cached region in a new snapshot must therefore validate/install once. Hot entries in the same run do not query the database again.

### 22.3 Arena policy

A code arena owns its Cranelift JITModule or LLVM object-linking resources, artifact/reconstruction metadata and accounting. Seal it after 128 regions or 4 MiB of emitted native code, whichever comes first. Create another arena for later compilations. Total native code budget is 64 MiB per worker; include allocation slack in process RSS monitoring because emitted byte count is not exact mapped memory size.

Between runs, evict sealed arenas in least-recently-entered order until within budget. Before freeing an arena: clear every native entry referencing it, remove its artifacts from the lookup map, drop all stored RegionFn values, assert no active native invocation, and then release executable memory through the backend-specific audited path (`JITModule::free_memory` for Cranelift, the pinned LLVM linker resource disposal for LLVM). Release its metadata pins afterward. Code is never freed during a guest run, destructor activation, or helper call. An open arena may be sealed at a run boundary to make it evictable.

If promotion would exceed the code budget during a run, stop promoting and retain valid installed lower-tier code; work without an installed entry uses the interpreter in auto mode. A fixture requiring a forced tier must report missing required compilation/entry as failure. Do not evict executing code or restart the test. Set `promotion_stopped=code_budget` in telemetry. This is not a semantic error.

### 22.4 Metadata reclamation and worker reset

Installed op and descriptor IDs are append-only and never reassigned while any artifact might refer to them. Memo/parse/LRIR caches may release unpinned entries between runs. Tombstoned IDs remain invalid; they cannot point to a different recipe. Cache accounting includes pinned metadata. If pinned metadata alone exceeds its budget after arena eviction, restart the worker between runs and capture a cold-cache reason. Do not silently retain unbounded metadata to make a reuse benchmark look good.

No source snapshot can be removed while an active frame, a query validation traversal, or a source-map diagnostic still uses it. Keep at most the active and immediately previous complete snapshot plus shared content blobs retained by live cache roots. Garbage-collect unreachable blobs between runs.

### 22.5 Required edit matrix

| Edit | Must change | Must remain reusable |
|---|---|---|
| Called function body | Its BodyKey, thunks, regions | Unchanged callee/caller bodies without body-derived assumptions |
| Uncalled function body | That body's source identity | All unrelated reached semantic recipes and native artifacts |
| Called signature | Signature/boundary plans and dependent region keys | Unrelated type/lookup buckets |
| New competing impl | Relevant impl-universe and dispatch results | Disjoint method/name namespaces |
| New previously missing name | Negative lookup and dependent failure recipe | Unrelated successful lookup facts |
| Struct field schema | Concrete nominal descriptor and dependent operations | Unrelated nominal descriptors |
| Runtime read permission | Run policy only | All semantically valid code; every effect rechecks policy |
| Source line insertion before unchanged function | Source map / positional intrinsic result | Position-independent region code |
| Cargo manifest/lock/config | Preparation ID; execution blocked until prepare | No old project-unit assumption may be reused before new preparation |

Every row gets a two-snapshot fixture with both semantic assertions and exact invalidation/reuse counters. “The second run passed” alone does not establish incremental behavior.
<a id="section-23"></a>

## 23. Permission broker and security boundary

This section describes `managed` execution. In `hybrid-native`, every managed effect still passes through this broker, but native code can bypass it entirely. The run must report `sandbox_strength="none_native"`, require explicit native authorization before dynamic loading, and refuse `--require-hardened-sandbox`. Host-memory guards are not disabled. See Sections 39–42.

### 23.1 Effective security claim

The managed mode provides capability-mediated supported APIs and a separate crash-isolated execution worker. It does **not** provide a hardened sandbox for adversarial source, a compromised native JIT, an exploitable parser, or malicious host dependencies. Native generated code executes in the worker process. A process boundary alone does not stop that process from making syscalls if a runtime/codegen bug gives it arbitrary native control.

Print `sandbox_strength="capability_mediation"` in machine results. `--require-hardened-sandbox` is a recognized flag that fails immediately with `LRR-E-SANDBOX-UNAVAILABLE`; it must not silently run with weaker isolation. Do not add Wasm, seccomp, namespace/container setup, or a general arbitrary FFI plugin system to this release. The closed authorized Cargo bridge is the sole native extension in hybrid mode. Those are not hidden implementation branches.

The reason to build the broker now is architectural: every guest effect already has a serialized capability boundary, so later containment would not require rewriting the language engine. The permission-oriented developer interface is inspired by Deno's documented explicit permission model, but no equivalence to Deno's implementation or security guarantees is claimed.[^R15]

### 23.2 Process model

The coordinator owns source-file authority, manifest access, runtime grants, captured output, and the child process lifecycle. Spawn `lrr-worker` with a cleared environment, a runtime-owned empty working directory, piped stdin/stdout, and separately captured stderr. Do not place source paths, secrets, permission values, or guest arguments in its OS environment. Send injected environment only as structured run data; guest args are unsupported in this release.

The worker receives immutable source bytes and resolved project metadata, not authority to load arbitrary files. Standard library host calls for filesystem/environment/network/process access are forbidden in worker modules except the internal pipe IO needed for protocol operation. `xtask architecture-audit` enforces this textual/module boundary. This audit is an engineering guardrail, not a security proof.

In managed mode, one persistent worker serves one coordinator. Hybrid mode uses a fresh worker per run request as required by Section 41. It executes one test at a time. Bounded baseline installation runs at safe points; optimizing compilation uses one separately supervised compiler worker with frozen inputs and no source-query, macro, guest-execution or build-tool authority. Coordinator IO/monitoring threads use std channels so cancellation and deadlines remain observable while the worker is busy.

### 23.3 Policy shape

```rust
pub struct RunPolicy {
    pub source_root_label: String,
    pub read_mounts: Vec<MountGrant>,
    pub write_mounts: Vec<MountGrant>,
    pub injected_env: BTreeMap<String, String>,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub max_effect_bytes: u32,
}
pub struct MountGrant {
    pub guest_prefix: Vec<String>,
    // Host directory capability exists only in coordinator memory.
    pub broker_mount_id: u32,
}
```

Output capture is a pre-granted, bounded per-run channel, not direct terminal access. Default runtime policy has no filesystem mounts and an empty environment map. Network, subprocess, system clock, OS randomness, shared memory, native loading, and arbitrary file handles have no effect IDs and are unsupported.

CLI `--allow-read PATH` and `--allow-write PATH` grant a directory under the selected project/source root. PATH must be relative, UTF-8, and contain no `..`; its path components also become the guest-visible prefix. `--allow-read .` grants the selected root. No absolute outside-root mount is supported. Grants are additive and independent: a write grant does not imply read authority. `--env KEY=VALUE` injects exactly that key/value; it does not read the host's existing variable of that name.

### 23.4 Filesystem authorization algorithm

Use cap-std directory capabilities rather than joining an authorized path string and calling ambient `std::fs` in the worker. The Dir API provides the directory-relative operations used at this boundary.[^R14]

At coordinator startup, open each approved directory grant with explicit ambient authority once. Resolve the user-provided grant beneath the selected source root and reject an escaping symlink. Store the resulting Dir in the broker table. For a guest request:

1. Validate run ID, action ID, effect ID, byte limits, and UTF-8 path.
2. Reject NUL, absolute paths, platform prefixes, and every `..` component. Normalize repeated separators and `.` components only.
3. Match component prefixes, not string prefixes. Select the longest matching grant of the required access class; ties use lowest mount ID.
4. Remove the matched prefix and operate relative to the retained directory capability. Never convert back to an unchecked ambient path.
5. For reads, open through that Dir and require the opened object to be a regular file before reading bytes. Enforce the per-effect byte limit while reading, not just from prior metadata. Decode UTF-8 only for read_to_string.
6. For writes, require an existing authorized parent directory, create or truncate the final regular file, and write the bounded byte buffer once. Directory creation, rename, deletion, append, permissions, and executable bits are unsupported.
7. Convert OS errors into the closed guest IO error mapping and append a bounded audit event. Send one reply tagged with the action ID.

A denied supported filesystem operation returns `Err` with `ErrorKind::PermissionDenied`; it is not a guest panic. Guest code may inspect that error. No bytes from an unauthorized file are read before authorization. A read larger than the effect limit returns the profile's `InvalidData` error and records the size-limit reason. A symlink that escapes the directory capability must fail even if its textual path matched the grant prefix.

Use opened-object metadata, not only a path-level precheck. Directory-capability behavior and platform race handling require integration tests. A blocked filesystem operation is handled by the coordinator watchdog, not assumed impossible. Grants are intended for trusted local test fixtures; this release does not promise hostile-filesystem containment.

### 23.5 Environment, output, and replay

`std::env::var` reads the injected map only. A missing key returns the facade's NotPresent error; it never probes the host environment. Record whether the key was present, but redact values from audit/trace logs. Explicit public results can include values only when guest code itself prints or returns them.

Capture stdout and stderr separately and preserve per-channel order. Each output effect is one atomic byte chunk in the event stream. Tests do not share capture buffers. `--nocapture` streams captured chunks to the coordinator's terminal as they arrive while still enforcing the output limit; it does not give the worker a host stdout handle.

Semantic differential tests use a fake broker with scripted replies and a recorded effect transcript. This permits interpreter/native comparison without repeating real writes. Production runs do not replay effects after a crash. Report already confirmed effects plus an `indeterminate_effect` marker for an unacknowledged submitted write.

### 23.6 Revocation and deadlines

A run policy is immutable. Changing flags creates a new policy/run, not a mutable global grant that races with execution. Every request checks the current run's policy even when the native code is reused from a more-permissive prior run.

The coordinator checks wall deadlines every 10 ms. At expiry, request cancellation, then kill the worker after a 100 ms grace period if it has not returned. A broker operation that is itself stuck causes the coordinator to report an indeterminate effect and terminate its process after killing the worker; do not leave an unbounded background IO thread attached to a live service. This is a failure mode, not a guarantee that an already submitted kernel write can be undone.

<a id="section-24"></a>

## 24. Cargo project and compile-time-tool preparation

### 24.1 Three kinds of Cargo work

Use project Cargo metadata for package identity and source discovery. Use the fixed nightly Cargo planner in Section 48 for exact compilation-unit graphs and compile-time-only preparation. Use the project's own Cargo/rustc and original workspace artifact directory for native bridge builds under Sections 35–43. These jobs have separate purpose tags, authorities, and cache directories. Neither metadata nor metadata's package-level feature union is a substitute for the selected unit graph.

Selected live roots support Rust 2015, 2018, 2021 and 2024. Root, dependency source and macro definition editions are retained individually. Accept ordinary stable Cargo workspace graphs, path/crates.io dependencies available offline, normal/dev/build dependencies, procedural-macro units, optional/target-specific dependencies, and feature forwarding. Cargo resolves features; do not implement a second resolver. The release still excludes custom target JSON, unstable manifest features beyond the specifically used planner switches, unavailable sources, arbitrary alternate-registry/source-replacement integrations, and nightly language extensions. Reject these during explicit preparation with the exact offending configuration.

### 24.2 Compile-time-only operation

Procedural-macro libraries and build-script dependencies must be actual compiled host artifacts. Build them with the planner's --compile-time-deps operation, using the recorded project rustc executable and the same selected features, target, and test intent as the live unit. This operation is a documented unstable Cargo interface, so the nightly planner, schema parser, and behavior probe are pinned.[^M01] Do not run cargo check on the selected live crate as a substitute. Do not create fake rmeta/rlib files, tell Cargo that compilation succeeded when no artifact exists, or parse private incremental caches.

Before publishing a preparation, prove with the compile-time probe that a syntactically valid live body containing an unresolved function does not get type-checked, while its build script executes and its proc macro library is built. Track child process roles and Cargo artifact messages. A cached build-script-executed JSON message is a report of available output, not proof the executable ran on this invocation. Failure of the pinned planner/compiler combination is LRR-E-TOOLCHAIN-CAPABILITY; no normal-build fallback is permitted.

### 24.3 Captured build outputs and compile environment

Publish ToolUnit and BuildOutput records keyed by exact unit identity: package, target kind/name, host/target role, cfg/test/features, profile, toolchain, and dependency edges. Capture cfg values, rustc-env, OUT_DIR contents, dependency metadata, source/include inputs, external declared environment/files, and output hashes. Namespace OUT_DIR per unit. Run-generated includes use these snapshots, not arbitrary current files from target directories. Record link directives for native preparation; they do not authorize managed FFI.

Stable output files generated earlier by an external generator may be included if explicitly declared as source inputs. LRR does not need to know how such files were produced, but it must freeze and fingerprint the actual bytes and directory membership it uses. A command to regenerate them is an explicitly authorized preparation action, never a hidden test-run action. This release's automatic command runner is Cargo build scripts only; an externally regenerated file is recaptured by prepare.

### 24.4 Authority and freshness

Without --allow-build-tools, a project requiring actual build/proc artifacts fails explicit preparation, not silently downgrades to a macro-free interpretation. --allow-build-tools authorizes native compile-time programs; --allow-macro-exec authorizes use of prepared procedural-macro libraries during expansion. Both are separate from managed runtime read/write grants and --allow-native. Cargo --offline suppresses Cargo fetching; arbitrary authorized build/macro programs still have ambient operating-system authority.

A prepared record becomes stale when any recorded relevant input, toolchain, dependency/configuration unit, tool artifact, or generated output changes. Check this before run and again before publication if preparation overlapped source edits. Cargo build scripts with coarse/default input tracking can make an ordinary source-body edit require preparation; report that fact instead of promising zero builds on every edit. Missing undeclared external input tracking prevents a hermeticity claim. The security/freshness report says recorded_inputs, never hermetic.

### 24.5 No-build live loop

lrr prepare captures a valid selected unit and its tool outputs. lrr test/run/watch then uses only source snapshots, existing macro libraries, the interpreter/JIT, and any explicitly authorized native bridge. New macro invocation inputs can trigger expansion but not compilation. Edits to macro implementation code or build inputs require another explicit prepare. A native API or generic instantiation miss similarly requires native preparation under Section 37 and never restarts a partially executed test.

Detailed command construction, environment precedence, graph validation, macro-server compatibility, and artifact evidence are normative in Section 48. New CLI flags and result fields are normative in Section 56. Legacy preparation manifests without those fields are rejected rather than inferred.

<a id="section-25"></a>

## 25. CLI, test runner, and agent service

### 25.1 Command surface

The commands below remain the managed defaults. Section 42 adds the exact native init/prepare/test flags and the two additional service methods. `--jit` selects a tier policy, not a security mode.

Implement the following grammar with clap. Reject unknown flags rather than silently passing them to Cargo or rustc.

```text
lrr run FILE.rs [--jit auto|off|force]
lrr test FILE.rs [FILTER] [--exact] [--ignored] [--include-ignored]
lrr run --prepared PREPARE_ID
lrr test --prepared PREPARE_ID [FILTER] [--exact]
lrr prepare --manifest-path PATH --package NAME --target-kind TARGET [feature flags] [--allow-build-tools]
lrr watch FILE.rs [--test] [FILTER]
lrr serve --stdio
lrr explain DIAGNOSTIC_CODE
lrr doctor
```

Common run/test/watch flags additionally include --allow-macro-exec and the execution/native fields in Section 42. The complete list also includes `--jit`, `--allow-read`, `--allow-write`, `--env`, `--fuel`, `--timeout-ms`, `--memory-mib`, `--output-limit-kib`, `--json`, `--nocapture`, `--trace-queries PATH`, `--diagnostic-dir PATH`, and `--require-hardened-sandbox`. Only the bounded resource overrides in Section 26 are accepted. Guest command-line arguments and std::env::args are outside this release: reject any nonempty trailing argument list or service args field with `LRR-E-ARGS-UNSUPPORTED`; do not advertise guest argument parsing. Main must take no parameters and return unit or a supported Result<(), E>, handled like the corresponding test return without test attributes.

`cargo-lrr` strips exactly the initial `lrr` argument supplied by Cargo subcommand invocation, then invokes the same CLI parser in-process. It does not shell out to a different runtime binary. Cargo-style package/target selection is handled through explicit prepared manifests, not by mimicking every cargo test flag.

All run/test/watch/service requests additionally carry --jit-tier=auto|cranelift|llvm (default auto) and --execution-contract=precise|throughput (default precise). These select compilation eligibility and metering independently from --execution=managed|hybrid-native and its authority. The throughput contract rejects an explicit numeric --fuel and reports null fuel; unavailable forced backends fail explicitly. Section 62 defines their implementation and acceptance.

### 25.2 Test discovery

First request TestCatalogSeal for the selected target/configuration. Expand declaration-producing macros and includes to a fixed point, apply cfg/attributes, and then enumerate final #[test] functions, including renamed or newly generated tests. No test body is executable-lowered at discovery. Each entry records canonical name, stable generated DefKey, expansion ancestry, ignore/should-panic metadata, and capability/diagnostic status. Unknown expansion is never reported as an empty catalog. Sort canonical names deterministically; duplicate test identities are errors. Attribute-generated async wrappers remain visible and, when selected, fail any unsupported reached operation rather than disappearing.

A test must have zero runtime parameters and no unbound generic parameters. Its supported result is unit or `Result<(), E>` where E has supported Debug. The runner inspects only a selected test's signature when about to execute it. Invalid signatures in unselected tests remain discovery metadata, not a reason to fail a different selected test.

A bare filter is a substring match; `--exact` is equality. `--ignored` selects only ignored tests; `--include-ignored` selects both; using both flags is a CLI error. Default ignores ignored tests. Zero selected tests is exit 5 with `LRR-E-TEST-NO-MATCH`, not silent success. `#[should_panic]` with no argument accepts ordinary guest panic; `expected="text"` requires a substring in the panic message. Unsupported attribute syntax fails only when selecting that test.

### 25.3 Per-test execution

Before each test: reset guest arenas/counters/output/permissions, increment run epoch, retain valid source/query/native caches, install root signature, and call with no arguments. After normal completion and cleanup, unit or Ok(()) passes; Err(E) fails with bounded Debug output. Ordinary guest panic passes only under matching should_panic. Profile faults, resource faults, double panic, codegen failures in force mode, and worker crashes always fail.

Tests run serially. Do not run multiple tests in one guest heap or persist managed guest statics. Managed source/Cranelift caches may be warm; managed guest state may not. In hybrid mode, native process statics can persist across tests inside one request, but a fresh process resets them for the next request; native contexts and managed arenas are fresh per test (Section 41). This distinction must appear in benchmark reports.

### 25.4 Result schema

Emit one JSON object per test, plus one final summary object. The following is a common-field excerpt; actual results also require Sections 42.4/56.6 fields for their applicable modes:

```json
{
  "schema_version": 3,
  "event": "test_result",
  "test": "tests::sum",
  "status": "passed",
  "profile": "lrr-2024-v2",
  "rust_validation": "live_not_performed",
  "sandbox_strength": "capability_mediation",
  "macro_execution": "none",
  "trait_resolution": "demand",
  "snapshot": "<digest>",
  "run_id": 1,
  "jit_mode": "auto",
  "jit_degraded": false,
  "native_entries": 0,
  "native_scalar_ops": 0,
  "native_helper_ops": 0,
  "interpreted_ops": 0,
  "fuel_used": 0,
  "stdout": "",
  "stderr": "",
  "stdout_bytes_total": 0,
  "stderr_bytes_total": 0,
  "output_truncated": false,
  "diagnostic": null,
  "timings_ns": {},
  "query_counts": {},
  "artifact_counts": {}
}
```

The stdout/stderr fields contain previews of at most 64 KiB each, cut at UTF-8 boundaries; also include stdout_bytes_total, stderr_bytes_total, and output_truncated. Full captured text is delivered in ordered output events of at most 64 KiB raw UTF-8 each, with channel, action_id, chunk_index, and data. This keeps the final JSON result within the frame limit even when the output quota is larger. In nonstreaming mode, emit those events before the final result; in streaming mode emit them as captured. The fixture harness reconstructs full output from events and never treats a truncated preview as the complete transcript.

The zero values above are schema illustrations, not expected telemetry for real tests. All counters come from actual execution hooks. Timings distinguish capture, parse/outline, query validation, semantic preparation, interpretation, native execution, native compilation, broker wait, cleanup, and total wall time. Nested timings may overlap only if explicitly labeled; do not sum them into a misleading total.

### 25.5 External stdio service

Use newline-delimited JSON on public stdin/stdout. Each request has schema_version=3, request_id, method, and params; unknown fields fail validation. Supported methods are `open`, `apply_changes`, `list_tests`, `run`, `cancel`, `stats`, and `shutdown`. Public messages never carry host pointers, guest handle IDs intended for execution, arbitrary native bytes, or a callable ABI. Hybrid requests name only a previously prepared native generation; its callable table is internal.

- `open` selects standalone source root or a prepared project and captures a snapshot. Return snapshot ID, selected targets, and test_catalog_status=unresolved unless a current complete catalog already exists; do not execute macros under an implicit permission.
- `apply_changes` supplies base_snapshot and a batch of `{path, text}` replacements/deletions (text=null deletes), with path relative to that snapshot root. Validate all paths/size limits first, then publish the batch atomically. It changes the service's in-memory source overlay, not disk files. A stale base fails with current snapshot ID.
- `list_tests` returns final test metadata after discovery for a named snapshot. It accepts allow_macro_exec explicitly and may run authorized expansion in its own epoch; it never executes a test body. A later run revalidates its own epoch and does not inherit authority implicitly.
- `run` names snapshot, selected tests/main, jit, jit_tier, execution_contract, and policy. Return accepted, then ordered progress/result events. Only one active run is allowed; queue at most two newer requests and coalesce only requests explicitly marked replace_pending=true.
- `cancel` targets a run/request ID and requests interruption; it does not silently cancel a different run.
- `stats` returns real counters and current cache sizes without guest execution.
- `shutdown` cancels active execution, performs bounded worker termination, and exits.

The coordinator must continue reading cancel/shutdown while the worker executes. Use one reader thread and a main event loop receiving std channels. Every public response includes the originating request ID. Do not interleave human log text on protocol stdout; use stderr for bounded runtime logs.

### 25.6 Internal worker wire protocol

Public service JSON remains schema version 3. LRR-owned internal framing is version 4, independently negotiated by the coordinator, execution worker and compiler subprocess. It replaces the previous length-prefixed JSON-only bulk transport. The pinned external procedural-macro server protocol is unchanged; never send these frames to that server or infer macro compatibility from internal version 4.

Each internal frame has this exact 32-byte header, followed by payload bytes. Encode/decode fields explicitly; never transmute a Rust struct or trust peer alignment. The total header plus payload is at most 16 MiB.

| Offset | Field | Required value/meaning |
|---:|---|---|
| 0 | magic, 4 bytes | ASCII LRR4 |
| 4 | version, u16 little-endian | 4 |
| 6 | kind, u16 little-endian | 1 = control JSON; 2 = raw data |
| 8 | payload_length, u32 little-endian | At most 16 MiB minus 32; check before allocation |
| 12 | reserved, u32 little-endian | Zero |
| 16 | transfer_id, u64 little-endian | Zero for control; nonzero admitted transfer for raw data |
| 24 | sequence, u64 little-endian | Consecutive per direction for control; consecutive per transfer for data, starting at zero |

Unknown kind/version, nonzero reserved fields, integer overflow, truncation and impossible lengths close the connection without retry or downgrade. Sequence/transfer IDs use checked arithmetic and never wrap or reuse within a connection. The first frame is a control handshake with framing version, profile/helper digests, worker build, role and host target. Reject incompatible or legacy peers before snapshot publication, effect execution, compiler-job admission or native loading.

Small control records remain bounded UTF-8 JSON with deny_unknown_fields and explicit tagged message schemas. Control kinds include handshake, transfer begin/commit/abort, run, cancel, effect request/reply metadata, telemetry, result and shutdown. Binary bytes, source chunks, checkpoint payloads, compiler jobs/IR/object bundles and bulk output are raw data frames; they are never hex/base64, JSON byte arrays or escaped JSON strings on this internal bulk path. This does not change public UTF-8 JSON output events or the upstream macro protocol.

A begin record binds transfer_id to role/kind, run_id, action_id or job/snapshot identity, declared total byte length, content digest and the applicable quota/authority. The receiver admits it before accepting data or reserving its declared storage. Each raw data payload is at most 1 MiB. A commit names the same transfer and digest; require exact accumulated length, consecutive chunks, correct digest and valid type/schema before publication. Empty transfers use begin/commit with no data frames. Duplicate, unknown, stale, cross-run, aborted, mismatched or out-of-order transfers fail closed; disconnect/cancellation discards incomplete transfers without replaying an effect.

Snapshot sources are UTF-8 validated on complete reassembly before parsing; a binary chunk may split a UTF-8 code point. Compiler objects and effect buffers are arbitrary bytes. Authorize a write request before admitting its data, and perform no write or expose read bytes until the relevant complete transfer is validated. Snapshot/query/compiler publication remains transactional. Chunking is a transport operation, not an observable extra guest write.

Bound each channel to 64 queued frames and 32 MiB of queued encoded bytes including headers, whichever fills first. Charge received buffers, reservations and reassembly against the corresponding total snapshot/effect/checkpoint/compiler/output limits; draining a queue cannot create unaccounted retained buffers. At most 64 transfers may be active per channel. Preserve backpressure and a dedicated protocol reader that can update cancellation while execution runs; the reader never accesses the VM/query DB. Never share a bulk-data mutex across guest execution or a blocking broker call. The compiler subprocess consumes its own immutable state.

Use owned bounded byte buffers and safe borrowed slices to avoid redundant intermediate copies after validation. This initial required transport uses process pipes/sockets, not shared writable memory. Shared memory is a later optimization only after a separately accepted ownership, sealing/revocation, quota and crash-cleanup contract demonstrates benefit; do not add it implicitly or call this framed transport zero-copy.

### 25.7 Exact service request examples

The following objects define the accepted parameter names; apply deny_unknown_fields to each tagged request type. `source.kind` is file or prepared, not a request to infer a mode from a path extension. All digest strings are 64 lowercase hexadecimal characters. `base_snapshot` must match the coordinator's current overlay base.

```json
{"schema_version":3,"request_id":1,"method":"open","params":{"source":{"kind":"file","path":"/work/demo/main.rs"}}}
{"schema_version":3,"request_id":2,"method":"list_tests","params":{"snapshot":"<snapshot-digest>","allow_macro_exec":false}}
{"schema_version":3,"request_id":3,"method":"run","params":{"snapshot":"<snapshot-digest>","selection":{"kind":"tests","filter":"case","exact":true,"ignored":"exclude"},"jit":"auto","jit_tier":"auto","execution_contract":"precise","policy":{"read":[],"write":[],"env":{}},"limits":{"fuel":10000000,"timeout_ms":10000,"memory_mib":256,"output_limit_kib":4096},"stream_output":false,"replace_pending":false,"allow_macro_exec":false,"execution":"managed","allow_native":false,"native_generation":null}}
{"schema_version":3,"request_id":4,"method":"apply_changes","params":{"base_snapshot":"<snapshot-digest>","changes":[{"path":"main.rs","text":"#[test] fn case() { assert_eq!(1_i64, 1_i64); }"}]}}
{"schema_version":3,"request_id":5,"method":"cancel","params":{"run_id":1}}
{"schema_version":3,"request_id":6,"method":"stats","params":{}}
{"schema_version":3,"request_id":7,"method":"shutdown","params":{}}
```

Angle-bracket digest values above are explanatory placeholders; the actual parser rejects them. A prepared open uses `source={"kind":"prepared","prepare_id":"<digest>"}`. A main selection uses `selection={"kind":"main"}`. Tests selection supports ignored values exclude, only, and include. A change with text=null deletes that overlay file; deleting the selected root is rejected before publication. The inherited fields shown are required in protocol v2, together with the explicit authority/epoch settings in Section 56; the CLI fills defaults explicitly. The accepted response supplies the assigned run_id. Each result/error echoes request_id and the actual snapshot.

An open has implicit user authority to read the selected source root for source capture; this is not a runtime filesystem grant. The stdio service is a local developer-control interface and is not exposed as an unauthenticated network server. Source overlays never modify disk. The explicit native_init_apply extension in Section 42 separately edits only the authorized workspace/generated bridge transaction; it is not an implicit overlay writeback.

### 25.8 Exit codes

Use 0 all selected tests passed / normal main unit return; 1 guest test failure or guest panic; 2 CLI/configuration/preparation error; 3 reached profile/runtime fault; 4 internal runtime/JIT/worker defect; 5 no tests selected; 124 deadline/cancellation termination. For a mixed test batch, choose the highest-priority class in this order: internal defect, deadline, profile fault, guest failure, no-match, success. Config/selection errors prevent the batch from starting.

Do not map every unsupported feature to a compiler error, and do not return 0 after skipping unsupported selected tests.

<a id="section-26"></a>

## 26. Diagnostics and resource limits

### 26.1 Diagnostic structure

Every diagnostic has `code`, `category`, `message`, `primary_origin`, `guest_stack`, `phase`, `snapshot`, `test`, and zero or more structured notes. Categories are configuration, unsupported_profile, execution_contract, memory, guest_panic, permission, resource, protocol, and internal. Phase distinguishes source_capture, outline, query, execution, cleanup, native_compile, and broker.

Use body-relative spans plus the active source map. Render one primary line/column range, source excerpt, actual/expected type names where relevant, and an actionable next step. For a missing context diagnostic, suggest a literal suffix or explicit annotation. For an unsupported user macro, name the macro and source invocation. Do not claim source is “invalid Rust” merely because LRR does not support it.

`lrr explain CODE` reads checked-in `compat/diagnostics.toml`, not a network service. Each entry names the triggering invariant, whether effects may already have occurred, whether cleanup runs, and at least one minimal example. Tests require every emitted code to exist in the manifest.

### 26.2 Fixed default limits

| Resource | Default | User override ceiling | Failure |
|---|---:|---:|---|
| Guest fuel/test in precise | 10,000,000 units | 1,000,000,000 | Runtime resource fault; throughput reports null |
| Guest accounted live memory | 256 MiB | 1,024 MiB | Runtime resource fault before allocation commit |
| Guest cumulative handles/test | 2,000,000 | 8,000,000 | Runtime resource fault |
| Guest call frames | 4,096 | 16,384 | Runtime resource fault |
| Guest cleanup depth/tasks | 65,536 | 262,144 | Runtime resource fault; no further guest cleanup |
| Test wall time | 10,000 ms | 600,000 ms | Coordinator deadline termination |
| Captured output/test | 4 MiB | 64 MiB | Runtime resource fault |
| Single filesystem request/reply bytes | 8 MiB | 64 MiB | Guest IO error / protocol rejection |
| Source file | 8 MiB UTF-8 | Fixed | Source capture error |
| Snapshot source total | 128 MiB | Fixed | Source capture error |
| Called body nodes | 100,000 | Fixed | Reached preparation resource fault |
| Structural slots/body | 200,000 | Fixed | Reached preparation resource fault |
| Query recursion | 256 | Fixed | Query-cycle/depth error |
| Type substitution | 128 | Fixed | Reached descriptor error |
| Const evaluation | 100,000 steps / 128 frames / 1 MiB result | Fixed | Constant resource error |
| Query memo cache | 128 MiB | Fixed | Evict between runs / reset worker |
| Parse cache | 128 MiB | Fixed | Evict between runs / reset worker |
| Installed LRIR metadata | 64 MiB | Fixed | Evict unpinned / reset worker |
| Native emitted code | 64 MiB | Fixed | Stop promotion during run, evict between runs |
| Worker RSS | 2 GiB | Fixed | Kill worker, internal resource report |
| Diagnostic stack | 64 frames rendered | Fixed | Truncate with omitted count |
| Query trace events | 100,000 | Fixed | Truncate trace with explicit flag |
| Formatter depth | 64 | Fixed | Formatting profile error |
| Public JSON message / internal v4 frame | 16 MiB encoded / 16 MiB including 32-byte header | Fixed | Protocol error |

Resource overrides may reduce values or increase them up to the stated ceilings; they cannot disable validity, ownership or memory checks. The separately selected throughput contract omits exact instruction fuel as specified in Section 16.3; an explicit fuel override is incompatible with it. Convert all MiB/KiB using powers of 1024 and check arithmetic overflow while parsing flags. Log requested and effective limits without silently clamping an excessive request.

### 26.3 Failure records and reproducibility

On internal defects, write a bounded artifact bundle only when the user supplied `--diagnostic-dir PATH` or a host test explicitly enables it. The directory is coordinator authority, not a guest write grant. Include source digests, reduced LRIR/CLIF, version hashes, opcode/region IDs, and counters; redact environment values and runtime file contents by default. Do not upload anything.

A reproducible test seed and reduced fixture are required before marking an internal defect fixed. Semantic source snippets may themselves contain sensitive user code; warn in the manifest that diagnostic bundles are local confidential artifacts.
<a id="section-27"></a>

## 27. Ordered implementation task ledger

### 27.1 Task discipline

Implement these task cards in the cross-ledger order of Section 57. A task is complete only when its named acceptance tests and required M-task integrations exist, pass, and are included in the corresponding release gate. Maintain `docs/tasks.json` with task ID, status (`not_started`, `in_progress`, `complete`), commit ID, acceptance-test names, and artifact paths. A task may not be marked complete because a later feature will supposedly make it work.

Every task must leave the host repository building. During early tasks, unsupported release functionality returns an explicit `LRR-E-NOT-YET-IMPLEMENTED` from a development-only command path. The release verifier rejects that code anywhere in the selected feature inventory or fixtures. Do not replace missing work with passing placeholders, ignored tests, expected crashes, or hard-coded fixture outputs.

A task's prerequisites follow Section 57; preceding tasks within its batch are required. A final milestone gate additionally requires the listed M integrations; provisional early-core gates are not release evidence. There is no parallel implementation requirement. Keeping one integration branch is the determined execution strategy for a low-autonomy implementation agent.

### 27.2 Gate command contract

Implement `cargo run --locked -p xtask -- gate NN`. It runs the listed host unit tests, fixture groups, and structural audits for milestone NN, writes `artifacts/gates/NN.json`, and returns nonzero on a failure, skip, missing test, malformed artifact, or stale fixture expectation. The file records the real command lines, exit codes, git revision, toolchain, and elapsed times.

`xtask verify-all` runs gates 00–12 plus all H/M gates in Section 59, architecture/vendor/manifest audits, the full fixture matrix, property suites, and benchmark collection. Benchmark targets are reported as targets, not fabricated passes. Functional release acceptance requires all mandatory correctness/security-boundary/laziness/native-use gates, and complete measured benchmark results.

### Milestone 00 — Reproducible repository and toolchain

**T000 — Create workspace and execution instructions.** Files: root Cargo.toml, rust-toolchain.toml, crates/lrr/Cargo.toml, xtask/Cargo.toml, AGENTS.md, docs/tasks.json. Use Sections 3–4 exactly. Create module roots and three bin entry points; no guest execution stubs are exposed as successful commands. Acceptance: host `cargo test --workspace --locked` succeeds; task ledger validates; `workspace_has_exact_members` and `toolchain_is_pinned` pass. AGENTS.md contains the operating rules in Section 32.

**T001 — Freeze dependencies and patch JIT symbol lookup.** Files: Cargo.lock, vendor/cranelift-jit, vendor/UPSTREAM.toml, patch diff. Resolve the exact direct versions, add only the Section 21 patch, and capture archive checksums. Acceptance: `vendor_patch_is_exact`, `direct_versions_match_spec`, and dependency license inventory pass. No git/unversioned direct dependency is permitted. A package-resolution failure blocks the gate with evidence; it is not permission to upgrade silently.

**T002 — Parser and native smoke tests.** Files: syntax/parse.rs, jit/abi.rs test module, xtask/bootstrap.rs. Implement the two smoke tests in Section 3, including one registered helper call and a deliberately unresolved native import. Acceptance: `parser_counts_cold_function_without_resolving`, `jit_adds_seven`, `jit_calls_registered_helper`, and `jit_does_not_resolve_ambient_symbol` pass. Run the bad-import case in a child test process so upstream error/panic behavior cannot compromise the host test runner.

**T003 — Gate harness and architecture audit.** Files: xtask/gates.rs, xtask/audit.rs, compat/features.toml, compat/diagnostics.toml. Add checks for forbidden imports, unsafe module locations, and missing diagnostic/feature fixture references. Acceptance: inject a forbidden `std::fs` use into a worker-module test sample and verify the audit fails; restore the real source and gate 00 passes. The audit scans parsed Rust paths where possible, not only fragile substring matches.

**Gate 00:** reproducible host build, pinned parser/JIT smoke, closed symbol table, functioning audit and ledger.

### Milestone 01 — Source snapshots and pure query core

**T010 — Stable keys and encodings.** Files: core/ids.rs, core/digest.rs, source/manifest.rs. Implement typed IDs and explicit StableEncode tags/lengths. Acceptance: maps with different insertion order have identical hashes; field/order/version changes alter expected hashes; distinct key families cannot collide through identical byte concatenation; source-location fields are excluded from semantic keys where specified.

**T011 — Source loader and immutable snapshots.** Files: source/loader.rs, source/snapshot.rs, source/changes.rs. Capture standalone roots/modules, UTF-8 content, and approved includes with size/race limits. Acceptance: `snapshot_is_immutable_after_disk_edit`, `module_path_escape_denied`, `module_rs_ambiguity_is_error`, `source_batch_is_atomic`, and `racing_capture_reports_limit` pass using controlled temporary directories.

**T012 — Syntax adapter and outline split.** Files: syntax/ast_adapter.rs, syntax/outline.rs, syntax/spans.rs. Extract item headers, nested body handles, source-relative spans, and parse diagnostics without semantic resolution. Acceptance: edit a macro-free function body and keep its header/bucket fingerprints unchanged; insert lines before an unchanged function and update its displayed location; cold missing-name and unsupported-expression fixtures outline successfully.

**T013 — Query fetch, dependency validation, and cycles.** Files: db/key.rs, db/value.rs, db/memo.rs, db/provider.rs. Implement Section 9's fetch algorithm with explicit QueryValue variants and no Any/downcast escape hatch. Acceptance: synthetic diamond dependency graph runs each provider once per invalidating change; unchanged-output recomputation keeps downstream reuse; active cycle returns a typed non-cacheable error; breaking that cycle by an edit succeeds immediately; repeated ordinary function identity lookup is not a cycle.

**T014 — Fine-grained input publication and negative dependencies.** Files: db/dependency.rs, db/trace.rs, source/changes.rs. Refresh all changed outlines before publishing header/body/scope inputs. Acceptance: adding a previously absent name invalidates cached missing lookup; body-only edits with unchanged generated declarations do not invalidate name buckets; duplicate candidates are deterministic; a failed query with unchanged dependencies is cached. Trace includes provider execute/reuse/invalidated events with real counts.

**T015 — Cache budgets and source-map attachment.** Files: db/memo.rs, syntax/spans.rs, engine/telemetry.rs. Implement between-run LRU eviction, metadata accounting, and span reattachment. Acceptance: constrained synthetic cache tests evict deterministic victims; a live parse/source-map handle is pinned; an edited line offset cannot produce an old line number. Gate 01 records provider counts for the edit matrix's initial rows.

**Gate 01:** immutable revisions, header/body separation, correct memo reuse including negative answers, bounded caches.

### Milestone 02 — Name resolution, body structure, and first execution

**T020 — Namespaces and imports.** Files: frontend/names.rs, project/cfg.rs. Implement local/module/prelude/type/macro buckets, import fixed point, aliases, visibility, and cfg membership. Acceptance: lexical shadowing, explicit-vs-glob resolution, missing/ambiguous imports, cyclic reexports, and inactive cfg items have dedicated deterministic tests. A macro-blocked namespace cannot be reported complete.

**T021 — Called-body structural index.** Files: syntax/body_index.rs, ir/pc.rs. Assign node, phase, slot, lexical scope, and loop-label identities without semantic evaluation. Acceptance: an entered function containing a cold invalid branch indexes without resolving that branch; nested function bodies are opaque; node/slot budgets fail only when that body is reached. Repeat indexing yields identical keys.

**T022 — Minimal LRIR types and verifier.** Files: ir/op.rs, ir/recipe.rs, ir/validate.rs, frontend/thunks.rs. Implement Literal, Unit, SetHint, ResultTransfer, ScopeEnter, ScopeLeave, and recipe exits first. Acceptance: malformed slot kinds and missing terminal exits fail structural verification; a legal Unsupported recipe passes structural verification but faults on execution. No successor thunk is requested by a branch recipe. CA027 additionally requires stable decoded lexical literal payloads and incompatible-cache rejection under Section 11.3.

**T023 — Primitive values and checked arenas.** Files: runtime/value.rs, runtime/arena.rs, runtime/cell.rs, runtime/shape.rs. Implement scalar descriptors, handle epochs, cell states, transfer tickets, and literal records. Acceptance: stale/wrong-arena/zero/out-of-range handles fail; bit-copy cannot bypass guest ownership; ValueRepr layout assertions pass. A run reset invalidates every previous guest handle. CA034 additionally separates virtual logical ordinals from physical arena handles and verifies materialized ownership.

**T024 — First VM path.** Files: vm/frame.rs, vm/step.rs, engine/run.rs. Execute `fn main() { let x: i64 = 40_i64; x; }` through LRIR with explicit PCs/scopes. Acceptance: no AST evaluator exists; opcode counters are nonzero and accurate; a following cold function is not semantically queried. The CLI prints a structured actual result, not a hard-coded success. CA027 additionally requires repeated interpreter literal execution with zero repeated lexical decoding, independent runtime bindings and unchanged reached diagnostics.

**T025 — Numeric operations and contexts.** Files: vm/numeric.rs, frontend/types.rs. Implement Section 11, starting with every supported primitive and Rust-compatible source-constrained numeric fallback. Acceptance: exhaustive i8/u8 arithmetic boundary loops; random wider arithmetic/cast comparisons; works/fails context examples; signed-min literal; distinct usize/u64 contracts. Gate verifies Rust-compatible constraint-aware i32/f64 fallback, preserves known source widths and rejects substitution of defaults for unresolved nonnumeric types.

**Gate 02:** real LRIR scalar execution, checked handles, deferred cold semantics, explicit numeric context behavior.

### Milestone 03 — Control flow, calls, and basic tests

**T030 — Blocks, let, assignment, and shadowing.** Files: frontend/thunks.rs, vm/control.rs, vm/step.rs. Implement block phase templates and runtime storage contracts. Acceptance: RHS-before-LHS assignment trace; new binding initializer sees the old shadowed binding; immutable reassignment faults; loop-local reincarnations do not reuse stale refs or leak ownership. Source invalidity after an early return remains untouched.

**T031 — If, short circuit, while, and loop.** Files: frontend/thunks.rs, vm/control.rs. Implement branch-only selected thunk fetch, labels, break values, continue cleanup, and backedge polling. Acceptance: cold undefined branch and short-circuit RHS have zero thunk/query events; hot condition loops terminate correctly; nested labeled break/continue choose correct scopes. Fuel exhaustion is deterministic.

**T032 — Function signatures and call packets.** Files: frontend/signatures.rs, vm/call.rs. Implement concrete boundaries, generic structural substitution, ordered arguments, and return checks. Acceptance: uncalled invalid function signature/body does not fail another call; an unused generic bound is ignored; incompatible actual parameter/return descriptors fault only at reached boundaries; effectful arguments execute once.

**T033 — Dispatcher-owned recursion and interruption.** Files: engine/dispatch.rs, vm/frame.rs. Keep guest recursion on explicit frames and enforce frame limits. Acceptance: recursive factorial with concrete types; deep recursion hits guest frame limit rather than host stack overflow; cancellation/fuel produce the specified status and no duplicated output. Host call stack depth stays bounded in a 2,000-frame fixture.

**T034 — Minimal test harness and builtin assertions.** Files: cli/args.rs, engine/run.rs, facade/macros.rs. Discover/select zero-argument tests, implement assert and panic through guest outcomes, and isolate each test heap. Acceptance: one selected test passes despite another broken body; exact/substr/no-match behavior works; should_panic cannot accept an unsupported-feature fault. Add the fixture manifest/harness format from Section 28. CA037 additionally tracks the complete upstream inventory, per-contract classification and runtime-semantics prerequisite; subset/component success does not open the JIT gate.

**Gate 03:** useful selected scalar tests, recursive calls, lazy branches, ordered effects through a fake broker, no host stack recursion.

### Milestone 04 — Aggregate ownership, references, and cleanup

**T040 — Nominal/tuple/array descriptors and construction.** Files: frontend/types.rs, runtime/object.rs, vm/step.rs. Implement deferred schemas and construction guards. Acceptance: named-field written evaluation order differs correctly from declaration-order storage; duplicate/missing field faults occur at reached construction; partial constructor panic drops completed fields once; cold unused schema errors are not forced.

**T041 — Places and leases.** Files: runtime/place.rs, runtime/reference.rs. Implement field/index/deref projection and checked reference leases. Acceptance: executed out-of-bounds indexing raises GuestPanic with cleanup; index-type mismatch is a runtime contract fault; same-location scalar assignment remains visible through a ref; whole move invalidates inline refs; shared writes fail; two mutable guest aliases serialize without a host RefCell panic.

**T042 — Copy, move, and partial initialization.** Files: runtime/cell.rs, runtime/shape.rs, vm/step.rs. Implement lazy Copy policies, field moves, assignment transfer, and type-with-Drop restriction. Acceptance: String move/use-after-move; Copy scalar repeat reads; partial tuple/struct moves preserve other fields; double-owner property tests fail the buggy model and pass the implementation.

**T043 — Lexical scopes and temporary annotations.** Files: syntax/body_index.rs, vm/control.rs, vm/cleanup.rs. Implement Sections 13.5 and 18.4 explicitly. Acceptance: direct extending borrow, tuple/struct extension, extending pattern, non-extending call, Rust 2024 if-let else, block tail, and short-circuit temporary lifetimes match the recorded supported oracle cases.

**T044 — User Drop and explicit cleanup machine.** Files: frontend/dispatch.rs, frontend/types.rs, vm/cleanup.rs. Enter selected Drop witnesses as guest frames; connect necessary predicate fulfillment from M026/M029 and retain ordered cleanup. Acceptance: normal/drop/replace/early-return paths drop once; cold Drop body enters only on drop; partial moves skip moved fields; conditional applicable Drop works and reached ambiguity fails explicitly.

**T045 — Guest panic unwinding and fatal faults.** Files: vm/control.rs, vm/cleanup.rs, cli/render.rs. Implement ordinary panic cleanup, double-panic termination, and no guest cleanup after fatal profile/memory/resource faults. Acceptance: separate fixtures assert each difference through drop output; worker crash is not should_panic success; hostpanic injection terminates only the worker process in integration tests.

**Gate 04:** ownership and cleanup invariants pass, references remain host-safe, panic/fault distinctions are observable and tested.

### Milestone 05 — Patterns, dispatch, and finite language completion

**T050 — Transactional pattern engine.** Files: vm/pattern.rs, frontend/thunks.rs. Implement the closed pattern forms, match ergonomics, pending bindings, and commit. Acceptance: failed arms preserve scrutinee ownership; true arms move once; conflicting moves fault before commit; or-pattern names/modes and binding order validate; overlapping alternatives can rerun false guards without replaying scrutinee evaluation.

**T051 — Match, if-let, while-let, let-else, and question mark.** Files: frontend/thunks.rs, vm/control.rs. Connect pattern phases and exact Option/Result flow. Result error conversion delegates to the reached From witness via M031. Acceptance: untaken arms remain unprepared; no-match and let-else fallthrough diagnostics are precise; successful ? does not force conversion; failing ? calls From once and validates the enclosing return contract.

**T052 — Source-aware trait and inherent dispatch integration.** Files: frontend/dispatch.rs, frontend/names.rs. Integrate M017–M027 symbolic receiver/member identities, conditional goals, projections, defaults, and dictionaries. Acceptance: generic trait method cannot be hijacked by actual-type inherent method; unused bound remains unforced; conditional impl succeeds when its premise succeeds; ambiguity/missing methods are precise; changed generated candidate invalidates prior selection.

**T053 — Named function values.** Files: runtime/value.rs, frontend/signatures.rs, vm/call.rs. Support named function-item values and concrete fn-pointer coercion with uniform guest dispatch. Acceptance: call a selected function through a local variable; incompatible fn signature faults; a numeric payload cannot become a callable; closure creation receives the specific unsupported diagnostic only when reached.

**T054 — Pure constants.** Files: frontend/constants.rs. Implement immutable ConstValue evaluator, memo dependencies, budgets, and runtime instantiation. Acceptance: array length constant, recursive bounded const fn, cycle, cold effect branch, reached forbidden effect, and two uses with independently mutable guest arrays. Constant memo contains no guest handles.

**Gate 05:** every supported core-language row in the feature manifest has positive and negative/reached-lazy fixtures.

### Milestone 06 — Facade, built-ins, and real capability broker

**T060 — Vec and slice operations.** Files: facade/collections.rs, runtime/object.rs, runtime/reference.rs. Implement exact Section 17 inventory and deterministic growth. Acceptance: push/pop/get/get_mut/index/slice, reallocation invalidation, nonreallocating push preservation, pop invalidating only removed element, consuming iteration drops remaining elements, quota reservation preserves prior ownership.

**T061 — String and formatting.** Files: facade/text.rs, facade/macros.rs. Implement UTF-8 operations, chars/bytes iteration, bounded formatter, and source-order argument handling. Acceptance: multibyte len/char/slice cases; invalid boundary panic/error; formatter unsupported spec; capture-name lookup; formatting never moves String; failure-only assert message remains lazy.

**T062 — Option, Result, scalar methods, and iterators.** Files: facade/option_result.rs, facade/collections.rs, facade/registry.rs. Implement the finite inventory and resumable iterator drivers, then required user-trait integration in M031. Acceptance: Option/Result explicit context, unwrap/expect panics, collect/enumerate/count and named-function callbacks, max range termination, custom Iterator/IntoIterator with normalized Item and exact cleanup.

**T063 — Macros and derives integration.** Files: facade/macros.rs, expand/builtins.rs, expand/derive.rs, frontend/types.rs. Use Sections 18 and 48–51, with M028 generated trait headers. Acceptance: builtin effects/lazy failure arguments, vec repetition with selected Clone, user macro_rules and all proc forms, generated unrelated names/impls, proper helper attrs, include/env capture, and source origins after edits. Never retain the revision-3 blanket user-macro rejection.

**T064 — Worker protocol and fake-broker equivalence.** Files: service/wire.rs, engine/worker.rs, effects/protocol.rs. Move execution into the persistent worker; implement bounded framing, snapshots, run/reset, effect requests, and cancellation atomic. Acceptance: malformed/oversized/truncated frames fail closed; duplicate replies do not replay effects; 100 sequential tests retain caches but no guest state.

**T065 — Real broker permissions.** Files: effects/policy.rs, effects/broker.rs, effects/files.rs, effects/output.rs. Implement cap-std grants, captured output, injected environment, and fixed errors. Acceptance: default FS deny, read-only grant, write-only grant, prefix confusion, `..`, absolute path, escaping symlink, oversized read, and reused native policy scenario through the fake artifact stub until JIT arrives. No worker std::fs/env/process operation is introduced.

**Gate 06:** finite facade complete, real tests can read approved fixtures, no ambient runtime authority, bounded worker IPC.

### Milestone 07 — Baseline observed-region JIT

**T070 — Freeze helper and reconstruction ABIs.** Files: jit/abi.rs, jit/helpers.rs, jit/verify.rs. Implement Section 19 baseline, direct-call and reconstruction helper schemas, effect classifications and closed registries. Acceptance: wrong status/frame/epoch, missing map, unlisted symbol, wrong scratch/view layout and mismatched registered signature fail before use; all unsafe remains in the five allowed runtime modules. CA028/CA032 additionally verify entry-map and TierTransfer schemas plus reserved reconstruction storage.

**T071 — Record real execution and form baseline regions.** Files: jit/record.rs, jit/plan.rs. Record successful reached operations and control edges with baseline 64-op bounds; represent native calls explicitly with their entered/completed outcomes. Acceptance: cold live branches/callees stay unprepared; a failed observation is excluded; unresolved demands split regions; prepared native calls may remain in a region; H019 supplies native integration acceptance. CA030 additionally requires actual native completion feedback with no synthetic or duplicated traversals.

**T072 — Generic Cranelift lowering.** Files: jit/lower.rs, jit/module_cache.rs. Emit helper sequence/status exits and closed imports; finalize before publication. Acceptance: force-mode repeated loop has actual native entries and helper-op counts; recursive guest calls still use dispatcher frames; a failed helper exits before following helper; interpreter/native transcript and fuel match.

**T073 — Precise exits and once-only resumption.** Files: engine/dispatch.rs, jit/helpers.rs. Implement canonical op PC, pending actions, and suppression on Deopt. Acceptance: inject guard misses after zero, one, and multiple completed operations; earlier writes/output/ownership transfers occur once; the missed operation executes once; later operations are not skipped. Artificial ABI inconsistency fails as an internal defect. CA028/CA031 additionally distinguish TierTransfer from Deopt suppression and preserve progress through rejected entry/cooldown.

**T074 — Promotion and tier selection.** Files: engine/install.rs, jit/plan.rs. Implement off/auto/force and jit-tier selection with bounded baseline promotion and Section 62 optimizing eligibility. Acceptance: baseline auto eligibility after 32 actual traversals, force after one real observation, no replay of one-shot work, explicit forced-tier failure and visible auto fallback; T089 integrates optimizing selection. CA030/CA033 additionally require native-driven eligibility and bounded reconsideration after workload phase changes. CA036 additionally records versioned effective scheduling settings and tests bounded alternative policy.

**Gate 07:** a real generic native tier executes only observed code with identical semantics and precise failure/effect exits.

### Milestone 08 — Guarded scalar native execution and bounded code cache

**T080 — Scalar contracts and verified frame views.** Files: jit/helpers.rs, jit/abi.rs. Implement baseline scalar staging/commit and the Section 61 pinned scalar frame view with ABI2 layout checks. Acceptance: literal/shape/move/descriptor misses occur before entry; no invalid alias or stale view; correct fuel and scalar result publication; NX018/CA011 integrate direct and optimized paths.

**T081 — Primitive native lowering.** Files: jit/lower.rs. Implement Section 21 baseline operations with checked overflow and concrete primitive identities; later typed optimizations use T085. Acceptance: boundary tables and 10,000 random pairs per admitted type match helpers; unsupported baseline operations use explicit helpers; LLVM/vector optimization cannot suppress faults or change arithmetic semantics.

**T082 — Artifact and specialization provenance.** Files: engine/install.rs, jit/module_cache.rs. Implement source/witness/type/generation/ABI/backend/contract keys and exact invalidation of inlined facts. Acceptance: Section 22 edit matrix; non-inlined caller reuse versus inlined caller invalidation; safe concrete-shape miss; T086/T088 verify bounded variants and cross-process relocatable reuse.

**T083 — Module arena retirement.** Files: jit/module_cache.rs. Implement region/code limits, safe boundary eviction, entry clearing, and metadata reclamation. Acceptance: tiny test budgets repeatedly create/evict arenas, no stale pointer is callable, active invocation prevents free, dropped entry cannot be relabeled as another artifact, and RSS stress does not grow without bound across source revisions.

**T084 — Native import and exit-map audit.** Files: jit/verify.rs, service/coordinator.rs, xtask/audit.rs. Audit closed helper/native/IR imports, typed memory access, matched call signatures, maps and source origins. Acceptance: reject unknown indirect targets, missing ownership maps, guest-derived addresses and invalid relocations before publication; actual CLIF/LLVM and machine-code evidence corresponds to entered artifacts.

**Gate 08:** primitive native execution, dependency-correct warm reuse, safe code lifetime, no hidden symbol fallback.

### Milestone G2 — Optimizing execution after native integration

**T085 — Typed SSA and scalar reconstruction.** Files: jit/typed.rs, jit/maps.rs, jit/verify.rs. Build reached typed SSA with effects, fault order, initialization, source stack and executed-prefix accounting; reconstruct scalar state at every required exit. Acceptance: CA011/CA012/CA018: guard, overflow, exact fuel and cancellation exits match the interpreter; dead-result arithmetic cannot erase a panic; no cold live body preparation. CA028/CA032 additionally require verified scalar entry maps, logical quota parity and allocation-free reconstruction from reserved backing. CA034 additionally verifies fresh physical publication and adopted backing lifetime. Depends: M042.

**T086 — Optimized loops and concrete variants.** Files: jit/optimize.rs, jit/plan.rs, engine/install.rs. Compile reached loops with native backedges, bounded variants, reached-call inlining and owned/native lease maps. Acceptance: CA013/CA014/CA015/CA021: loops stay native across iterations; unseen edges resume exactly; variant cap holds; native pointers and leases never outlive moves/reallocation/drop; inlined source cleanup and frame limits match. CA028/CA031/CA032 additionally cover live loop entry, owned transfer maps, shape selection/cooldown and virtual-allocation/frame accounting. CA034 additionally verifies bounded virtual-allocation metadata and alias materialization. Depends: T085.

**T087 — LLVM hot tier and prepared native IR.** Files: jit/llvm.rs, jit/abi.rs, project/native_ir.rs, native/schema.rs. Pin and validate LLVM 23.1.1 against the exact producer; lower typed reached units and import admitted abort scalar leaf IR through closed native references. Acceptance: NX024/NX025/CA016/CA017: real LLVM execution and actual dependency-body inlining; unavailable/mismatched producer, unsupported IR/state identity and unknown symbols fail or retain a reported direct-call path as prescribed. Depends: T086,H013.

**T088 — Background compiler and relocatable reuse.** Files: jit/compiler_worker.rs, jit/object_cache.rs, jit/module_cache.rs, service/coordinator.rs. Compile frozen plans in one bounded worker; validate completion and publish immutable artifacts; persist only validated objects/maps/closed relocations. Acceptance: CA019/CA020/CA022/NX028: stale jobs never install; CPU/contract/ABI/generation mismatch rejects; fresh worker rebinds code with zero restored guest state or pointers; code/compiler/cache budgets hold. CA029 additionally adopts genuine background completions during long native loops through bounded safepoint transfer. CA035 additionally requires bounded all-job output draining, state transitions and cancellation disposal. Depends: T087.

**T089 — Execution contracts and optimization integration.** Files: engine/telemetry.rs, cli/args.rs, xtask/optimized.rs. Integrate precise/throughput selection, forced-tier diagnostics, every direct ABI/optimizing fixture, and Section 61 matched performance experiments. Acceptance: NX011–NX030 and CA011–CA037 execute on macOS with truthful tier/transport/contract counters, exact-once failure paths and separate raw/integrated performance results; every required negative control runs; measured misses cannot be labeled near-native. Depends: T088.

**Gate G2:** actual direct Rust ABI calls, typed hot loops, reconstruction, admitted LLVM IR inlining, bounded background compilation and safe cross-process code reuse. Run this batch after M042 and before T100; it is not permission to start JIT before the interpreter-first priority is satisfied.

### Milestone 09 — Cargo preparation, CLI, and agent workflow

**T090 — Cargo unit and tool preparation integration.** Files: project/cargo.rs, project/prepare.rs. Use Cargo metadata plus pinned exact unit graph and authorized compile-time-only operation. Acceptance: normal/dev/build/proc/optional/target-specific and forwarded-feature units resolve correctly; offline miss fails; denied tools never execute; authorized tools prepare without checking live runtime body. M002/M004 evidence is mandatory.

**T091 — Prepared units and staleness.** Files: project/units.rs, source/manifest.rs. Capture cfg/test/feature/alias/tool/output identities under schema5. Acceptance: lib-tests/integration/run units differ; manifest/tool/generated/native inputs trigger correct explicit reprepare; an unrelated macro-free body edit with no build-input change does not. Generated source is loaded only through captured records; rlib is never directly executed.

**T092 — CLI and execution contract schema.** Files: cli/args.rs, cli/render.rs, cli/exit_code.rs, bin/cargo-lrr.rs. Implement Sections 25/56 service3 flags/results, independent jit-tier/execution-contract and execution authority, limits and exit priority. Acceptance: strict golden parsing, precise default, numeric fuel versus throughput conflict, unavailable forced backend, actual counters/null throughput fuel, and no authority implied by a backend choice.

**T093 — Stdio service and atomic overlays.** Files: service/requests.rs, service/coordinator.rs. Implement public schema-3 NDJSON and internal v4 binary-bulk frames, progress/results, edit batching, stale-base failures, cancellation and bounded queues. Include CA023–CA025 framing/transaction controls. Acceptance: one client edits called function and reruns with cache reuse; concurrent cancel is processed; malformed request does not mutate snapshot; guest/protocol bytes cannot inject stdout log records.

**T094 — Watch, shutdown, and worker recovery.** Files: service/watch.rs, engine/worker.rs. Implement 100 ms polling, coalescing, snapshot capture, manifest-stale behavior, deadline/grace/kill, and safe restart between runs. Acceptance: repeated rapid source changes run only committed snapshots; no mixed revisions; crash resets native cache claims; shutdown reaps worker and releases broker resources.

**Gate 09:** end-to-end standalone and prepared Cargo workflows, structured agent service, actual warm edit/test loop.

### Milestone 10 — Differential, property, mutation, and robustness evidence

**T100 — Native Rust oracle harness.** Files: xtask/fixtures.rs, fixtures/native. Build only valid supported fixtures using pinned rustc, each owning root/dependency edition, opt0, overflow checks, debug assertions, and a fresh temp executable path. Acceptance: oracle cache keys include all source/helper/compiler inputs; unsupported-relaxed fixtures are never sent to rustc as expected-pass cases; outputs/drop order/results compare with explicit normalizations.

**T101 — Tier and execution contract equivalence.** Files: xtask/fixtures.rs, fixtures/runtime. Run off/auto/force and explicit backend controls under both contracts with fresh state and scripted effects. Acceptance: values, source origins, effects, ownership/drop and panic/fault paths agree within each contract; precise fuel agrees exactly, throughput fuel is null; forced backend fixtures require actual entries and no skipped cases.

**T102 — Generators and reducer.** Files: crates/lrr/tests/property_*.rs, xtask/reduce.rs. Implement Section 29's expression/control/state model generators and deterministic seed replay. Acceptance: injected arithmetic, ownership, stale-cache, and replay bugs are caught; reducer produces a smaller failing fixture preserving expected failure class. Persist seeds and reduced cases as regression tests.

**T103 — Protocol, parser, and quota stress.** Files: crates/lrr/tests/stress_*.rs. Exercise malformed UTF-8/JSON, oversized lengths, parser recovery, module cycles, handle exhaustion, deep refs/types, and cancellation during compile/effect/cleanup. Acceptance: no unbounded allocation, hang, host panic on guest input, or unauthorized effect; timeouts are classified and subprocesses reaped.

**T104 — Full feature/diagnostic coverage audit.** Files: compat/*.toml, xtask/audit.rs. Require positive/reached-negative/cold-lazy coverage for every feature and exact inventory coverage for every opcode/facade/diagnostic. Acceptance: deleting a referenced fixture or leaving a release TODO/not-yet-implemented code makes the gate fail. Coverage is about actual exercised paths, not only file presence.

**Gate 10:** reproducible semantic, laziness, safety-boundary, cache, and native-use evidence for the entire declared profile.

### Milestone 11 — Measurements and bounded optimization

**T110 — Benchmark driver and datasets.** Files: xtask/bench.rs, benches/cases. Measure Sections 30/43/61/62 cold/warm/edited/ABI/loop/IR/cache scenarios, including compile contention and ordinary native versus inlined baselines. Acceptance: raw paired samples and commands retained; actual target identities and retained calls confirmed in assembly; first-result and steady-state times separate; precise and throughput never conflated.

**T111 — Attribute optimization and boundary costs.** Files: engine/telemetry.rs, docs/performance.md. Measure dispatcher, helpers, cell traffic, maps, guards, native leases, direct/catch/codec entries, compiling/linking and cache costs; include B01–B10 wire/copy/allocation/encoding costs, scalar invoker comparisons and CA026 identity-preserving reuse controls. Acceptance: actual tier/transport counters reconcile with effects; sampled versus diagnostic instrumentation costs identified; raw ABI and integrated misses reported without unsupported claims.

**T112 — Profile and tune the adopted architecture.** Files: jit, engine, native, benches. Tune typed IR, guards, loop layout, inlining budgets, promotion and data/storage paths within Sections 61–62. Acceptance: rerun applicable equivalence/laziness/ownership/authority gates and publish raw before/after results; reject changes that erase required behavior or introduce an unapproved backend/semantic contract.

**Gate 11:** complete measurements, bounded memory across edits, documented performance limitations, no speed claims without raw results.

### Milestone 12 — Release handoff

**T120 — User and agent documentation.** Files: README.md, docs/profile.md, docs/security.md, docs/performance.md, AGENTS.md. Include supported examples, explicit Cargo subset, runtime vs Rust validation distinction, default deny examples, numeric annotation limitation, and no-hardened-sandbox warning. Acceptance: every documented command is exercised in a clean integration test; screenshots or examples do not imply unimplemented capabilities.

**T121 — Reproduce from clean checkout.** Run the Section 31 release runbook in a clean macOS environment for the primary release phase; repeat in Linux x86-64 for the later port with the exact toolchain and dependency lock. Acceptance: all mandatory gates pass without untracked source files; fixture/benchmark artifacts are attached; version/license/vendor manifest complete; no ignored selected fixture, stale fixture expectation, or degraded JIT run remains.

**T122 — Common-engine acceptance demonstration.** Execute the two-package demo: list/select a test; pass despite broken cold code; run a hot primitive loop through native code; process Vec/String data; deny then permit a fixture read; edit a called body through stdio; show unchanged-body/code reuse; revoke permission without recompilation; run native Rust validation separately on a valid version. Acceptance: a recorded JSON transcript and command script reproduce every assertion. This is the required common-engine evidence artifact, not a narrative claim or the final revision-5 gate; M048 consumes it after the macro/trait/hybrid evidence is complete.

**Gate 12:** reproducible release of the one specified runtime, with actual functionality and all limitations accurately stated.
<a id="section-28"></a>

## 28. Fixture format and mandatory concrete cases

### 28.1 On-disk format

Each fixture is a directory containing `fixture.toml`, `main.rs` or a Cargo workspace, and optional captured source files/broker inputs. Every fixture has a unique stable ID. The manifest schema is strict; unknown fields fail the harness.

```toml
schema_version = 1
id = "L001"
category = "relaxed"
source = "main.rs"
entry = "case"
modes = ["off", "auto", "force"]
oracle = false

[expect]
status = "passed"
stdout = ""
stderr = ""

[laziness]
no_body_preparation = ["broken"]
no_marker_preparation = ["cold_branch"]

[force]
minimum_native_entries = 0
minimum_native_scalar_ops = 0
```

`no_marker_preparation` refers to a `/*lrr-marker:NAME*/` comment immediately before an expression. The fixture loader associates the marker with that following expression node's full span after parsing, then asserts zero Thunk-provider execution and zero installed/native op origin for that span. Marker comments are test metadata, not runtime semantics. Do not assert an entire function was unparsed when the architecture deliberately parses its file.

For expected faults, add `diagnostic_code`, `phase`, and `cleanup_ran=false` where appropriate. For panic, add `panic_contains`; it remains distinct from a fault. A fixture may specify `expected_drop_log` separately from stdout using the fake broker's event transcript. A missing expected observation is a failure.

Native-oracle fixtures use a root-level zero-argument function named case and set oracle=true. Their source must be valid Rust in the declared subset and use explicit numeric types where context would differ. The oracle harness removes only selected test-control attributes from this root function, appends a native-only main wrapper calling case, and captures panic/result status in a separate host-controlled result file. It preserves guest stdout/stderr unchanged. The wrapper may use ordinary native Rust facilities not supported in guest source. No such wrapper is fed into LRR.

Compile oracle source with the pinned rustc, `--edition=2024 -C opt-level=0 -C overflow-checks=yes -C debug-assertions=yes -C panic=unwind`, and the fixture's recorded cfg(test) setting. For unit-return cases, the wrapper uses catch_unwind only on the native side and suppresses the default panic hook; guest panic text/status and destructor output are captured separately. Do not compare compiler diagnostics or libtest's human framing to runtime output. Relaxed/profile-fault fixtures have no native-pass oracle.

### 28.2 Minimal lazy execution fixtures

**L001 — Broken cold function and untaken branch.** Expect pass in all modes, no_body_preparation `broken`, and no marker preparation `cold_branch`.

```rust
fn broken() { missing_everything(); }
#[test]
fn case() {
    if true { assert_eq!(2_i64 + 3_i64, 5_i64); }
    else { /*lrr-marker:cold_branch*/ another_missing_call(); }
}
```

**L002 — Reached invalid branch.** Same source as L001 with condition false. Expect `LRR-E-NAME-MISSING` at the marked call, not at discovery. `broken` remains unprepared. This ensures a cold error was deferred rather than erased.

**L003 — Short circuit.** Expect pass; both markers remain unprepared.

```rust
#[test]
fn case() {
    assert_eq!(false && /*lrr-marker:and_rhs*/ absent_bool(), false);
    assert_eq!(true || /*lrr-marker:or_rhs*/ absent_bool(), true);
}
```

**L004 — Early return skips later ordinary semantics.** Expect value1 and no executable preparation of marker. Use an ordinary unresolved call, not an unknown macro that could affect declaration discovery.

```rust
fn answer() -> i64 {
    return 1_i64;
    /*lrr-marker:after_return*/ unknown_call();
}
#[test] fn case() { assert_eq!(answer(), 1_i64); }
```

**L005 — Unused trait bound.** Expect pass; this unused AbsentContract predicate has zero forced goals. The solver exists but does not eagerly validate this function bound.

```rust
trait AbsentContract { fn use_it(&self); }
fn ignore<T: AbsentContract>(_: T) -> i64 { 8_i64 }
#[test] fn case() { assert_eq!(ignore(3_i64), 8_i64); }
```

**L006 — Mismatched return is path-lazy.** `choose(true)` passes; a companion fixture calling choose(false) faults at call-return boundary. The String branch is unprepared in the passing fixture.

```rust
fn choose(ok: bool) -> i64 {
    if ok { 7_i64 } else { /*lrr-marker:wrong_return*/ String::from("x") }
}
#[test] fn case() { assert_eq!(choose(true), 7_i64); }
```

**L007 — Cold closure and supported user macro.** An uncalled function contains closure creation and another contains a valid macro_rules invocation. The scalar test passes with no executable preparation of either body; declaration-sensitive macro expansion may occur and is counted separately. Calling the macro-generated supported expression succeeds. Creating the closure faults at the reached operation. A genuinely unresolved macro needed for a declaration seal fails discovery with its origin, not a generic user-macro-unsupported error.

### 28.3 Numeric context and arithmetic fixtures

**N001 — Context from a reached call.** Expect pass; x's literal record becomes u64 at use64's argument boundary.

```rust
fn use64(x: u64) -> u64 { x }
#[test] fn case() { let x = 3; assert_eq!(use64(x), 3_u64); }
```

**N002 — Source constraints before numeric fallback.** Native-oracle pass: the later source use64 call constrains the addition to u64 before execution, without executing or preparing the callee body to infer that width. Full acceptance requires that source constraint propagation; an earlier unsupported diagnostic is an implementation gap.

```rust
fn use64(x: u64) -> u64 { x }
#[test] fn case() { let x = 3 + 4; let _y = use64(x); }
```

**N003 — Context from a declared return.** Native-oracle pass: `fn sum() -> u64 { 3 + 4 }` and case comparing it to 7_u64. Assert exactly one addition execution. This distinguishes strict context from simply rejecting all unsuffixed syntax.

**N004 — Arithmetic panic boundaries.** Generate separate native-oracle cases for i8 max+1, u8 zero-1, i64 min/-1, zero divisor, shift by width, unary negative minimum variable, and valid negative-minimum literal. Avoid compile-time unconditional-panic rejection in oracle sources by routing operands through named runtime identity functions and using them in reached expressions. Each mode must match runtime panic status/fuel and not execute a following output marker.

**N005 — No implicit numeric widening.** `fn takes(_: u64) {}` called with `1_u32` faults at argument boundary after evaluation. A companion explicit `1_u32 as u64` passes. usize/u64 mismatch gets its own case despite equal machine width.

**N006 — Float/cast exactness.** Native-oracle cases cover f32 rounding after each operation, float comparisons involving NaN created through arithmetic, float-to-integer saturation, signed/unsigned narrowing, and char/u8 supported casts. Compare NaN class, not unspecified payload bits. JIT executes these through helpers and reports zero direct float fast-path operations.

### 28.4 Ordering, ownership, and cleanup fixtures

**O001 — Argument order.** Native-oracle pass; stdout is exactly `LR`.

```rust
fn left() -> i64 { print!("L"); 1_i64 }
fn right() -> i64 { print!("R"); 2_i64 }
fn add(a: i64, b: i64) -> i64 { a + b }
#[test] fn case() { assert_eq!(add(left(), right()), 3_i64); }
```

**O002 — Assignment order.** Native-oracle pass; stdout is `RI` and the first element becomes9. RHS evaluation precedes LHS index evaluation.

```rust
fn rhs() -> i64 { print!("R"); 9_i64 }
fn index() -> usize { print!("I"); 0_usize }
#[test] fn case() { let mut a = [1_i64]; a[index()] = rhs(); assert_eq!(a[0_usize], 9_i64); }
```

**O003 — Move then use.** A String is moved from a into b, then a is read. Expect `LRR-E-USE-AFTER-MOVE`; no guest cleanup runs after that fatal fault. b remains a single owner until physical arena teardown. Companion scalar Copy case passes.

**O004 — Partial move and ordered drop.** Native-oracle pass; output reflects explicit first-field drop and later remaining field drop, each once.

```rust
struct D(&'static str);
impl Drop for D { fn drop(&mut self) { print!("{}", self.0); } }
struct Pair { a: D, b: D }
#[test]
fn case() {
    let p = Pair { a: D("A"), b: D("B") };
    let a = p.a;
    drop(a);
}
```

Expected stdout `AB`. Add a companion with Drop on Pair: moving p.a faults with `LRR-E-PARTIAL-MOVE-DROP`. Pair's field b must not be silently discarded in the passing case.

**O005 — Panic cleanup.** Native-oracle should-panic fixture: create D("A"), then D("B"), then panic("stop"). Expected drop output `BA` and one ordinary panic. A Drop implementation that itself panics during unwinding produces `LRR-E-DOUBLE-PANIC`, never should-panic pass.

**O006 — Borrow alias relaxation.** No native-pass oracle; expect pass with last write observed, proving no dynamic exclusivity checker was accidentally introduced.

```rust
#[test]
fn case() {
    let mut x = 1_i64;
    let a = &mut x;
    let b = &mut x;
    *a = 2_i64;
    *b = 3_i64;
    assert_eq!(x, 3_i64);
}
```

**O007 — Inline reference invalidation.** Borrow a field, move the enclosing non-Copy aggregate to another local, then dereference the old ref. Expect the stale-location diagnostic. A companion Vec-buffer reference remains valid across moving the Vec without reallocation. Reallocation and pop-last-element variants fail according to lease rules.

**O008 — Temporary extension.** Native-oracle pass for direct `let r = &String::from("x"); assert_eq!(r.as_str(), "x");`, tuple/struct extending borrows, and `let ref r = String::from("x");`. A helper `fn id<'a>(x: &'a String) -> &'a String { x }` used to borrow a temporary through a non-extending call gets a relaxed expected stale-reference fault when used later, not a native-pass oracle.

**O009 — Pattern guard ownership.** Match a non-Copy struct with a false guard that prints `G`, then a later arm consumes it and prints `A`. Expect `GA`, no premature drop, and no scrutinee reevaluation. Add overlapping or-pattern alternatives with a false guard and assert two guard executions before later-arm selection.

**O010 — Replacement and destructor panic.** Replace a live D value in a mutable local. Old Drop panics. Assert the replacement already committed and guest cleanup handles the new owner once during unwind. This fixture catches implementations that drop-before-commit or roll back after a side effect.

### 28.5 Native execution and exact-exit fixtures

**J001 — Hot integer loop actually enters native code.** Native-oracle pass; force and auto must show positive native entries and direct scalar operations. Expected sum is499500.

```rust
#[test]
fn case() {
    let mut i: i64 = 0_i64;
    let mut sum: i64 = 0_i64;
    while i < 1000_i64 {
        sum = sum + i;
        i = i + 1_i64;
    }
    assert_eq!(sum, 499500_i64);
}
```

**J002 — Same generic body, different scalar shape.** Call the same generic addition with i64 and u64 under its valid source-selected trait contracts. Expect guarded baseline fallback and, after qualifying observations, separately keyed optimized variants within the four-variant cap. The preceding output/effect count must be exact; runtime shape cannot change source member identity.

**J003 — Injected guard miss after effectful prefix.** Host harness builds a valid observed region whose earlier generic op mutates a guest cell, then causes scalar staging to miss on a later op. Assert the earlier mutation's version/counter increases once, not twice. This is a VM unit fixture, not arbitrary source codegen. Run misses at every eligible op index.

**J004 — Call/return recursion under native regions.** A repeated recursive function reaches native regions, yet maximum native guest-call recursion remains one region invocation. Guest frame count records the actual recursion depth. No guest function is linked as a direct native callee.

**J005 — No native one-shot replay.** A test prints X once and returns, with force enabled. Expect stdout X, native_entries=0, and any compiled candidate must remain unentered until a future legitimate execution. Compiling an observed region is permitted; executing it solely for benchmarking is forbidden.

**J006 — Native overflow parity.** Warm a valid checked addition region, then pass maximum and one on the same guarded type. Expect the shared overflow panic, the same origin and fuel as interpreter, and no succeeding output. A wrapping result is a test failure even if the runtime later panics elsewhere.

**J007 — Closed imports and arena retirement.** Unknown symbols, unregistered indirect targets, wrong matched signatures and unchecked guest-derived loads fail before publication. A registered generation-bound indirect call succeeds under its accepted ABI. Tiny budgets retire code only after all active references drain; revisited PCs install valid entries without stale pointers.

### 28.6 Cache/edit fixtures

**E001 — Macro-free cold body edit.** Run J001 plus an uncalled function with no declaration-producing macro and no changed build-script input. Modify only that cold body's bytes through apply_changes. Repeat the test. Expect unchanged reached BodyKeys, zero Thunk recomputation for unchanged reached phases, native artifact cache hits, and no semantic preparation of the cold body in either revision. Parse/outline work for the changed file is allowed and recorded separately.

**E002 — Called body edit.** A main test calls helper returning1; edit helper to return2 and edit the test assertion appropriately, leaving a separate hot support function unchanged. Expect changed helper/test bodies reprepare and unchanged support native code reused. A second variant edits helper body only while test checks a property valid for both values, isolating caller reuse.

**E003 — Negative lookup becomes positive.** First run reaches missing function and faults. Add its declaration, keep caller body bytes unchanged, rerun. Expect negative lookup invalidation and successful execution, not a cached failure. Conversely removing a used function must invalidate a cached success.

**E004 — Candidate-set change.** Add a competing unbounded impl relevant to a reached method name. Expect deterministic dispatch ambiguity and no reuse of old selected-target fact. A separate unrelated impl edit must not invalidate a disjoint method bucket.

**E005 — Schema and location changes.** Change a used struct field descriptor and assert dependent TypeId/artifact invalidation. Separately insert newlines before an unchanged function with line!; output line changes while position-independent native region code remains reusable. Diagnostic spans must point to the new line too.

**E006 — Guest reset.** Execute the same test100 times with new run epochs. Captured output, variables, cells, and broker resources are fresh; source/query/native caches reuse. Host test deliberately attempts an old handle through a test-only checked API and receives stale-handle failure.

### 28.7 Permissions, Cargo, and protocol fixtures

**P001 — Deny then grant.** Fixture file contains `hello`. First run without grant calls read_to_string and sees PermissionDenied. Second run with read grant returns hello. Third run revokes grant while reusing code and again denies. File contents must never appear in an unauthorized result or trace.

**P002 — Escape attempts.** Separate cases for `../secret`, absolute path, `fixtures2` vs granted `fixtures`, nested escaping symlink, encoded NUL, and a write through a read-only grant. Expect denied errors and unchanged outside files. Test normal in-root symlinks according to the pinned cap-std behavior; only successful confined resolution is accepted.

**P003 — Environment/output limits.** A host environment secret is absent from guest var unless explicitly injected. stdout exceeds quota and produces the specified resource fault. Trace logs redact injected values. --nocapture cannot bypass the quota.

**P004 — Cargo units and explicit tool authority.** A two-package workspace with aliases, forwarded features, optional and target-specific dependencies prepares/runs through the exact unit graph. Root test intent does not enable dependency cfg(test). Without build-tool or macro authority, their sentinels do not execute and a required action fails at its own phase. With explicit authority, build/proc variants prepare/expand successfully without checking live runtime bodies.

**P005 — Stdio cancellation and snapshot atomicity.** Open, run loop, cancel, apply a two-file batch, rerun. Responses retain correct request/run/snapshot IDs; no mixed old/new helper pair executes. Oversized/malformed/truncated JSON cannot mutate an active snapshot or lead to unbounded memory.

**P006 — Worker and broker failure.** Inject a worker abort, a broken effect pipe before reply, and a stuck broker IO operation in dedicated child processes. Expect bounded termination, explicit crash/indeterminate-effect/deadline classification, and no retry of a possibly completed write.

### 28.8 Completion of the matrix

These concrete cases are the minimum regression set, not a substitute for inventory coverage. For each feature/API/opcode not individually covered above, add a smallest positive fixture and a reached error fixture. Add a cold-lazy fixture when that feature can occur in an untaken expression or uncalled body. `xtask coverage-audit` maps every compat manifest entry to actually executed fixture IDs and fails on an uncovered entry.

<a id="section-29"></a>

## 29. Property testing, model checking, and regression reduction

### 29.1 Generator families

Use proptest with deterministic persisted seeds and bounded sizes. Never generate arbitrary Rust and assume every failure is a runtime bug. Each generator has a declared semantic domain and an independent oracle appropriate to that domain.

1. **Primitive expression generator.** Generate typed scalar trees up to depth8 with explicit types, literals, unary/binary ops, and small conditionals. Use the shared checked primitive specification as a local oracle and native Rust for supported well-typed sampled cases. Compare values/panics across interpreter and JIT. Generate an untaken malformed child separately and assert it is not prepared.
2. **Control-flow generator.** Generate bounded loops, blocks, named calls, early returns, and event-emitting helpers. The model records the ordered event list and return value. Compare event lists exactly, including guard false paths and call arguments.
3. **Ownership state model.** Generate allocate, initialize, copy, move, partial move, borrow, replace, drop, forget, and read operations on a small abstract heap. The model tracks owners, epochs, live fields, and expected faults. After every valid operation compare counts/states; after fault ensure no additional guest Drop is emitted.
4. **Query dependency model.** Generate acyclic derived-fact graphs plus selected cycles, input edits, unchanged-output recomputation, and missing-name buckets. Compare memoized output to full recomputation and verify provider-count invariants. Include negative dependencies and complete candidate-set changes.
5. **Region exit model.** Generate short observed op sequences with controlled guard misses/panics/effects at each position. Compare interpreter and native canonical states after exit, not merely final output. No operation before the exit can be executed twice.
6. **Protocol/limits generator.** Generate valid/invalid length framing, UTF-8 JSON strings, out-of-order source chunks/replies, stale run IDs, and byte limits. Verify bounded failure and no partial publication.

Run 1,000 generated cases per family in the ordinary gate and 10,000 per family in the release stress run. Arithmetic fast-path pair tests additionally use the counts in T081. Record test version, seed, shrink history, and final reduced input.

### 29.2 Mutation checks

Before trusting the harness, temporarily inject these bugs behind test-only mutation flags: ignore one negative dependency; restart native guard exit at region beginning; skip a fuel charge; treat every handle as Copy; omit Vec buffer-version validation; select first impl in map order; allow filesystem string-prefix matching; and accept unknown helper symbols. Each mutation must cause at least one named test to fail. Remove/disable mutation flags in release binaries; keep mutation tests as host-harness demonstrations.

This is not a proof of correctness. It establishes that the chosen tests can detect the exact failure modes the architecture is designed to avoid.

### 29.3 Reduction algorithm

For a failing source fixture, repeatedly try deleting unexecuted declarations, replacing unused statements with unit, shrinking integer literals, reducing loop bounds, removing unmatched match arms, and shrinking aggregate fields while preserving the same failure category and distinguishing oracle behavior. Apply one candidate edit at a time to a copy, recapture, and rerun the predicate. Never mutate the original fixture until a reduced result is confirmed.

For a query/state/region model failure, shrink the operation sequence and object/index space using proptest's shrinker plus a deterministic deletion pass. Persist the smallest found case with its expected outcome and invariant ID. Add the fixed case to the ordinary fast regression suite so the issue cannot reappear only in a long random run.

### 29.4 Unsafe and process stress

Run host memory-model tests under Miri where compatible, excluding actual Cranelift execution. Use native subprocess stress for JIT helpers and code lifetime. Miri is a host implementation-checking tool here, not the guest interpreter and not a requirement on users running LRR. The release runbook records whether that supplementary tool was available; mandatory native/property gates do not claim Miri coverage when it was not run.

Perform 1,000 source-edit/run cycles with tiny code-cache budgets and 10,000 test-run resets with small heaps. Track live guest handles at successful cleanup, code arena counts, pinned metadata, and process RSS. A monotonically growing retained structure must be explained by an explicit bounded cache or fixed by reclamation. Passing only a single run is insufficient for a persistent runtime.
<a id="section-30"></a>

## 30. Performance evaluation and acceptable claims

### 30.1 What must be measured

Measure end-to-end latency from a submitted source snapshot/run request to final selected-test result, not just time inside generated machine code. Separate cold process startup, initial source parsing, query/IR preparation, actual interpretation, JIT compilation, execution, broker wait, and teardown. For warm runs retain only the caches allowed by this specification; create a fresh guest heap every time.

The purpose of this architecture is to avoid unnecessary semantic/codegen work and improve iterative testing on its supported subset. It is not guaranteed to beat optimized native Rust, rustc incremental compilation, or a tuned Cranelift AOT workflow. In particular, helper-heavy small regions can be slower than an optimized interpreter. Benchmark evidence decides the claim; it does not justify deleting required semantic checks.

### 30.2 Fixed benchmark suite

Create these datasets and keep generator seeds/source sizes in the repository:

| Dataset | Construction | Question |
|---|---|---|
| B01 cold selected test | One live function and 10,000 uncalled syntactically valid function bodies, some semantically invalid | How much parse/index work is paid, and is body semantic work confined to the selected path? |
| B02 untaken large branch | One called function with a tiny taken branch and a large cold branch | Does structural body indexing dominate, and are cold thunks/codegen still zero? |
| B03 warm unchanged test | Run the same selected test 100 times in one worker with fresh guest state | What is the real cache-validation/dispatch floor? |
| B04 cold-body edit | Edit an unrelated uncalled body before each run | Does header/body splitting preserve reached artifacts? |
| B05 called-body edit | Change one small called helper per run | How quickly does invalidated code become executable without a native rebuild? |
| B06 arithmetic loop | J001 scaled to enough iterations for stable timing | Does direct scalar JIT work offset helper/staging costs? |
| B07 call-heavy recursion | Small concrete recursive and mutually recursive functions | What is dispatcher/call-contract overhead? |
| B08 Vec/String processing | Parse no external formats: build strings, count chars, push integer results into Vec | What does managed-memory/facade overhead cost? |
| B09 query candidate changes | Add/remove relevant and irrelevant impl/name declarations | Are invalidation scope and validation costs bounded and correct? |
| B10 brokered fixture IO | Read small/large permitted regular files and deny an ungranted file | What are IPC/permission costs without bypassing the broker? |
| B11 Cargo preparation | Accepted two-package and 20-package local source workspaces | What is explicit setup cost, separate from each run? |
| B12 longevity | 1,000 edits and 10,000 run resets under constrained cache budgets | Does memory remain bounded and code reclamation remain safe? |

B01 and relaxed-invalid B02 have no native Rust end-to-end baseline because rustc correctly rejects their malformed cold code. Create separate valid versions for comparison and label them distinctly. Do not call a faster relaxed run a compiler speedup over a build that could not legally complete.

### 30.3 Measurement procedure

Build the host runtime in release mode once. Record CPU model, core count, RAM, kernel, filesystem, runtime/compiler versions, build flags, source bytes, functions, packages, and selected test. Isolated steady-state benchmarks run without competing compilation jobs. Separately measure end-to-end automatic tiering with its normal background job, including its CPU contention and publication costs. Preserve raw measurements rather than only an average.

For cold startup, use 30 separate coordinator/worker processes and state whether OS filesystem caches were warm; do not claim a truly cold filesystem without controlling it. For warm and edited latency, perform five warmup iterations then 100 measured iterations. For throughput, use 30 measured samples whose loop counts produce a nontrivial duration without exceeding fuel/resource ceilings. Raise fuel through the documented flag consistently across modes; record it.

Report median and the 95th percentile by sorting samples and selecting the nearest-rank percentile. The median is the middle sample (average the two central samples for an even count). The 95th percentile is the sample at ceiling(0.95 × sample_count), using one-based indexing. Report the sample count and spread. No unexplained significance test or claim of statistical certainty is required.

Compare LRR off/auto/force. A force mode is a correctness/execution exercise, not necessarily the fastest mode. Include JIT compilation in first-use timing; present steady-state native execution separately and clearly. Do not omit failed samples or watchdog events; list their classification.

### 30.4 Targets, not invented results

Set these engineering targets and report achieved/missed after implementation:

- Uncalled/untaken executable nodes: zero semantic thunks and zero native code, with parsing/indexing reported separately.
- Warm unchanged source: zero Thunk-provider recomputation for previously reached recipes; valid artifacts reused. Query dependency validation is allowed.
- Cold-body edit: zero retranslation of unchanged reached bodies, except a declared global configuration/candidate dependency actually changed.
- Warm small supported test: target median below 50 ms in the persistent service on the recorded reference machine, excluding explicit initial Cargo preparation but including per-run query validation and result delivery.
- Arithmetic steady-state: target at least 1.5× the off-mode throughput when direct scalar operations dominate, with all guards/fuel and equivalent work retained. This may be missed by the deliberately simple helper-based ABI.
- Long-running worker: remain within declared caches/RSS policy with no stale pointer/handle and no accumulating guest state.

The first three and memory correctness are mandatory architectural gates. The latency/throughput figures are hypotheses and nonblocking broad-workload performance targets for 0.2; Section 61's near-native claim additionally requires its matched-baseline performance gate; measured misses must be published prominently. Shipping a useful correct prototype with an accurately reported missed throughput target is preferable to claiming a speedup obtained by skipping work. No number in this document is a measured performance result.

### 30.5 Permitted optimization process

When a target is missed, use profiling to identify the dominant component, change the responsible representation or lowering within the revision-5 architecture, and rerun all equivalence/laziness tests. Candidate local changes are prebinding worker-local IDs, reserving host Vec capacity from known bounded structure, avoiding repeated type-name formatting, using a direct active-run PC table, and sharing immutable source/descriptor storage.

Implement the prescribed optimizing tiers in Section 62 and rerun equivalence/laziness tests after each integration. Remove dynamic checks only with preserved proofs; never adopt unchecked native aliases, suppress overflow, precompile cold live blocks, mutate active snapshots or retain guest state between tests. Resource-contract changes must be explicitly selected and reported. Cross-boundary inlining uses prepared IR, never guessed machine code.

<a id="section-31"></a>

## 31. Release runbook and completion evidence

### 31.1 Required generated artifacts

A completed repository contains: `Cargo.lock`; vendor provenance/license files; task ledger with all tasks complete; compat manifests with all fixture links; native-oracle fixture results; off/auto/force equivalence results; query/edit transcripts; permission/protocol stress results; actual JIT entry/CLIF evidence; raw benchmarks plus a summarized report; and the final two-package demo transcript.

Generated artifacts belong under `artifacts/` and include a version/build hash. Do not commit binaries, native code pointers, guest secrets, or large unbounded logs. Keep small deterministic fixture expectations and regression seeds under version control. Large run artifacts can be archived separately, but a release manifest must identify their digest and location.

### 31.2 Clean build sequence

Run from the repository root on the selected macOS target first. Repeat on Linux x86-64 during the later port:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
cargo +1.99.0 metadata --format-version 1 --locked > artifacts/host-metadata.json
cargo +1.99.0 fmt --all -- --check
cargo +1.99.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 build --release --locked -p lrr
cargo +1.99.0 run --release --locked -p xtask -- verify-all
cargo +1.99.0 run --release --locked -p xtask -- demo
cargo +1.99.0 run --release --locked -p xtask -- release-manifest
```

Create `artifacts/` before redirection (run `mkdir -p artifacts`). Host dependency fetching during initial repository setup is distinct from guest offline preparation. After the host lock/vendor/cache are available, repeat the release check with host Cargo `--offline` to prove reproducibility from the populated cache. Never report this as proof that an empty machine can build without downloading the declared dependencies.

`clippy -D warnings` applies to local production/test modules, excluding unchanged upstream vendored warnings only through a narrowly scoped documented dependency build configuration. Do not disable broad warning classes to hide actual guest-input unwraps or unsafe-contract problems.

### 31.3 Demo script

`xtask demo` creates an accepted two-package workspace in a temporary directory, prepares it offline, and starts the built lrr stdio service. It submits the following request sequence and asserts every response:

1. Open the prepared library-test target and list two tests without body preparation.
2. Run the selected good test while the other contains unresolved names and a cold closure.
3. Run the hot loop enough times to show actual native entries and direct scalar operations.
4. Run Vec/String transformation and verify its result against the manifest expectation.
5. Read a fixture without grant and verify PermissionDenied; grant its directory and verify content.
6. Apply one atomic source batch changing a called helper and a matching assertion; rerun successfully and inspect precise invalidation.
7. Change only the broken cold function; rerun and prove unchanged hot artifacts reused.
8. Remove the read grant and rerun the same code; verify denial without requiring recompilation.
9. Cancel a bounded infinite loop and verify the worker/coordinator remain in a defined state.
10. Run the native oracle separately on the valid counterpart fixture and record its compiler/toolchain result; this is explicitly outside guest execution.
11. Shut down and verify process/resource cleanup.

The demo records timestamps and actual counters but does not impose a performance promise on arbitrary hardware. It must fail rather than merely print an explanation if any required behavior is missing.

### 31.4 Final release checklist

Release only after all of these are true: selected test execution works; cold bodies/branches remain unprepared; interpreter and native semantics agree for the full fixture matrix; actual native/scalar execution is demonstrated; guest ownership/reference invariants hold; query negative dependencies invalidate correctly; code/metadata reclamation is bounded; Cargo subset and preparation errors are explicit; grants deny by default and survive cache reuse correctly; unsupported hardened-sandbox request fails closed; host-input failures do not become unbounded hangs; and documentation states the relaxed profile and absent rustc validation.

Do not advertise “runs any Rust crate,” “zero compilation,” “borrow-checker compatible,” “fully sandboxed,” or a speedup without scoped evidence. The release target is **a lazy Rust interpreter for the admitted 2015/2018/2021/2024 language profile, with Cranelift and LLVM optimizing tiers, a default capability-mediated mode and explicitly unsandboxed Cargo-native integration, designed for repeated live-code tests without recompiling the live crate.** Describe current implementation separately. Revision 5 requires every edition/native gate in Section 44, every expansion/trait/cache gate in Sections 57–59, direct ABI and optimizing acceptance in Sections 61–62, all HN/MX/TR/NX/CA fixtures, and the profile/freshness/authority/reset/contract labels. Earlier core gates are component evidence, not an independent release. Passing the original managed gates alone does not complete this release.

<a id="section-32"></a>

## 32. Instructions to the implementation agent

Copy the following operating rules into AGENTS.md, with a link to the checked-in specification:

> Implement the fixed LRR architecture and task ledger in order. Do not choose a different frontend, query framework, native backend, language profile, sandbox backend, or tiering strategy. Preserve explicit interpreter/JIT/capability semantics. Before changing code, identify the task and invariant being implemented. Add its acceptance test before marking the task complete. Run the current milestone gate after each integrated task. Never mark a selected fixture passed by skipping it, catching all errors as success, or hard-coding an expected output. Never load arbitrary native runtime symbols or invoke Cargo, rustc, a linker, or build scripts during live execution. Explicit authorized tool preparation may build compile-time tools and capture build outputs. Live procedural expansion may execute only in the matched helper under its independent macro grant; never run it inside a pure query provider. Explicit authorized native preparation creates the immutable schema-checked generation loaded only in an authorized hybrid worker. Use pinned dependency source to repair mechanical API mismatches. Record an actual contradiction as a minimal witness plus regression and erratum, not an undocumented design fork. Finish with reproducible release artifacts and an honest feature/performance/security report.

### 32.1 Handling a discovered specification contradiction

No long design can guarantee the absence of implementation-level mistakes. The repair procedure is nevertheless determined:

1. Reduce the contradiction to the smallest source/state/protocol example and identify the conflicting paragraphs/invariant IDs.
2. Preserve safety and exactly-once effects first, snapshot/cache correctness second, path laziness third, declared valid-Rust subset behavior fourth, and performance last. These priorities cannot be reversed to make a benchmark pass.
3. Choose the smallest local correction consistent with the locked architecture and closed profile. For supported syntax, use the pinned native oracle only where this document intends Rust-equivalent behavior; do not use rustc rejection to invalidate an intentionally relaxed fixture.
4. Add the witness as a failing regression, implement the correction, rerun affected earlier gates, and record it in docs/spec-errata.md with the changed rule and test ID.
5. An external API/package failure remains a reproducibility blocker until evidenced and resolved; it does not authorize switching backends, broadening scope, or silently upgrading.

This is a maintenance process, not a menu of designs. The execution agent should not stop to ask architectural preference questions already answered here.

### 32.2 Mechanical implementation defaults

Use BTreeMap/BTreeSet for deterministic control-plane indexes; Vec/typed indexed tables for hot runtime data; Arc for immutable source/query data; no Rc/RefCell guest ownership model; no async runtime; no unsafe outside the three declared JIT modules and the two reviewed hybrid boundary modules `native/loader.rs` and `native/ffi.rs` in local production code. Generated bridge entry points have a separately audited narrow unsafe shell; generated Rust API calls and codecs remain safe Rust. Return typed Result/Outcome enums for recoverable errors. Use checked arithmetic and try_reserve for guest-influenced sizes. Use stable explicit encodings rather than serializing Rust memory layout.

New helper functions need unit tests and effect classifications. New diagnostics need manifest entries. New source reads need snapshot dependencies. New mutation paths need ownership/epoch/quota tests. New native op lowering needs a staging guard, exact commit/exit behavior, and differential boundary cases. A new dependency or supported guest feature is out of scope for this release unless a mechanical requirement already specified here cannot otherwise be met and an erratum records the narrowly necessary fix.

### 32.3 Evidence versus assumptions

The implementation report distinguishes: implemented and tested behavior; implemented but not exercised supplementary checks; unsupported profile features; measured performance; target misses; and unresolved defects. Do not describe a planned acceptance test as already passing. No reasoning argument replaces the required execution evidence. No test result establishes full Rust language equivalence or hostile-code sandbox security.

<a id="section-33"></a>

## 33. Primary sources and dependency provenance

The architecture and numeric/dispatch/ownership/profile decisions are prescriptions of this document. Sources below support specific upstream API/compiler facts; they do not endorse the whole design or prove its implementation. Upstream pages were consulted on October 4, 2026. Runtime/dependency versions are pinned in Section 3 and must be verified by bootstrap; no joint build was performed while writing this specification.

[^R01]: Rust compiler developer guide, [THIR](https://rustc-dev-guide.rust-lang.org/thir.html) and [MIR construction](https://rustc-dev-guide.rust-lang.org/mir/construction.html). Used to establish the typed-frontend dependency of the normal compiler IR path, not to claim that rustc cannot ever be modified.

[^R02]: rust-analyzer, [Architecture](https://rust-analyzer.github.io/book/contributing/architecture.html). Used for the syntax-layer reuse boundary and separation of item structure from body semantics.

[^R03]: Rust compiler developer guide, [Queries](https://rustc-dev-guide.rust-lang.org/query.html), and Salsa, [Overview](https://salsa-rs.github.io/salsa/overview.html). Background for input/derived-fact memoization. LRR implements its own execution-query algorithm; the isolated upstream macro adapter may retain a private Salsa interner without exposing body inference.

[^R04]: Rust project, [Announcing Rust 1.99.0](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/). Toolchain version/date reference; actual package compatibility is a bootstrap gate.

[^R05]: `ra_ap_syntax`, [package manifest](https://docs.rs/crate/ra_ap_syntax/0.0.356/source/Cargo.toml) and [SourceFile API](https://docs.rs/ra_ap_syntax/0.0.356/ra_ap_syntax/ast/struct.SourceFile.html). Parser version, required toolchain/edition metadata, parse API, and syntax-node ownership boundary.

[^R06]: `cranelift-jit` 0.136.2, [JITModule API](https://docs.rs/cranelift-jit/0.136.2/cranelift_jit/struct.JITModule.html). Native definition/finalization and memory-lifetime constraints.

[^R07]: Versioned package documentation for [serde](https://docs.rs/serde/1.0.229/serde/), [serde_json](https://docs.rs/serde_json/1.0.151/serde_json/), [blake3](https://docs.rs/blake3/1.8.7/blake3/), [clap](https://docs.rs/clap/4.6.7/clap/), [toml](https://docs.rs/crate/toml/1.1.6+spec-1.1.0), [cap-std](https://docs.rs/cap-std/4.0.3/cap_std/), and [proptest](https://docs.rs/proptest/1.11.0/proptest/). Individual version provenance, not evidence that the complete dependency set was built together.

[^R08]: Rust Reference, [Operator expressions](https://doc.rust-lang.org/reference/expressions/operator-expr.html) and [Expressions](https://doc.rust-lang.org/reference/expressions.html). Primitive arithmetic, cast, overflow, and operand-order reference for the supported subset. LRR's additional dynamic guards and strict inference policy are its own profile rules.

[^R09]: Rust Reference, [Destructors](https://doc.rust-lang.org/reference/destructors.html). Source for specified field/drop order and edition-sensitive temporary behavior. The document freezes an explicit supported 2024 annotation algorithm and requires native differential tests rather than assuming all current language behavior is implemented.

[^R10]: Rust Reference, [Method-call expressions](https://doc.rust-lang.org/reference/expressions/method-call-expr.html). Receiver-candidate ordering reference. LRR resolves necessary header goals without a global coherence pass; symbolic member identity and concrete fulfillment are separate.

[^R11]: `cranelift-jit` 0.136.2, [JITBuilder API](https://docs.rs/cranelift-jit/0.136.2/cranelift_jit/struct.JITBuilder.html). Explicit symbols, platform fallback, and additive callback ordering. Motivates the narrow vendored clear-lookup patch rather than an ineffective None-returning callback.

[^R12]: `cranelift-jit` 0.136.2, [backend source](https://docs.rs/cranelift-jit/0.136.2/src/cranelift_jit/backend.rs.html). Inspected builder lookup storage and x86-64 relocation/PIC setup. The local patch changes only fallback lookup availability.

[^R13]: Cargo book, [cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) and [Dependency resolution](https://doc.rust-lang.org/cargo/reference/resolver.html). Metadata interface and the reason project compatibility must declare feature/unit limitations instead of treating the package graph as a complete executable build plan.

[^R14]: `cap-std` 4.0.3, [Dir API](https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html). Directory-capability operations used by the host broker. The application still needs correct grant selection, path validation, quotas, and integration tests.

[^R15]: Deno documentation, [Security and permissions](https://docs.deno.com/runtime/fundamentals/security/). Permission-interface inspiration only; LRR does not claim Deno-equivalent implementation or containment.

---

<a id="section-34"></a>

## 34. All official editions: required semantics and migration gates

### 34.1 Edition identity is part of every executable contract

`lrr-2024-v2` is the only runtime profile. Select Edition::Edition2015, Edition2018, Edition2021 or Edition2024 from the owning standalone or Cargo root declaration; preserve each dependency unit and macro token context's declared edition for its syntax and edition-sensitive semantics. Standalone run/test accepts --edition with these four values and defaults to 2024; Cargo roots use their target edition. Set generated packages to `edition = "2024"`. Every parse key, crate key, temporary-scope recipe, pattern recipe, portable cache header, bridge schema, and oracle fixture includes the edition/profile digest. Reject revision-2 cache records; never relabel their cached recipes as 2024. The edition's initial release was Rust 1.85; that is a minimum edition support fact, not a promise that every newer feature is in the live subset.[^H03]

A native dependency retains its declared edition and native semantics. Live source dependencies and macro-export facets likewise retain their own editions, with the supported-operation compatibility rules in Section 34.7. No released edition is rejected solely because it predates 2024. Unknown or unreleased selected editions fail LRR-E-LIVE-EDITION; unknown source editions fail LRR-E-TOOLCHAIN-CAPABILITY. Every declared edition remains part of the unit identity.

The runtime host toolchain remains the pinned toolchain in Section 3. The bridge toolchain is recorded independently in native preparation. No native Rust ABI crosses from a bridge built with that toolchain into LRR's host Rust code. Section 39's C ABI is the compatibility boundary. Changing either toolchain invalidates the corresponding cache, not automatically all unrelated caches.

### 34.2 If-let cleanup algorithm

Introduce `ConditionalScope { clauses, consequent, result_destination }`. Each pattern clause owns a `ClauseScope` with scrutinee temporaries and successfully committed bindings. Its bindings are visible to later clauses and the consequent, never to the else branch.

Execute a single if-let as follows:

1. Enter the condition scope. Evaluate its scrutinee exactly once, using the existing eager operand order and construction guards.
2. Run the existing non-consuming pattern test against the scrutinee. A mismatch is ordinary control flow, not a panic.
3. On match, commit bindings and enter the consequent. Transfer the consequent's owned result to the surrounding expression destination before cleaning consequent-local storage.
4. Clean the consequent, then the successful clause's bindings and remaining scrutinee temporaries, then leave the condition scope. Continue to the join.
5. On mismatch, clean scrutinee temporaries and leave the condition scope **before** starting the else expression. The else has an independent scope. An absent else yields unit after cleanup.
6. On break, continue, return, or guest panic, use the same scope stack; do not invent a separate edition-specific unwinder. On fatal managed faults, retain the original no-user-cleanup rule.

The Rust 2024 change specifically narrows failed if-let temporaries before else. A syntactic conversion to a match expression would not preserve that change because a match scrutinee has different temporary-scope behavior.[^H01] Do not reuse the old if-let lowering under a new profile string.

### 34.3 Block-tail algorithm

Introduce `TailScope` for the complete final expression of every entered block, including a function body. Allocate only temporaries actually produced while executing it. Evaluate the tail; transfer its resulting owned value to its outer destination; clean the tail's remaining non-extended temporaries; then clean the block's locals. On a control transfer from inside the tail, the ordinary scope stack produces the same inner-to-outer cleanup.

A result moved out of the tail is not a temporary to destroy there. A borrowed result remains a reference: if its referent dies at the narrowed boundary, subsequent managed access fails the existing liveness guard. Do not extend a referent simply because a returned reference needs it. Explicit syntactic let-extension annotations still override the default temporary boundary. This preserves the distinction between Rust 2024's narrowing rule and the separate let-extension rules.[^H02]

### 34.4 Required let-chain subset

Support top-level `&&` sequences in `if` and `while` conditions containing both boolean clauses and `let PATTERN = EXPR` clauses. The language remains eager and left-to-right. Later clauses are neither prepared nor executed until all preceding clauses succeed. A binding from a successful earlier clause is in scope for later clauses and the consequent. This is the edition-gated let-chain form documented in the Edition Guide.[^H04]

`ConditionPlan` contains a structurally indexed vector of clause node IDs, not prepared executable bodies. Creating it does not resolve clause operands. At runtime, keep `next_clause`, a vector of active pattern clause scopes, and a destination for the selected branch. Boolean clauses use a short temporary scope and destroy their temporaries after evaluating the boolean, before entering the next clause. Pattern clauses retain their storage through subsequent clauses and the consequent.

A false boolean or failed pattern exits the chain. Clean active clause scopes from last to first; within a clause clean committed bindings before its remaining scrutinee temporaries, each in the established reverse creation order. Then enter else or leave the while loop. A successful while chain has a fresh set of clause scopes for each iteration; continue cleans the iteration and condition scopes before reevaluation. Break cleans them before reaching the outer destination. The condition itself is not replayed after a cleanup suspension.

A `let` clause under `||`, logical negation, or a non-top-level arithmetic/call expression is unsupported and errors only when that malformed reached condition must be prepared. Parentheses around an ordinary boolean operand are supported. Do not flatten a parenthesized condition containing let clauses into a different grammar. Pattern-matching match guards and let chains in match guards are outside this release even if a later pinned compiler supports them; ordinary boolean match guards remain supported.

### 34.5 Match ergonomics under the 2024 profile

Carry `DefaultBindingMode = Move | Ref | RefMut` through each reached pattern. Begin with Move. Implicitly peeling a shared reference changes the mode to Ref. Peeling a mutable reference changes Move to RefMut but never upgrades Ref to RefMut. Explicit reference patterns may consume their reference layer only while the current mode is Move.

Before applying an explicit `mut`, `ref`, or `ref mut` modifier to an identifier, require mode Move. Otherwise return `LRR-E-EDITION-PATTERN`. In particular, `mut` does not reset an inherited mode to Move. The Edition Guide documents these 2024 restrictions.[^H05] This is a check of the pattern actually being matched, not a global type-checking pass.

The existing non-consuming test/commit split remains mandatory. Testing a pattern never copies an owning value. A failed pattern cannot move fields or create lasting native borrows. Apply the mode restriction at the node reached by matching; do not reject an unvisited match arm solely because its binding form would be invalid if selected. Whole-file syntax errors still follow the parser recovery policy in Section 8.

### 34.6 Keywords, prelude, macros, and unsafe syntax

Reserve `gen` under the 2024 parser, preserve raw identifiers such as `r#gen`, and retain token distinctions needed for reserved guarded-string syntax. These are grammar/lexing rules, not permission to execute generators. The supported-profile manifest must explicitly mark generators as unsupported.[^H06]

Reserve the 2024 prelude's async-related trait names as named unsupported facade identities. A request requiring their executable trait semantics produces `LRR-E-FEATURE-ASYNC`; do not silently resolve a different glob-imported method because the prelude entry was omitted. Full async, Future execution, and closure support are still excluded. The native island can use those features internally behind a supported synchronous wrapper without teaching the live runtime to execute them.[^H07]

Guest macro_rules expansion uses the pinned matcher with definition-edition fragments: expr and expr_2021 are distinct in 2024. Token origin editions and hygiene survive expansion. A macro-export facet from an older-edition crate must not be reparsed as a 2024 definition. Section 49 and MX009 cover both matching and emitted-code edition behavior.[^H08]

Generated host/bridge code uses `#[unsafe(no_mangle)]` for exported entry points, explicit `unsafe extern "C"` declarations when needed, and explicit unsafe blocks around each raw pointer operation. These are generator requirements even though live guest unsafe checking is not implemented. Direct user-authored foreign ABI calls remain unsupported; `unsafe { ... }` in live code does not enable arbitrary dynamic symbol access.[^H09]

### 34.7 Mandatory edition fixtures

Create these native-oracle fixtures first, compiling the oracle with `--edition=2024`. Check the actual oracle output into the fixture directory and assert the specified relationships; any mismatch blocks the edition gate rather than being explained away as profile relaxation.

| Fixture | Required case and assertion |
|---|---|
| E2401 | If-let mismatch with a Drop-observable scrutinee: its drop occurs before `else_entered`. |
| E2402 | If-let success borrowing the scrutinee: access works in the consequent; consequent locals drop before the scrutinee; `after_if` is last. |
| E2403 | Block containing an ordinary local and a Drop-observable temporary in its tail: tail temporary drops before the local. |
| E2404 | `let x = &Temporary(...)` and tuple/struct extending forms: their referents survive the let statement and drop at the containing block boundary. |
| E2405 | Let chain with an initial false boolean and an invalid later call: later call has zero preparation/evaluation counters. |
| E2406 | Let chain whose second pattern fails: earlier bindings and temporaries clean up before else; earlier effects occur once. |
| E2407 | While let chain with continue and break: every iteration gets distinct cells; no previous iteration's binding remains live. |
| E2408 | Fully explicit reference patterns accepted; inherited-mode explicit modifiers yield `LRR-E-EDITION-PATTERN` only on the reached pattern. |
| E2409 | `r#gen` declaration and call work; unrecoverable bare-keyword declaration has the parser's edition diagnostic, not a missing-symbol fallback. |
| E2410 | Existing tail/if-let fixtures migrated to 2024 run identically in interpreter and forced-JIT modes, with identical drop/effect transcripts. |
| E2411 | Edition/profile change invalidates the parse, pattern, temporary, and executable recipe caches even for identical bytes. |
| E2412 | A live 2024 test calls an opaque older-edition native library; the library is not fed to the 2024 live parser. |

Use local fixture types and `println!` in Drop implementations; no external dependency is necessary. For E2401, a tuple struct containing a discriminant and a drop-logged field lets a pattern mismatch while guaranteeing an observable scrutinee destructor. For E2403, use a temporary whose method returns a scalar so the temporary itself is not moved out as the block result. Do not write fixtures that accidentally transfer the object being measured and then assert a nonexistent temporary drop.

### 34.7 Dependency and generated-token edition compatibility

Selected roots support all four released editions. For supported operations originating in roots, dependency units or macro tokens from 2015, 2018, or 2021, preserve their effective edition rather than changing it to the root edition. Determine the relevant token/span edition using the pinned syntax/expansion adapter and the compiler's corresponding edition-sensitive rule. Different tokens in an expansion can have different origins; a single global edition flag is insufficient.

Implement the pre-2024 temporary rules for those origins: an if-let scrutinee is not narrowed before else, and block tails do not introduce the 2024 short temporary boundary. Existing explicit extending-let rules still apply. Preserve pre-2024 match ergonomics, including mut resetting the binding mode to move and explicit ref/ref-mut/reference-pattern behavior accepted before 2024. Add native oracle pairs with the same supported source compiled in its owning edition. A macro from an older crate that emits the relevant construct gets its own mixed-edition pair, using the actual compiler's span-edition decision as the regression oracle.

Use the owning unit's edition prelude: 2015/2018 v1 set, 2021's additional conversion/iteration names, and 2024's additional async-related reserved identities. Rust2015 root and dependency source uses its explicit extern crate/import rules; register extern crate aliases and macro_use imports structurally rather than assuming the 2018 extern prelude. Unsupported executable features remain excluded in every edition; parsing an old keyword as an identifier must not make it a 2024 keyword accidentally.

For method syntax into_iter, preserve the compatibility exceptions for arrays before 2021 and boxed slices before 2024. Do not hide the underlying IntoIterator impl from explicit trait calls or for loops; the exception belongs to source method lookup. The method token's effective edition remains a ResolveMember input, including macro-generated method calls.[^M17] Extend M005/M007/M025/M030 and MX009/TR019/TR034 with these edition-paired cases. This is required compatibility behavior within the same frontend, not another runtime backend.

<a id="section-35"></a>

## 35. Native integration: locked design and exact reuse boundary

### 35.1 One engine and prepared native entries

```text
live source -> demand-driven frontend -> interpreter / Cranelift / LLVM
                                                     |
                              exact binding and ownership contract
                                    /                         \
                     Rust function-pointer ABI        explicit codec-v1
                       direct or catch entry            generated C entry
                                    \                         /
                                  one immutable native generation
                                   optimized Cargo dependencies
```

`rust-abi-v1` is the default binding transport. Ordinary native operations retain native-resident values across calls; codec-v1 is selected only for an explicitly requested conversion boundary, never as generated default glue or an automatic fallback. Section 61 defines exact producer ABI export, native storage and direct/caught entries. `codec-v1` remains an explicit conversion transport defined by Sections 39.2–39.7. The C control surface handles registration/storage and typed interpreter invocation; it is not the direct call's argument representation. Section 62 permits LLVM to inline eligible prepared native bodies while retaining the same generation and ownership identities.

Managed mode loads no native dependency generation. Hybrid mode explicitly authorizes a dependency-closed set of Cargo library units linked into one immutable generation. The guest heap does not become layout-compatible with Rust; only validated native residents, call-scoped leases and proven typed views can cross directly. Arbitrary symbols, live/native type unification, compilation of live Cargo units and native-to-live callbacks remain forbidden.

### 35.2 What existing artifacts actually provide

An `.rlib` is a compiler-interpreted intermediate library, not a dynamic plugin; `.rmeta` is metadata, not executable code. Cargo/rustc remain responsible for artifact selection and monomorphization. The exact-version preparation adapter uses rustc ABI/layout queries during the generated native crate compilation and can capture concrete LLVM IR. Live query providers never decode rustc metadata, compile a specialization or select a symbol by name. Registration exposes only manifest-declared typed pointers and explicit codecs.[^H10][^H11]

The implementation reuses **compatible Cargo build products**, not arbitrary bytes found under a directory named `target`. Cargo's build directory can be separately configured, and its intermediate layout is internal. Use Cargo's artifact messages for actual paths.[^H12] Never require the path to literally contain `target/release/deps`, and never choose the newest-looking hash-suffixed library by filename.

Cargo incremental state remains Cargo/rustc-owned. There is no loader for `incremental/` object shards, query caches, dependency graphs, or work-product indexes. LRR benefits when Cargo subsequently uses those caches during native preparation. This is distinct from reusing an unchanged compiled dependency without invoking its compiler at all.

### 35.3 The selected decisions

| Decision | Fixed implementation | Accepted cost |
|---|---|---|
| Reuse authority | Cargo builds a generated member in the original workspace and reports which artifacts were fresh | Initial preparation may rebuild units whose configuration does not match existing artifacts |
| Native call boundary | Compiler-described Rust function-pointer ABI by default; explicit codec-v1 conversions remain supported | Unsupported signature/producer cases fail before entry; caught panics retain an adapter |
| Native granularity | Dependency-closed library units, not unchanged functions inside a mutable crate | Editing native source requires preparation; no arbitrary hot patch of optimized internal calls |
| Code-generation reuse | Compatible dependency artifacts plus cached specialization/ABI and optional IR bundle | Missing concrete instances or IR require authorized preparation |
| Loading | One immutable cdylib per worker process, loaded explicitly and never unloaded while the process lives | A new native generation requires a new worker |
| Execution permission | Explicit `--execution hybrid-native --allow-native` and preparation authorization | Whole-run sandbox guarantees are withdrawn |
| Managed memory rules | Checked canonical values, native resident storage and call/optimized-interval leases | Layout matching alone never proves native validity or aliasing |
| State reset | Fresh hybrid worker per request; serial selected tests share the process | Guest state and pointers do not persist; relocatable code can be rebound after validation |
| Incremental frontend reuse | Validated portable source/query checkpoints across hybrid workers | Checkpoint import/remapping adds some startup work |
| Cache miss | Reached diagnostic with an exact preparation request; never compile or replay implicitly | A first use of an undeclared API requires explicit preparation |
| Public API discovery | Explicit deterministic binding manifest, compiled for validation | No magical reflection over every public function in an arbitrary rlib |
| Native-to-live callbacks | Prohibited in this release | Closure-heavy callback APIs need a native wrapper that completes the whole operation on the native side |

These decisions replace revision 2's blanket ban on native artifacts. They do not reopen frontend/backend selection.

### 35.4 The fast loop that must work

For a live application library depending on native `numeric_kernel`:

1. Run explicit preparation once. Cargo may report `numeric_kernel` as fresh from an existing optimized build. It compiles/links the generated bridge if needed.
2. Capture a live test and its called source bodies without rustc. Run the test. Calls into `numeric_kernel` use the selected direct or codec binding and execute the compiled implementation; admitted optimized callers may inline prepared native IR.
3. Edit only a live function body or test assertion. Capture a new live snapshot, reuse portable source/query work, and execute with the same native-generation file. **No Cargo process, no rustc process, and no linker process may start during the run.**
4. When a test changes only its inputs, the already-bound native function remains available. When it reaches a previously unbound native specialization, stop with the prescribed missing-binding diagnostic; do not re-execute prior test effects after preparing it.
5. An edit to a native package, relevant manifest, or known build input invalidates preparation. Run explicit native prepare, let Cargo reuse what it can, then create a new generation and a new worker.

For an integration-test-only edit, the application's unchanged compiled library can itself be native. For a unit test of edited private implementation details, that library remains live. The compiled release library does not expose private unit-test functions and normally lacks `cfg(test)` code; never promise to obtain either from an ordinary `.rlib`.

### 35.5 No invisible split inside an optimized crate

A native function or an LRR optimized artifact importing its prepared IR may contain inlined bodies, constant-folded facts and direct calls that bypass the dispatcher. Imported native bodies remain tied to their immutable generation and require no native hot patching. Consequently, changing a live function cannot update an optimized native caller merely by changing an entry table. The implementation refuses any native dependency path that reaches a unit designated live. Native inputs are frozen as a closure, and their behavior is tied to the generation.

Do not use symbol interposition, `LD_PRELOAD`, demangled names, instruction patching, or guessed codegen-unit boundaries to defeat this rule. A developer needing a frequently edited implementation to remain live must place its stable compute dependency behind a real crate boundary. The planner reports the dependency path creating the conflict and the units to move; it does not silently run the old implementation.

### 35.6 Optimization and incremental-build policy

The default native profile is the project's existing `release` profile. Do not create a global `lrr` profile or set global RUSTFLAGS merely to build the bridge, since that can destroy the desired cache match. Preserve the selected profile's semantics and report actual optimization, debug-assertion, overflow, and panic settings. The LRR live side still has its specified checked arithmetic; hybrid execution can therefore intentionally combine checked live code with unchecked-overflow native release code.

Add only a package-specific override for the generated bridge: `opt-level=3`, `incremental=true`, `codegen-units=16`. Do not change native dependency overrides. Cargo's normal release defaults disable incremental compilation, while package-level configuration can enable it for the generated workspace member. Existing native workspace crates use whatever incremental setting the user already chose.[^H13] The bridge override is not a claim that it reproduces whole-program release-LTO performance.

Use the native profile recorded in `.lrr/native.toml`. `release` is its generated default; a user can explicitly name an already existing custom optimized profile without changing the architecture. Never infer that a directory name proves optimization. Native generic code instantiated in the bridge is compiled at the bridge's optimization level; a generic implementation is not necessarily machine code available from the original rlib for every type argument.[^H16]

<a id="section-36"></a>

## 36. Binding manifest, native types, and native closure selection

### 36.1 Files and naming

Native configuration lives at `WORKSPACE/.lrr/native.toml`. The generated package lives at `WORKSPACE/.lrr/native-bridge/` and is a real member of the original workspace. Its package name is `lrr-native-bridge-` followed by the first 12 hexadecimal characters of the canonical workspace-path digest. That name remains stable across binding/body edits. The artifact generation digest, not the package name, changes when inputs change.

The generated crate has no normal dependency on the LRR runtime, its parser, Cranelift, or its host-side serialization libraries. It contains the small generated ABI/codec/store support code and depends only on declared native crates. This prevents the application's dependency graph from inheriting LRR's compiler dependencies or optimization profile.

Use `toml_edit` only to edit manifests while preserving surrounding comments/order as far as its API supports.[^H15] Write a content-addressed backup before edits, use compare-and-swap against the original bytes, and produce the changed-file list. Never overwrite concurrent user edits. Do not insert a nested `[workspace]` into the bridge; that would prevent original-workspace dependency reuse.

### 36.2 Exact configuration grammar

The following is a normative example. Paths in a real file must exist and package IDs are obtained from Cargo metadata; descriptive placeholders below are not accepted literally.

```toml
schema = 2
profile = "release"
native_toolchain = "project"
default_transport = "rust-abi-v1"
prepare_llvm_ir = false
# "host" means omit --target, preserving Cargo's host-build mode.
target_mode = "host"

[live]
package = "app"
target = "lib-tests"
features = []
default_features = true

[[native_dependency]]
alias = "kernel"
package = "numeric_kernel"
source = { path = "../numeric_kernel" }
default_features = true
features = []

[[native_type]]
name = "Plan"
guest_path = "kernel::Plan"
rust_type = "kernel::Plan"

[[binding]]
name = "dot_f64"
guest_path = "kernel::dot_f64"
rust_path = "kernel::dot_f64"
kind = "function"
generic_types = []
generic_consts = []
args = ["f64", "f64"]
result = "f64"

[[binding]]
name = "new_plan"
guest_path = "kernel::Plan::new"
rust_path = "kernel::Plan::new"
kind = "associated"
generic_types = []
generic_consts = []
args = ["usize"]
result = "owned:Plan"

[[binding]]
name = "plan_apply"
# This explicitly requests a managed byte snapshot result conversion.
transport = "codec-v1"
guest_path = "kernel::Plan::apply"
rust_path = "kernel::Plan::apply"
kind = "method"
generic_types = []
generic_consts = []
args = ["mut:Plan", "bytes_ref"]
result = "bytes"

[[extra_input]]
path = "native-data/tables.bin"
kind = "file"

[[build_env_input]]
name = "KERNEL_VARIANT"
```

A path dependency is resolved relative to the workspace root, not the generated member. When serializing its generated Cargo dependency, rewrite that path relative to the member or use a canonical absolute path; preserve the canonical Cargo package identity. Also support a crates.io registry dependency written as `{ registry = "crates-io", version = "=X.Y.Z" }`, resolved from the existing lockfile and offline cache. An alias must be a valid Rust crate identifier and is recorded independently of the package name.

For native graph features, delegate ordinary optional/target-specific dependencies, feature forwarding, proc macros, and build dependencies to Cargo. Version 1 bridge setup accepts ordinary original-workspace crates.io and path roots; git roots, alternate registries, custom target JSON, and source-replacement configurations fail a precise preparation diagnostic. Do not extrapolate the managed metadata subset into a home-grown resolver for the native graph.

Supported live target values are `lib-tests`, `lib-run`, `bin:NAME`, and `test:NAME`. A target of `test:NAME` can list the same package's normal library as a native dependency. A target of `lib-tests` cannot list its own normal library as native: that would run two semantically different copies of the implementation under test. Validate this before invoking a build.

### 36.3 Binding syntax is data, not arbitrary generated source

Parse `rust_path`, `rust_type`, and concrete type arguments with the syntax adapter under edition 2024. Accept only the restricted path/type forms below; then print canonical Rust syntax from that AST. Never concatenate unchecked user strings into a Rust function body.

Allowed type expressions are primitive names, fully qualified paths rooted in declared native aliases or the standard library, and nested generic applications of those paths. Const generic arguments are suffixed integer literals, booleans, or char literals within the supported compiler grammar. Do not accept arbitrary const blocks, statements, macros, unsafe blocks, lifetimes other than erased reference spelling, or expressions that execute during generation. A native type must be `'static`; the generated code asserts that bound through an ordinary generic helper.

Supported binding kinds are function, associated, and method. Emit a fully qualified target call; the manifest supplies all required generic type/const arguments. Methods use the first argument as the receiver and a concrete receiver type; do not ask LRR's trait solver to infer the native method. A trait-associated call is permitted only through a separately validated fully qualified `<Concrete as Trait>::method` path whose types all belong to the native side. Its selection is compiled and checked by rustc.

Each binding has a deterministic `BindingKey`: ABI version, native schema version, dependency alias identity, public guest path, canonical Rust target path, concrete generic arguments, binding transport, argument/result type and transfer modes, panic entry kind, producer ABI/IR schema and descriptor identities. Sort bindings by key bytes; assign operation IDs starting at 1. Reserve operation 0 for the generated native-handle drop operation. Additions can renumber IDs, so every call recipe includes the generation and schema digest. Never persist an operation number without those digests.

The manifest is authoritative for **what can be called through LRR**, not a claim to enumerate all methods in the native crate. A missing method on a native type is a reached `LRR-E-NATIVE-BINDING-MISSING`; LRR does not inspect native source and guess another method. Native opaque types do not automatically implement live traits. Cross-world implementations or native generic instantiations requiring live types are diagnosed by Section 55; only explicit declared native bindings are executable at this boundary. Explicit conversions to live scalar/data types remain possible through declared bindings.

### 36.4 Explicit codec transport and direct type modes

For `rust-abi-v1`, arguments/results use native concrete types and explicit scalar, owned, shared-borrow or exclusive-borrow modes from Section 61. `owned:Plan` and `mut:Plan` identify registered native resident types; primitive spelling identifies validated scalar values. Borrowed byte/string spellings map to the admitted slice/reference contracts. Copying `bytes`/`string`, recursively serialized tuples/options/results and their wire tags below belong to explicitly requested `codec-v1` conversion bindings; using them in a direct binding without an explicit registered native type is a preparation error. `transport` defaults to `default_transport`; `prepare_llvm_ir` can be overridden per direct binding and never implies a codec fallback. The tables below remain required for bindings explicitly selecting codecs. Native String/Vec/container values that stay native use registered resident types and direct bindings instead. A needed managed/native conversion still allocates/copies as declared; no zero-copy claim may hide it.

Implement exactly this codec algebra:

| Codec | Native Rust type | Guest contract |
|---|---|---|
| `unit`, `bool`, `char` | `()`, `bool`, `char` | Exact primitive descriptor; validate bool and Unicode scalar values |
| Signed/unsigned scalar names through 64 bits | Corresponding integer type | Exact width/sign; usize/isize are 64-bit for this target |
| `f32`, `f64` | Corresponding float | Transfer IEEE bit pattern; no implicit numeric conversion |
| `string` | `String` | Owned UTF-8 snapshot; consumes a guest String argument |
| `bytes` | `Vec<u8>` | Owned byte snapshot; consumes a guest Vec<u8> argument |
| `str_ref` | `&str` | Read-only staged view for this call |
| `bytes_ref` | `&[u8]` | Read-only staged view for this call |
| `bytes_mut` | `&mut [u8]` | Mutable staged view with exact-length writeback |
| `owned:NAME` | Concrete native type NAME by value | Move a checked opaque native handle |
| `ref:NAME` | `&NAME` | Shared call-scoped native-object borrow |
| `mut:NAME` | `&mut NAME` | Exclusive call-scoped native-object borrow |
| `option<CODEC>` | `Option<T>` | Exact variant and recursively encoded owned value |
| `result<OK,ERR>` | `Result<T,E>` | Exact variant and recursively encoded owned value |
| `tuple<C0,...,Cn>` | Tuple, at most 8 elements | Ordered recursive owned codecs |

Composite codec leaves may use only owned/scalar codecs, not reference codecs. References are allowed only as top-level input arguments, not as returned values. Opaque handle fields are never examined as guest struct fields. An opaque handle is non-Copy in LRR even when the native type happens to implement Copy. Cloning requires an explicit binding. This makes ownership independent of unexamined native trait metadata.

Neither transport bridges arbitrary live structs or general typed references into the guest heap, trait objects, native function pointers, closures, iterators, futures, borrowed returns or guest-supplied raw pointers. The explicit exception is the direct call-scoped contiguous byte/string lease in Section 61.6.1. Arbitrary managed `Vec<T>` is not implicitly converted to a native collection. A concrete native collection can be stored as an opaque type with explicit operations. This is a useful execution boundary, not Rust ABI reflection.

### 36.5 Closure validation and source ownership

Compute the live-unit set from selected targets plus explicitly live source dependencies. Compute each native root's resolved normal target-dependency closure through Cargo preparation. Build scripts and procedural-macro executables belong to explicitly identified compile-time host units, not executable guest units; the same package can have a separate live-source or native-runtime unit. Require the native runtime closure to be disjoint from live runtime units except the integration-test/package-library distinction above.

If a native crate depends on a crate designated live, fail with a deterministic shortest dependency path. Do not build the native crate against the last compiled version of that live crate. If a package appears in both graphs as different feature/configuration units, treat those as distinct nominal worlds and refuse guest-value interchange unless they meet through an explicit data codec. Do not equate native type identities by their text name.

Native-runtime source bodies are absent from live semantic indexing. Their source is captured for native freshness/provenance, not interpretation. Within a native island, macro expansion/trait solving/const evaluation/borrow checking remain Cargo/rustc work. A separately identified compile-time macro dependency can also be used by the live expander, even when a related runtime crate is native; its output is live syntax, not a native implementation. Macro reexports require the macro-export facet in Section 55, never executable symbol inspection. A live test does not recompile either island or macro tool.

<a id="section-37"></a>

## 37. Cargo bridge preparation and existing artifact adoption

### 37.1 Separate init, prepare, and run

`native init` modifies configuration and generated workspace membership, but does not execute native code. `native prepare --allow-native-build` invokes Cargo and its authorized native build tools, emits a bridge, validates it in a probe process, and publishes a generation. `test/run --execution hybrid-native --allow-native` consumes that published generation without invoking a build tool.

Native authorization is not inferred from finding `.rlib` or `.dylib` files. `--allow-native-build` authorizes build scripts, procedural macros, configured compiler wrappers, linkers, and other native build-time execution within the declared policy. `--allow-native` authorizes the generated library and its transitive native runtime code. Neither flag is implied by a file read grant.

Cargo `--offline` prevents Cargo's own dependency fetching; it does **not** sandbox build scripts or stop those programs from using the network. Display this fact before the first native build in human mode and include it in structured authorization metadata. Build and runtime authorizations remain separate because a cached generation can be run without rebuilding and a generation can be prepared without executing tests.

### 37.2 Workspace initialization transaction

Implement `lrr native init --manifest-path ROOT --bindings PATH --apply` as follows:

1. Resolve the original workspace root from Cargo metadata without building. Canonicalize the root and validate the native configuration schema. Reject a nonexistent selected live target or an unsupported cross target.
2. Read and hash the root manifest, lockfile if present, and relevant Cargo configuration. Determine the stable generated package name and directory.
3. Refuse to replace an existing generated directory unless its ownership marker names this workspace and generator. A normal user package at that path is a collision, not a migration candidate.
4. Generate a package manifest, library source, ABI/codec/store modules, and `.lrr-generated.json`. Write each through a temporary file and atomic rename; do not rewrite unchanged bytes or timestamps.
5. Add the generated package to root `[workspace].members`. When the original root is a non-workspace package, create a workspace table containing the package and generated member, with resolver 3 only where this does not change an already explicit resolver. Do not silently upgrade an existing workspace resolver; native dependencies use its established resolver.
6. Preserve pre-existing `default-members`. If an existing virtual workspace omitted that field, record its previously resolved member list as `default-members` excluding the new bridge, so an ordinary root `cargo build` does not unexpectedly start building the bridge. For a root package, preserve Cargo's root-package default selection.
7. Add the selected profile's package override for the generated bridge only: opt-level 3, incremental true, and codegen-units 16. Refuse a conflicting hand-edited override rather than overwriting it.
8. Atomically update the root manifest after verifying its bytes still match step 2. Write a restoration record containing exact original bytes and generated file hashes.
9. Run native-project Cargo metadata offline to update the lockfile for the new package/dependency edges. This initialization step deliberately does **not** use `--locked`, because adding a package can require a lockfile update. Capture a before/after lockfile diff. If metadata would require an uncached dependency or change a pre-existing resolved package unexpectedly, fail and restore only files still matching this transaction's writes.
10. Verify the generated package is a member of the original workspace and that native package identities resolve to the intended existing packages. Never accept a second workspace with an independently resolved dependency graph as equivalent.

Before `--apply`, the command emits the exact planned file changes and performs no writes. This preview/apply distinction is a transaction safety feature, not an alternative integration architecture. Normal prepare never changes user manifests or updates lockfile resolution.

### 37.3 Native toolchain and invocation identity

`native_toolchain="project"` means resolve the Cargo/rustc toolchain that applies in the original workspace under its rustup override and explicit preparation environment. Record absolute tool locations, `cargo --version`, `rustc -vV`, target specification identity, and relevant configuration before building. An explicitly named installed toolchain is also a configuration input; there is no automatic fallback to a different version. Codec-only generated source requires native edition-2024 support, so reject a compiler older than 1.85. Direct bindings initially require the exact accepted Rust 1.99.0 producer/exporter pair on macOS ARM64; an older, newer or unmatched compiler is unsupported for that transport until its separate ABI acceptance passes.

Preserve the project's existing compilation target mode. For `target_mode="host"`, do not inject `--target`, even if the host triple is known; explicit target mode can change Cargo build sharing. A declared target triple must equal the selected supported execution target: the macOS host target now, or the separately validated Linux x86-64 target in the later port. Record whether it was explicit. Respect existing target-dir/build-dir configuration and pass a directory override only if it was explicitly recorded in native configuration.

Do not change project flags or wrappers merely to gather telemetry. Direct preparation explicitly installs the coordinator-selected exact-version exporter wrapper for the generated bridge only, and IR-enabled preparation adds its recorded compiler output configuration. Delegate other compiler invocations unchanged; report any resulting Cargo fingerprint/rebuild changes. Reject prefer-dynamic, Rust dylib dependencies, linker-plugin-LTO-only artifacts without a linked generation, and unsupported link scripts. Record target/CPU/linker flags and system dependencies. A non-PIC link failure never authorizes silently rebuilding the dependency closure with other flags.

### 37.4 Exact build subprocess

After init has produced a valid lockfile, the default preparation subprocess is equivalent to:

```sh
# Executed with cwd at the original workspace root and its recorded toolchain.
cargo build \
  --manifest-path /absolute/workspace/Cargo.toml \
  --package lrr-native-bridge-WORKSPACE_ID \
  --lib \
  --profile release \
  --locked \
  --offline \
  --message-format=json
```

`WORKSPACE_ID` is substituted by the generator, never passed literally. Add a target argument only for an explicitly configured target mode. Use an argument vector, not a shell command string. Environment and working directory are part of the build record.

This command selects **only the generated bridge as a build root**. Its native dependencies are built/reused normally; the selected live application/test unit must not be among the compiled units. Never substitute `cargo test --no-run`, `cargo build --workspace`, or a root-package build: those could type-check the live code and break the cold-error property.

Parse Cargo's JSON lines into compiler-artifact, build-script-executed, compiler-message, and build-finished records. Non-JSON output from other build tools is bounded and stored separately. A malformed would-be artifact record is fatal; an ordinary non-JSON log line is not treated as an artifact. Wait for both process success and build-finished success. Cargo reports actual artifact filenames, profile fields, features, and whether a compilation step was fresh; use those fields rather than parsing human progress text.[^H17]

Identify the bridge artifact by exact package ID, target name, and crate type `cdylib`, then by its reported filename. All expected native runtime roots must appear in the accepted build graph. A pre-existing file with the correct basename is irrelevant if this successful build did not select it.

### 37.5 Reuse audit and reporting

For every selected artifact, record package ID, target metadata, edition, features, profile fields, actual filenames, content hashes, and `fresh`. A fresh dependency establishes that Cargo did not invoke rustc for that step. A non-fresh dependency establishes only that compilation ran, not how much rustc incremental work it reused. Do not manufacture a function-level reuse percentage from that flag.

Report these separate numbers:

- native Cargo units reported fresh;
- native units reported rebuilt;
- generated bridge unit fresh/rebuilt;
- bytes of selected `.rlib` inputs, classified as compatible selected artifacts rather than proven resident code;
- Cargo preparation wall time, bridge publication time, and probe time;
- build scripts listed by Cargo, without claiming each listed script actually reran.

Cargo emits cached build-script results as well as newly produced results, so a build-script-executed message alone is not evidence that a script process ran.[^H17] Process-level instrumentation used in acceptance tests can distinguish this, but production operation must not alter compilation flags just to obtain it.

Adding a bridge as a build root can change resolved feature combinations, and selecting a release/custom profile can change unit identity. A reuse miss is therefore legitimate. Print the observable mismatch where known—features, optimization, target mode, compiler/configuration—and otherwise say `Cargo rebuilt this unit; exact fingerprint cause not reported`. Never guess a cause from an opaque artifact hash.

### 37.6 Adopting artifacts that predate LRR

No prior LRR sidecar is required. Existing ordinary Cargo `.rlib` outputs are adopted by building the bridge against the same workspace/profile/toolchain and accepting Cargo's selected artifacts. A minimal bridge must be compiled and linked at least once unless an identical prepared generation already exists. LRR does not promise zero first-use preparation.

The bridge source need not mention every function in every native dependency. It mentions only declared bindings and concrete types. Cargo can reuse optimized non-generic code already in its dependencies. It may still generate code for a bound generic specialization or inline body while compiling the bridge; this is not a failure of cache reuse.

Release LTO settings remain the user's selected profile settings. Linking a new bridge may perform LTO work again even when all dependency Cargo units are fresh. Existing code that was optimized only as part of another executable's final whole-program link is not extracted from that executable. The generation caches the completed bridge so subsequent live edits do not pay that link again.

### 37.7 Build failure rules

On any Cargo, compiler, generator, or linker failure, retain the previous immutable generation as an explicit historical artifact but do not silently run it against changed native inputs. The current preparation request fails. Diagnostics include the native unit/binding source mapping and never blame unrelated live cold code.

Generated bindings have source maps back to `.lrr/native.toml`. A missing symbol, wrong generic argument, unsafe-only API, privacy violation, or unsupported Rust signature is reported against the binding. The generator must not insert unsafe around an arbitrary API call just to make the wrapper compile. Only safe public Rust APIs are automatically bound; a user needing an unsafe operation must provide a separately authored, reviewed safe native wrapper with an actual contract.

<a id="section-38"></a>

## 38. Input fingerprints, immutable native generations, and freshness

### 38.1 Generation record

Publish the following logical record as canonical JSON. Fields containing collections are sorted deterministically. Hash the specified stable encoding rather than JSON whitespace.

```rust
struct NativeGeneration {
    schema_version: u32,              // native-generation schema2; outer preparation schema5
    generation: Digest,
    bridge_abi: Digest,               // C control/codec ABI
    producer_abi: Option<Digest>,     // direct Rust ABI schema
    binding_request: Digest,
    exporter_receipt: Option<Digest>,
    prepared_llvm_ir: Option<Digest>,
    bindings: Digest,
    generated_source: Digest,
    project_identity: Digest,
    native_toolchain: ToolchainRecord,
    cargo_invocation: CargoInvocationRecord,
    selected_units: Vec<NativeUnitRecord>,
    source_inputs: Vec<InputFileRecord>,
    build_env_inputs: Vec<NamedValueDigest>,
    extra_inputs: Vec<InputFileRecord>,
    build_outputs: Vec<InputFileRecord>,
    dynamic_dependencies: Vec<NativeDynamicDependency>,
    bridge_file: ContentAddressedPath,
    bridge_file_hash: Digest,
    probe_result: ProbeRecord,
    semantic_settings: Vec<NativeProfileRecord>,
}
```

The digest includes compiler/configuration identity, selected dependency artifact hashes, all binding/codec/producer ABI schemas, exporter identity and successful same-invocation receipt, optional LLVM IR/import manifest, generated source, relevant manifests/lockfile/configuration, source-input digests, declared extra build inputs, and the bridge bytes. It does not include wall-clock timestamps, Cargo fresh/rebuilt flags, elapsed times, temporary staging directory names, the live test body hash, or the user's run-time filesystem grants. Compute it from an explicit GenerationIdentity record that also excludes the generation field itself and the derived content-addressed output path; never hash a self-referential serialized manifest. Put per-prepare observations, raw Cargo messages, fresh flags, and timings in a separate attempt report referencing the generation. An unchanged native prepare can therefore reuse an identical generation while reporting different freshness observations.

Do not put a bare `.dylib` path in the prepared live manifest. Store a generation identity plus an immutable content-addressed location and expected hash. The user-visible trust decision names that generation and the workspace from which it was prepared.

### 38.2 Native source input inventory

Before invoking Cargo, hash native package source-tree membership/content, declared external inputs, and manifests/configuration, excluding generated output directories. After the build, compare those same original source inputs to reject a source race; use Cargo dep-info to refine the published future invalidation inventory. Generated OUT_DIR files are expected to change during the build: capture them after Cargo finishes and verify a second stable read before publication. No post-build-only snapshot may be used to claim the source stayed unchanged throughout compilation.

Use public dep-info as a source-input aid, not Cargo's private fingerprint database. Cargo documents `.d` dependency files for external build systems.[^H12] Discover candidate `.d` files only in directories of artifacts actually reported by Cargo; parse their Make-style rules and associate them by matching a rule's target to a reported artifact path. Do not guess the dependency file's basename, or assume that stripping `lib` from an rlib name is a stable API.

The parser must handle escaped spaces, backslashes, line continuations, multiple targets, and empty phony dependency rules on the selected platform. Canonicalize paths relative to the recorded compilation/workspace location. Reject malformed or ambiguous matches. A `.d` file is not executable metadata, and its contents never determine a function address.

Hash listed source/include inputs and each native package manifest. Record directory membership for the module/source directories containing those files so additions/removals can conservatively invalidate preparation. For a native package without reliable dep-info, conservatively fingerprint that package's complete source tree, excluding only known VCS and Cargo/LRR output directories. Report the broader invalidation scope. Never claim fine-grained input tracking if a fallback inventory was used.

For native packages with build scripts, include the script's source inputs, its generated OUT_DIR contents from Cargo's reported output path, and the declared `extra_input`/`build_env_input` entries. Conservatively hash source files in that build-script package as well. This can make unrelated edits invalidate preparation; that is preferable to assuming an unrecorded generated input is unchanged.

Arbitrary native build programs can read undeclared external files, system headers, environment variables, or network responses. No finite parser can discover all such reads. This plan inherits Cargo's ordinary build-input reporting assumptions and additionally requires external inputs important to the project to be declared in `.lrr/native.toml`. The generation identifies a compiled snapshot; it does not certify that all possible external build inputs are current. The report must state `native_freshness="recorded_inputs"`, not `hermetic`.

This distinction matters for integration tests: changing `tests/new_case.rs` must not invalidate a normal native library when that file is absent from the library's recorded input set. Editing tests embedded in `src/lib.rs` may invalidate that native unit even if they are cfg-disabled, since source-file tracking is coarser than semantic tracking. Keep such an edited library live when that matters.

### 38.3 Check inputs without rebuilding

Before each hybrid run request, the coordinator checks recorded manifest/lock/config hashes, source input membership/hashes, declared build-environment inputs, generated bridge source hash, and generation file integrity. Metadata checks may avoid rereading unchanged files within a service, but any suspicious metadata change requires a content hash. Native library input changes in an in-memory source overlay are also changes; never compare only disk when an overlay changes a native source path.

A changed native input produces `LRR-E-NATIVE-PREPARE-STALE` before a worker loads the library. The diagnostic names changed inputs and the exact explicit prepare command. There is no hidden Cargo no-op invocation, compiler wrapper interception, or “run stale anyway” default.

If only live source bytes changed and the native input inventory is unchanged, reuse the generation. Registry sources are hashed like other sources; do not assume their cache location makes them immutable. If a Cargo clean removes the original `.rlib` files after publication but the bridge and required dynamic dependencies remain intact, execution may continue from the published generation: already linked rlib files are not runtime inputs. Re-preparation still needs Cargo to find or rebuild them.

### 38.4 Publication transaction

After successful Cargo compilation:

1. Finish collecting artifact/input records and compute their digests. Ensure native inputs have not changed while the build/capture occurred; if they did, fail with `LRR-E-NATIVE-INPUT-RACE` and require another prepare.
2. Copy the reported bridge artifact to a private staging file. Do not hard-link a mutable Cargo output into the immutable cache. Hash after copying.
3. Inspect the dynamic-library metadata without executing it to check architecture, pointer width, exported entry name, and declared dynamic dependencies. On macOS, validate Mach-O metadata using bounded recorded `otool`/`nm` invocations and use Cargo-reported `.dylib` paths. During the later Linux port, validate ELF metadata with bounded recorded `readelf` invocations and Cargo-reported `.so` paths. Do not execute `ldd` on an untrusted library. Never accept a native artifact from a different platform or architecture.
4. Run the handshake probe in a fresh supervised subprocess **after** native-build authorization. Dynamic loading can execute constructors. The coordinator never loads the library itself.
5. Require the probe to return the expected ABI size/version, binding digest, target width, and panic-mode flag. The probe performs no user binding calls. A crashed or timed-out probe fails preparation.
6. Write the generation manifest beside the copied library, fsync files where supported, and atomically rename the private staging directory to its digest path. Existing identical content is reused; a same-name content mismatch is corruption and fails closed.
7. Atomically publish the prepared-project record referencing that generation. Only now is it eligible for run authorization.

Store generation artifacts under the user cache root `lrr/native/v1/<generation>/`, not in mutable Cargo incremental directories. The cache may contain the final linked native library; LRR's Cranelift JIT cache remains in-memory. Keep generated source and Cargo provenance alongside the library for diagnostics and reproducibility. Apply size quotas and explicit garbage collection between runs; a running worker holds a generation lease.

### 38.5 Dynamic native dependencies

Normal platform shared libraries and native dependencies emitted by Cargo build scripts may remain dynamic. Record DT_NEEDED names, resolved library locations, available content hashes, and the loader search environment used by the successful probe. Preserve required recorded runtime search paths in the hybrid worker environment; do not silently use an arbitrary later LD_LIBRARY_PATH.

Do not promise the generation is portable to another machine. A dependency may itself use dlopen or locate plugins dynamically; trusted-native mode cannot statically enumerate every such operation. That remains native behavior with ambient authority. A missing declared dependency fails before the test starts; a later native plugin load failure is an ordinary native call result/panic or process failure.

Reject Rust `dylib` dependencies in v1. The selected native Rust closure is statically linked into one cdylib so its Rust standard library, allocator-owned objects, and panic runtime remain on one native side of the bridge. C/system dynamic libraries do not justify loading multiple independent copies of a Rust dependency and exchanging their Rust values.

<a id="section-39"></a>

## 39. Native control interface and explicit codec transport

### 39.1 Two explicitly selected execution transports

Section 61 governs `rust-abi-v1`: matched Rust function-pointer entries, producer layouts, native residents and leases, with C-compatible registration/storage/interpreter control. Sections 39.2–39.7 govern only `codec-v1`, LRR's versioned C-compatible message interface. No `ValueRepr`, VmContext, host Rust allocation/reference/trait object or `std::any::TypeId` is serialized into either control surface. Direct native objects retain their native layout only under the producer's validated contract; no host Rust type is transmuted into the producer's type.

The native generation does not depend on the LRR runtime or Cranelift crates. Generated templates and schema constants are versioned preparation inputs. Native runtime function bodies may use their own Rust toolchain only when the selected transport has an accepted compatibility contract. Both interfaces preserve allocation ownership and prohibit unwinding into LRR.

### 39.2 Entry table

For codec-v1 export the application entry symbol `lrr_native_v1`, with `#[unsafe(no_mangle)]`. A generation with direct bindings additionally exports the registered `lrr_native_rust_abi_v1` control entry from Section 61. Runtime/system symbols emitted by the toolchain are not interpreted as bindings. Its signature is:

```rust
// Specification types. Keep their C header and Rust definitions mechanically checked.
pub unsafe extern "C" fn lrr_native_v1(out: *mut NativeApiV1, out_size: u32) -> u32;

#[repr(C)]
pub struct NativeApiV1 {
    pub magic: u64,                       // 0x4c52524e41543031
    pub abi_version: u32,                 // 1
    pub struct_size: u32,
    pub pointer_width: u32,               // 64
    pub little_endian: u32,               // 1
    pub flags: u64,                       // bit 0: cfg(panic="unwind"); other bits zero
    pub binding_schema_digest: [u8; 32],
    pub create: Option<unsafe extern "C" fn(u64, *mut *mut core::ffi::c_void) -> u32>,
    pub call: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, u64, *mut NativeReplyV1) -> u32>,
    pub release_reply: Option<unsafe extern "C" fn(*mut core::ffi::c_void, u64, u32) -> u32>,
    pub drop_handle: Option<unsafe extern "C" fn(*mut core::ffi::c_void, u64, u32, *mut NativeReplyV1) -> u32>,
    pub destroy: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut NativeReplyV1) -> u32>,
}

#[repr(C)]
pub struct NativeReplyV1 {
    pub status: u32,
    pub flags: u32,
    pub lease_id: u64,
    pub data: *const u8,
    pub len: u64,
}
```

`create` takes the test's nonzero run epoch, which must fit in u32; reject larger values before context creation. `drop_handle` takes a native handle and its nominal type ordinal. `release_reply` takes `adopt_result_handles` equal to 0 or 1; no other value is valid. `destroy` consumes the context and can return only a zero-length, lease-free status reply. It is legal only after every reply lease and guest-owned native handle has been released. A successful destroy physically frees an empty bridge context; it does not substitute for the guest's ordered drop machine. On destroy failure the worker reports a native infrastructure failure and exits without retry.

On the supported 64-bit macOS and later Linux targets the API table is 104 bytes aligned to 8; the reply is 32 bytes aligned to 8. Assert these values and every field offset on both sides. Never cast a shorter or differently versioned table to this structure. The entry checks non-null output, correct alignment, and sufficient size before writing; returns `ABI_MISMATCH` otherwise. The loader allocates `MaybeUninit<NativeApiV1>`, calls the entry, and assumes initialization only after success. It verifies every header field and schema digest before using a function pointer. All five function entries must be Some; nullable function-pointer fields permit rejecting missing entries without constructing an invalid Rust function-pointer value. The pointer cannot prove a hostile library safe; authorization is the trust boundary.

Load using `libloading`'s Unix API with `RTLD_NOW | RTLD_LOCAL`. Do not use `RTLD_GLOBAL`, environment-wide symbol injection, or arbitrary symbol fallback. Retain the library object in the worker until process exit; never reload a changed file at the same path. Paths are immutable generation locations. The coordinator never calls `Library::new`.

### 39.3 Status and effect classification

Define `AbiStatus` as a `u32` constant table, not a Rust enum accepted from foreign memory:

| Value | Name | Meaning |
|---|---|---|
| 0 | OK | A complete success reply or successful administrative operation. |
| 1 | ABI_MISMATCH | Header/version/size/flags mismatch; no binding entry. |
| 2 | BAD_REQUEST | Malformed schema-driven wire value or unsupported ordinal; no binding entry. |
| 3 | STALE_HANDLE | Wrong epoch, absent object, consumed handle, or nominal type mismatch; no binding entry. |
| 4 | ALIAS_CONFLICT | Call-scoped mutable/shared access conflict; no binding entry. |
| 5 | QUOTA | Preflight reservation failed; no binding entry, unless the reply's executed flag explicitly says otherwise. |
| 6 | NATIVE_PANIC | A native unwind was caught after binding/drop entry. |
| 7 | POSTCALL_FAILURE | The binding ran, but its result or required writeback could not be encoded. |
| 8 | BRIDGE_BUG | An unexpected internal failure; worker termination, no retry. |

The FFI function's immediate return is an administrative status: zero means the output structure has been initialized and its status is authoritative. Nonzero means no reply lease exists and the call was not entered, except an unrecoverable host/native process termination for which no status arrives. Keep this distinction consistent; do not put two competing business outcomes in the return value and reply.

Reply `flags` use bit 0 for `binding_entered`, bit 1 for `has_mutable_writeback`, and bit 2 for `has_pending_result_handles`. Unknown bits are an ABI error. `binding_entered` is set immediately before invoking the typed Rust binding or native destructor. A panic reply must have it set. This is execution provenance, not evidence that the native function had an external effect. Any entered call is treated as potentially effectful. A native `Result::Err` is an ordinary successful encoded Rust result, not `NATIVE_PANIC`.

### 39.4 Wire format

Use a small schema-driven binary codec. Do not use bincode, transmute, native `repr(Rust)` layouts, or JSON as the execution ABI. JSON remains the user protocol. All integer fields are little-endian and are read by checked cursor methods using byte copies. Never create an aligned integer reference into an arbitrary request buffer.

Request layout, in order:

```text
8 bytes  ASCII "LRRNAT01"
u32      wire_version = 1
u32      binding_ordinal
u64      call_sequence (strictly increasing within the context)
u32      argument_count
u32      staging_group_count
repeat argument_count times: u32 encoded_length, encoded argument bytes
repeat staging_group_count times:
    u32 group_id, u8 access (0 shared / 1 mutable), u64 byte_length, raw bytes
```

Lengths and total offsets must fit the configured limits before allocation. Group IDs are consecutive starting at 1; duplicates, gaps, trailing bytes, and unreferenced groups are rejected. A borrowed argument's encoded bytes are `(u32 group_id, u64 offset, u64 length)`. An opaque-object argument is `(u64 native_handle, u32 type_ordinal)`. The binding schema, not a user-supplied tag, fixes whether it is owned, shared, or mutable.

For all other argument/result values use the following recursive codec: unit is zero bytes; bool is one byte 0/1; char is u32 validated as a Unicode scalar; signed/unsigned primitives use exactly their declared width; floats use raw IEEE bits; String and Vec<u8> use a u64 byte length followed by bytes; tuple fields are encoded in declared order; Option uses one byte 0/1 then its payload; Result uses one byte 0/1 for Ok/Err then its payload. `usize` and `isize` have eight bytes but remain distinct schema identities. String decoding validates UTF-8. Reject every invalid tag, surplus byte, truncated field, oversized sequence, or recursion deeper than 32.

Result reply data contains `u32 result_length`, encoded result or a bounded panic/error record, then `u32 writeback_count` and mutable groups encoded as `(group_id, byte_length, bytes)` in ascending group order. Every mutable group is returned exactly once even after a caught native panic; its length must equal the request length. No length-changing borrow mutation is supported. Borrowed strings are shared only. A native output must fit the reply limit or become `POSTCALL_FAILURE`; do not partially install an oversized result.

Defaults: request 16 MiB, reply 16 MiB, at most 64 arguments, 64 staging groups, 32 nesting levels, 64 KiB of panic/diagnostic text, and at most 65,536 native objects per test. These constrain bridge-managed data, **not allocations internal to arbitrary native code**. Allocation failure that aborts the native process remains a process failure.

### 39.5 Staging guest borrows

The worker copies supported guest byte/string ranges to call-local native staging groups. It does not reinterpret the guest heap as a `Vec`, `String`, slice, or Rust object. Group shared ranges that refer to the same guest backing allocation into one staging buffer, retaining their original relative offsets, so overlapping shared slices point into the same allocation during the call. Copy only a bounded covering range; preserve enough offset metadata to map it back exactly. A shared string view is created only after UTF-8 and character-boundary validation.

For mutable byte slices, use a group per disjoint mutable range. Reject overlap with any other shared or mutable range in the same call. Disjoint mutable subranges of one guest allocation are supported; obtain disjoint native slices by checked partitioning or separate staging buffers. Native code must not retain a borrow past the synchronous call. Pointer identity across separate calls, addresses of distinct guest allocations, vector capacity, and allocator identity are outside the hybrid profile. Do not claim arbitrary pointer-observing safe wrappers have full rustc equivalence.

Reserve guest writeback destinations before entry. On success or caught panic, copy back every mutable group's final bytes **before** installing a result or starting guest panic cleanup. This preserves mutations that happened before a native panic. It is not transactional rollback. Internal native pointer replacement, borrowed returns, self-referential borrowed outputs, and closures retaining those references are excluded from the codec.

### 39.6 Native-owned opaque objects

Both transports use one per-test native context and one slot registry, with common generation/type ordinals, ownership states and leases under Section 61.6. A generation exposing both registration tables must bind them to that same context schema and binding/type manifest. Create and destroy the shared context exactly once per test, not once per transport. The C codec API stays version 1; its opaque context hides this internal storage change.

Generate type-specific storage operations for each declared native type. Slots own correctly aligned native storage, a type ordinal and initialization/ownership/borrow bookkeeping. Use stable boxed slot storage rather than embedding movable values in a growable vector. Typed generated operations construct, borrow, move or drop the selected concrete T; no unchecked runtime downcast or host transmute is allowed. Bookkeeping leases must not retain host Rust references that conflict with JIT/native access.

The context owns append-only slot identities. Slot zero is unused. Native handle high 32 bits are the run epoch, low 32 bits the slot index; slots are never reused in that context. A codec `owned:NAME`, `ref:NAME` or `mut:NAME` carries the common native handle and type ordinal; decoding looks up and leases the same resident storage used by direct calls. It never serializes the native object's fields or creates a transport-specific copy. Reserve output slots before entry, commit owned transfers once, and reject duplicate ownership or owned-and-borrowed uses before taking a value. Returned codec handles remain pending under their reply lease until adoption into the same registry. The direct-new/codec-method/direct-drop sequence in Section 36.2 must work with one object and one destructor.

A `NativeObjectRecord` in a dedicated worker arena stores the library generation, context identity, native handle, nominal `NativeTypeKey`, ownership state, and active call lease. Its guest `ValueRepr.kind` is 14; payload is a checked worker arena handle, never a native address. Native types are nominal, opaque, non-Copy, and non-field-projectable. A method receiver can borrow them through the ordinary call syntax; a native clone must be an explicitly bound method returning a new owned handle. Copy/Clone/Send/Sync/Drop traits are not inferred from native memory or symbol names.

Every native type must be `'static`. Native APIs returning borrowed lifetimes, opaque `impl Trait`, trait objects, or references to their arguments require a user-authored safe native wrapper returning a supported owned result. The generator rejects those direct declarations. This is a schema restriction, not a hidden trait solver in LRR; generated ordinary Rust compile checks validate each selected concrete type and call.

### 39.7 Reply leases and allocator separation

The bridge owns reply bytes until `release_reply`. It returns a nonzero monotonic lease ID and an immutable pointer valid through release. Only one outstanding reply is allowed per context. Do not make another call, drop, or destroy while a reply is outstanding. Empty administrative success may use a zero lease and null data pointer with length zero.

The worker copies and validates the reply, reserves guest allocations, and constructs a pending guest result transaction. Newly returned opaque handles are held by the reply lease, not yet owned by the guest. After every destination is ready, call `release_reply(context, lease, 1)` to adopt them, then publish the already-built guest result without further allocation. On a pre-publication failure, release with adoption 0; the bridge destroys unadopted native results inside its protected cleanup path. If that cleanup panics, fail the worker, not the preceding test as an expected panic.

Response bytes are always freed by the bridge. Native objects are always destroyed by the bridge. Guest strings and bytes are always allocated/freed by the worker. Never free a bridge buffer with the worker allocator, return a Rust String across FFI, or allow a host Drop impl to unload a library before objects allocated by it are dead. A process-level fatal failure bypasses all remaining native user destructors and lets the operating system reclaim memory on worker exit.

<a id="section-40"></a>

## 40. Boundary execution machine, aliases, panic, and semantics

### 40.1 Why this adds runtime guards, not a borrow checker

The native Rust compiler optimized the island under Rust's reference-validity rules. The relaxed managed interpreter cannot pass arbitrary aliased values into those assumptions merely because sandboxing is disabled. An `&mut T` native parameter must be exclusive for the actual call. A `&T` must point to a valid initialized native T. The required dynamic call guards are limited to values being passed now; they never inspect future uses, cold branches, or global trait/coherence obligations.[^H18]

No whole-program alias pass is added. Existing managed aliases can remain as dormant checked handles while a synchronous native call runs because there are no native-to-guest callbacks and no concurrent live execution. The worker marks involved guest locations locked for that call. Overlap between arguments used concurrently by native code is rejected. A resumed guest access sees direct mutations or completed codec writeback; stale/moved handles still fail the normal guards. Do not confuse dormant managed handle existence with creating a native Rust reference for that dormant handle.

A native function can create internal threads using its own Rust code and owned data. The supported borrowing wrappers cannot let a non-'static borrow escape into an unjoined thread without unsafe code. Unsafe native implementations remain trusted; the runtime cannot prove them safe. A returned object can encapsulate synchronization or interior mutability; access still goes through its explicitly bound methods.

### 40.2 Native call state machine

Both transports implement one forward-only semantic transaction:

```text
Resolve -> EvaluateArguments -> Validate -> Reserve -> LeaseOrStage -> Ready
                                                                      |
                                                                   Entered
                                                                      |
                                          +---------------------------+----------------+
                                          |                           |                |
                                      Success                     CaughtPanic      ProcessFailure
                                          |                           |                |
                              AdoptAndReleaseOrWriteBack   ReleaseOrWriteBack        terminate
                                          |                           |
                                    PublishResult              PublishGuestPanic
```

Resolve binding identity before evaluating a missing direct callee's arguments. Evaluate left-to-right once; preserve existing receiver/argument effects across suspension. Validate generation, concrete types, transfer modes, ranges and alias groups. Reserve result handles/storage, continuations and panic bookkeeping before entry. Codec calls stage their explicit conversion buffers; direct calls acquire leases on native or admitted contiguous storage.

Commit the caller continuation, logical transfers, precise fuel charge if selected, and entered call identity before invoking the target. A direct call may execute inside JIT code; its effect state is still canonical or recoverable through the matched exit map. A signal/crash/timeout after entry reports may_have_executed. Never retry, switch transports, compile a missing specialization, or rerun a test after an entered call.

Success adopts exactly one result and releases leases or performs codec writeback before publishing the next PC. A caught panic preserves completed mutations, does not adopt a normal result, and enters guest cleanup. Abort/foreign-unwind violation and malformed post-call state are infrastructure failures. A post-call deoptimization reconstructs the continuation after the call, never the call opcode. Section 61 defines native resident transfers and catch entry ABI; codec byte/reply validation remains required only on the codec path.

### 40.3 Generated codec binding body recipe

For every codec-v1 binding the generator emits exactly one typed arm; rust-abi-v1 uses the typed address declarations/invokers and optional catch adapters of Section 61:

1. Match the ordinal and decode against that arm's static schema.
2. Validate all slot identities, access modes, and non-overlap before taking any owned slot.
3. Acquire checked object and staging guards in ascending slot/group order.
4. Decode explicitly copying strings/bytes to Rust-owned allocations and transfer owned resident slots from the common native registry. No arbitrary receiver method lookup happens here: the path is explicit in generated source.
5. Mark entry and invoke the ordinary safe Rust expression. Write the concrete return type in a local annotation to make compiler errors local to this binding.
6. Drop temporary argument borrows, but retain buffers containing mutable writeback.
7. Encode the result and mutable groups; allocate/record pending output handles.
8. Return a leased reply. Any native unwind from the call or its argument cleanup is caught by the arm's native unwind boundary, then encoded with writeback.

A binding to `kernel::sum::<u64>` literally calls that path with decoded values. A method binding uses either the explicitly declared inherent associated path or fully qualified native trait path supplied in the manifest grammar. It does not ask LRR's method resolver to implement the native crate's trait system. The schema's live names and types become an opaque native declaration namespace; unlisted native methods remain missing even if the underlying Rust crate implements them.

Generate a diagnostic source map from each arm back to manifest line and binding ID. A private function, unsafe direct call, wrong generic arity, inaccessible type, or unsatisfied bound is a native preparation compile failure. Report the rustc message and generated arm path; do not weaken the generated code or fall back to unchecked ABI calls to make it compile.

### 40.4 Panic and process failure

Direct abort entries and Rust-ABI catch adapters follow Section 61. The remaining reply-encoding rules here apply to codec-v1. Unwind generations contain recoverable panics within the native generation for both transports; abort generations terminate the worker; no unwind is permitted through a JIT or host frame.

Generated bridges built with `panic=unwind` catch Rust unwinding **within the bridge**, before the extern C function returns. They encode a bounded panic description and keep response/writeback storage outside the catch closure. Use `AssertUnwindSafe` only around the explicitly audited state machine; after a panic, a partially consumed native object is either absent or marked poisoned, never silently restored. A native type's application-level invariants after panic are those of that native implementation; LRR does not promise rollback.

A bridge built under a Cargo profile with `panic=abort` is allowed to preserve reuse. Its table reports that mode. Any abort, segmentation fault, illegal instruction, process exit, allocator abort, or cross-boundary unwind violation is `native_process_failure`, never a guest panic and never a passing `#[should_panic]` test. Rust's ABI/unwinding rules do not permit treating arbitrary unwinds across a non-unwinding C boundary as normal return values.[^H20]

Install no process-global panic hook that calls into the worker. The bridge may leave the native hook intact; stderr is captured as native process output. Expected-panic matching uses the encoded payload, not a heuristic parse of stderr. Only string and static-string payloads get their text; other payloads become a fixed opaque-payload description. Destroying an exotic payload can itself panic: catch that cleanup inside the bridge where possible and otherwise terminate as infrastructure failure. Native panics from `Drop` during an already active guest panic are double-panic termination under the existing cleanup rules.

The host runtime remains built with panic=abort. It does not try to catch bridge panics in the interpreter or Cranelift helpers. The generation's separate Rust standard library and panic strategy belong to the native side of the C ABI.

### 40.5 Drops and fatal faults

Add native object destruction as an explicit action in the common drop machine. Normal scope exit, assignment replacement, moved argument cleanup, and ordinary guest panic schedule each live native handle exactly once in guest drop order. `drop_handle` removes ownership from its slot before invoking native Drop, so an unwinding destructor cannot be called a second time. Bridge-owned fields are dropped by ordinary native Rust, not reinterpreted by LRR.

Per-test success/panic cleanup must leave the native object table empty and no reply leased before context destroy. Remaining unexplained entries are a runtime ownership bug and fail the test infrastructure; explicitly forgotten values follow Section 44.7 instead. Native process statics are not entries in this table and do not reset here.

On a fatal managed runtime fault during a hybrid run, stop guest cleanup as specified in Section 13 and terminate that worker **without** dropping a context containing native objects. Deliberately leak that context until process exit; do not let a convenient Rust RAII wrapper invoke arbitrary native Drop after the fatal fault. Managed-only physical allocations may be reclaimed normally. Future requests use a new worker.

### 40.6 Semantic profile and result meaning

Live code retains checked arithmetic; its debug_assertions cfg is the captured live Cargo unit value (true for standalone defaults). Native code retains the exact recorded Cargo profile. In particular, an ordinary release profile can have unchecked overflow and disabled debug assertions; do not claim the combined run is equivalent to ordinary debug `cargo test`. Preserve and display those settings rather than silently rebuilding optimized artifacts with different flags.[^H13]

A hybrid result establishes that the selected live path passed under `lrr-2024-v2` against the recorded native generation, subject to the listed boundary semantics. It does not certify that the live crate compiles with rustc or that a different native profile behaves identically. Keep separate CI tasks for normal Cargo tests/validation. This separation is not optional product wording: it prevents a fast test loop from being mistaken for Rust acceptance.

Native filesystem, network, clock, randomness, environment, subprocess, logging, and global-state operations bypass the managed broker. The bridge need not enumerate their permissions because the whole mode explicitly authorizes native ambient authority. Managed facade calls still obey their existing grants. A managed file denial does not imply the native island could not read the same file. The result must report `sandbox_strength="none_native"` for the entire hybrid run.

<a id="section-41"></a>

## 41. Query/JIT integration, process lifecycle, and warm reuse

### 41.1 Pure native declaration inputs

Add input records `NativeNamespace`, `NativeBindingSchemas`, `NativeTypeSchemas`, and `NativeGenerationManifest`. These are immutable, coordinator-supplied, validated data. No provider opens a shared library, runs Cargo, reads native memory, or invokes a native function. Binding preparation is a separate effectful coordinator operation; a query cache is not a build scheduler.

Add these query keys:

| Query | Key | Pure result |
|---|---|---|
| `NativeExportBucket` | Native namespace identity, path/name | Explicit declared candidates, including a fingerprinted empty set. |
| `NativeSignature` | Binding key plus schema digest | Concrete argument/result/ownership recipe. |
| `NativeNominal` | Native type key plus schema digest | Opaque nominal descriptor, non-Copy policy, available bound methods. |
| `NativeCallRecipe` | Reached PC plus resolved concrete binding key | Marshal plan and continuation layout, with no loaded pointer. |

`NativeBindingKey` consists of dependency unit identity, declared binding name, concrete type/const substitutions, ownership/codec signature, and binding-schema version. The live call recipe additionally depends on the current generation's binding table mapping, but not on its absolute path or function address. When only native implementation code changes and the schema remains identical, the source-level signature may remain reusable; installed runtime binding targets must still be rebound to the newly authorized generation. A matching name is insufficient to reuse a native handle or pointer.

A native crate's explicit namespace is complete **for the declared binding surface**, not a claim to index its entire real Rust API. Importing a name outside that surface yields a reached missing-binding diagnostic. Keep native identities separate from synthetic std facade identities and live source identities. The same package/unit cannot simultaneously provide a live definition and native definition under the same import alias.

### 41.2 Callable and LRIR changes

Extend CallableId with NativeBinding(NativeBindingKey) and reached LRIR with NativeCallOp(PreparedNativeCallId). Call resolution, argument order, source views, coercion hints and origin tracking remain common. Native schema lookup is pure and occurs on reached calls. A native body never becomes a live source thunk.

The interpreter invokes the generated typed C slot invoker or the explicit codec state machine. Cranelift/LLVM may call matched Rust ABI entries directly inside a reached compilation unit. Checked direct lowering uses preflight/completion helpers; the scalar optimized path retains typed values and inline bookkeeping under reconstruction maps. A codec call remains an explicit conversion operation; no compiler guesses a cheaper transport.

All tiers implement Section 40's effect/ownership state machine. Native calls can mutate, fail or panic regardless of a successful observation. Every pre-call guard resumes before entry and every post-call exit resumes after the completed call. Section 62 additionally allows admitted prepared native IR to be inlined without cloning native mutable state or changing profile semantics. Batch wrapper APIs remain useful but are not the only prescribed way to amortize crossings.

### 41.3 Four cache domains

Keep four independently owned cache domains:

1. **Cargo cache:** existing dependency artifacts and rustc incremental data. Cargo alone determines compiler reuse during preparation.
2. **LRR native-generation cache:** immutable linked bridge plus provenance. It survives process restart; a valid generation can be loaded without running Cargo.
3. **LRR source/query cache:** validated portable pure facts and reached LRIR recipes; no process addresses or guest state.
4. **LRR compiled-code cache:** content-validated relocatable Cranelift/LLVM objects with dependency/CPU/ABI/profile/map identities and closed relocations. Fresh workers reconstruct installed entries; they never deserialize an executable memory image.

Give all four separate hit/miss counters. A Cargo dependency hit does not imply a Cranelift hit; a bridge load is not JIT compilation; a portable query import is not native code restoration. Never print one undifferentiated “incremental cache hit” counter.

### 41.4 Hybrid worker lifetime

Each `run`, `test`, or service `run` request in hybrid mode starts a fresh `lrr-worker` process. The coordinator checks generation freshness and authorization first, spawns with the recorded native loader environment and current working directory, then sends the immutable live snapshot and portable checkpoint. The worker loads one generation, validates the table, and runs the request's selected tests serially. After results/checkpoint transfer, it exits. No fork of a multithreaded interpreter or native-loaded process is used.

Create a fresh native context and managed heap for each test. Drop those at normal completion before the next test. Native process-global variables, TLS, allocator state, once-cells, and threads may persist between tests **inside that same request**, as they can in a native test executable. The result includes `native_global_reset="per_run_request"` and the selected test order. Users requiring a process boundary between two tests submit separate run requests; this is use of the same fixed lifecycle, not a second worker policy.

On the next edit/run request, the old worker is gone and all native process state starts fresh. Native effects outside the process, such as files, network writes, subprocesses, and shared memory, are not undone or reset. Process isolation here provides lifecycle/crash handling, not a hostile-code sandbox.

Fresh hybrid workers may reuse validated relocatable Cranelift/LLVM objects under Section 62.9, rebinding closed native/helper relocations to the new worker and generation. They never restore process addresses, heaps, native globals or executable memory images. A miss interprets or recompiles reached code. Persistent managed mode may retain validated in-memory code. Report actual object-cache hits and installed entries separately; the required design is not evidence of implemented cross-request reuse.

### 41.5 Portable source/query checkpoint

Implement the portable checkpoint before calling the hybrid edit loop incremental. The worker exports pure artifacts to the coordinator after a normally completed run request, and the coordinator supplies them to the next worker. The coordinator owns at most 64 MiB per open project; no checkpoint is accepted directly from a service client. Use the existing framed chunk protocol with an independent declared length/digest. A failed/crashed worker contributes no new checkpoint; the last validated checkpoint is retained.

Checkpoint header:

```text
format = "lrr-portable-v2"
runtime_build_id
semantics_profile_digest
syntax_adapter_version
lr_ir_version
native_binding_schema_digest
snapshot_source_manifest_digest
record_count
encoded_length
payload_digest
```

Portable v2 replaces the earlier unspecified serde-record payload; a v1 checkpoint is a reported cold-cache miss, never reinterpreted as v2. The portable payload uses versioned explicit records with stable field/tag/length encodings, not memory dumps or serde layout as an ABI. Serde may render inspection JSON; encode the portable payload once and transfer its bytes through internal v4 raw frames. Decode/remap/validate once per import, then reuse validated records under ordinary dependency checks. A record contains stable query key, stable symbolic result, its output fingerprint, ordered dependency stamps with stable keys/fingerprints, estimated bytes, and usage rank. Export only the following core whitelist in v2: `Outline`, `BodyStructure`, `Signature`, `ScopeBucket`, `LookupPath`, `Thunk`, `NativeExportBucket`, `NativeSignature`, and `NativeCallRecipe`. Export only record variants whose full payload has a defined portable encoding. Omit an ineligible record; do not discard checks to increase hit rates.

For this whitelist, replace interned local integers with stable symbolic equivalents: canonical DefKey, namespace path, body-hash/node-preorder pair, symbolic LocalId/slot, TypeRecipe rather than run-local TypeId, and source byte spans keyed by content. `Thunk` records contain symbolic recipes and deferred child keys, not installed PC indexes or runtime closures. `BodyStructure` contains owned structural nodes, not rust-analyzer syntax pointers. A source map is valid only for its exact source content digest.

Do not export ParseFile's upstream tree, runtime TypeDescriptors containing arena IDs, live values, references, object handles, effects, test outcomes as memoized execution, compiled pointers, native contexts, cancellation state, permissions, or loaded-library handles. Parse trees can be rebuilt on demand for new/reached uncached syntax. Unexported query kinds are recomputed under the ordinary DB rules.

Import procedure:

1. Validate frame sizes, digest, versions, quotas, record shapes, duplicate keys, and dependency-key syntax before mutating SourceDb.
2. Capture/publish the current snapshot's actual input records. Never replace current input bytes or their fingerprints with checkpoint claims.
3. Intern stable identities in a deterministic first pass, then remap symbolic records in a second pass. References to absent records remain unresolved query keys, not dangling IDs.
4. Insert imported memos with `verified_revision=0`. On demand, run normal dependency validation against current inputs and recursively validate/recompute dependent queries. A cached negative lookup is reusable only if all completeness/search dependencies still match.
5. If a result needs an unexported parse tree or TypeSchema, rebuild that dependency normally. A record whose representation cannot be remapped becomes a miss, not an internal panic.
6. Install a reached PC only after its imported Thunk and dependencies validate. The JIT receives normal validated recipes and cannot tell whether they were imported or newly computed.

On export, evict by lowest usage rank until within 64 MiB, breaking ties by canonical query key for deterministic tests. Cache export is best-effort performance work: an allocation/quota failure drops export and reports it, without changing the already completed guest result. Import corruption drops the entire checkpoint and records a cold fallback; it never changes native-generation authorization or loads arbitrary code.

### 41.6 Invalidation table

| Edit/change | Required response |
|---|---|
| Only selected live test body changes | New live snapshot; unchanged native generation; invalidate changed body's structural/semantic records only as their inputs require. |
| A cold live function changes | Preserve native generation and independent live recipes; body remains unprepared if uncalled. |
| Native library's recorded input changes | Reject execution as native preparation stale before library load. |
| Native binding added or concrete generic changed | Require prepare; update schema; invalidate affected native lookups including negative results. |
| Native implementation changes but surface is identical | New native generation and fresh worker; source signature facts may be reused after validation. |
| Same nominal spelling comes from a different package/feature unit | New native type identity; no cross-generation object transfer. |
| Rust edition or live semantics version changes | Invalidate affected live parse/structure/pattern/recipe caches and portable checkpoint compatibility. |
| Cargo toolchain, profile, flags, target mode, or lockfile changes | Require native preparation; Cargo decides actual artifact compatibility. |
| Managed permissions change | Reuse pure source facts; enforce new managed policy; native mode still reports no sandbox. |
| Native authorization removed | Refuse hybrid run; do not silently substitute managed source execution. |
| Worker crashes or is cancelled inside native code | Discard worker and incomplete checkpoint; no automatic test replay. |

Each row requires a two-run fixture with explicit input changes, expected counters, and absence/presence of Cargo invocations. Time alone is not evidence of reuse.

<a id="section-42"></a>

## 42. CLI, service messages, permissions, and result reporting

### 42.1 Exact workflow

The following commands name the runtime to be built, not a tool already installed by this document:

```sh
# Authorize only the specified workspace edits; print the diff without --apply.
lrr native init --manifest-path Cargo.toml --bindings .lrr/native.toml --apply

# Explicitly permit Cargo, native build programs, linking, and the loader probe.
lrr native prepare --manifest-path Cargo.toml --bindings .lrr/native.toml \
  --allow-native-build --offline --locked --json

# Repeated runs after preparation do not invoke Cargo/rustc/linkers.
lrr test --prepared .lrr/prepared-hybrid.json \
  --execution hybrid-native --allow-native --exact tests::dot_product --jit auto --json

# Normal default remains managed; this does not authorize any native library.
lrr test --prepared .lrr/prepared-managed.json --execution managed --jit auto --json
```

`native init` without `--apply` is a read-only diff/plan operation. Applying it does not authorize a native run. `native prepare` requires `--allow-native-build` every explicit invocation or an equivalent service authorization value scoped to the workspace and configuration digest. `test/run/watch --execution hybrid-native` requires `--allow-native` even after an authorized build. `--allow-native` in managed mode is a CLI error, not a latent permission.

Offline means Cargo is instructed not to fetch missing dependencies. Build scripts, procedural macros, linker plugins, and probe-time constructors execute native code with host authority after native-build authorization; that flag does not sandbox them. The selected implementation does not add a network broker to Cargo's child process tree. A prepare diagnostic must make that trust scope explicit.[^H21]

`--require-hardened-sandbox` always refuses hybrid mode, before any library is loaded. Ordinary managed `--allow-read`/`--allow-write`/`--env` flags retain their meanings only for managed facade operations. They cannot restrict native code. The CLI's first hybrid result/summary explicitly identifies native authority; it does not suppress the label after the first run.

### 42.2 Working directory, environment, output, and subprocesses

Run the hybrid worker with the prepared workspace root as current directory. Inherit the invocation environment except for LRR's internal coordination secrets/FD variables, then apply the recorded loader-path environment and explicit native environment overrides. Manifest build-environment inputs are separately checked for freshness. Managed injected environment values remain isolated from this ambient native environment; report both namespaces without dumping secret values. Log environment input names and keyed/digest summaries, never raw secret contents.

Capture native stdout/stderr through dedicated process pipes. The worker's internal framed protocol uses a separate inherited Unix socket/FD, never stdout. A native `println!` cannot corrupt the coordination stream. The coordinator drains output while the worker runs and enforces bounded stored previews and output limits. Tag output `origin="native_process"` or `origin="managed_broker"`; there is no guaranteed total order across these distinct streams. Within each stream retain received order. Do not use the managed broker transcript as if it captured every native effect.

Native code may spawn subprocesses. For cancellation, put the hybrid worker in its own process group and terminate that group after the existing grace period. A descendant can deliberately escape that group or externalize effects; this is not a containment claim. If protocol EOF follows a native process exit without a completed result, mark unfinished tests as infrastructure failures even if the OS exit status was zero.

The supervisor enforces wall time at the process boundary. Guest fuel and managed heap quotas do not bound a long native call or native allocator. Report `fuel_coverage="live_only"` and `memory_limit_coverage="managed_and_bridge_buffers"`. The process can be killed on timeout; killing cannot undo already performed IO.

### 42.3 Service extension

Use public protocol version3 and reject versions1/2 with a migration diagnostic. Run requests contain execution, allow_native, native_generation (null for managed), and allow_macro_exec alongside the inherited snapshot/selection/jit/policy/limits/output fields. The CLI supplies explicit defaults; serialized requests must include them. A hybrid generation must match the prepared project. Macro authority is independent. Arbitrary paths, dynamic library bytes, function addresses, and custom binding bodies are never run parameters.

Add coordinator methods `native_init_preview`, `native_init_apply`, `native_prepare`, and `native_status`. `native_init_apply` requires the preview's input digest and `allow_workspace_edit=true`; stale previews fail. `native_prepare` requires `allow_native_build=true` and the bindings/config digest. These operations are permitted only while no run is active. A run that discovers stale preparation returns a structured failure; it does not enqueue native_prepare or authorize it on the user's behalf.

Example run request (the repeated-digit digests illustrate encoding; actual requests must name existing prepared/snapshot identities):

```json
{
  "schema_version": 3,
  "request_id": 17,
  "method": "run",
  "params": {
    "snapshot": "0000000000000000000000000000000000000000000000000000000000000000",
    "selection": {
      "kind": "tests",
      "filter": "tests::dot_product",
      "exact": true,
      "ignored": "exclude"
    },
    "jit": "auto",
    "jit_tier": "auto",
    "execution_contract": "precise",
    "execution": "hybrid-native",
    "allow_native": true,
    "native_generation": "1111111111111111111111111111111111111111111111111111111111111111",
    "policy": {
      "read": [],
      "write": [],
      "env": {}
    },
    "limits": {
      "fuel": 10000000,
      "timeout_ms": 10000,
      "memory_mib": 256,
      "output_limit_kib": 4096
    },
    "stream_output": false,
    "replace_pending": false,
    "allow_macro_exec": false
  }
}
```

The canonical JIT field is `jit`; do not introduce a second `mode` spelling. Managed/hybrid-native execution and off/auto/force JIT selection are independent dimensions. All run fields described here and in Section 56 are required; CLI defaults are managed/false/null/false and are serialized explicitly.

The version-4 internal worker handshake additionally carries execution mode, generation digest, and portable-cache version. The worker reports library handshake completion before any test starts. Prepare probe and execution worker have distinct request types; a probe cannot accidentally execute test bindings.

### 42.4 Hybrid test result additions

Retain all common result fields and add these required hybrid fields:

```json
{
  "execution": "hybrid-native",
  "profile": "lrr-2024-v2",
  "rust_validation": "live_not_performed_native_prepared",
  "sandbox_strength": "none_native",
  "native_generation": "generation-digest",
  "native_freshness": "recorded_inputs",
  "native_global_reset": "per_run_request",
  "native_profile": "release",
  "native_toolchain": "recorded-rustc-version",
  "native_panic_strategy": "unwind",
  "native_binding_entries": 0,
  "native_handle_drops": 0,
  "native_bytes_in": 0,
  "native_bytes_out": 0,
  "cargo_invocations_during_run": 0,
  "compiler_invocations_during_run": 0,
  "portable_queries_imported": 0,
  "portable_queries_validated_hit": 0,
  "portable_queries_recomputed": 0,
  "jit_code_restored_from_prior_worker": false,
  "relocatable_code_cache_hits": 0,
  "relocatable_code_cache_misses": 0,
  "fuel_coverage": "live_only",
  "memory_limit_coverage": "managed_and_bridge_buffers"
}
```

These zero-valued counters illustrate field types, not a completed run. jit_code_restored_from_prior_worker is true only after at least one validated cached object from an earlier worker is rebound and installed; it never means restoring a process memory image. The counters record actual hits/misses, not a fixed false policy. Keep the legacy native_entries counter restricted to Cranelift region entries; jit_region_entries counts Cranelift plus LLVM entries, with actual_tier_entries supplying the per-backend split. These fields are no longer aliases when LLVM executes. `native_binding_entries` counts calls into Cargo-native code and must never be inferred from Cranelift entries. Report compiler/Cargo absence using process-invocation hooks and acceptance-test supervision, not a hardcoded zero in every result.

Prepare results separately report `cargo_units_fresh`, `cargo_units_rebuilt`, bridge compile/link elapsed time, native artifact filenames/digests, observed profile fields, configured bridge incremental setting, and `rustc_incremental_hit_measurement="not_observed"` unless genuine compiler evidence is collected. Cargo's fresh flag does not reveal how many internal rustc queries were reused in a rebuilt unit.[^H17]

### 42.5 Watch behavior

Managed watch remains persistent-worker operation. Hybrid watch retains a coordinator, polls source changes, and starts a fresh worker for each authorized run request. The initial `--allow-native` applies to the currently prepared generation for that watch session. A newly prepared generation requires a new explicit authorization or an explicit service run naming it; do not silently expand a session's trust to unrelated native bytes.

Live-only edits reuse the generation and portable checkpoint. Native input, manifest, lockfile, bridge schema, compiler configuration, or generation changes stop automatic execution with a preparation-stale event. The user/agent explicitly prepares and resumes. A cancelled native call is never retried automatically on the same snapshot; a subsequent explicitly requested run is a new execution with its own possible external effects.

<a id="section-43"></a>

## 43. Required hybrid evidence, diagnostics, and performance gates

### 43.1 Golden fixture workspace

Create `fixtures/hybrid/workspace` with edition-2024 live packages and these roles: `native_math` (public scalar functions and a generic identity), `native_state` (opaque Counter and Drop logs), `live_unit` (live library with private tests and an intentionally broken cold body), and `api_tests` (live integration tests against a normal native library). A fifth `native_build` fixture uses a tiny build script and a local proc-macro fixture only to test preparation authority; neither executes as live code.

Keep native fixture dependencies local/path and standard-library-only wherever possible. No network is necessary. Give each acceptance test a separate temporary workspace and Cargo target directory unless it specifically tests shared-cache reuse. Cache-sharing tests reuse one directory intentionally and record the ordered build/run operations. Do not let an earlier global CI build fabricate the baseline being measured.

Instrumentation belongs in the coordinator process-launch adapter and native test wrappers: record command argv, role, workspace, timestamps, artifact JSON, native entry count, ordered append-only effect log, and generation hash. A wrapper that simply returns an expected number without calling its dependency does not establish artifact reuse. Inspect the generated wrapper source and require the returned number to change after a native dependency edit.

### 43.2 Mandatory cases

| ID | Setup/action | Required evidence |
|---|---|---|
| HN001 | Build native_math in release before generating any LRR bridge; then prepare | Native dependency artifact is reported fresh under a matching configuration; no live crate compiler invocation; final bridge returns a dependency-computed value. |
| HN002 | Repeat unchanged native prepare | Bridge and native dependency artifacts are fresh; publication reuses an identical generation. |
| HN003 | Edit only a live test's arithmetic/assertion and run again | Zero Cargo/rustc/linker invocations; same generation; new live outcome; portable unchanged query hits. |
| HN004 | Keep invalid cold function and invalid untaken branch in live_unit | Selected live test passes; those operations unprepared; Cargo did not compile live_unit. |
| HN005 | Change native_math implementation | Run fails stale before loader; explicit prepare rebuilds affected units; next run observes new value. |
| HN006 | Add a binding or new concrete generic type | Reached missing-binding diagnostic before prepare; explicit prepare compiles new wrapper; no automatic test replay. |
| HN007 | Change Cargo profile, RUSTFLAGS, compiler, features, or explicit target mode individually | Preparation fingerprint changes; Cargo may rebuild; report actual fresh flags rather than asserting all changes are hits. |
| HN008 | Delete `.rmeta`/keep irrelevant stale `.rlib`; or provide only cargo-check artifacts | Prepare lets Cargo rebuild correct code or fail; runtime never loads metadata/archive files. |
| HN009 | Use a nonstandard configured target/build directory | Artifact discovery follows Cargo JSON; no hard-coded target/release path. |
| HN010 | Native closure depends on live_unit | Deterministic closure-overlap diagnostic before build, with shortest dependency path. |
| HN011 | Live integration test against its package's ordinary native lib | Test source edits do not compile/test-harness-build the lib when its recorded inputs are unchanged. |
| HN012 | Select private unit test and mark the same library native | Explicit unsupported partition error; never silently switch to public integration semantics or missing cfg(test) code. |
| HN013 | Scalar, Unicode, byte, String, Option/Result, tuple round trips | Exact codec values; invalid encodings fault without binding entry; FFI allocations freed on their allocating side. |
| HN014 | Opaque object new/method/move/drop sequence | One owner, correct type/generation, exactly one native destructor, explicit clone produces independent handle. |
| HN015 | Pass one object as both mutable/shared, mutable/mutable, or owned/borrowed arguments | Reject before native entry; effect counter remains zero; no RefCell panic. |
| HN016 | Overlapping shared bytes and disjoint mutable bytes | Shared pointer overlap relation preserved within the call; disjoint writebacks correct; shared/mutable overlap rejected. |
| HN017 | Native mutates staged bytes then panics under unwind profile | Final mutation copied back before guest cleanup; one native entry; should_panic matches payload only. |
| HN018 | Native abort/segfault/exit and timeout fixtures | Coordinator survives; infrastructure failure or timeout, never should_panic success; no automatic replay. |
| HN019 | Force reply allocation/decoding/publication failure after native entry | Effect log has one entry; status marks may-have-executed; no fallback or repeated call. |
| HN020 | Normal, early-return, guest panic, native Drop panic, fatal managed fault | Defined destructor order; double panic fails; fatal fault does not execute further native Drop. |
| HN021 | Test batches mutate native statics | Same-request tests see declared shared process state; a new run request starts from initialized state. |
| HN022 | Managed mode or hybrid without native authorization | Loader constructor sentinel never fires; precise permission/configuration error before load. |
| HN023 | Hybrid native reads file/network despite absent managed grants | Result always reports no sandbox; managed facade still denies its ungranted access; tests use local controlled resources. |
| HN024 | Build script/proc macro fixture without build authorization | Neither executes. With authorization it may execute only during prepare; no build during run. |
| HN025 | Constructor writes a sentinel during loading | Sentinel occurs only in authorized probe/worker processes, never coordinator or init preview. |
| HN026 | Native stdout includes JSON-looking and binary/invalid-UTF8 output | Internal protocol stays intact; output captured separately with safe encoding/truncation and origin tags. |
| HN027 | Corrupt/truncate bridge manifest, API version, reply, or checkpoint | Fail closed before unsafe interpretation; corrupt pure checkpoint may cold-fallback only; wrong library never accepted. |
| HN028 | Add a formerly missing native binding/path | Negative lookup invalidates; unrelated live query remains reusable; new generation required. |
| HN029 | Native build input changes during build or capture | No generation published; previous authorized generation not silently relabeled current. |
| HN030 | External declared file/env changes; build script undeclared-input example | Declared change marks stale; report recorded-input rather than hermetic guarantee for undeclared case. |
| HN031 | Delete original rlibs after publishing valid bridge | Run uses final generation with zero build; next prepare rebuilds missing Cargo inputs as required. |
| HN032 | Mixed native release overflow/debug_assert behavior and checked live arithmetic | Report accurate distinct profile semantics; do not compare against debug Cargo results as equivalent. |
| HN033 | Guard exits before/after native call under off/auto/force | Same values/effects/call count; completed native call never reexecutes on JIT exit. |
| HN034 | Import portable checkpoint after body/signature/impl/edition edits | Every dependency validated; stale IDs absent; changed result matches cold interpreter. |
| HN035 | Native dynamic dependency missing or loader environment changed | Reject load/preparation as prescribed; no use of arbitrary fallback libraries. |
| HN036 | Cancel inside native call; native child process writes controlled log | Worker group termination attempted; output explains external effects may persist; queued execution not mistaken for retry. |

The parser/ABI tests additionally fuzz arbitrary request/reply bytes with bounded lengths, invalid native handles, duplicate groups, and malformed UTF-8. Fuzz only the safe decoders with arbitrary bytes; never fuzz `dlopen` of arbitrary attacker-written machine code and call it a safety proof. Native bridge object tests may use Miri independently as host test tooling, but Miri is not an execution backend and is not required on the user's edit/run path.

### 43.3 Diagnostic registry additions

Every entry below must define phase, whether binding entry can have occurred, cleanup policy, and corrective command in `compat/diagnostics.toml`:

| Code | Trigger / prescribed action |
|---|---|
| `LRR-E-EDITION-PATTERN` | Reached 2024-incompatible binding modifier; use an explicit compatible pattern. |
| `LRR-E-NATIVE-AUTHORIZATION` | No native build/run authorization; stop before invoking the associated authority. |
| `LRR-E-NATIVE-PARTITION` | Native runtime closure intersects live units; identify dependency edge to change. |
| `LRR-E-NATIVE-CONFIG` | Unsupported manifest/schema/profile/target condition; name exact field. |
| `LRR-E-NATIVE-WORKSPACE-RACE` | Preview/apply original file digest changed; regenerate preview. |
| `LRR-E-NATIVE-PREPARE-STALE` | Recorded native inputs changed; require explicit native prepare. |
| `LRR-E-NATIVE-INPUT-RACE` | Inputs changed during native preparation; no publication. |
| `LRR-E-NATIVE-BUILD` | Cargo/rustc/linker failure; attach actual generated binding origin and diagnostics. |
| `LRR-E-NATIVE-BINDING-MISSING` | Reached unprepared path/type specialization; give binding-manifest example and prepare command. |
| `LRR-E-NATIVE-ABI` | Version, table, wire, or schema mismatch; no retry and no reinterpretation. |
| `LRR-E-NATIVE-HANDLE` | Wrong type, epoch, generation, ownership, or already consumed object. |
| `LRR-E-NATIVE-ALIAS` | Incompatible concurrent argument borrows; identify argument positions and ranges. |
| `LRR-E-NATIVE-CODEC` | Unsupported or malformed value; no implicit layout or arbitrary serde fallback. |
| `LRR-E-NATIVE-POSTCALL` | Call entered but result/writeback failed; effect state may-have-executed, no retry. |
| `LRR-E-NATIVE-PROCESS` | Native abort/crash/exit/protocol loss; infrastructure failure, no guest-panic match. |
| `LRR-E-NATIVE-DEPENDENCY` | Missing recorded dynamic library or unresolvable load environment. |
| `LRR-E-PORTABLE-CACHE` | Invalid pure checkpoint; report discarded import and cold execution, unless resource policy forbids proceeding. |

Native caught panics use the common guest-panic representation rather than inventing a diagnostic fault category. `LRR-E-NATIVE-POSTCALL` and process failures must include the active binding ID, source call origin, and entry sequence when available. Diagnostic formatting itself must not call native Debug or Drop.

### 43.4 Performance experiment and honest acceptance targets

Measure five separate scenarios on the same pinned macOS machine for primary acceptance; repeat on a pinned native Linux machine for the later port, with at least 30 timed samples after setup: first bridge preparation from compatible pre-existing artifacts; unchanged explicit prepare; fresh-process warm-generation test; live-only edit followed by test; and native dependency edit followed by prepare/test. Publish medians and p95 with source/project sizes, CPU, storage, toolchains, profiles, LTO settings, test count, process startup, and whether OS file caches were warm.

Use four workloads: one scalar native call; one native byte-buffer batch; an opaque-object lifecycle with many method calls; and a larger dependency dominated by native execution. Compare supported cases with normal `cargo test --release` and managed-only LRR where the same code is in scope. Do not compare against `cargo clean` unless the experiment is explicitly a cold-build comparison. Include a deliberate unfavorable tiny-call loop to quantify marshalling/dispatcher overhead.

Correctness/reuse gates are hard requirements: HN001 proves dependency freshness in the controlled matching fixture; HN003 proves zero compiler/linker invocations after live edits; every hybrid mode preserves result/effect semantics; native entry and process reset counters are actual. Latency targets are **unmeasured engineering targets**, not product facts: under 100 ms median for a small fresh-worker warm-generation live-only test after capture on the reference machine, and native bulk compute within 20% of the same wrapper in a minimal native harness when boundary costs are amortized. Missing these targets requires a profiling report and a concrete local optimization issue; never remove guards, reuse native global state, skip tests, or hardcode a success to hit them.

Profiles with LTO, very large generated monomorphizations, heavy constructors, large source snapshots, or frequent fine-grained native crossings may miss the targets. Publish that evidence. “Instant” means the user-visible goal of no repeated compiler/linker on the common edit loop, not a promise of zero startup or zero work.

<a id="section-44"></a>

## 44. Ordered edition/hybrid implementation tasks and gates

### 44.1 Execution order

Apply H001–H003 while implementing the original core tasks, before considering the Rust 2024 edition ready. Build the standalone Cargo probe H004 immediately after bootstrap; it validates the reuse premise without waiting for the whole interpreter. Finish the common engine gates with 2024 semantics. Then implement H005–H028 in order. Each task is required; the probe is not a substitute for hybrid runtime integration.

Keep an implementation ledger with task ID, commit, commands actually run, fixture IDs, artifact paths, and pass/fail evidence. A source file existing is not an acceptance result. Each gate is a script under `xtask`, exits nonzero on skipped/missing tests, and writes a machine-readable report with the runtime build ID.

### 44.2 Edition gate

**H001 — Edition identities and parser.** Files: core/profile.rs, syntax/parse.rs, project/units.rs, fixtures/edition2024. Retain 2024 as the standalone default while admitting declared 2015/2018/2021/2024 selected roots. Include edition in every affected cache key and checkpoint header, update CLI/help/result fixtures, and preserve native dependency editions. Acceptance: parser smoke uses Edition2024; owning-edition parser/CLI/Cargo-root pairs and E2409/E2411 pass; all four official roots, dependencies and macro origins retain their supported edition semantics, while unknown/unreleased editions are rejected.

**H002 — 2024 temporary lifetime machinery.** Files: syntax/body_index.rs, vm/control.rs, vm/cleanup.rs. Implement the explicit success/failure boundaries, tail temporary drain, and let-chain clause scopes in Section 34. Add native-oracle output fixtures before implementation. Acceptance: E2401–E2407/E2410 pass under off/auto/force with equal drop transcripts and no preparation of skipped clauses.

**H003 — Pattern ergonomics and reserved namespace.** Files: vm/pattern.rs, frontend/names.rs, facade/prelude.rs, compat/features.toml. Thread DefaultBindingMode through matching, reject only the reached prohibited modifier, and add unsupported async-prelude identities and edition tokens. Acceptance: E2408 plus shadowing/explicit-reference fixtures; full prior pattern suite rerun. Gate `xtask edition2024-gate` requires H001–H003 and all E24 fixtures except native-dependent E2412, which joins the final hybrid gate.

### 44.3 Native reuse proof and preparation gate

**H004 — Artifact reuse probe.** Files: tools/cargo-reuse-probe, artifacts/probe. Run the standalone procedure in Section 45 with a matching release baseline. Acceptance: Cargo reports native dependency fresh when first compiling the bridge; bridge changes do not compile live source; native source edit changes returned value and marks the relevant dependency dirty. Record raw JSON and toolchain. A failure must be reduced before native-loader implementation.

**H005 — Native transport configuration.** Files: native/schema.rs, project/native_config.rs. Parse schema2 with direct default, explicit codec bindings, concrete types/modes and per-binding IR requests. Acceptance: canonical keys include transport/producer/IR; schema1 migration assigns codec explicitly; malformed paths, arbitrary code, duplicates and unknown transport/mode fail before tools run.

**H006 — Native/live partition.** Files: project/native_units.rs, project/cargo.rs. Resolve package/target aliases with Cargo metadata, form runtime/build closures, distinguish lib-tests from integration-test units, and reject native dependence on live units. Acceptance: HN010–HN012; preparation logs show no live-unit build command.

**H007 — Workspace transaction.** Files: project/native_workspace.rs. Implement preview/apply backups, marker ownership, member/default-member preservation, package-specific profile overrides, compare-and-swap writes, and safe rollback. Acceptance: no writes in preview; repeated init is byte-identical; user comments preserved where toml_edit supports them; HN007/HN009 workspace cases and concurrent-edit fixtures pass.

**H008 — Producer and exporter invocation capture.** Files: project/toolchain.rs, project/native_build.rs. Record exact toolchain/configuration and install the ABI exporter wrapper only for the generated bridge; preserve delegate invocations and explicit IR output configuration. Acceptance: codec compiler minimum enforced; direct Rust 1.99.0 producer/exporter match required; host/explicit target preserved; wrapper/IR rebuild effects reported; no fallback compiler or telemetry-only flag mutation.

**H009 — Cargo message collector.** Files: project/native_build.rs, project/native_messages.rs. Build only the generated bridge package, consume bounded JSON lines, retain non-JSON output as diagnostics, find exact cdylib artifact, and track fresh/rebuilt units. Acceptance: HN001/HN002/HN008/HN009/HN024; cached build-script output is not falsely counted as a script execution.

**H010 — Native input inventory.** Files: project/native_inputs.rs, project/dep_info.rs. Implement dep-info lexer/rules, source-directory membership, conservative fallback inventory, external file/env entries, generated OUT_DIR capture, and pre/post build source-race checks. Acceptance: escaped-path unit tests, HN029/HN030, native-source overlay change, and integration-test-only change that does not invalidate normal library when inputs permit.

**H011 — Immutable generation and receipt publication.** Files: native/generation.rs, native/probe.rs, project/native_prepare.rs. Publish schema2 library, binding request, ABI sidecar, successful same-invocation receipt and optional IR import bundle atomically. Acceptance: HN025/HN027/HN031/HN035 plus NX011/NX012/NX028 after H013 integration; no coordinator dlopen, stale sidecar or source/probe failure publication.

Gate `xtask native-prepare-gate` requires H004–H011, the original bootstrap gate, and HN001/HN002/HN007–HN012/HN024/HN025/HN027/HN029–HN031/HN035 preparation assertions. No interpreter changes can bypass this gate by loading an arbitrary test `.dylib` path.

### 44.4 ABI and boundary gate

**H012 — Native control and ABI schema.** Files: native/abi.rs, native/codec.rs, native/schema.rs, templates/native-bridge/ffi.rs. Implement direct control registration, bounded stable producer ABI/layout records and explicit codec-v1 wire interface. Acceptance: NX012/NX013/NX022/NX023; C control layouts and codec golden bytes match; unsupported ABI tags/widths, malformed lengths, null/mismatched entries and pointer injection reject before entry.

**H013 — Specialization and compiler metadata generator.** Files: native/generate.rs, tools/native-abi-exporter, templates/native-bridge. Generate typed concrete function-pointer declarations, per-entry interpreter invokers, catch adapters and same-invocation ABI/layout/optional IR exports. Acceptance: NX011/NX013/NX014/NX024: real generic scalar/aggregate/ZST/native-trait specializations; private/unsafe/wrong-signature/world cases reject; no guessed Rust ABI transmute or live-unit compilation; T087 completes IR execution acceptance.

**H014 — Native resident storage and ownership.** Files: templates/native-bridge/store.rs, native/object.rs. Implement generation-typed initialized/uninitialized native storage, epoch handles, moves/drop and codec adoption under one logical owner. Acceptance: HN013/HN014/NX015/NX016: stale/type/generation rejection, zero-sized Drop and by-value ownership exactly once, allocator pairing, no duplicated owner across transports.

**H015 — Direct leases and explicit codec staging.** Files: native/borrow.rs, native/call.rs, templates/native-bridge/staging.rs. Pin direct native/byte storage with ordered leases; preserve existing codec staging/writeback only for codec bindings. Acceptance: HN015/HN016/NX017: shared overlap, mutable conflict, disjoint ranges, zero length, UTF-8, no escaped reference, direct mutation visible without copyback; no silent copying fallback.

**H016 — Native entry and panic transaction.** Files: native/call.rs, native/panic.rs, templates/native-bridge/entry.rs. Implement forward-only entry, raw abort pointers, Rust-ABI contained catch adapters and codec panic replies. Acceptance: HN017/HN019/NX019/NX020: native mutation and cleanup on caught panic, fatal abort attribution, payload cleanup failure and no unwind through LRR; entered calls never retry.

**H017 — Cleanup integration.** Files: vm/cleanup.rs, runtime/value.rs, runtime/shape.rs, native/object.rs. Add kind14 descriptors, explicit native drop actions, native non-Copy/opaque behavior, per-test empty-context assertion, and fatal no-user-Drop teardown. Acceptance: HN014/HN020, no native value enters a managed struct-layout operation, and mem::forget policy is defined as Section 44.7 prescribes.

Gate `xtask native-abi-gate` requires H012–H017 and HN013–HN020 against the controlled native workspace; use a narrow ABI driver before integrating the full JIT. A mock FFI function alone cannot satisfy it.

### 44.5 Execution, incrementality, and release gate

**H018 — Native namespace/query records.** Files: db/key.rs, db/provider.rs, frontend/names.rs, frontend/dispatch.rs, native/schema.rs. Implement the four pure native queries and complete explicit native namespace. Acceptance: HN006/HN028; no native call or loader runs in a query provider; native runtime bodies have zero executable preparation; separately identified macro-export header reads are permitted and counted.

**H019 — Direct native calls in both tiers.** Files: vm/call.rs, engine/dispatch.rs, jit/record.rs, jit/lower.rs. Add NativeBinding/NativeCallOp, interpreter slot invokers and measured fixed scalar invokers (NX029), resident/conversion accounting (NX030), and matched Rust ABI calls inside reached regions, with checked and scalar paths. Acceptance: HN003/HN004/HN033/NX018/NX021: real interpreter/JIT entries, pre/post-call exits exactly once, identical ownership under the same contract; T089 supplies optimized/LLVM integration.

**H020 — Fresh hybrid worker and supervision.** Files: service/coordinator.rs, engine/worker.rs, native/loader.rs. Spawn one process per hybrid run request, one native generation per process, one object context per test; retain no loaded library in coordinator; implement process-group deadline handling and no-dlclose lifetime. Acceptance: HN018/HN021/HN025/HN036; repeat request resets a native static; tests in one batch demonstrate declared shared globals.

**H021 — Dedicated output/control channels.** Files: service/wire.rs, service/coordinator.rs, cli/render.rs. Separate native stdout/stderr from the internal v4 control/raw-data socket; bounded draining/capture, origin tagging, and missing-result infrastructure errors. Acceptance: HN026 and native exit(0) fixture; parser never treats native JSON-like logs as protocol messages.

**H022 — Portable checkpoint encoding and import.** Files: cache/portable.rs, cache/remap.rs, db/memo.rs, source/snapshot.rs. Implement whitelist, stable symbolic records, version checks, deterministic identity remapping, import with unverified revision, and current-input dependency validation. Acceptance: HN003/HN027/HN028/HN034; corrupt checkpoint cold-falls back; stale negative lookup and body indexes never execute.

**H023 — Portable checkpoint transport and bounds.** Files: service/wire.rs, service/coordinator.rs, cache/portable.rs. Keep pure query checkpoints bounded and pointer-free, encode once and transfer through internal v4 raw chunks with CA023/CA024 controls; transport relocatable compiled objects only through T088's separate validated artifact channel. Acceptance: HN003/HN027/HN028/HN034: fresh worker restores pure facts but no guest state/pointers; corrupt checkpoints cold-fall back; separate code cache requires relocation validation and never uses query deserialization.

**H024 — CLI/service authority and freshness.** Files: cli/args.rs, service/requests.rs, native/generation.rs. Add explicit mode/authorization methods, mandatory pre-run native freshness checks, watch stale-stop, and mutually exclusive hardened sandbox request. Acceptance: HN003/HN005/HN022/HN023/HN024; a service native miss cannot trigger prepare; generation changes require newly scoped authorization.

**H025 — Telemetry and diagnostics.** Files: engine/telemetry.rs, cli/render.rs, compat/diagnostics.toml. Separate Cargo/native/JIT/query cache counters, settings/freshness/security/reset labels, effect-state reports, and actual invocation accounting. Acceptance: result schema fixtures, HN007/HN019/HN032, and test harness fails if any counter is fabricated or double-counted.

**H026 — Full hybrid conformance.** Files: xtask/hybrid.rs, fixtures/hybrid, fixtures/edition2024. Run every HN fixture in every applicable live tier; compile native oracle only for intended valid-profile comparisons. Acceptance: HN001–HN036 and E2412 pass; skipped required fixture is failure; controlled native crashes do not stop the whole report.

**H027 — Native performance evidence.** Files: xtask/bench_hybrid.rs, artifacts/hybrid-performance.json, docs/performance.md. Run Sections 43/61/62 native-pointer, typed C, direct Rust, codec, checked/scalar JIT, loop and LLVM-inline comparisons. Acceptance: NX026/NX027 raw 50-batch paired samples, producer/CPU/profile identity and assembly evidence; raw versus full-runtime overhead and unsuccessful targets reported independently.

**H028 — Native release audit and handoff.** Files: xtask/release.rs, docs/security.md, docs/hybrid-native.md, AGENTS.md. Audit direct/codec/IR scope, native/live identities, separate authorities, no live build tools and exact-once effects; final acceptance includes T089. Acceptance: all native behavior/ABI/ownership fixtures proven, none_native tested, no unresolved required direct transport or optimizer requirement labeled complete; retain provisional scope until T089 and release gates pass.

### 44.6 Required release commands

Implement these commands in xtask before the corresponding task is complete:

```sh
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- edition2024-gate
cargo +1.99.0 run --locked -p xtask -- native-prepare-gate
cargo +1.99.0 run --locked -p xtask -- native-abi-gate
cargo +1.99.0 run --locked -p xtask -- hybrid-gate
cargo +1.99.0 run --release --locked -p xtask -- hybrid-bench
cargo +1.99.0 run --locked -p xtask -- release-audit
```

The host Cargo commands above compile/test LRR itself. Assertions that a guest run invokes no compiler concern the supervised runtime subprocess tree **after** these host tools start the fixture, not the outer CI command. Distinguish host build, native preparation, and live execution roles in every process trace.

### 44.7 Additional corner-case decisions

`mem::forget` on an opaque native handle marks it intentionally forgotten in the worker and native context. It schedules no user Drop. The context's success invariant accepts explicitly forgotten slots as intentional leaks and transfers them into a process-lifetime forgotten store; the bridge context can then be destroyed without dropping those native values. The store is released only by process exit, without user destructors. Cap forgotten handle count/metadata under the test's cumulative quotas and report them; do not confuse a deliberate forget with an unexplained ownership leak. In managed mode the original physically reclaimable representation rule still applies; native resources cannot be reclaimed by a foreign allocator without potentially running user code.

Native constants and statics are not directly exposed as guest addresses. Admitted compiler IR may reference the existing generation's state only through the closed internal import contract in Section 62; it must not duplicate or expose that state. Expose a getter/setter binding explicitly. Native public enum fields or struct construction require explicit constructor/accessor bindings or owned data conversion; reflection through layout is excluded. Registered native function pointers remain internal callable capabilities; guest function values carry checked binding identities, never pointer bits.

A live function returning or accepting a native type may mention its opaque schema in its signature. That is a concrete call-boundary contract, not permission to materialize its fields. A live nominal type cannot be declared Copy merely because its only field is a native opaque handle; the existing Copy recipe rejects that reached copy.

Native `Result::Err` values that a test wants to print must use a supported owned Debug representation, such as String, or an explicitly bound conversion. Do not call native formatting automatically from diagnostics. Rust-standard-library APIs on the native side are compiled ordinary code; their presence does not expand the live std facade.

<a id="section-45"></a>

## 45. Standalone Cargo reuse probe and first implementation experiment

The accompanying `lrr_cargo_reuse_probe` source bundle contains `probe.py` and README.md. It is a small executable experiment for the artifact-reuse premise, not an implementation of LRR. It uses only the Rust standard library and Python's standard library, and creates all Cargo packages in a new or empty directory. The source was syntax-checked as Python while preparing this document; **Cargo/rustc were unavailable, so the Rust builds, cache-hit expectations, and dynamic-library calls have not been run here**.

Execute on the macOS development machine first, with platform-specific Cargo artifact discovery and native loading. Repeat on Linux x86-64 in the later port:

```sh
python3 probe.py --allow-native-build --work-dir /tmp/lrr-reuse-check
```

The path must be new or empty. The explicit flag authorizes native build tools and generated library execution. The script retains the workspace and per-command stdout/stderr under its `evidence` directory. It fails rather than quietly discarding an unexpected artifact-reuse result.

The probe performs this exact sequence:

1. Create an edition-2024 workspace with a `native_math` library and a live package whose cold function is deliberately ill-typed. The live package must never be compiled.
2. Build `native_math` under the ordinary release profile and record the actual `.rlib` path, content hash, Cargo messages, and tool version. There is no pre-existing LRR sidecar or bridge.
3. Add a generated-style cdylib workspace member that calls `native_math::scale` through ordinary Rust and exports a scalar C entry. Add only its package-specific optimization/incremental override and update the lockfile offline.
4. Build only that bridge package. Require Cargo's native_math artifact to be fresh and byte-identical to the baseline while the new bridge is compiled.
5. Load the reported `.dylib` in a disposable Python child using ctypes and verify the native computation. This child is intentionally unsandboxed and handles no LRR protocol.
6. Repeat unchanged bridge build and require both bridge and dependency to be fresh.
7. Edit only the live test source and load the already linked native library without any Cargo command. This demonstrates reusability of the native artifact; **it does not interpret or execute the changed test**.
8. Add a new exported wrapper for a concrete generic native function. Rebuild bridge; require dependency to stay fresh and bridge to rebuild. Verify both native exports.
9. Change the native implementation. Require the dependency and bridge to rebuild and the loaded result to change.
10. Write `evidence/report.json` with all commands, artifact records, hashes, native outputs, errors, and final pass/fail. The program exits nonzero on any failed assertion.

The probe's scalar export does not implement the production schema/buffer/opaque-object ABI. H012–H019 must build that separately. A passing probe establishes that the chosen matching Cargo configuration can reuse its library artifact through a newly linked bridge; it does not prove universal `.rlib` compatibility, arbitrary generic instantiation reuse, live test execution, sandboxing, or a latency target.

When the controlled probe misses the first dependency hit, inspect recorded profile/features, host-versus-explicit-target mode, RUSTFLAGS, compiler wrappers, Cargo configuration inherited from parent directories, and actual compiler/toolchain. Preserve the failing report. The architectural requirement is that Cargo decides compatibility; production preparation may rebuild a genuinely incompatible unit and report it. The controlled CI fixture must be repaired to create a truly matching baseline rather than suppressing its assertion.

<a id="section-46"></a>

## 46. Additional primary sources for Rust 2024 and native reuse

The choices in Sections 34–45 are this specification's engineering prescriptions. Sources below support upstream language, Cargo, and FFI facts, not a claim that this runtime exists or that the proposed benchmarks have passed. Consulted October 4, 2026. Public online pages can evolve; the implementation must retain the compiler/dependency pins and recorded build provenance used for its evidence.

[^H01]: Rust Edition Guide, [if-let temporary scope](https://doc.rust-lang.org/edition-guide/rust-2024/temporary-if-let-scope.html). Edition-2024 failure-path temporary lifetime change.

[^H02]: Rust Edition Guide, [tail-expression temporary scope](https://doc.rust-lang.org/edition-guide/rust-2024/temporary-tail-expr-scope.html), and Rust Reference, [destructors and temporary scopes](https://doc.rust-lang.org/reference/destructors.html). Tail temporary cleanup and detailed edition-specific scope rules.

[^H03]: Rust Edition Guide, [Rust 2024](https://doc.rust-lang.org/edition-guide/rust-2024/index.html). Edition availability beginning with Rust 1.85; subsequent individual features can have later stabilization versions.

[^H04]: Rust Edition Guide, [let chains](https://doc.rust-lang.org/edition-guide/rust-2024/let-chains.html). Edition requirement and condition-chain behavior; the LRR oracle is the pinned newer compiler, not a claim every feature existed in 1.85.

[^H05]: Rust Edition Guide, [match ergonomics reservations](https://doc.rust-lang.org/edition-guide/rust-2024/match-ergonomics.html). Explicit modifiers and reference patterns under inherited binding modes.

[^H06]: Rust Edition Guide, [gen keyword](https://doc.rust-lang.org/edition-guide/rust-2024/gen-keyword.html) and [reserved syntax](https://doc.rust-lang.org/edition-guide/rust-2024/reserved-syntax.html). Parser/token changes, not implemented generator support.

[^H07]: Rust Edition Guide, [2024 prelude](https://doc.rust-lang.org/edition-guide/rust-2024/prelude.html). Additional names that can affect lookup even when live async execution is excluded.

[^H08]: Rust Edition Guide, [macro fragment specifiers](https://doc.rust-lang.org/edition-guide/rust-2024/macro-fragment-specifiers.html). Native macro expansion is delegated to the dependency's compiler and edition.

[^H09]: Rust Edition Guide, [unsafe attributes](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html), and Rust Reference, [external blocks](https://doc.rust-lang.org/reference/items/external-blocks.html). Generated edition-2024 FFI syntax requirements.

[^H10]: Rust Reference, [linkage](https://doc.rust-lang.org/reference/linkage.html). The roles of rlib and cdylib and Rust library linkage; LRR's reuse design delegates compiler-specific handling to Cargo/rustc.

[^H11]: Rust compiler developer guide, [libraries and metadata](https://rustc-dev-guide.rust-lang.org/backend/libs-and-metadata.html). Compiler metadata is not a language-independent dynamic reflection API.

[^H12]: Cargo Book, [build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html). Target/build directories, internal cache layout, and public dep-info integration boundary.

[^H13]: Cargo Book, [profiles](https://doc.rust-lang.org/cargo/reference/profiles.html). Release defaults, incremental settings, profile selection, override limitations, and differing native/live overflow/debug settings. LRR's bridge-only override is a chosen policy, not a Cargo default.

[^H14]: libloading 0.8.9, [API documentation](https://docs.rs/libloading/0.8.9/libloading/). Dynamic library/symbol lifetime API. A loader API is not a sandbox.

[^H15]: toml_edit 0.23.7, [API documentation](https://docs.rs/toml_edit/0.23.7/toml_edit/). Preserving manifest formatting where supported while applying checked workspace edits.

[^H16]: Rust compiler developer guide, [monomorphization](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html), and Cargo Book, [profiles: overrides and generics](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides-and-generics). Why new concrete generic bindings can require code generation despite existing dependency artifacts.

[^H17]: Cargo Book, [external tools and JSON messages](https://doc.rust-lang.org/cargo/reference/external-tools.html). Compiler-artifact paths, profile/features, `fresh`, and cached build-script-output messages.

[^H18]: Rustonomicon, [FFI](https://doc.rust-lang.org/nomicon/ffi.html). Native argument validity, ownership, external calling conventions, callbacks, and resource-boundary concerns. The exact LRR codecs and dynamic access rules are specified here rather than supplied by that chapter.

[^H19]: Rust Reference, [type layout](https://doc.rust-lang.org/reference/type-layout.html) and [ABI](https://doc.rust-lang.org/reference/abi.html). Explicit layouts and the reason guest managed values are not native Rust values.

[^H20]: Rust Reference, [functions: ABI and unwinding](https://doc.rust-lang.org/reference/items/functions.html), and Rustonomicon, [FFI and unwinding](https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding). Native catch boundaries versus process termination; no Rust unwind crosses LRR's extern-C interface.

[^H21]: Cargo Book, [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html), [environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html), and [workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html). Native preparation authority, configuration inputs, workspace membership, shared lockfile/profile context, and output provenance.


<a id="section-47"></a>

## 47. Revision 5 contract and demand boundaries

### 47.1 The one selected architecture

Retain the independent live frontend, canonical tagged-value VM and demand-driven trait engine. Implement the direct Rust ABI native generation in Section 61 and typed optimizing execution in Section 62. Cranelift and LLVM lower reached LRR operations; rustc HIR/THIR/MIR stays outside the live frontend and may be used only by the explicit native preparation adapter. The macro adapter remains limited to token processing, execution protocol and hygiene.

```text
Explicit preparation                         Live request
--------------------                         ------------
Cargo exact unit graph                       immutable source/config snapshot
       |                                             |
compile-time-deps                                    discovery demands
       |                                             |
macro libraries + build outputs  --------> hygienic expansion records
                                                     |
                                           sealed declaration indexes
                                                     |
                                  source-context member identification
                                                     |
                                  necessary concrete goals/dictionaries
                                                     |
                                  interpreter / Cranelift / LLVM tiers
                                                     |
                                  direct Rust ABI / explicit codec bindings
                                                     |
                                        optimized Cargo-native island
```

Expansion can execute an already-compiled procedural macro. That is not interpretation of the macro's implementation and is not compilation of the live source. Tool compilation occurs only in prepare. Generated code is not specially trusted: it uses the same managed operations and checked references as handwritten code. Native macro execution has a different security label from managed execution of its output.

### 47.2 Demand classes and telemetry

Every frontend action includes a DemandReason with one of: SelectedRoot, ImportDiscovery, MacroName, ScopeCompleteness, ImplCompleteness, TestDiscovery, CallIdentity, CallContract, ImplApplicability, ProjectionLayout, DynView, CopyPolicy, DropPolicy, ConstantUse, or ExecutableStep. Record the parent demand ID, source origin, duration, cache outcome, and number of newly indexed headers/prepared bodies. The identifier is diagnostic causality, not a semantic cache key.

The four kinds of laziness are fixed:

| Kind | Required behavior | Work that may still precede execution |
|---|---|---|
| Body laziness | No executable recipe for an unentered body or untaken ordinary expression | Parsing, scope indexing, declaration-sensitive expansion |
| Call laziness | Do not create a callee frame or compile its regions before an actual call | Read signatures or impl headers to resolve the call |
| Goal laziness | Prove only goals required by a reached operation or selected representation | Predicates needed to choose a conditional impl; necessary supertrait/projection facts |
| Tool reuse | Reuse prepared macro binaries/build outputs and native artifacts | Explicit preparation when recorded tool inputs changed |

Do not redefine an impl catalog as complete merely to retain a stronger-looking laziness counter. A macro in an otherwise unused body can generate an impl affecting a reached call. Unknown output must be expanded or reported as blocking that call. A body with only an ordinary unresolved name is different and remains unprepared. Test both cases.

### 47.3 Feature completion and nonclaims

Mandatory: stable macro_rules matching/hygiene; function-like, attribute, derive proc macros; inert derive helpers; cfg/cfg_attr/import/reexport expansion; generated include source and captured compile-time env; tests generated by those mechanisms; conditional impl selection; source-correct generic method identity; associated types/consts/GATs; higher-ranked binder matching; user language traits listed in Section 54; managed dyn views and opaque return identities. These are release gates, not future suggestions.

Not claimed: accepting nightly specialization, negative trait bounds, trait aliases, arbitrary generic const expressions, executing arbitrary unsafe/native std internals, full closure capture semantics, async executor semantics, or arbitrary native/live ABI interchange. Ordinary stable source with an unsupported executed language/library operation reports its exact origin and unsupported feature. Expansion is not considered failed merely because its output contains such an operation in a cold body. Generated tests are never silently dropped for that reason.

A passing LRR test still is not proof that rustc accepts the program. Unused bounds and bodies are not validated; conflicting reached impls fail locally; dynamic ownership checks replace borrowing proofs. Native preparations remain normally checked Rust. The feature report distinguishes macro_frontend=general_stable, trait_resolution=demand, rust_validation=not_performed_live, and native_validation=rustc_at_prepare.

### 47.4 Definition of done for this revision

A release must run a selected test with a generated type/method, a conditional impl, and an associated type while an ordinary uncalled function contains a type error; run a proc-macro-generated test; consume build-script-generated include source; select a bound trait method instead of a same-named inherent method; and reuse an optimized native binding in that same system. It must show cache invalidation after a macro changes an impl, with no guest effects replayed. A separate normal cargo test proves the valid-fixture expected results. None of these outcomes may be satisfied by delegating the selected live test to rustc.

<a id="section-48"></a>

## 48. Exact Cargo units, tool preparation, and macro ABI compatibility

### 48.1 Pins and authority separation

The host runtime keeps Section 3's fixed toolchain/dependency bootstrap. Set tools/cargo-planner.toml to nightly-2026-10-01 and record the downloaded Cargo binary's absolute path, cargo -Vv, and content hash in the tool manifest. Do not use an unqualified nightly channel. This pin is a specification choice requiring the bootstrap capability tests; its availability and the mixed Cargo/rustc pairing are not claimed as tested here.

Resolve the project's rustc once with the project's explicitly selected rustup toolchain; capture rustc -vV, --print sysroot, host triple, and absolute executable. The nightly Cargo planner uses that executable via RUSTC. It must not silently switch macro libraries to the planner's rustc, because the procedural-macro runtime server must match the compiler that built them. Native bridge preparation continues to use the project's own Cargo as in Section 37. Keep planner cache under .lrr/tool-target/<planner-and-project-compiler-key>, separate from the original optimized target directory. Compile-time tool reuse is initially limited to this planner cache; native optimized-artifact reuse remains unchanged.

Select Cargo's documented --compile-time-deps and unit-graph interfaces. They are unstable interfaces, not a stable public SDK; exact output and behavior are compatibility-gated.[^M01] Do not use RUSTC_BOOTSTRAP, Cargo's private channel-override environment, a rustc wrapper returning fake success, or a normal build of the live crate.

### 48.2 Unit identity, not package identity

```rust
struct CargoUnitKey {
    package: CargoPackageId,
    target_name: String,
    target_kind: CargoTargetKind,
    role: UnitRole,             // Live, Native, BuildHost, ProcMacroHost, MacroExport
    compile_target: TargetTriple,
    test_intent: bool,
    edition: Edition,
    features: SortedSet<String>,
    profile_fingerprint: Digest,
    cfg_fingerprint: Digest,
    dependency_edges: Digest,
    rustc_fingerprint: Digest,
}
```

Package metadata identifies manifests and aliases; the unit graph identifies actual compilation modes/features/edges. Record distinct units when a package is a normal dependency and a host build dependency, or a lib-test target and an ordinary library. Never union their cfg/features or merge their OUT_DIRs. Cargo's resolver separates feature contexts in ways a flat package list cannot represent.[^M03]

Capture the graph with the exact target selector and feature flags used for preparation. Read Cargo's graph schema through a versioned parser behind project/cargo_units.rs. Unknown required schema fields/values fail capability validation. Normalize unit indices to content identities before persistence; Cargo array ordering is not a stable identity. Every recorded dependency alias resolves to an exact unit and role. cfg(test) applies to the selected test unit, not recursively to all its libraries.

### 48.3 Command construction

Construct argv arrays, not shell strings. Run in the original workspace root with the recorded environment and --manifest-path. The examples below show the required operations; paths and package/target values are filled from the validated manifest. Install tools only in the explicit bootstrap/install workflow, never during a run.

```sh
# Bootstrap installs only; not executed by lrr test.
rustup toolchain install nightly-2026-10-01 --profile minimal
rustup component add rust-analyzer --toolchain 1.99.0

# Lib unit-test intent. RUSTC must point to the selected project compiler.
RUSTC=/absolute/project/toolchain/bin/rustc \
  /absolute/nightly-2026-10-01/bin/cargo check \
  -Z unstable-options --compile-time-deps \
  --manifest-path /absolute/workspace/Cargo.toml \
  --package app --lib --profile test \
  --target aarch64-apple-darwin \
  --target-dir /absolute/workspace/.lrr/tool-target/KEY \
  --locked --offline --message-format=json-render-diagnostics
```

For an integration test use --test NAME with --profile test; for an ordinary library use --lib without test intent; for a binary use --bin NAME. Pass --no-default-features and --features exactly as selected; never add --all-features or --all-targets as a convenience. Obtain the unit graph with the same command selection, replacing --compile-time-deps and the message format by --unit-graph; retain -Z unstable-options. The pinned Cargo source determines the test-intent encoding; the check command's handling of profile test is specifically verified by the probe, not assumed from a profile name.[^M04]

Do not place --compile-time-deps on cargo test: command support is verified for the selected check operation. The graph must contain the intended live target with test mode and not build its runtime body. A graph mismatch stops before launching actual build programs. Capture effective target cfg via the recorded project compiler during preparation; that compiler information probe is permitted and separately tagged.

Environment precedence: start from explicit build environment inputs plus the approved host tool environment, apply project configuration, set the selected RUSTC and target directory, then allow Cargo to supply its per-unit values. Reject conflicting RUSTC/RUSTC_WRAPPER/RUSTC_WORKSPACE_WRAPPER overrides unless the exact wrapper is declared as part of the toolchain manifest and included in the capability probe. The default is no wrappers. Never leak the runtime's RUST_LOG, injected guest variables, or run IDs as accidental macro compilation inputs.

### 48.4 Compile-time-only capability probe

Before interpreter work depends on this integration, construct a fixture workspace containing a proc-macro crate, a build.rs that emits a constant and cfg, and a live library whose cold function calls an undefined name. Request lib-tests and one integration-test variant. Assert that prepare succeeds, the macro library exists, the output constant/cfg/env are captured, and no rustc invocation compiles/checks the ordinary live runtime unit. The source should fail a separate ordinary cargo check, proving the invalid body was not accidentally valid.

A logging compiler wrapper is allowed only in this controlled probe to record real arguments before forwarding to the actual compiler. It must never skip an invocation or forge an artifact. Associate invocations with the unit graph. Exempt compiler information probes; fail on live crate code-generation or analysis invocations. Tag builds of the same package as a declared BuildHost unit separately rather than misclassifying them as live. Record complete argv, source path, target crate kind, and compiler exit status, with private environment values redacted.

Run prepare again unchanged and require Cargo-reported freshness where applicable. A build-script-executed message may recur even without executing the script; use a controlled script counter only in the fixture and actual compiler/process records to distinguish output reporting from execution. Never claim no work solely because preparation was fast.[^M02]

### 48.5 Build-output publication

Capture each build-script-executed message's package association, cfgs, env, and out_dir, plus link metadata needed for native provenance. Cross-reference exact output directory and build unit against the graph. If multiple units cannot be disambiguated from the available record, run the planner for one selected unit context at a time and maintain distinct output manifests; never arbitrarily assign a package's last message to every unit.

Before build, fingerprint recorded sources/config and declared external inputs. After build, verify those inputs did not race; then read OUT_DIR tree contents into immutable GeneratedBlob records and verify a second stable read. Exclude sockets/devices and reject escaping symlinks. Persist virtual original paths, immutable byte digests, directory membership, and producer unit. Do not erase unrelated files from OUT_DIR on Cargo's behalf; Cargo does not promise to clean it between builds.[^M05]

CompileEnv contains per-unit Cargo package variables, OUT_DIR, rustc-env results, declared env inputs, target/host facts, and the cfg set. Map OUT_DIR include paths to captured blobs. Build scripts that produce C/native libraries may run, but those link products are only consumed by explicitly prepared native islands; emitting a link directive never makes guest extern calls legal.

Freshness follows Cargo's declared/default build-input assumptions plus the specification's recorded external inputs. A script without narrow rerun directives may invalidate on broad package changes. Report that behavior. A new output filename or a removed output invalidates membership and include-negative-lookup records. A generated .rs file is parsed and resolved as source when demanded; it is not trusted as a prechecked implementation.

### 48.6 Procedural-macro server selection

Use the rust-analyzer procedural-macro server supplied with the exact project toolchain, located through the recorded sysroot/component manifest. Verify its path, hash, protocol capability handshake, and compiler compatibility before loading any macro. Fail with LRR-E-MACRO-SERVER-MISMATCH when the matched server is absent or incompatible. Installation is a user-authorized prepare/bootstrap action; never silently download it inside a test.

The lrr-expand-adapter client must speak the matched protocol using the pinned upstream client/codec implementation. Support only the server/compiler tuples recorded by passing fixtures; the initial required tuple is the project's pinned 1.99.0 toolchain. Additional project compilers require a new capability fixture and manifest entry, not a guess based on a major version. Keep a compatibility table of server hash, protocol version, compiler commit, adapter build, and tested token/span features.

Load only the exact proc-macro artifact path/hash Cargo reported for the relevant host unit. A .dylib with the right filename but wrong hash/toolchain is rejected. The macro .dylib stays in the helper process; never dlopen it in the coordinator, guest worker, or native bridge. Rust's procedural-macro ABI is not this project's stable C ABI. The separate-process adapter mirrors the upstream architecture, not a promise that private protocol versions stay compatible.[^M06]

Erratum E-M013-001 records a required compatibility gap: the unchanged rustc `multiple-derives.rs` auxiliary prints to native stdout, corrupting the matched API6 bidirectional postcard response stream. The terminal decode-error/no-retry regression is failure-handling evidence, not successful procedural fixture execution. Full compatibility requires a reproducible diagnostic/framing repair within the exact server, pin, protocol and authority contracts; no guessed-text filter, transport substitution, fabricated expansion or exemption of printing macros is authorized. See [the witness and pending correction](docs/errata/m013-procedural-stdout.md).

E-M013-001's narrow correction permits the registered `stdio-v1` prepared-server variant on macOS ARM64/Rust1.99.0. Explicit preparation must verify the original compiler/sysroot/component tuple, audit the exact pinned macro source and versioned mechanical ABI/entry patches, and build only that external host tool with its separate locked dependency graph. Preserve the upstream protocol main loop and API6 codec; duplicate the response descriptor and route native macro stdout to diagnostics. No SDK installation, download or server compilation occurs during live MacroRun execution. The compatibility probe requires separate macro authority and must exercise real attribute token/span echo and function-like identity expansion.

Preparation schema5 retains the optional typed `prepared_macro_server` record: original component-owned server path/content, exact compiler and rustc-dev identity, adapter build, source commit and complete registered repair-source hashes, build receipt, prepared executable content, protocol and actual capability outcomes. Missing or unregistered provenance cannot authorize an outside-sysroot path. None retains the original component requirement. Include every additional file in pre/post preparation and live adoption/publication freshness checks. The toolchain stable-encoding domain advances for this record; prior preparation publication keys must be regenerated. This exception changes neither frontend/backend selection nor the separate build/macro authorities, and grants no Linux acceptance or full language-fixture pass.

### 48.7 Tool source pinning and adapter boundaries

Vendor a coherent upstream source snapshot from the commit recorded in the pinned ra_ap_syntax crate's .cargo_vcs_info.json; compile only the macro adapter's dependency closure. At bootstrap extract and record that commit, fetch that exact upstream tree through explicit dependency setup, preserve its workspace manifests/member paths, record archive hashes/licenses, and lock the transitive dependencies. Exclude the vendored workspace from the top-level member glob and depend on its needed crates by exact local paths. Do not run a vendored cargo --workspace build of the whole IDE. Its internal syntax types and the host ra_ap_syntax types meet only through owned adapter records. Do not choose upstream master based on examples in source links. If required package source or protocol handling differs, port the narrow protocol/hygiene adaptation into lrr-expand-adapter with fixtures; keep the source pin fixed until an explicit dependency upgrade.

Adapter modules: token_wire, edition, declarative, builtin, hygiene, proc_client, span_store, and protocol. They return owned versioned records. A private upstream interner/database is permitted only for token/symbol/span/hygiene needs. An architecture audit rejects dependencies or calls into infer_body, whole-body type inference, rustc_driver, MIR, or an analyzer semantic model. No upstream ID is serialized into LRR cache keys without stable remapping.

<a id="section-49"></a>

## 49. Hygienic token and macro expansion implementation

### 49.1 Wire model

```rust
struct TokenForest {
    schema: u32,
    roots: Vec<TokenTree>,
    contexts: Vec<ContextRecord>,
    spans: Vec<SpanRecord>,
}
enum TokenTree {
    Group { delimiter: Delimiter, open: SpanKey, close: SpanKey, children: Vec<TokenTree> },
    Ident { text: String, raw: bool, span: SpanKey },
    Punct { scalar: char, joint: bool, span: SpanKey },
    Literal { token_text: String, span: SpanKey },
}
struct SpanRecord {
    source: SourceIdentity,
    byte_range: ByteRange,
    context: SyntaxContextKey,
    edition: Edition,
    origin: OriginChainKey,
}
```

Delimiter includes None/invisible. Keep joint spacing for punctuation and lifetime token sequences. Preserve literal token spelling until the relevant parser interprets it. Token IDs are stable only within the versioned token-content/context table; pointer addresses and upstream arena indices never become identities. Empty groups and distinct open/close spans must round-trip. Rust-to-proc-macro token conversion has specific differences for punctuation, lifetimes, and implicit groups; the adapter must follow the compiler protocol rather than string-concatenating tokens.[^M07]

A context record includes parent context, expansion identity, transparency mode, defining crate/unit, and edition. Retain original source and generated-origin links independently of display line positions. Implement caller-provided span operations with the recorded snapshot: call_site, mixed_site, definition-related operations the matched stable server exposes, resolved_at/located_at, start/end/source_text where supported. Unknown protocol-required span operations fail the handshake/capability fixture, not return invented offsets.

### 49.2 Declarative macros

Use the pinned upstream matcher/transcriber through the adapter. Implement macro_rules definition parsing, ordered rule selection, fragment parsing under definition edition, repetition nesting/cardinality/separators, metavariable substitution, and leftover-token diagnostics. No custom regular-expression macro expander is permitted. Keep rule parse errors attached to macro-definition demand; invocation matching does not type-check expression fragments.

Resolve macro_rules textual scope in item/block order, including generated macro definitions, and path-based scope for macro_export/use/reexports. Preserve $crate's defining crate through dependency renames. mixed-site hygiene must keep local identifiers from being accidentally captured while allowing the language-defined item lookup behavior. An exported macro's definition edition governs edition-sensitive fragments; the invoking crate's 2024 edition does not rewrite an older definition's expr matching.[^M09]

The adapter's builtin expander supplies compiler-known derives and frontend builtins. LRR may retain runtime intrinsic recipes from Section 18 only after exact identity resolution and certified no-item behavior. A user macro named assert is not an assertion intrinsic. An imported alias of a genuine intrinsic can be recognized by identity.

### 49.3 Procedural macro requests

A request includes consuming unit, tool artifact identity, macro exported name/kind, invocation input tokens, optional attribute tokens, scope/parent origin, call/mixed/definition span contexts, explicit compile environment, working directory, and expansion epoch. Derive requests send the annotated item; attribute requests send attribute arguments and the item excluding the applied attribute as required; function-like requests send invocation contents. Use the matched protocol's native conventions through the adapter, not invented TokenStream serialization.

Process outputs as tokens plus diagnostic/panic status. Parsing output must use the invocation's grammar slot: item sequence, expression, pattern, type, trait/impl items, or statement context. Do not force every output into a synthetic module. Preserve attribute replacement versus derive append behavior and recursively schedule resulting cfg/attributes/macros. Capture diagnostics and emit their origin chain even if the helper panics. Never reinterpret a proc-macro panic as a guest #[should_panic] success.

A server crash invalidates its whole consuming-unit session and fails the current request. Do not restart and repeat the invocation within the same test after possible macro effects. A new explicit run gets a new session. Bound stdout/stderr and keep them separate from the protocol stream and guest output.

### 49.4 Derive helpers and transformation order

**Known protocol capability gap E-M010-001:** the supplied Rust 1.99.0 API6 export inventory carries macro name/kind but no helper-attribute list. Two controlled compiled variants with the same derive export and different generated helper declarations produce identical inventories while native helper acceptance differs; see [the witness and regression](docs/errata/m010-compiled-helper-metadata.md). Unavailable metadata must not become an empty list or a ready negative helper lookup. Report the missing required capability as LRR-E-MACRO-SERVER-MISMATCH when metadata is required. Exact helper support remains mandatory. A declaration from the pinned source facet may supply helper metadata only when bound to the exact captured macro-host source/configuration, preparation and loaded export; unexpanded/static spelling guesses are insufficient. Captured original-function facets have component evidence, while generated declarations and complete facet integration remain unaccepted; this note authorizes no alternative server, frontend, tuple or rustc metadata decoder.

Register helper attribute names from each loaded derive export and associate them with that derive invocation's permitted scope. Follow the reference's helper visibility and ordering rules rather than stripping unknown attributes early. cfg/cfg_attr determine which transformations exist; replacing attributes can introduce or remove subsequent attributes. Run a deterministic queue that preserves lexical order where expansion order is constrained. If macro name resolution depends on another expansion, defer that queue item instead of guessing an attribute is inert.

Generated impls and free items are inserted with the correct parent module/block identity. A derive may produce a function needed by another item; an attribute may replace one test with several tests or no tests. A complete discovery result must reflect the final output, not the input item's old name. Detect duplicate definitions through ordinary namespace diagnostics after integration.

### 49.5 Includes and externally generated source

**Known pinned concat gap E-M011-002:** actual Rust 1.99.0 macOS comparison of `concat!(0x10)` observes `0x10` from the pinned syntax-only calculation and `16` from native Rust. Numeric correctness remains mandatory. The pinned numeric normalization correction now has native radix/separator/sign/suffix/float/u128-boundary and rejection regressions; complete literal/error/nested expansion and full M011 acceptance remain pending. The mismatch witness is not a passing compatibility fixture. E-M011-001 exposes the unchanged syntax-only calculation without a SourceDatabase/MacroCallId; it authorizes no alternative evaluator or pin. See [the witness and pinned repair](docs/errata/m011-concat-numeric-spelling.md).

Evaluate include path construction with the compile-time builtin evaluator, not the guest VM. Allowed dependencies include literals, concat, env/option_env in valid forms, and captured string includes. Missing env is a compile-input diagnostic when the path is demanded. Resolve relative paths against the containing original/virtual source file; nested included files keep their own directory base. Preserve include's non-hygienic lexical integration rather than creating a fresh module scope.

The coordinator maps paths to approved source roots or captured build-output roots. New files discovered by a macro-produced include cause NeedInput(GeneratedSourceRequest). Validate authority and capture bytes before resuming. A captured source file with an include path outside source authority fails even in managed runtime read-allowed mode unless source authority separately covers it. Native macro ambient filesystem reads are a distinct non-hermetic behavior and are not claimed to be mediated by this path mapper.

### 49.6 Expansion identities and reproducibility

ExpansionKey contains the macro definition/tool identity, invocation structural identity, input token/context digest, consumer cfg/env digest, parent hygiene context, definition/call editions, grammar slot, and engine version. For procedural expansion additionally include the request expansion epoch. Generated DefKeys derive from parent expansion identity plus structural item path and header identity; never use an output byte offset as the sole identity. Preserve stable paths when an unrelated generated body changes; otherwise invalidate conservatively.

Do not serialize token text through cargo expand and reparse it. Its output is a useful debugging view but does not preserve all semantics.[^M10] Tests must compare token/hygiene behavior using actual execution/name resolution, not only prettier text.

<a id="section-50"></a>

## 50. Discovery fixed point, nested impls, and generated tests

### 50.1 Work records

```rust
enum DiscoveryJob {
    ParseSource(SourceIdentity),
    ApplyCfg(ItemKey),
    RegisterHeader(ItemKey),
    ResolveImport(ImportKey),
    ResolveMacro(MacroCallKey),
    Expand(MacroCallKey),
    Integrate(ExpansionRecordKey),
    ScanNestedDeclarations(BodyKey),
}
struct Seal {
    domain: SealDomain,
    unit_graph: Digest,
    epoch: ExpansionEpoch,
    members: Digest,
    dependencies: SortedVec<InputFingerprint>,
    unresolved_frontiers: u32,       // exactly zero for a published seal
}
```

Scopes hold a queue plus a set of unresolved import/macro frontiers. Use deterministic source/module order and stable IDs for ready jobs; queue ordering must not change chosen method candidates. Mark dependencies when discovering both present and absent exports. A seal records the set of sources/expansions scanned, not only items that happened to contribute a chosen name.

### 50.2 Fixed-point algorithm

1. Start from the selected unit roots and already captured modules. Apply cfg before registering active items. Parse and register ordinary declaration headers; reserve opaque macro frontiers with their source/context.
2. Resolve currently available imports and macro names. Parse macro_rules definitions when their scope/name is demanded. Queue resolved invocations necessary for the requested seal.
3. Expand ready invocations through pure declarative queries or authorized procedural requests. Validate output size/kind; integrate all active generated headers/imports/macros and nested declaration containers.
4. Revisit blocked imports/macros when relevant namespace membership changes. Register negative buckets only when their source seal is complete. Never label an unresolved frontier as a missing export.
5. If a complete pass adds nothing and unresolved frontiers remain, report the sorted cycle/unresolved macro diagnostic with the blocked demand chain. Do not return a partial seal. If no frontiers remain, hash sorted membership and publish the seal.

A macro name can become visible only after an earlier expansion. Rust compiler expansion interleaves name resolution and integration rather than performing one independent textual expansion pass; this design follows that dependency requirement.[^M11] Cyclic reexports with no originating definition settle to a complete empty result only after all producers are known. A cycle containing an unexpanded producer is not the same case.

### 50.3 Impl discovery is broader than function execution

For ImplCatalogSeal, scan the active declaration skeleton of all reachable live crate units plus the synthetic trait facade. Traverse nested block/function syntax for impl items. Index impl headers independently of executable body entry. Unknown macros in these locations might emit impls; expand them conservatively unless an audited exact-identity summary proves they cannot emit declarations. This includes an expression macro that returns a block with a nested impl. Do not scan cfg-disabled bodies.

Do not type-check conditions, match patterns, local expressions, or impl method bodies during this scan. Record source-body parse/structural cost separately from prepared-executable counters. A macro expansion failure can block this seal even when the containing function is never called. That is the price of correct candidate completeness; there is no sound rule saying a syntactically cold location cannot contribute an impl.

Impl catalog entries include trait identity and arguments, self type pattern, generic binders, where predicates, associated-item headers, visibility/context, unit/world, and source/expansion dependencies. Traitless inherent impls have their own index. Adding an impl invalidates successful dispatch as well as missing/ambiguous results; a formerly unique selection can become ambiguous under relaxed source.

A no-item summary must be transitive over arguments that the macro treats as Rust syntax. For example, assert! or vec! can contain an expression block with an impl or a nested user macro; scan those expression arguments for declaration producers even when the outer intrinsic itself emits no item. Conversely, stringify! treats its input as tokens rather than expanded Rust and must not trigger fictitious nested declarations. Each intrinsic summary specifies consumed grammar slots, which slots are recursively expanded, and whether the remaining tokens are inert. Test nested producer macros inside cold assertions and ordinary stringified tokens separately. The scanner also traverses closure/async/anonymous-const bodies for declarations, even when their runtime execution is excluded.

### 50.4 Scope and test discovery

A ScopeSeal needs all producers that can change the relevant scope's item/import namespace. A TestCatalogSeal needs active modules and macro expansions that can introduce, rename, transform, or remove test items in the selected target. TestCatalogSeal can share completed expansion records with ImplCatalogSeal; do not execute a procedural invocation twice in an epoch for two consumers.

Enumerate tests only after transformation. Apply filter/ignore selection to the final catalog. An unsupported generated async/closure body is still a catalog entry; when selected, its execution failure is visible. An expansion error necessary for catalog completeness makes discovery fail. Neither --exact nor a substring filter authorizes silently assuming unknown generated tests cannot match.

A selected ordinary test's signature is checked at entry, as before. #[test] on an invalid unselected signature is metadata; a generated attribute that changes which functions exist is discovery work. Keep these two failure points distinct.

### 50.5 Publication and effects

Before guest execution starts, publish all required global declaration/impl/test seals for that run. Local source-entry facts may still be queried later, but no later expansion may change a published catalog's membership. If a reached expression-only expansion unexpectedly emits an item, the no-item classification was wrong: fail LRR-E-EXPANSION-SEAL-VIOLATION, preserve already completed guest effects, and require a new run after the frontend bug is fixed. Do not reinterpret earlier selections or restart from test entry.

The implementation must use conservative classifications so this is an internal bug path, not an ordinary user restriction. Unknown user and procedural macros never receive a no-item summary. The declaration prepass may therefore expand them earlier than their expression would execute, without executing generated ordinary operations. Only audited compiler-owned runtime macros can bypass that conservative prepass.

### 50.6 Limits and failure behavior

Per request: at most 256 nested expansion frames, 100,000 expansion invocations, 2,000,000 generated tokens, 32 MiB of a single macro output, 256 MiB cumulative generated bytes, 100,000 import edges, and 1,000,000 integrated headers. Check limits before allocation and integration. The default external macro invocation deadline is 10 seconds and cumulative expansion deadline 60 seconds; these are runtime enforcement constants, not estimates for this task. A supervisor timeout terminates the helper session and current run without retry. Explicit CLI limits may reduce these values; increasing limits requires a recorded configuration, never automatic backoff.

Use separate diagnostics for parse, macro-not-found, expansion-cycle, helper-panic, helper-crash, authority-denied, output-size, timeout, and catalog-incomplete. The message includes the requested test/path/method and the generating source chain. No cached overflow/timeout result can be reused as a semantic no-solution.

<a id="section-51"></a>

## 51. Effectful expansion, epochs, immutable records, and trust

### 51.1 Fixed procedural-macro cache policy

The default and only automatic policy is: reuse compiled macro libraries across requests, but do not reuse their invocation results across expansion epochs without re-executing the invocation. Use one epoch per live run request. Within the epoch, execute a given invocation record once and share its output among discovery, queries, and tests in that request. Repeated source edits create new epochs. A stable macro output can still preserve downstream pure-query and JIT eligibility after its new output digest is compared.

This conservative rule handles arbitrary procedural macros that read environment/files, consult time, mutate global state, or run subprocesses. An input-token digest alone does not prove the output remains current. Upstream macro tooling likewise treats nondeterministic expansion as an incremental-database problem.[^M06] Never claim complete filesystem-read tracking from Cargo dep-info or proc-macro API metadata. A future hermetic execution service would be a separate design change, not an automatic optimization here.

Declarative macro expansion is pure given definition/input/context/config records and can be cached normally across epochs. Compiler-owned env/include builtins depend on explicit captured inputs and absence/membership fingerprints. They do not inherit arbitrary procedural side effects.

### 51.2 Session lifecycle and process-global state

Create one procedural-macro server session per consuming crate unit per request, with the matched toolchain and captured environment. Load that unit's declared macro libraries and process queued invocations serially in deterministic scheduler order. Separate units get separate sessions so compile environments and global state cannot leak between them. Terminate sessions at request completion/failure. Helper processes may be started lazily when the first actual procedural request is needed; binary installation/compilation may not be lazy.

Do not promise behavior for macros whose outputs depend on unspecified cross-invocation scheduling or ambient nondeterminism to match every rustc build. Preserve constrained attribute/derive order and document the deterministic LRR schedule. Such macros are native effectful tools; record their outputs/provenance for reproducing a run. A recorded expansion transcript is diagnostic evidence, not automatically trusted executable input in another request.

### 51.3 Suspension, not side effects inside a query

A query provider returns QueryPoll::NeedInput(ExpansionRequest) when a required procedural output is absent. Unwind the provider's temporary dependency collector without publishing a value. The engine/coordinator saves the demand token and, when necessary, the VM's already-evaluated operands/continuation. The coordinator executes the macro helper outside SourceDb, validates the reply, and publishes an immutable ExpansionRecord input. Re-run the pure query using that record. This may repeat pure computations but never guest or macro effects.

A pending query result has no red/green status. Do not cache missing expansion as a negative name lookup or NoSolution goal. A failure record for the current epoch may be shared within that request to prevent repeated native invocation; it is not a cross-epoch success or permanent semantic absence. Cancellation while a native invocation is pending closes the request and helper; do not continue guest execution using a late reply.

### 51.4 Snapshot and generation identity

A request's base source/config/build/native identities are immutable. Expansion records are append-only derived inputs indexed by the frozen epoch and macro request key. Published declaration seals reference exact records; later additional records cannot mutate an existing sealed universe. No expansion result is committed to a different source revision even if the invocation text looks identical. Span records carry a source snapshot identity so diagnostics never map generated output onto changed disk contents.

Before starting an invocation, verify macro library and source-generation hashes. After it returns, reject replies whose request ID/session/unit/epoch or expected input digest differs. Store all source dependencies needed by generated includes. Native-generation pointers remain worker-local; expansion records cannot contain guest/native addresses or loaded library handles.

### 51.5 Security labels and permissions

There are four scoped native authorities: live build-tool preparation, live procedural-macro execution, native-bridge preparation, and native runtime execution. The first pair uses --allow-build-tools/--allow-macro-exec; the second uses --allow-native-build/--allow-native. Managed read/write grants authorize none of these. A project can run managed guest code while having explicitly unsandboxed macro helpers; the result must display both facts. Example fields are guest_effect_policy=capability_mediation, macro_execution=native_authorized, and request_isolation=not_hardened. Do not label the whole request sandboxed merely because generated guest code uses checked handles.

Without macro-exec authority, a required proc invocation fails before executing its library. No cached result may bypass that requirement because cross-epoch result reuse is disabled. Without build authority, prepare may adopt an already verified matching prepared tool manifest but may not run Cargo build jobs; missing preparation fails. Native runtime authority remains bound to the exact bridge generation.

The helper has ambient host authority in this release. Its separate process provides crash isolation, not containment of malicious native code. It can spawn processes, read files, or make network requests even with Cargo offline. State these limitations in CLI help and result metadata. A request for hardened containment fails closed; do not silently authorize native helpers.

For recorded source/build inputs, compare relevant hashes before and after each native expansion session before publishing a seal. A mismatch fails the request rather than mixing source versions. An arbitrary undeclared native read remains outside this verification and is reported as non-hermetic. The helper's direct filesystem behavior must not be confused with the coordinator's captured include path mapping.

### 51.6 Cache invalidation examples

| Change | Required invalidation/revalidation | Reusable work after validation |
|---|---|---|
| Ordinary macro-free function body | Its body structure/recipes and dependents | Unrelated headers, completed declarative expansions |
| Body passed to an attribute macro | That invocation output and any changed generated headers/bodies | Unchanged output-derived facts after digest comparison |
| New proc epoch, no source edit | Re-execute demanded procedural invocations | Pure downstream facts if new outputs/context match |
| Add generated impl | Catalog membership, selected/missing/ambiguous goals and dispatch | Disjoint catalogs/ordinary executable recipes |
| Change macro_rules definition edition | Matching/hygiene expansions and consumers | Other macro definitions |
| Change OUT_DIR file bytes/membership | Build-output record, include expansions, derived headers | Unaffected recorded units |
| Change native binding implementation only | Native generation rebinding | Compatible source-level binding signatures |

An unchanged expansion token string with changed hygiene/span/edition context is not an unchanged semantic result. Hash the owned context graph and source-position-dependent behavior. Cache accounting reports expansion execution separately from query recomputation and native compilation.

<a id="section-52"></a>

## 52. Source-aware type terms, call-local elaboration, and member identity

### 52.1 Four distinct representations

Do not use one TypeId for every purpose. Implement these distinct types:

```rust
struct SourceViewId(u32);       // Symbolic meaning at this source location.
struct RuntimeTypeId(u32);      // Concrete managed representation.
struct ParamEnvId(u32);         // Definition assumptions and binders.
struct RuntimeSubstId(u32);     // Actual type/const substitutions for a frame.

enum TyTerm {
    Primitive(PrimitiveType),
    Nominal(DefKey, Vec<GenericArg>),
    Tuple(Vec<TyTerm>),
    Array(Box<TyTerm>, ConstTerm),
    Slice(Box<TyTerm>),
    Ref(RegionTerm, Mutability, Box<TyTerm>),
    Param(ParamKey),
    Bound(BinderDepth, BoundIndex),
    Infer(InferVarKey),
    Projection(ProjectionKey),
    Function(Binder<FunctionSignature>),
    Dyn(DynSchemaKey),
    Opaque(OpaqueKey, Vec<GenericArg>),
    Never,
}
struct TraitRef { trait_key: DefKey, self_ty: TyTerm, args: Vec<GenericArg> }
struct ProjectionKey { trait_ref: TraitRef, item: AssocItemKey, item_args: Vec<GenericArg> }
```

GenericArg distinguishes type, const, and region. ParamKey includes defining item and ordinal; T in unrelated functions is not the same variable. Bound uses de Bruijn depth/index with checked shifting during substitution. Infer variables are local to one elaboration/goal transaction and never serialized as unresolved VM data. Unit/world identity is part of DefKey. Opaque identity includes its defining return position and captured parameters.

A SourceView record holds TyTerm, originating definition context, applicable local constraints, and source provenance. It does not hold a guest value. Runtime descriptors still control scalar widths, aggregate schemas, moves, and memory checks. SourceView may remain symbolic while the descriptor is concrete. Both are required at member/operator sites.

Existing common-engine uses of the name GenericEnv mean the following owning frame context, not a plain type-to-descriptor map:

```rust
struct GenericEnv {
    definition: ParamEnvId,
    symbolic_substitution: SymbolicSubstId,
    runtime_substitution: RuntimeSubstId,
    dictionaries: DictionaryEnvId,
}
```

The call packet's separate fields are assembled into this frame context at entry. Preserve a separate SourceViewId on each value/place slot; do not encode it by overloading ValueRepr.type_id. Record SlotId-to-SourceView tables in portable recipes and bind runtime-local IDs when installing a frame.

### 52.2 Propagation rules

| Expression/source | Resulting source view | Concrete runtime information |
|---|---|---|
| Parameter declared T | Param(T) | Actual argument's descriptor |
| let y = x | Preserve x's source view | Move/copy actual value under policy |
| let y: Written = x | Written after local substitution/coercion | Check reached concrete contract |
| Field on S<T> | Declared field type with symbolic T substitution | Force only accessed field layout |
| Borrow/deref | Ref wrapper or projected Deref::Target | Checked lease/result-place descriptor |
| Named function call | Declared return term under symbolic call arguments | Actual returned descriptor |
| Trait member result | Trait-associated signature under selected symbolic TraitRef | Normalize needed concrete projection |
| If/match result | Parent hint if available; otherwise reached arm's view with explicit provenance; numeric variables obey Section11 source constraints | No cold-arm execution or guest thunk preparation; numeric width constraints may be collected under Section11 |
| Cast | Explicit written target | Checked executed conversion |
| Opaque return | OpaqueKey and declared bounds | Hidden actual descriptor retained privately |
| dyn view | Declared object schema and associated bindings | Underlying checked object and dictionary |
| Function item | Concrete item identity plus declared signature | Function-value handle |

If a result must supply a nonnumeric source distinction that cannot be recovered from reached context, return LRR-E-SOURCE-TYPE-CONTEXT with the smallest annotation site. Do not recover that distinction by querying a whole-body inference engine or using future statements. Numeric inference variables are the specific exception authorized by Section11: collect their source constraints before fallback, including later uses, without executing those uses or preparing their guest thunks. This exception does not default arbitrary unknown type variables, strengthen rigid definition parameters, or select members from actual runtime descriptors. A reached ambiguous source view is not permission to use the actual descriptor to select a different Rust member. Section11 numeric-width rules take precedence over the former strict no-future-use numeric policy; see E-NUM-001.

### 52.3 Source-level member algorithm

Resolve `receiver.member::<Args>(...)` in two stages. Stage A uses source view, lexical trait imports, the definition ParamEnv, and explicit/local call hints. It selects an inherent item identity or a particular trait member identity and an adjustment recipe. Stage B uses concrete substitutions and any needed argument facts to fulfill the chosen trait dictionary. If Stage A is legitimately unresolved pending arguments, retain a candidate set whose namespace and source context are already fixed.

Build candidate receiver types by autoderef, insert borrowed candidates in the reference-specified order, and append the supported unsized candidate. For a Param(T), inspect bound-provided trait members before unrelated in-scope traits, preserving supertrait identities. An inherent impl cannot be attached to an arbitrary Param(T) based solely on the concrete runtime type. For symbolic Wrapper<T>, inherent impls of Wrapper<U> may structurally match U=T, subject to necessary where predicates; do not flatten Wrapper<T> into Wrapper<i32> early.

Resolve trait names in bounds when needed, not merely whenever a signature is read. Elaborate supertrait references with their generic arguments. Two paths to the same associated-item identity and equivalent substitutions are one candidate; two distinct traits with the same member name remain distinct. Method-generic arguments and associated bindings belong in the request. Return hints can constrain generic arguments where allowed, but must not invent a priority among unrelated equally ranked members.

After choosing an identity, check visibility, supported receiver form, and mutability/lease feasibility. If an &mut self candidate is selected on a non-writable place, fail; do not retry a lower-ranked &self method. Actual user Deref calls are executed only for the chosen adjustment recipe, as guest frames in left-to-right adjustment order. Cache a recipe, not the resulting reference or deref side effect.

### 52.4 Definition context inside implementations

Inside a trait default body, Self remains a rigid trait parameter with the trait's symbolic bounds. A reached `self.other()` selects the trait member before actual Self is substituted. Inside `impl Trait for Concrete`, Self is the impl's declared concrete self type, but an explicitly qualified `<Self as Trait>::other` keeps exact trait identity. Inside `impl<T> Trait for Wrapper<T>`, Self is Wrapper<T>, not an unconstrained Param and not a concretized runtime Wrapper.

Closely reproduce the reference's context-specific lookup rules. Do not apply one rule “trait methods always win” to fix the generic bug: the direct call on a concrete receiver must still select a higher-priority inherent method where Rust does. Store the member choice with the source definition and ParamEnv, then instantiate it rather than performing a runtime method-name search.

### 52.5 Minimal unification and hints

Unification operates on local terms with an occurs check, bound-variable shifting, and nominal identity. Treat definition Params as rigid in Stage A. Treat only explicit Infer variables as bindable. Follow declared aliases and necessary associated equalities through NormalizeProjection. A const parameter may bind to a known finite const value; unresolved symbolic const expressions remain terms until layout/selection needs a value.

When binding a generic signature against an actual argument, preserve both a symbolic substitution from source argument views and a concrete substitution from descriptors. A call f::<T>(x) inside a generic caller carries the caller's symbolic T into the callee signature while its RuntimeSubst has the actual type. Do not use the latter to strengthen what methods were available in the caller's definition context.

An unused generic parameter can stay unbound. A used return-only parameter may bind from the reached return as in the existing profile, but its source-facing view remains the declared symbolic result. If two reached contracts disagree, fail LRR-E-GENERIC-BINDING without inspecting another path. Never default unknown type variables to unit or copy a compiler fallback rule without a specific fixture.

### 52.6 Higher-ranked binders and region facts

Retain `for<'a>` as a binder. To test a universally quantified goal, instantiate its bound regions/types as fresh placeholders in a higher universe; instantiate candidate bound variables in a local universe and check that no outer variable captures a newer placeholder. Reject leaks. For an existential local variable, only substitute terms whose universe is visible to it. Alpha-renamed binders canonicalize identically; a free 'static and a universally quantified 'a never canonicalize identically.

The allowed region facts are syntactic equality, 'static outliving a named region, explicit ParamEnv outlives assumptions and their transitive consequences, and type-outlives decomposition over declared type structure when available. Implicit well-formed reference parameter facts may be recorded from the signature without analyzing a body. Do not infer liveness of references from source control flow; guest memory uses checked leases. If a lifetime predicate would affect reached selection and cannot be established from these facts, return IndeterminateRegion rather than asserting success or eliminating a candidate. This is a deliberate boundary between header reasoning and borrow checking.

HRTB matching and leak checking are mandatory even when runtime references erase region data. Cases that need actual storage lifetime proofs retain a precise unsupported-proof diagnostic. The reported execution is not full lifetime validation. Associated-item binders and GAT use-site lifetimes use the same substitution engine, not a second ad hoc string-replacement implementation.[^M14]

### 52.7 Ordering-sensitive source distinctions

Compound assignment is a regression requirement: a source expression with operands known primitive before runtime substitution evaluates RHS before LHS place; an overloaded/generic compound assignment evaluates its LHS receiver first. A generic instantiation with i32 must retain the generic order. Record this as a distinct AssignKind in the recipe, selected before operands execute.[^M12]

Use the same discipline for implicit borrows in equality/ordering, indexing to a place, deref coercions, and method receiver adjustment. An optimization may change helper implementation, not source-defined operand order or member identity. A JIT guard checks concrete tags after these decisions and may resume the same operation's helper; it does not redo member lookup with a different namespace.

<a id="section-53"></a>

## 53. Tabled demand-driven trait and projection engine

### 53.1 Goals, results, and witnesses

```rust
enum Goal {
    Implemented(TraitRef),
    Normalize(ProjectionKey, TyTerm),
    ConstEqual(ConstTerm, ConstTerm),
    TypeEqual(TyTerm, TyTerm),
    Outlives(RegionTerm, RegionTerm),
    TypeOutlives(TyTerm, RegionTerm),
    DynCompatible(DynSchemaKey),
}
enum GoalResult {
    Unique { answer: CanonicalAnswer, witness: WitnessKey },
    NoSolution,
    Ambiguous { residual: Vec<CanonicalGoal>, candidates: Vec<CandidateKey> },
    IndeterminateRegion { residual: Vec<CanonicalGoal> },
    Overflow { limit: GoalLimit },
}
enum Witness {
    Impl { impl_key: DefKey, subst: Vec<GenericArg>, children: Vec<WitnessKey> },
    Param { env: ParamEnvId, predicate: PredicateKey, fulfillment: DictionarySlot },
    Intrinsic { rule: IntrinsicRuleKey, children: Vec<WitnessKey> },
    Dyn { schema: DynSchemaKey, slot: TraitRefKey },
}
```

NeedInput is outside GoalResult because it is an unfinished query, not a semantic answer. NoSolution means all relevant candidates were examined against a sealed universe and failed. Ambiguous means multiple answers or a relevant unresolved predicate remains. Overflow and IndeterminateRegion are not NoSolution. Store no entry pointer or guest object in a witness.

CanonicalGoalKey contains normalized goal terms, canonical ParamEnv facts, binder universe mapping, live/world/unit identity, catalog membership dependencies, target/edition/semantics version, and solver-rule version. Canonicalization assigns inference-variable ordinals by first occurrence; it does not replace rigid parameters by their runtime values. Preserve trait arguments and associated-item arguments. T: Convert<A> and T: Convert<B> are different goals even if a method name matches.

### 53.2 Candidate enumeration

Require ImplCatalogSeal before enumerating potentially applicable impls. Index by exact trait identity and outer self-type constructor; include blanket generic candidates whose self type is a parameter. For inherent members use the separate inherent index and Stage A rules. The index is an optimization only: a debug oracle enumerates all headers and proves the indexed set is complete in property tests.

For each candidate, allocate fresh local inference variables for its generic binders, unify its self type and trait arguments with the goal, and normalize only projections needed by that match. A structural mismatch is a definite rejection. On match, create child goals for where predicates, implicit required header bounds, and associated equalities that establish applicability. Do not inspect method bodies or all methods' independent bounds. Check selected method-specific bounds only for that reached member.

Candidate evaluation is transactional: each has its own inference snapshot, constraints, dependency collector, and proof edges. Failed candidate bindings cannot leak into the next candidate. Record every consulted header/predicate/seal, including failed candidates, so adding/removing a premise invalidates prior success. An unrelated candidate that structurally cannot match may contribute only the bucket membership dependency.

### 53.3 Parameter assumptions versus concrete fulfillment

At symbolic resolution, a ParamEnv predicate can justify that a method is available on abstract T. Its witness is Param, containing a lazy dictionary slot, not proof that every future runtime type satisfies the predicate. When a concrete call needs that dictionary, instantiate the predicate and search concrete impl/intrinsic/dyn witnesses.

Do not satisfy `Widget: Describe` using the very generic `T: Describe` assumption currently being fulfilled after T=Widget. That would make every bound tautologically true. Carry an exclusion set of in-progress parameter-dictionary slots; only independently provided caller witnesses may discharge them. An unused dictionary remains unforced, so an otherwise invalid `ignore::<Widget>()` with an unused bound can still run under the relaxed profile.

A where-clause determining an impl's applicability is forced at reached selection even when the selected method does not use it. For `impl<T: Encode> Describe for Wrapper<T>`, choosing Describe for Wrapper<Widget> requires Widget: Encode. An ordinary function's unused `T: Encode` bound is not automatically forced. This distinction prevents “no global checking” from becoming “choose an impl whose condition is false.”

### 53.4 Evaluation scheduler and recursion

Implement goal tables as an explicit worklist, not recursive host calls with unbounded depth. Table states are New, Evaluating, Suspended, Complete. Each table stores candidate strands, child-table dependencies, canonical answers, and caller continuations. When a new answer arrives, wake dependent strands. Evaluate a strongly connected component until no new answers/constraints are produced, then finalize it according to its cycle kind.

Ordinary trait/projection cycles are inductive. A cycle with no base proof does not prove itself. If no candidate outside the cycle yields a consistent answer, return NoSolution or Ambiguous according to residual inference, never Unique merely because a goal is already on the stack. A projection that recurs with ever-growing type structure hits the explicit size/depth budget. Do not cache its overflow as absence.

Structural auto-trait rules use coinduction only in an SCC consisting entirely of the approved auto-trait structural goals, with no conflicting negative fact or escaping projection/ordinary-trait edge. Mark provisional cyclic assumptions; commit them only if every noncyclic premise succeeds. An SCC involving a normal trait uses the inductive rule even if an auto-trait node appears in it. Dedicated positive/negative recursive tests are required before enabling this shortcut.

Limits: 256 active binder/goal depth, 50,000 tables per demand, 200,000 candidate strands, 1,000,000 goal work steps, and 1 MiB canonical term bytes per demand. Poll cancellation every 256 steps. Results include counters and limit name. Test reduced budgets to force all overflow paths.

### 53.5 Answer combination and ambiguity

Combine equivalent canonical answers and identical implementation witnesses reached through different assumption paths. Do not collapse two different impl identities merely because they return the same type. Exactly one applicable witness with fully discharged needed constraints is Unique. More than one distinct applicable witness is Ambiguous. An unresolved candidate that could conflict keeps the whole selection ambiguous until resolved. Never prefer a more specific-looking impl without the explicitly excluded specialization rules.

The runtime does not validate global coherence or orphan rules. Handwritten invalid overlapping impls may remain untouched; if they affect an actual selection, fail with sorted origins. This is defined relaxed-source behavior, not emulation of rustc's global diagnostics. Native islands continue to receive normal compiler coherence validation.

Before invoking a selected trait member, compare the selected impl method's instantiated argument/return shape with the selected trait declaration contract at this call. A malformed unrelated method in the same impl is unexamined. A missing selected method uses a valid trait default or fails LRR-E-TRAIT-MEMBER-MISSING. Do not silently synthesize a default body.

### 53.6 Associated types and GAT normalization

A projection is identified by trait, concrete/symbolic Self and trait arguments, associated item identity, and that item's generic arguments. First resolve equality facts in ParamEnv where available. Otherwise select a witness for the owning trait. Read only the selected associated definition, substitute impl parameters and item parameters with correct binder shifting, and recursively normalize only as far as the consumer requires.

An unused field schema may retain a projection thunk. A field access, returned call descriptor, operator output, iterator Item, or dyn associated binding can force it. Resolve GAT where predicates needed for this use; preserve lifetime placeholders rather than mapping all GAT instantiations to one erased key. Stable associated-type defaults in traits are not assumed: source constructs unsupported by the pinned stable compiler fail their reached declaration requirement, not masquerade as a standard default mechanism.[^M13]

Projection equality can add inference bindings within a goal transaction, with occurs/leak checks. Report a cycle with its associated-item path. Do not treat a projection as a fresh arbitrary nominal type to bypass a cycle or missing impl. Associated constants similarly select the owning witness, substitute arguments, and run the pure constant evaluator for that item alone; validate its concrete result contract. A cold associated constant with an error remains uncomputed.

### 53.7 Intrinsic and structural rules

Create compat/traits.toml with exact synthetic core/std trait identities, parameters, associated members, and intrinsic rule IDs. Primitive arithmetic/comparison and approved container/function/reference behaviors use these identities. A user trait called Copy or Iterator is not a language trait. Compiler-derived impl headers are ordinary catalog entries; intrinsic bodies do not override their bounds.

Sized is a layout fact over supported descriptors; ?Sized removes an implicit bound rather than asserting NotSized. Unsize/CoerceUnsized/DispatchFromDyn use the supported built-in reference/Box/array/slice/dyn rules and are not exposed as user-defined stable impl mechanisms. Fn/FnMut/FnOnce intrinsic witnesses exist for named function items and supported function pointers, with associated Output and call ownership rules; this does not add closure capture support.

For the supported auto traits Send, Sync, and Unpin, use structural rules over actual fields/type arguments and explicit supported header facts, including approved reference/container behavior. These facts do not grant a guest permission to spawn threads or cross a native ABI. Negative impl syntax/nightly features outside the stable profile remain rejected when needed; internal negative facade facts are explicit synthetic rules. Avoid introducing an unverified blanket “all managed values are Send” rule.

### 53.8 Caching and dictionary layout

A DictionaryEnv maps exact canonical TraitRef plus ParamEnv origin to Unforced, Resolving, Ready(WitnessKey), or Failed(CurrentEpochDiagnostic). A witness lookup is pure; method execution is not. Ready dictionaries contain member identities and lazy associated-item/projection entries. They never contain references to an earlier run's guest values.

On first call to a dictionary member, create a CallRecipe referring to the witness/member/signature and relevant source dependencies. Method bodies are prepared by the existing VM on entry. Multiple calls reuse the witness and recipe; counters must distinguish dictionary hits from method invocations. After an impl header/expansion changes, invalidate both selected and failed dictionary memo entries through their catalog dependencies. A changed method body leaves witness selection reusable but invalidates its executable recipes.

Published JIT artifacts refer to immutable call recipe IDs whose generation includes those dependencies. No old artifact may invoke a newly ambiguous or differently selected member merely because its receiver tag still matches. The dispatcher owns the transition and exact-once call event; a new selection never occurs behind a raw cached function pointer.

<a id="section-54"></a>

## 54. Trait-aware runtime operations and managed object views

### 54.1 Required trait inventory

Implement exact headers for Copy, Clone, Drop, Default; Debug, Display, fmt::Write; PartialEq, Eq, PartialOrd, Ord; Hash, Hasher; Add/Sub/Mul/Div/Rem, BitAnd/BitOr/BitXor, Shl/Shr, Neg/Not and their assignment counterparts; Deref/DerefMut, Index/IndexMut; From/Into, TryFrom/TryInto, AsRef/AsMut, Borrow/BorrowMut; Iterator, IntoIterator, FromIterator, Extend, ExactSizeIterator, DoubleEndedIterator, FusedIterator; Sized, Unpin, Send, Sync; and the function-call traits for named functions/pointers. Match the pinned toolchain's stable header signatures and defaults in checked-in facade declarations. Include known conversions/blanket impl headers only once; do not duplicate inherent-like conveniences that bypass selection.

User-defined traits with the supported header syntax are not limited to this inventory. The inventory names built-in language/library integration points and required facade APIs, not all traits the solver may accept. A user trait's ordinary methods execute through the same generic call engine.

Create an audit mapping each language desugaring and facade method to its exact trait item identity, selected-signature contract, ownership action, continuation state, and fixture. No operation is implemented by matching the text of a method name against a runtime type.

At M028, expand a fixed matrix of unit/tuple/struct/enum/generic/reference/const-array inputs for every mandatory built-in derive. Record every generated compiler-owned path and associated-item header in compat/derive-output.toml. Implement required facade helper identities (including pinned formatter helpers or discriminant helpers actually emitted) with native differential tests before declaring derives complete. Do not reject an emitted compiler helper as an unlisted std path while claiming that built-in derive is supported. Do not automatically import arbitrary private std APIs: this allowlist is generated from the pinned required derive matrix, reviewed, and versioned. Same-spelled user paths do not receive intrinsic authority.

### 54.2 Copy, Clone, Drop, and Default

Read a value by consulting the required Copy policy on its concrete descriptor under the appropriate instantiated header facts. A selected Copy witness permits managed bitwise/value copying only if the runtime's representation supports it; invalid claimed Copy on owning noncopyable fields fails a memory/policy guard even in relaxed source. Copy selection alone never makes opaque native handles copyable. Clone always runs the selected Clone member, including manual side effects; it is not replaced by copying just because Copy exists.

Drop selection happens when a value actually requires cleanup. Read the relevant Drop impl and field schema on demand, invoke its body as a guest frame, then process fields once in prescribed order. Conditional Drop impls are not rejected solely for having predicates: select under the same required-goal rules, with an ambiguity diagnostic if relaxed source makes selection nonunique. Do not certify that the compiler's separate declaration-validity restrictions were checked. A default constructor invokes Default::default through a dictionary; no zero-initialization shortcut for arbitrary user types.

Built-in derive output establishes the same bound-dependent policy as Rust for the pinned compiler. The release oracle includes a generic derive whose bound is stricter than what a naive field-only runtime inspection would infer. Keep a normally rejected-source fixture separate from valid native-oracle fixtures.

### 54.3 Operators and reference-producing methods

Primitive fast paths remain available when source member identity and concrete tags agree with the intrinsic witness. All other arithmetic/bitwise operations call the selected language trait member with its proper ownership. Equality/order borrow the operands as required, evaluate each once, and preserve short-circuit behavior at the higher expression level. && and || are intrinsic Boolean control flow, never trait overloads.

Index to a place selects Index or IndexMut according to the source place-use context, runs the selected method once, and validates the returned managed reference/element descriptor before creating a projection. Deref/DerefMut similarly return checked leases rather than exposing host pointers. A user method can have visible effects; search/goal normalization never runs that body. Place formation and subsequent read/move are separate phases. Do not execute index/deref again during assignment or JIT fallback.

Augment LRIR with SourceAssignKind, MemberRequest, ApplyAdjustment, AwaitTraitCall, and CommitProjection continuations. Generic helper lowering ends a native region at any guest call or unresolved resolver request. Direct primitive JIT paths require the intrinsic witness identity in their dependency/guard set, not only a matching i64 tag.

### 54.4 Iterators and conversion protocols

Desugar for through IntoIterator::into_iter, normalize its IntoIter and Item projections, retain the iterator as an owning hidden local, and repeatedly invoke Iterator::next as guest calls. Match the actual Option<Item> result; a mismatched result is a call contract fault. A break/return/panic cleans the iterator exactly once. An invalid cold next implementation is not prepared before iteration begins; the first next call enters it normally.

Implement Iterator::next as its required dispatch member and the defaults size_hint/count/nth/last/fold/for_each/collect/enumerate/map/filter through facade state machines and named function/function-pointer callable witnesses. Closure creation remains unsupported as an executable operation, but a named predicate passed to filter works. Implement DoubleEndedIterator::next_back, ExactSizeIterator::len, FromIterator<T> for Vec<T>, and Extend<T> for Vec<T> with explicit loops and generic calls. No adapter callback runs on a host recursion path or within a host destructor.

`?` remains built-in syntax for the supported Option and Result identities. Result error propagation requests From<SourceError> for the enclosing Result error type and invokes its member once. Do not implement unstable user Try/FromResidual traits merely to support this syntax. Into/TryInto blanket behavior is an explicit facade impl header delegated through From/TryFrom, not a direct cast. Normalize the selected Error associated type for a reached TryFrom boundary.

### 54.5 Formatter and hashing drivers

A Formatter is a managed, call-scoped facade object with a bounded output sink, formatting flags, recursion budget, and reference to the active formatting continuation. User Display/Debug::fmt methods run as guest frames. Support Formatter::write_str, write_fmt, debug_struct/debug_tuple/debug_list/debug_set/debug_map and their field/entry/entries/finish/finish_non_exhaustive operations; fmt::Write::write_str/write_char/write_fmt; and the fixed format-string forms in Section 17. Add width/precision/alignment/radix/exponent support only when the explicit formatter parser/renderer inventory and native fixtures in this milestone are implemented; required release inventory is Section 17's declared subset, not every std formatting option.

Builder entries store checked borrowed argument views and dictionary requests until emitted; obey their source lifetimes and fuel/output limits. Calls to write!/writeln! on a guest String or custom fmt::Write route through trait frames. On fmt::Error, unwind formatting state without replaying already written portions. An assertion failure message uses this system only after the comparison fails. Captured argument evaluation order and Debug side effects must match native fixtures.

Hasher is likewise a guest trait object/dictionary with write and primitive write_* methods. Derive Hash walks fields through the generated body and selected Hash members; a recording hasher fixture verifies calls. Do not pretend the facade's hash algorithm matches a native random-state hasher; no arbitrary std collection ABI is imported. Native hash-based libraries remain usable behind explicit bindings.

### 54.6 Box and dynamic trait views

Add ValueKind15 ManagedBox and ValueKind16 DynView; preserve kinds0–14 and the 16-byte ValueRepr layout. Their payloads are checked arena handles. ManagedBox owns one stable heap cell and its type schema; moving the box does not relocate its pointee. Support Box::new, deref, mutable deref, drop, array-to-slice unsizing, and concrete-to-dyn unsizing. No raw conversion or allocator API is exposed.

A DynView stores a checked object/place/owner handle, the principal trait identity/arguments, required associated-type bindings, supported auto-trait facts, region source view, and a witness/dictionary key. It is not a native two-word fat pointer. Shared/mutable reference dyn views preserve the original lease permissions and generation. Owned Box<dyn Trait> owns the underlying box; dispatch does not create a second owner.

Before coercion, check the required dyn-compatibility header rules of the principal trait and supertraits: callable receiver/signature restrictions, relevant Self uses, forbidden generic dispatch methods, associated constants/GAT restrictions, and required associated bindings. Methods explicitly excluded by Self:Sized are not dispatch slots. This header work may inspect uncalled method signatures but never their bodies. Follow the pinned stable reference rules, including supported trait upcasts.[^M15] An invalid dyn schema fails at coercion, not at the first arbitrary slot load.

Create dictionary slot recipes lazily; their keys are exact trait member identities, not numeric positions copied from native vtables. A dyn call resolves the already selected member through the object's concrete witness and enters its guest body. An upcast to a supported supertrait preserves the object and selects the supertrait dictionary with correctly substituted arguments. Eq/Send-like marker facts do not fabricate call slots. Deallocation/drop uses the underlying concrete descriptor exactly once.

### 54.7 Opaque types and impl Trait

Argument-position impl Trait is represented as a fresh anonymous generic parameter with declared bounds and ordinary call-local binding. Return-position impl Trait receives an OpaqueKey for the definition and capture set; clients see only that opaque identity and its declared bounds. Rust 2024 capture rules and explicit use<...> are recorded in the opaque schema, rather than inferred from a single observed return.[^M16]

The runtime stores the actual hidden descriptor and a lazy dictionary environment when a return is observed. Within one source revision and generic instantiation, every observed return for that opaque definition must agree on the hidden type; incompatible reached returns fail LRR-E-OPAQUE-HIDDEN-TYPE. No cold path is inspected to establish the agreement. Declared trait predicates are fulfilled when their witness is required, not all at return. Associated projections through the opaque type require its authorized bound identity and hidden witness; an unrelated inherent method of the hidden type is not exposed.

Async functions and closure-producing impl Trait may parse/expand and remain cold; their executed operations are still outside this release's runtime profile. Report that limitation at the generated source origin. Do not substitute a native executor or a native future representation behind a managed opaque descriptor.

### 54.8 Pure constants and const generics

Extend ConstTerm to parameter, literal, named associated constant, and the stable supported pure expression forms. Substitute concrete const arguments at reached signatures/layouts, evaluate array lengths and required equality with the pure constant VM, and include values in nominal descriptor identity. Accept stable min-const-generic uses for integer/bool/char parameter forms supported by the pinned compiler. Do not treat type/const parameter lists as one untyped sequence.

Associated const selection uses the same trait witness as the selected member; default associated const expressions retain symbolic Self context. Effectful ordinary guest code is forbidden in constant evaluation. A procedural macro used to produce constant syntax is expanded by the external expansion phase, not invoked inside the pure constant evaluator. Unstable generic const-expression features are precise reached declaration diagnostics. Cycles and size overflow remain separate from trait NoSolution.

<a id="section-55"></a>

## 55. Live, native, macro-export, and compile-time worlds

### 55.1 Role separation

A package may participate in multiple roles, but each role has a distinct CargoUnitKey and permitted operations. Live units provide source declarations and guest bodies. Native units provide only explicit runtime bindings and native opaque identities. BuildHost and ProcMacroHost units provide executable preparation/expansion tools. MacroExport facets expose macro definitions/reexports and the declaration metadata needed to resolve those exports without importing native runtime bodies.

An edge from a proc macro to a helper crate is a host compilation dependency, not a live runtime dependency. A live test may use a compiled derive library and a separately compiled native runtime crate. Macro output can refer to the runtime crate, but those runtime names must resolve either to a designated live unit or an explicitly declared native binding surface. Do not merge them because package names agree.

### 55.2 Native macro reexports

A native crate can reexport macro_rules or a proc macro. Capture the crate's source-based macro-export facet using its exact configured unit, edition, imports, and generated outputs. This facet runs declaration/macro resolution only and may need the same expansion authority as other discovery. It does not analyze native function bodies for execution, decode rustc metadata, or enumerate native runtime methods.

Preserve defining-crate identity for $crate through exported macro expansion. A generated `$crate::helper(...)` resolves to that native namespace's declared binding, not a same-named live crate. If no binding exists, report LRR-E-NATIVE-BINDING-MISSING at the generated call with the macro ancestry. Do not expose a private native symbol automatically. The declared binding manifest is authoritative for runtime access; a macro's ability to spell a path is not authority to call it.

Telemetry distinguishes native_macro_export_headers_read from native_runtime_bodies_prepared (the latter must remain zero). Reading macro-export source for resolution is compatible with native artifact reuse; it must not be misreported as interpreting native implementation code.

### 55.3 Live derived types and compiled generic libraries

A live struct with a generated trait impl has a managed nominal identity and layout. It is not the native compiler's nominal type, even if source names and fields match. A native generic library cannot be instantiated with it just by reusing an rlib or selecting a managed dictionary. The bridge's concrete type manifest remains the only native type universe.

Required behavior for a cross-world request: resolve the binding/schema, detect that a type/trait witness belongs to the live world where the native schema requires a native type, return LRR-E-NATIVE-WORLD-MISMATCH before native entry, and name the owning units, concrete native type and declared transport expected. No hidden rustc invocation, serialization guess, generated callback trampoline, or stale compiled version of the live type is allowed.

The supported boundary is an explicitly prepared binding: matched direct Rust ABI with concrete native residents/leases under Section 61, or an explicitly selected codec conversion. Native libraries may internally use derives, trait objects, async, closures and full trait solving; only admitted concrete signatures cross the boundary. A live trait algorithm operates on live types and source-provided traits/facade traits. ABI compatibility never merges the two nominal worlds.

### 55.4 Graph cycles and duplicated source

The native runtime dependency closure must not depend on a designated live runtime unit. Host tool dependencies are analyzed separately: the same source package can legitimately be compiled as a BuildHost helper and interpreted as a live runtime unit, but its host compilation is then explicit preparation and edits to that source can make preparation stale. Report the path and role rather than rejecting all same-package appearances or hiding the compilation.

For an integration-test target, its ordinary package library may be native; for private lib-tests of that library, keep the library live as already specified. Macro-generated tests follow the selected target's same cfg/test identity. Native and live versions of one source package remain nominally distinct. A declared conversion may construct a value in the other world; direct ABI transport alone does not make a managed live type a native type. A feature-unified host graph cannot be reused as a live feature graph.

A hybrid preparation references a current live/tool preparation manifest and a current native generation. Prepare the live tool graph first with its own authority, then native prepare with --allow-native-build. If there are no live compile-time tools, the live preparation still records unit/config identities. Native build authorization does not authorize the live macro helper during later runs. Failure in either preparation leaves the prior complete generation intact and unusable for changed inputs, never partially updated.

### 55.5 Cache and execution effects

A macro-only source edit can require tool recompilation and re-expansion but need not rebuild native runtime dependencies. A stable native library edit invalidates native preparation and rebinding, but not unrelated declarative macro results. Changing a live trait impl invalidates live witness/dispatch/JIT dependencies, not native compiled impl selections. The native bridge remains valid only if its own recorded inputs/schema stay valid.

Hybrid workers are still fresh per run request. Fresh workers import portable expansion/declaration/goal records only after current-epoch procedural revalidation; no worker-local dictionary IDs, guest handles, vtable slots, or native pointers are portable. Validated relocatable code can cross worker lifetimes under Sections 22/62; installation rebinds only closed relocations and verifies current dependencies. The coordinator may retain validated canonical source records and artifact identities, never guest state or executable pointers.

<a id="section-56"></a>

## 56. Interfaces, query keys, diagnostics, and service contract

### 56.1 Pure-query extension

```rust
enum QueryPoll<T> {
    Ready(Result<T, Diagnostic>),
    NeedInput(InputRequest),
}
enum InputRequest {
    Expand(ExpansionRequest),
    CaptureGeneratedSource(GeneratedSourceRequest),
}
```

QueryPoll is used by all entry points that transitively need expansion or source capture. A caller cannot unwrap it into a missing-name error. SourceDb providers do not own subprocesses; coordinator services fulfill requests. Demand tokens include revision/epoch/query key/phase; when a provider suspends, discard its incomplete dependency collector, retaining only a trace for diagnostics. Re-entry recomputes pure facts and publishes a memo only after completion. VM phase state is separately retained by the dispatcher.

The trait solver's internal tables are not ordinary query recursion. A provider for a top-level goal owns its tabled worklist and publishes a final answer/dependency set; child goals within an SCC use the solver's tables. Do not let the generic query database's “active key cycle” error prevent legitimate tabled trait evaluation. Conversely, a source include/import/alias cycle is not automatically a coinductive trait proof.

### 56.2 Required queries and inputs

| Query | Key additions beyond source identity | Result |
|---|---|---|
| ExpandDeclarative | Definition/input tokens, hygiene, grammar slot, editions | Pure token forest and diagnostics |
| ExpansionInput | Invocation plus current epoch/tool/context | Existing immutable record or NeedInput |
| ExpandedOutline | Original/expansion source identity and cfg | Final active headers and producer frontiers |
| ScopeSeal | Unit/scope/namespace, requested revision/epoch | Complete membership certificate |
| ImplCatalogSeal | Live unit graph, relevant facade/world, epoch | Full candidate producer certificate |
| TestCatalogSeal | Selected target/test intent and epoch | Final test identities and sources |
| SourceView | Reached node/phase, symbolic environment and local facts | Symbolic term plus constraints |
| ResolveMember | Symbolic receiver/env/imports/hygiene/item arguments | Exact member/candidate identity and adjustments |
| SelectImpl | Canonical goal/env/world/catalog dependencies | Unique/no/ambiguous/indeterminate/overflow |
| NormalizeProjection | Exact associated identity and substitutions | Type/const term and witness dependencies |
| DynSchema | Principal/supertraits/bindings/source env | Validated managed object view schema |
| CallRecipe | Chosen identity/witness/signature/source context | Phase sequence, no pointers or effects |

Add immutable inputs ToolchainRecord, CargoUnitGraph, ToolUnit, BuildOutput, CompileEnv, MacroLibrary, ExpansionRecord, GeneratedSource, and ExpansionEpoch. Store absent env/files/exports explicitly. StableEncode tags are versioned and collision-separated. Decode persisted input with all limits and duplicate/key consistency checks; a content hash does not make malformed cache data trustworthy.

### 56.3 Portable caching and semantic fingerprints

Do not include an epoch number in every downstream semantic key unconditionally: that would discard all reuse even when a proc expansion is unchanged. Instead make eligibility depend on a current-epoch validation record, then compare the stable semantic token/context/output digest. Old downstream entries cannot be considered green until all procedural producer records they depend on have been revalidated. A pure unchanged output can then preserve its result fingerprint and dependent code eligibility.

The validation record's session/epoch is freshness evidence; generated semantic identity uses macro definition/structural invocation/context plus stable output structure. Keep a separate provenance list so new epochs do not overwrite previous diagnostics. If macro global state or output identity makes a stable mapping uncertain, assign a new generated identity and invalidate conservatively. Do not reuse stale spans or pretend unchanged text implies unchanged hygiene.

### 56.4 CLI additions and exact authority semantics

Add to lrr prepare: --allow-build-tools; --project-toolchain NAME; --target-kind lib-tests|lib-run|bin:NAME|test:NAME; --features LIST; --no-default-features. Add to lrr run/test/watch/list-tests: --allow-macro-exec. Retain --execution hybrid-native --allow-native, with the generation supplied by the validated prepared manifest, and retain runtime read/write/env grants. None implies another. --allow-build-tools is not accepted on test as a way to build on demand; report that preparation must be explicit.

Add lrr prepare --explain-tools, which prints the resolved tool/native/live unit roles and recorded inputs without invoking builds. If tool graph resolution itself requires unavailable metadata, fail rather than fetching. Add lrr inspect-preparation PREPARE_ID for toolchain/library/output/source hashes and freshness status. Secrets in env are redacted by default.

```sh
# Build/capture compile-time tools once, without checking ordinary live bodies.
lrr prepare --manifest-path Cargo.toml --package app \
  --target-kind lib-tests --allow-build-tools --project-toolchain 1.99.0

# Reuse prepared binaries; procedural invocation output is revalidated this request.
lrr test --prepared PREPARE_ID --allow-macro-exec --exact generated_case --jit=auto

# Native runtime use requires its existing, separate native preparation/authorization.
lrr test --prepared PREPARE_ID --allow-macro-exec \
  --execution hybrid-native --allow-native --exact generated_case --jit=auto
```

PREPARE_ID above is a real prepared-manifest digest or a bounded local path to a validated schema5 prepared manifest. A 64-hex argument selects the content-addressed preparation store; any other argument is an explicit coordinator-readable manifest path. The hybrid generation comes from that manifest, not from a positional argument to --allow-native. Commands may report an unsupported selected executable feature but may not call Cargo to repair it. --jit=off/auto/force all use identical expansion/resolution/authority behavior.

### 56.5 Protocol version and events

Use public service schema 3, LRR internal framing 4, portable query format lrr-portable-v2 and preparation schema 5. Native-generation schema is 2. Codec C ABI stays 1; direct C control ABI is rust-abi-v1; the JIT helper/map ABI is version 2. Reject older executable/preparation/service contracts explicitly. A matching codec payload layout alone does not authorize loading an old generation. Regenerate CLI/service examples and record jit_tier, execution_contract, native transport, producer ABI, optional IR bundle and code-cache outcomes.

```json
{
  "schema_version": 3,
  "request_id": 7,
  "method": "run",
  "params": {
    "selection": {
      "kind": "tests",
      "filter": "generated_case",
      "exact": true,
      "ignored": "exclude"
    },
    "jit": "auto",
    "jit_tier": "auto",
    "execution_contract": "precise",
    "replace_pending": false,
    "allow_macro_exec": true,
    "execution": "managed",
    "allow_native": false,
    "native_generation": null,
    "policy": {
      "read": [],
      "write": [],
      "env": {}
    },
    "snapshot": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "limits": {
      "fuel": 10000000,
      "timeout_ms": 10000,
      "memory_mib": 256,
      "output_limit_kib": 4096
    },
    "stream_output": false
  }
}
```

The complete schema also includes inherited limit/output/cancellation settings with explicit defaults supplied by the CLI. A request cannot change source/unit configuration during a run. Emit phase events prepare_tool, expand_macro, discover, resolve_member, solve_goal, interpret, jit_compile, native_call. Internal macro traffic uses a separate bounded channel from guest logs and user-facing stdio messages. Every event carries request/run/snapshot/epoch and origin where applicable.

### 56.6 Result fields

Add semantics_profile=lrr-2024-v2, runtime_version=0.2.0, trait_resolution=demand, borrow_validation=not_performed, macro_execution=none|native_authorized (result field, distinct from the request grant allow_macro_exec), expansion_epoch, macro_invocations, macro_result_revalidations, declarative_cache_hits, expanded_tokens, generated_tests, forced_trait_goals, dictionary_hits, normalized_projections, blocked_seals, toolchain_records, and preparation_freshness=recorded_inputs. Keep guest_effect_policy distinct from request_isolation. Hybrid still reports sandbox_strength=none_native. Managed plus native macros reports sandbox_strength=capability_mediation_guest_only, never a hardened whole-request sandbox.

Also report execution_contract=precise|throughput, jit_tier_requested, actual_tier_entries, native_transport_entries, producer_abi_digest, native_ir_digest, reconstruction_exits, specialization_hits/misses, background_compile_outcomes, and relocatable_code_cache_hits/misses. In throughput, fuel_used is null, not zero or an estimate.

No macro invocation means macro_execution=none even if the permission was granted; include the grant separately. Report actual counters, not inferred estimates from source syntax. An expansion failure gets phase=expansion or discovery, not execution_status=passed. A native helper crash never satisfies a guest expected panic.

### 56.7 Required new diagnostics

Register these exact families with phase, severity, source fields, and fixture references: LRR-E-TOOLCHAIN-CAPABILITY; LRR-E-UNIT-GRAPH; LRR-E-TOOL-PREPARE-STALE; LRR-E-MACRO-SERVER-MISMATCH; LRR-E-MACRO-NOT-FOUND; LRR-E-MACRO-PERMISSION; LRR-E-MACRO-PANIC; LRR-E-MACRO-CRASH; LRR-E-MACRO-TIMEOUT; LRR-E-MACRO-OUTPUT; LRR-E-EXPANSION-CYCLE; LRR-E-EXPANSION-SEAL-VIOLATION; LRR-E-CATALOG-INCOMPLETE; LRR-E-GENERATED-SOURCE; LRR-E-SOURCE-TYPE-CONTEXT; LRR-E-TRAIT-NO-SOLUTION; LRR-E-TRAIT-AMBIGUOUS; LRR-E-TRAIT-GOAL-LIMIT; LRR-E-TRAIT-REGION-INDETERMINATE; LRR-E-PROJECTION-CYCLE; LRR-E-TRAIT-MEMBER-MISSING; LRR-E-DYN-INCOMPATIBLE; LRR-E-OPAQUE-HIDDEN-TYPE; LRR-E-NATIVE-WORLD-MISMATCH. Existing generic type/contract/visibility/memory diagnostics retain their roles.

Every trait diagnostic names the source member identity, symbolic receiver, actual descriptor when known, needed goal chain, and relevant candidate origins. A procedural expansion diagnostic includes both macro definition and invocation origins and the blocked discovery demand. Limit diagnostic rendering to bounded chains/candidate samples while retaining counts. Do not print arbitrary private environment values.

<a id="section-57"></a>

## 57. Ordered implementation tasks and cross-ledger integration

### 57.1 Fixed order

The T and H cards in Sections 27 and 44 remain requirements, with their updated revision-5 expectations. Execute this order, not “finish the old macro-free release, then add compatibility”:

| Batch | Tasks in exact order | Gate after batch |
|---|---|---|
| A | T000–T003, H004, M001–M003 | bootstrap, compile-time capability |
| B | T010–T015, M004–M012 | source/query and token/expansion substrate |
| C | T020–T025, T030–T034, H001–H003, M013–M016 | first execution and complete discovery |
| D | T040–T045, M017–M027 | symbolic terms, goals, corrected calls |
| E | T050–T054, T060–T065, M028–M034 | traits/derives/facade execution |
| F | T070–T074, T080–T084, M035–M037 | JIT and incremental equivalence |
| G | T090–T094, H005–H028, M038–M042 | Cargo/native/tool worlds and service |
| G2 | T085–T089 | typed optimization, reconstruction, LLVM, background compilation and code reuse |
| H | T100–T103, M043–M045, T104, T110–T112, M046–M047, T120–T122, M048 | full revision-5 release evidence |

A T or H gate whose final expectation requires a later M task is run provisionally only for its already-implemented subset during the batch and may not be marked release-complete until that M dependency passes. T100–T103 first establish oracle/equivalence/property harnesses for the existing core cases; M043 extends them to the new fixture matrix before T104 performs exhaustive coverage. docs/tasks.json stores this distinction explicitly; no skipped test can count as release success. Early scalar operations can use certified intrinsic witnesses, but the public trait dispatcher is not declared complete before M026. Section 44's H order is retained within its batches. This table resolves ordering, not multiple architecture choices.

Revision-5 integration rule: existing completed cards retain their historical component receipts and release_complete=false. A revision-4 pass never satisfies a changed direct-ABI, optimizing, protocol or contract acceptance. H011/H013/H019/H028 and baseline JIT cards can finish their currently available components in order; their final native/optimized assertions require T089 and the Section 58 matrix. G2 is mandatory before the release/evidence batch. Record pending revision acceptance explicitly, without adding unexecuted IDs to passed fixture arrays.

Ordering erratum E-57-001 extends that same provisional rule to an M component whose named final fixture requires a later task in this table. For example, M009's complete MX015/16/17 behavior requires T034's guest harness and M013–M016's generated discovery/catalog/origins, which follow M009. Complete the current component and run its available acceptance in the fixed order; record completion_scope="component", release_complete=false, explicit integration_dependencies and pending_acceptance fixture IDs. Those pending fixtures are neither skipped passes nor fulfilled by a native-only comparison. Re-run their complete LRR assertions after the recorded dependencies and M043's full fixture oracle, and require actual final evidence before release_complete=true. This preserves every fixture and the fixed order; it changes no architecture or feature requirement. See [the minimal witness and regression](docs/errata/e57-component-integration-order.md).

Language-fixture workflow: use the official rustc checkout at `rustc/rust` as the primary source of language tests as each feature is implemented. Copy relevant cases incrementally into `fixtures/upstream/rustc-checkout`, preserving original sources, helper crates, directives/revisions and expected output files, with the upstream commit and file hashes recorded. That directory retains the original rustc MIT OR Apache-2.0 license texts and copyright notice; LRR adaptations and harness code live outside it. Keep LRR-specific authority, suspension, cache and seal regressions in addition to the upstream cases. Imported or partially exercised cases do not count as full fixture passes. Reuse follows the cross-ledger order and macOS-first validation; it does not require the entire compiler suite to run before current milestone progress.

### 57.2 Tool and expansion tasks

**M001 — Vendor adapter and establish pins.** Files: crates/lrr-expand-adapter, vendor/rust-analyzer-macros, tools/cargo-planner.toml, vendor/UPSTREAM.toml. Extract exact source commit from the pinned syntax package, vendor licensed source and lock dependencies, implement forbidden-inference audit. Acceptance: adapter builds; provenance hashes validate; intentional import of infer_body fails audit. Depends: T003.

**M002 — Planner/unit capability probe.** Files: project/cargo_units.rs, xtask/compile_time_probe.rs, fixtures/macros/tool-workspace. Implement Section 48 argv/schema capture and failing-live-body probe. Acceptance: MX001/MX002; no ordinary live unit checked; normal oracle check rejects the broken body; test intent/features match graph. Depends: M001.

**M003 — Matched macro-server handshake.** Files: adapter/proc_client.rs, project/toolchain.rs. Locate server in selected sysroot, validate hash/protocol/compiler, load exact artifact and invoke a token echo macro. Acceptance: MX003/MX004; mismatch refused before loading; binary output logged as evidence. Depends: M002.

**M004 — Tool/build manifest schemas.** Files: project/tool_manifest.rs, source/generated.rs, compat/preparation-v4.json. Encode unit roles, compile env, OUT_DIR trees, source/external-input hashes; transactional publication. Acceptance: MX005/MX006; two unit contexts never share outputs; racing generated write rejects publication. Depends: T015,M003.

**M005 — Token wire and stable contexts.** Files: adapter/token_wire.rs, syntax/spans.rs, expand/identity.rs. Implement lossless owned token/context model and stable remapping. Acceptance: MX007, randomized token round trips, no pointer-derived keys. Depends: M004.

**M006 — Declarative matcher/transcriber.** Files: adapter/declarative.rs, expand/declarative.rs. Wire pinned mbe and fragment editions; syntax-only matching. Acceptance: MX008–MX010 and macro_rules recursive/leftover/error fixtures. Depends: M005.

**M007 — Hygiene and macro namespaces.** Files: adapter/hygiene.rs, frontend/macro_names.rs, expand/scopes.rs. Implement textual/path scopes, $crate, raw identifiers, reexports and renames. Acceptance: MX011/MX012, definition/call-site collision tests. Depends: M006.

**M008 — Pure-query suspension.** Files: db/provider.rs, engine/dispatch.rs, service/expansion.rs. Implement QueryPoll/NeedInput without caching incomplete results; preserve VM phases. Acceptance: MX013, pure provider audit, cancellation/late-reply rejection. Depends: M007.

**M009 — Procedural requests and sessions.** Files: adapter/proc_client.rs, service/macro_session.rs. All three proc forms, per-unit epoch sessions, exact-once invocation, bounded logs/deadlines. Acceptance: MX014–MX017 and helper panic/crash denial. Depends: M008.

**M010 — Attributes, helpers, and cfg.** Files: expand/attributes.rs, expand/derive.rs, project/cfg.rs. Implement transformation order/helper visibility/cfg_attr; integrate replacements/appends. Acceptance: MX018–MX020. Depends: M009.

**M011 — Includes and compile environment.** Files: expand/builtins.rs, source/generated.rs, service/source_capture.rs. Implement include/env/option_env/concat path demands and immutable capture. Acceptance: MX021–MX024; no ambient worker file reads; env absence invalidates. Depends: M010.

**M012 — Expansion epochs and revalidation.** Files: expand/cache.rs, source/changes.rs, service/run.rs. Disable cross-epoch automatic proc-output reuse, retain pure output-derived reuse after validation. Acceptance: MX025/MX026; mutable external-input macro changes result without rebuilding binary. Depends: M011.

### 57.3 Discovery and resolution tasks

**M013 — Discovery worklist and seals.** Files: expand/discovery.rs, frontend/imports.rs, db/seals.rs. Implement Section 50 fixed point and all incomplete states. Acceptance: MX027/MX028 and generated import/macro cycles. Depends: T034,M012.

**M014 — Nested impl producer scan.** Files: syntax/declaration_scan.rs, frontend/impl_index.rs. Traverse nested function/closure/const/block syntax for declaration producers; do not executable-lower it. Acceptance: MX029/MX030; impl from a cold expression macro affects a reached selection; ordinary cold type error still ignored. Depends: M013.

**M015 — Generated test catalog.** Files: frontend/tests.rs, cli/list.rs. Enumerate transformed tests only after seal, filter final names, retain unsupported generated bodies as visible entries. Acceptance: MX031/MX032; failed expansion not no-match/pass. Depends: M014.

**M016 — Origin chains and discovery diagnostics.** Files: syntax/origin.rs, cli/render.rs, compat/diagnostics.toml. Render generated failure at invocation/definition/include chain; update spans after edits. Acceptance: MX033/MX034; redacted bounded diagnostic bundle. Depends: M015.

**M017 — Symbolic type/binder terms.** Files: traits/term.rs, frontend/source_view.rs, traits/subst.rs. Add TyTerm/RegionTerm/ConstTerm/ParamEnv and occurs/leak-safe substitution. Acceptance: TR001/TR002; type/lifetime/const arg namespaces distinct. Depends: T045,M016.

**M018 — SourceView propagation.** Files: frontend/source_view.rs, vm/frame.rs, ir/recipe.rs. Every required expression propagates symbolic and runtime views separately. Acceptance: TR003–TR005, alias/field/return/default-Self variants. Depends: M017.

**M019 — Impl and trait header lowering.** Files: traits/header.rs, traits/index.rs, compat/traits.toml. Lower generic predicates and associated headers without bodies; index blanket candidates. Acceptance: TR006 and indexed-versus-full-catalog property model. Depends: M018.

**M020 — Canonical goals and transactions.** Files: traits/canonical.rs, traits/unify.rs, traits/table.rs. Implement canonicalization, isolated candidate substitutions, explicit result states. Acceptance: TR007/TR008; failed candidate cannot constrain another. Depends: M019.

**M021 — Conditional impl selection.** Files: traits/select.rs, traits/table.rs. Evaluate necessary candidate predicates and exact trait arguments; preserve ambiguity. Acceptance: TR009–TR011; unrelated unused bounds remain unforced. Depends: M020.

**M022 — Tabled recursion and limits.** Files: traits/scc.rs, traits/worklist.rs, traits/limits.rs. Inductive goals, approved auto-trait SCC handling, no false success on cycles. Acceptance: TR012/TR013 and reduced-budget limits. Depends: M021.

**M023 — Projection and associated constants.** Files: traits/normalize.rs, frontend/constants.rs. GAT substitution and associated const evaluation on demand. Acceptance: TR014–TR016; cold associated constant uncomputed. Depends: M022.

**M024 — Higher-ranked and region facts.** Files: traits/regions.rs, traits/universe.rs. Implement header-fact closure and placeholder leak checks; indeterminate distinct from no solution. Acceptance: TR017/TR018; for-all and static-only witnesses do not unify accidentally. Depends: M023.

**M025 — Source member identification.** Files: frontend/member.rs, frontend/autoderef.rs. Candidate order, generic bound priority, exact item identity, explicit qualified paths. Acceptance: TR003/TR019/TR020; mutability failure never picks lower-ranked member. Depends: M024.

**M026 — Lazy dictionary fulfillment and calls.** Files: vm/dictionary.rs, vm/call.rs, traits/witness.rs. Symbolic assumptions versus concrete proof, exclusion of circular assumption fulfillment, defaults and supertraits. Acceptance: TR021–TR023; selected method contract checked; other missing methods remain cold. Depends: M025.

**M027 — Selection invalidation.** Files: db/dependency.rs, traits/cache.rs. Successful/negative/ambiguous goal dependencies include producers and catalog membership. Acceptance: CA001–CA003; changed method body reuses witness but not old body. Depends: M026.

### 57.4 Runtime and native tasks

**M028 — Standard trait headers and derives.** Files: compat/traits.toml, facade/traits.rs, expand/builtin_derive.rs. Implement exact inventory and real generated bounds. Acceptance: TR024/TR025 header/eligibility assertions and native-derived-header comparison; generated method execution joins M029–M034 and the final gate. Depends: T065,M027.

**M029 — Copy/Clone/Drop/Default integration.** Files: runtime/shape.rs, vm/cleanup.rs, vm/trait_ops.rs. Use selected witnesses and existing ownership machine; no manual-impl rejection. Acceptance: TR026–TR029 and double-drop/move property suite. Depends: M028.

**M030 — Operator/Index/Deref continuation paths.** Files: vm/trait_ops.rs, vm/place.rs, ir/op.rs. Selected adjustments once, source-aware assignment order, returned lease validation. Acceptance: TR030–TR033 and mutation ordering oracle. Depends: M029.

**M031 — Iterators and conversions.** Files: facade/iter.rs, vm/iterator.rs, vm/control.rs. User Iterator/IntoIterator projections, specified named-function adapters, Result From conversion. Acceptance: TR034–TR037; break/return drops once. Depends: M030.

**M032 — Managed Box/dyn views.** Files: runtime/box_value.rs, runtime/dyn_value.rs, traits/dyn_schema.rs, vm/dictionary.rs. New tags, layout checks, header dyn compatibility, associated bindings, supertrait upcast, ownership. Acceptance: TR041–TR044 and stale/mutable lease tests. Depends: M031.

**M033 — Formatting and Hash callbacks.** Files: facade/fmt.rs, facade/hash.rs, vm/format.rs. Bounded managed formatter/builders, write!, user trait calls, recording Hasher. Acceptance: TR038–TR040, no host callback/unwind shortcut. Depends: M032.

**M034 — Opaque and const-generic contracts.** Files: traits/opaque.rs, frontend/constants.rs, vm/call.rs. Argument/return impl Trait identity, observed hidden-type agreement, capture terms and const descriptor keys. Acceptance: TR045–TR047. Depends: M033.

**M035 — JIT witness guards and optimized dependencies.** Files: jit/plan.rs, jit/verify.rs, jit/maps.rs. Preserve source-selected member identity across baseline and optimizing guards/maps. Acceptance: CA004/CA005 and CA014/CA021 under actual off/auto/force entries; T089 supplies optimized acceptance. Depends: T084,M034.

**M036 — Portable source/goal cache.** Files: source/checkpoint.rs, traits/cache.rs, expand/cache.rs. Remap IDs, revalidate new-epoch procedural producers, forbid pointers/session state. Acceptance: CA006/CA007 in fresh hybrid workers. Depends: M035.

**M037 — Effects and suspension fault injection.** Files: tests/effects.rs, tests/query_suspend.rs. Inject NeedInput, goal overflow, macro failure and cancellation around already-evaluated arguments. Acceptance: CA008–CA010, no duplicate output/write/move/drop/native entry. Depends: M036.

**M038 — Compile-time/live/native graph integration.** Files: project/units.rs, project/native_units.rs, project/prepare.rs. Apply exact role graph to existing H preparation flow. Acceptance: NX001/NX002; BuildHost duplication distinct from forbidden native-runtime cycle. Depends: T094,H028,M037.

**M039 — Macro-export facets.** Files: project/macro_export.rs, frontend/macro_names.rs, native/schema.rs. Source macro reexports with cfg/edition/$crate, explicit runtime helper binding only. Acceptance: NX003/NX004, native bodies prepared zero. Depends: M038.

**M040 — Native-world mismatch guards.** Files: native/call.rs, frontend/types.rs, cli/render.rs. Reject live nominal/trait witness interchange and preserve argument effects. Acceptance: NX005/NX006; no compilation/native entry on mismatch. Depends: M039.

**M041 — CLI/service schema v3.** Files: cli/args.rs, service/wire.rs, compat/service-v3.json. Independent authorities, epoch/events/results, public v3 and internal v4 identities, legacy rejection. Acceptance: NX007/NX008 and strict JSON golden tests. Depends: M040.

**M042 — Freshness and authorized watch.** Files: service/watch.rs, project/tool_freshness.rs. Live-only edit re-expands; tool/native input change stops for explicit prep; snapshot authority consistent. Acceptance: NX009/NX010; no hidden compiler orchestration in run tree. Depends: M041.

### 57.5 Evidence and release tasks

**M043 — Expanded/native fixture oracle.** Files: xtask/macro_oracle.rs, fixtures/macros, fixtures/traits. Build/run valid fixtures with pinned rustc/project compiler and compare names/results/effects; relaxed invalid sources have separate expectations. Acceptance: every MX/TR/NX/CA fixture has a runnable harness and no skip. Depends: T103,M042.

**M044 — Hygiene/binder property tests.** Files: tests/properties/tokens.rs, binders.rs, impl_index.rs. Alpha-renaming, substitution roundtrips, candidate index completeness, context collision and negative-cache generation. Acceptance: seeded tests detect deliberately injected hygiene/rigid-param bugs. Depends: M043.

**M045 — Process and protocol robustness.** Files: tests/macro_protocol.rs, tests/build_outputs.rs. Exercise CA023–CA025 binary transfers and independent protocol versions; fuzz sizes/truncation/wrong epoch, malicious span/OUT_DIR paths, crash/hang/session-global fixtures. Acceptance: bounded failures and no coordinator library load. Depends: M044.

**M046 — Performance decomposition.** Files: xtask/bench_frontend.rs, docs/performance.md. Measure tool preparation, expansion, sealing, goals, interpreter/JIT, native crossings separately for cold/warm/body-edit/macro-edit cases. Acceptance: raw measurements, no invented instant-run claim, all prescribed unfavorable cases included. Depends: M045.

**M047 — Contradiction and schema audit.** Files: xtask/spec_audit.rs, docs/feature-traceability.md. Match features/diagnostics/tasks/fixtures; reject obsolete bans/expected failures, stale profiles/wire versions and missing source links. Acceptance: intentional stale macro-unsupported/trait-bound rejection fixture is detected. CA037 additionally rejects hidden runtime cases, false full passes and runtime blockers; separate compiler-only outcomes never gate guest JIT. Depends: M046.

**M048 — Revision-5 release.** Files: docs/feature-traceability.md, xtask/release.rs, artifacts/release. Complete Sections 59/61/62 with hashes and actual backend/transport/contract observations. Acceptance: all required macro/trait/native/optimization fixtures and task gates have real platform-specific evidence; new requirements cannot inherit a revision-4 pass; final hybrid runs compile no live Cargo unit. Depends: M047,T089.

<a id="section-58"></a>

## 58. Mandatory macro, trait, native, and cache fixtures

### 58.1 Harness and observation schema

Add fixtures/macros, fixtures/traits, fixtures/frontend-cache, and fixtures/native-macros. Each case has a strict fixture manifest, source workspace or standalone source, exact preparation authority, exact runtime/macro/native authority, expected results/events, and a declared native oracle status. Ordinary valid Rust uses the recorded project compiler; deliberately relaxed source has oracle=false and a documented expected LRR result. Never change a relaxed fixture to compile merely to make its native oracle pass.

Extend the fixture schema with version 2 fields: preparation selector/toolchain; macro_permissions; expected_expansion_count or bounded comparison; expected_generated_test_names; expected_member_ids; expected_goal_outcomes; no_forced_goals; no_body_preparation; expected_process_roles; required_cache_hits/misses; expected_epoch_behavior; and expected_origin_chain. Older common-engine fixture schema1 remains readable with explicit default-none macro/native settings, but it cannot express or satisfy the new fixtures.

For JIT equivalence, first record expansion output once for the controlled deterministic fixture epoch, then run off/auto/force with fresh guest state against that same frozen expansion/cfg/native generation. Separately test real re-expansion across requests. Otherwise a deliberately nondeterministic macro can create different programs and invalidate a meaningless cross-tier comparison. The test-only frozen expansion mode is not exposed as production cache reuse. Assert genuine native entries when a fixture claims JIT execution.

### 58.2 Shared fixture proc-macro crate

Create a proc-macro package named lrr_fixture_macros in a controlled test workspace. It has edition2024 and no dependencies. The following is real fixture source, not the runtime implementation. unwrap is permitted in this deliberately small host fixture; production transport code follows checked error handling.

```rust
use proc_macro::TokenStream;
use std::str::FromStr;

#[proc_macro]
pub fn identity(input: TokenStream) -> TokenStream { input }

#[proc_macro_attribute]
pub fn make_tests(_attr: TokenStream, _item: TokenStream) -> TokenStream {
    TokenStream::from_str(
        "#[test] fn generated_one() { assert_eq!(2_i64 + 3_i64, 5_i64); }\n\
         #[test] fn generated_two() { assert_eq!(7_i64, 7_i64); }"
    ).unwrap()
}

#[proc_macro_derive(EmitItems, attributes(lrr_helper))]
pub fn emit_items(_input: TokenStream) -> TokenStream {
    TokenStream::from_str(
        "fn generated_helper() -> i64 { 41_i64 }\n\
         impl Mark for Other { fn mark(&self) -> i64 { 43_i64 } }"
    ).unwrap()
}

#[proc_macro]
pub fn deliberately_panics(_input: TokenStream) -> TokenStream {
    panic!("controlled fixture macro panic")
}
```

Keep EmitItems in isolated fixture modules that define Mark and Other, so output names are unambiguous. Additional fixture macros must implement: token-preserving recursive group traversal; malformed-output generation; input-body-dependent declaration generation; controlled external file read; a session counter; generated include path; hanging/crashing child behavior tested only under the supervisor. Do not implement hygiene tests by printing/reparsing every input token, because that would destroy the property being tested.

### 58.3 Macro/build/discovery matrix

| ID | Concrete setup and required assertion |
|---|---|
| MX001 | Build script emits generated.rs and cfg; live cold function has undefined call. compile-time-only prepare succeeds with no live runtime rustc invocation; ordinary cargo check fails. |
| MX002 | Same package has normal/dev/build feature variants. Compare selected lib-tests and integration-test unit graphs; cfg(test), aliases, and tool features remain distinct. |
| MX003 | Load the Cargo-reported proc artifact in the matched server and expand identity!(3_i64). Assert correct tokens and compiler/server hashes. |
| MX004 | Substitute wrong server hash/compiler tuple and then wrong artifact hash. Both fail before library loading; no guest execution. |
| MX005 | Two build units each emit a file of the same name with different values. Includes read the appropriate unit's value, never whichever OUT_DIR was last reported. |
| MX006 | Controlled concurrent write during OUT_DIR capture. Preparation rejects racing publication; later stable prepare succeeds. |
| MX007 | Token trees with invisible groups, lifetimes, joint operators, raw identifiers, doc attributes, and nested delimiters round-trip with context/spans preserved. |
| MX008 | macro_rules repetition emits a tuple/array and invokes nested macros. Values/order match native, cardinality mismatch produces expansion error. |
| MX009 | A 2021-definition expr matcher and a 2024-definition expr matcher receive const-block input; result/rule match follows definition edition. expr_2021 explicitly preserves older behavior. |
| MX010 | Recursive macro hits reduced expansion limit; leftover input and no rule match are distinct errors, not host stack overflow. |
| MX011 | Exported macro references $crate::helper through a renamed dependency; a caller helper with same name cannot capture it. |
| MX012 | Textual macro shadowing before/after definitions and a generated local identifier collision match the native oracle. |
| MX013 | Missing expansion suspends a pure lookup; after publication it resolves. No cached negative answer, duplicate macro invocation, or guest effect. |
| MX014 | Function-like proc macro generates an expression, a type fragment, and an item in separate valid invocations. Each is parsed in its actual grammar context. |
| MX015 | make_tests replaces an annotated dummy function. Only generated_one/generated_two appear; --exact selects the transformed name. |
| MX016 | EmitItems attached to Marker generates helper function and Mark impl for Other. Both are visible even though unrelated to Marker's trait universe. |
| MX017 | Controlled macro panic/crash/hang/oversized output. Correct phase/diagnostic, helper terminated, no retry and no should_panic pass. |
| MX018 | Two replacing attributes and a derive alter names/attributes in known sequence. Compare final items and helper-attribute visibility to native. |
| MX019 | cfg(false) removes a failing proc macro and cfg_attr(true) adds one. Only active invocation runs; inactive failure is not discovered. Use valid cfg names with explicit cfg values. |
| MX020 | An undeclared helper attribute fails; a helper declared by the invoked derive is retained and accepted in its documented scope, not globally. |
| MX021 | include!(concat!(env!("OUT_DIR"), "/generated.rs")) contributes functions/types/impls used by the selected test. |
| MX022 | Nested relative includes retain the included file's base directory and caller lexical context. An escaping source path is denied with both origins. |
| MX023 | env!/option_env! distinguish explicit value and absence per unit. Change captured env and require dependent expansion invalidation, not ambient worker lookup. |
| MX024 | Externally generated source file changes bytes or membership between prepares. New source appears only in new snapshot; removed file invalidates a prior include success. |
| MX025 | Proc macro reads controlled external file during expansion. Change file between runs without changing binary; second epoch re-expands and observes new output. Mark native macro inputs non-hermetic. |
| MX026 | Session counter macro: shared invocation record reused once within an epoch, independent consuming units and later epochs use separate fresh sessions. |
| MX027 | Macro generates an import and another macro needed by a later invocation. Worklist reaches a complete fixed point without order-dependent missing names. |
| MX028 | Cyclic unresolved macro/import producer graph fails catalog-incomplete/cycle; simple cyclic reexports with a genuine originating definition resolve. |
| MX029 | A macro in an uncalled function emits an impl needed by a called function. The impl is indexed; cold ordinary statements remain unprepared. |
| MX030 | Expression-position macro expands to a block with an impl, inside a cold branch. Selection sees that impl; no generated ordinary operation executes. |
| MX031 | Attribute generates ignored, expected-panic, and normal tests; list/filter/ignore decisions use final attributes and identities. |
| MX032 | A required expansion errors while tests are filtered. Discovery fails, not zero-tests success; generated async test stays visible and fails its reached unsupported feature when selected. |
| MX033 | Generated method contains a reached type/operation error. Diagnostic traces method output, derive/attribute invocation, and source file; no misleading line from previous edit. |
| MX034 | Body-sensitive attribute changes only output bodies, then changes an impl header. First edit preserves unrelated header queries; second invalidates catalog and dispatch. |

### 58.4 Generic identity regression: complete source

TR003 uses the following valid Rust and must pass both the native oracle and every LRR tier. Assert the selected associated-item identities as well as the return values.

```rust
trait Describe { fn describe(&self) -> i64; }
struct Widget;
impl Describe for Widget { fn describe(&self) -> i64 { 1_i64 } }
impl Widget { fn describe(&self) -> i64 { 2_i64 } }
fn through_bound<T: Describe>(x: &T) -> i64 {
    let alias = x;
    alias.describe()
}
#[test]
fn case() {
    let w = Widget;
    assert_eq!(through_bound(&w), 1_i64);
    assert_eq!(w.describe(), 2_i64);
}
```

The implementation is wrong if both calls return2, both return1, or the result happens to be right while the recorded symbolic member is wrong. Add variants passing through a generic field, a function return, and a trait default method. Do not fix this test with a hard-coded priority for the trait name Describe.

### 58.5 Conditional impl and projection: complete source

TR009/TR014 share this fixture. Encode is forced only because selecting Measure for Wrapper<T> requires it; Measure's unused associated constant is not evaluated.

```rust
trait Encode { fn code(&self) -> i64; }
struct Item;
impl Encode for Item { fn code(&self) -> i64 { 7_i64 } }
struct Wrapper<T>(T);
trait Measure {
    type Output;
    const UNUSED: i64;
    fn measure(&self) -> Self::Output;
}
impl<T: Encode> Measure for Wrapper<T> {
    type Output = i64;
    const UNUSED: i64 = 99_i64;
    fn measure(&self) -> i64 { self.0.code() }
}
fn measured<T: Measure<Output = i64>>(x: &T) -> i64 { x.measure() }
#[test]
fn case() { assert_eq!(measured(&Wrapper(Item)), 7_i64); }
```

A separate relaxed fixture replaces UNUSED's initializer with an unsupported/broken pure expression and still passes while no use forces that constant. It has oracle=false. A third fixture uses Wrapper<NoEncode> and must report NoSolution at the reached Measure selection, not silently execute a method because its header matched structurally.

### 58.6 GAT and impl-generation sources

TR015 uses a lending associated type, exercising lifetime binders without requiring a borrow checker:

```rust
trait Lend {
    type Item<'a> where Self: 'a;
    fn lend<'a>(&'a self) -> Self::Item<'a>;
}
struct Number { value: i64 }
impl Lend for Number {
    type Item<'a> = &'a i64;
    fn lend<'a>(&'a self) -> Self::Item<'a> { &self.value }
}
#[test]
fn case() {
    let n = Number { value: 17_i64 };
    assert_eq!(*n.lend(), 17_i64);
}
```

MX016's consuming source is:

```rust
use lrr_fixture_macros::EmitItems;
trait Mark { fn mark(&self) -> i64; }
struct Other;
#[derive(EmitItems)]
#[lrr_helper]
struct Marker;
#[test]
fn case() {
    assert_eq!(generated_helper(), 41_i64);
    assert_eq!(Other.mark(), 43_i64);
}
```

MX029's source declares a macro returning a block containing `impl Mark for Other`, invokes it in an uncalled function, then calls Other.mark in the test. A companion adds an ordinary unresolved call after that macro invocation and sets oracle=false. Require no_body_preparation for the cold function, but require its macro expansion/impl header to be present. This directly tests the declared boundary between discovery and execution laziness.

### 58.7 Source-dependent assignment order: complete source

TR033 expects stdout `RL|LR`, matching the primitive-versus-generic ordering distinction. The actual generic instantiation is i32 in both paths; a runtime-tag-only decision is therefore insufficient.

```rust
use std::ops::AddAssign;
fn index() -> usize { print!("L"); 0_usize }
fn rhs<T: Default>() -> T { print!("R"); T::default() }
fn primitive(xs: &mut [i32]) { xs[index()] += rhs::<i32>(); }
fn generic<T: AddAssign<T> + Default>(xs: &mut [T]) {
    xs[index()] += rhs::<T>();
}
#[test]
fn case() {
    let mut a = [1_i32];
    primitive(&mut a);
    print!("|");
    generic(&mut a);
    assert_eq!(a[0_usize], 1_i32);
}
```

A helper/guard failure injected after rhs must not print R twice. The fixture records SourceAssignKind and actual operand-event order in all tiers.

### 58.8 Trait fixture matrix

| ID | Required setup and observation |
|---|---|
| TR001 | Alpha-renamed type/lifetime/const binders canonicalize identically; free and bound variables remain distinct. |
| TR002 | Occurs-check and placeholder leak failures roll back local substitutions; no corrupt cached term survives. |
| TR003 | Complete generic Describe source above; generic1/direct2, exact member IDs. |
| TR004 | Generic receiver passes through let alias, reborrow, field projection, and named function return; trait identity stays bound-derived. |
| TR005 | Trait default calls a second trait member while actual type has same-named inherent method; default uses trait, concrete direct call uses inherent. |
| TR006 | Index all concrete/unbounded/conditional/blanket headers; compare filtered candidates to full enumeration under generated additions. |
| TR007 | Same trait with different generic arguments chooses distinct impls and cache entries; same name is insufficient. |
| TR008 | First candidate partially unifies then fails; second candidate succeeds without leaked inference bindings. |
| TR009 | Conditional Measure/Encode fixture above; nested predicate is forced and correct method executes. |
| TR010 | False conditional predicate yields NoSolution; overlapping relaxed impls yield Ambiguous, never first-listed success. |
| TR011 | Ordinary function has unused impossible bound and passes under relaxed profile with zero forced proof for that predicate; used bound fails. |
| TR012 | Inductive recursive goal with no base proof does not succeed; a separate finite recursive proof with a base does. |
| TR013 | Auto-trait recursive SCC follows approved coinduction; mixed ordinary/auto cycle does not inherit automatic success; forced limits are not NoSolution. |
| TR014 | Measure::Output normalizes to i64 only when call/result requires it; irrelevant associated constant uncomputed. |
| TR015 | Lending GAT source above; use-site region/associated arguments preserved, returned lease remains checked. |
| TR016 | Projection cycle produces attributed cycle/limit outcome; associated const depends on concrete generic arg and has distinct cache keys. |
| TR017 | `for<'a> Read<'a>` generic impl matches; static-only impl cannot fulfill a universal binder. Valid and relaxed-negative versions separated. |
| TR018 | A required undecidable region fact produces IndeterminateRegion, not successful dictionary or false NoSolution; explicit header fact permits a companion. |
| TR019 | Same-rank visible trait ambiguity, explicit UFCS disambiguation, and supertrait path deduplication. |
| TR020 | Mutable-receiver lookup chooses according to source ordering; unwritable receiver errors rather than selecting a lower-ranked callable. |
| TR021 | Lazy dictionary resolves once per instantiated TraitRef; call count may increase without repeated goal solving. |
| TR022 | A parameter assumption cannot prove itself after concrete substitution; missing real impl fails when first used. |
| TR023 | Missing unrelated required method does not block relaxed selected default method; missing selected method fails. |
| TR024 | Built-in and custom derive-generated headers use ordinary candidate engine; conditional generic bounds appear in dependencies. |
| TR025 | Generic derive bound sensitivity differs from naive field-only policy; compare pinned compiler acceptance on valid cases and explicitly classify relaxed negatives. |
| TR026 | Manual Clone emits a marker even for a Copy type; cloning runs it exactly once. |
| TR027 | Invalid claimed Copy on an owning String-containing type faults safely; normal manual scalar Copy works. |
| TR028 | Selected Drop and field cleanup order match native; unused Drop body cold; changed Drop impl invalidates policy. |
| TR029 | Manual Default creates nonzero value; generic Default call respects trait identity and bound selection. |
| TR030 | Custom Add/PartialEq use exact language traits and expected move/borrow effects; same-named user trait is not substituted. |
| TR031 | User Deref emits a marker; method search emits none, chosen adjustment emits exactly one. |
| TR032 | IndexMut returns a checked reference to an element; assignment writes once; stale/wrong element contract fails safely. |
| TR033 | Complete assignment-order source above prints RL|LR under off/auto/force. |
| TR034 | User IntoIterator returns custom Iterator with associated Item; for sums values and drops iterator once on break. |
| TR035 | Named-function map/filter/count and collect::<Vec<T>> call custom next with correct count/order. |
| TR036 | Result ? invokes manual From error conversion once; successful path does not force conversion dictionary. |
| TR037 | TryFrom/TryInto Error projection and AsRef/AsMut method identity; no implicit native conversion. |
| TR038 | Manual Display::fmt uses write! into bounded formatter; output and side effects match native. |
| TR039 | Derived Debug struct/tuple/list output and manual fmt::Write builder path; capture lifetimes valid, output failure not replayed. |
| TR040 | Derived/manual Hash writes to a recording Hasher in field order; no dependence on native RandomState. |
| TR041 | &dyn Trait and &mut dyn Trait dispatch via exact item slots with associated type binding. |
| TR042 | Box<dyn Trait> owns one object, mutable/dropped views behave correctly, destructor runs once. |
| TR043 | Supertrait upcast preserves data identity and substituted trait args; stale views remain rejected. |
| TR044 | Nondyn-compatible generic method/associated member schema fails at coercion without checking any method body. |
| TR045 | Argument impl Trait uses anonymous bound parameter; return impl Trait hides actual inherent methods but permits declared trait methods. |
| TR046 | Two observed hidden types for one opaque definition/instantiation fault; unvisited conflicting return remains unprepared. |
| TR047 | Const generic arrays and associated constants distinguish instantiations; unnecessary const evaluation remains zero. |

### 58.9 Native/world and incremental matrix

| ID | Required setup and observation |
|---|---|
| NX001 | Live generated test calls an existing optimized native binding. No live compiler invocation; artifact freshness and real native entry recorded. |
| NX002 | Same source package occurs as live runtime and declared BuildHost. Only host unit is compiled in explicit preparation; native-runtime dependency on live remains rejected. |
| NX003 | Native crate reexports proc/macro_rules macros through a renamed alias. Expansion succeeds via macro-export facet; native runtime bodies prepared remain zero. |
| NX004 | Exported macro emits $crate::helper. Declared binding works; absent binding is reached missing-binding fault, no arbitrary symbol lookup. |
| NX005 | Live derived struct passed where bridge expects native opaque type. World mismatch occurs before native entry and without compiler fallback. |
| NX006 | A bridge generic binding is not prepared for a requested concrete native instantiation. Explicit preparation required; no test replay. |
| NX007 | build/macro/native/read/write permissions are independent. Every omitted needed authority fails at its own phase; managed guest label does not hide native macro authority. |
| NX008 | Service3/protocol limits, wrong epoch, old preparation schema rejection, and noisy native logs cannot corrupt control stream. |
| NX009 | Edit live macro input body; binary stays reusable, invocation re-expands. Edit macro implementation; run rejects stale prep until explicit build. |
| NX010 | Change build-script input/generated file/native source in separate cases; each invalidates proper preparation only. No auto compiler during watch rerun. |
| CA001 | Add generated impl after previous NoSolution; new epoch observes success, negative dependencies invalidate. |
| CA002 | Add competing generated impl after success; new epoch observes ambiguity, previous JIT dispatch cannot bypass it. |
| CA003 | Change selected method body only; witness reused, body/region invalidated, new value executes. |
| CA004 | JIT receiver tags unchanged but selected witness generation changed; dispatcher invalidates stale call recipe. |
| CA005 | Native region reaches a user trait call/projection demand; exits to canonical continuation and returns once. |
| CA006 | Persist portable records, restart worker, revalidate procedural producer output, re-intern all IDs. No pointer/guest/session state restored. |
| CA007 | Proc output semantically unchanged after re-execution preserves pure downstream fingerprints; changed hygiene/span-sensitive output invalidates them. |
| CA008 | Suspend between receiver and argument phases; fulfill input once, later argument effect count is one. |
| CA009 | Macro/helper/goal failure after completed output preserves output once; runtime does not restart call/test. |
| CA010 | Cancellation and stale late replies during expansion/goal evaluation; no generation published to wrong revision and no user effects after cancellation is accepted. |

### 58.10 Required negative test hygiene

A proc macro fixture allowed native execution can itself compromise its helper; run malicious/crash probes only against controlled fixtures with explicit subprocess supervision. Do not run untrusted third-party macros as part of a general test search. For native oracles, place expected compile errors in separate source crates so they cannot invalidate unrelated valid-case builds. Check generated test counts/names as well as pass/fail values. A run of zero tests is never evidence of procedural test support.

### 58.11 Direct ABI and optimizing execution matrix

These are required acceptance contracts, initially unexecuted. Add unchanged upstream language/ABI fixtures with provenance when implementing each case; custom wrapper, mutation, tier and effect assertions live outside the upstream directory. A compiler-only probe is a component result, not a full LRR fixture pass.

| ID | Required setup and observation |
|---|---|
| NX011 | Prepare a declared dep generic scalar instance through the actual Rust 1.99.0 producer. Same-invocation ABI sidecar, typed pointer, Cargo reuse and no live-unit build are verified; real LRR invocation returns the expected result. |
| NX012 | Swap/truncate ABI sidecar, producer ID, library bytes, request digest and receipt separately. Each mismatched generation rejects before call entry, with no guessed compiler or fallback transport. |
| NX013 | Differential Rust-pointer and JIT callers exercise signed/unsigned widths, bool, char, floats, Pair, Cast, Indirect and hidden return forms; unsupported forms reject. Each admitted ABI class actually executes. |
| NX014 | Prepared zero-sized type with Drop and a by-value aggregate preserve initialization, return and exact destructor counts across interpreted and compiled entries. |
| NX015 | Construct, borrow, move, replace and drop a native generic container, including direct-new/codec-method/direct-drop through one context/registry; source storage retires without double Drop; duplicate ownership and use-after-move fail before entry. |
| NX016 | Wrong nominal type, worker epoch, generation, slot and uninitialized resident storage all fail admission; matching size/name never grants compatibility. |
| NX017 | Direct byte/string leases test shared overlap, mutable conflicts, disjoint ranges, empty slices, bounds/UTF-8 and stable backing. Mutation is visible without staging/copyback; unpinnable input does not silently copy. |
| NX018 | Hot scalar direct call has actual matched Rust ABI entries without per-call codec/dispatcher/helper paths; retained assembly and counters confirm the intended path. |
| NX019 | Unwind generation mutates a borrow then panics. Its Rust-ABI adapter contains the unwind, consumes owned arguments once, preserves mutations and enters guest cleanup without adopting an output. |
| NX020 | Abort generation, foreign-unwind violation and exotic panic-payload cleanup failure produce supervised infrastructure outcomes, never expected guest panic success or retries. |
| NX021 | Guard miss before call and forced exit after call preserve side-effect counter exactly once; post-call reconstruction resumes after the completed invocation. |
| NX022 | Native/live type mismatch, missing concrete specialization, forbidden callback/borrowed return/track_caller case and unknown symbol all fail at the documented phase without tools or effects. |
| NX023 | Fuzz schema counts/tags/offsets, ABI components and registration entry IDs; oversized/malformed plans never execute native pointers. |
| NX024 | Prepare IR for an accepted concrete scalar leaf. Retain its producer/LLVM/import identities; live LLVM caller actually inlines the body, confirmed by assembly and semantic comparison. |
| NX025 | IR with global/TLS/allocator/unwind/address-identity dependencies or unknown import is rejected by the initial import class; its existing declared direct binding still works. No duplicate native state or default symbol lookup. |
| NX026 | Same-pointer Rust and Cranelift ABI benchmark uses at least 50 paired independent batches, retained calls and real result consumption; report confidence interval and raw target achieved/missed/inconclusive honestly. |
| NX027 | Full LRR checked/scalar/optimized/LLVM workloads versus ordinary-call and inlined Rust baselines report all runtime/compile/load/conversion costs separately; raw ABI success cannot satisfy integrated near-native claims. |
| NX028 | New worker rebinds a valid artifact to the exact generation with no preserved guest state or pointer; native source/ABI/CPU/profile/IR changes invalidate it. |
| CA011 | Optimized scalar locals stay in registers, yet every pre-op guard/overflow/fault exit reconstructs the exact logical locals and source PC. |
| CA012 | Precise low fuel and early faults during batched checks consume only the executed prefix and occur before the same observable operation as the interpreter. |
| CA013 | Native backedges execute multiple iterations per entry; an unseen branch exits at that branch without checking its cold body or replaying prior iterations. |
| CA014 | At most four optimized concrete variants plus baseline exist; a fifth shape uses a correct fallback; witness identity changes invalidate affected variants. |
| CA015 | Hoisted native object lease releases on every exit; move/drop/conflicting alias or buffer reallocation invalidates pointer assumptions before use. |
| CA016 | LLVM lowering executes real reached LRR scalar control flow with correct overflow/panic semantics and source locations; forced unavailable LLVM fails explicitly. |
| CA017 | Prepared native leaf inlining preserves native release arithmetic inside a checked live caller; unsupported import keeps a labeled direct call, not a claimed inline entry; fully inlined execution has zero physical ABI entries and correct separate logical/inline counters. |
| CA018 | Both contracts preserve results/ownership/effects; precise fuel agrees exactly, throughput reports null and rejects explicit numeric fuel. Poll intervals and timeout behavior are actually exercised. |
| CA019 | A compile job finishing after source/witness/generation changes is discarded; no old target installs under a new epoch and no compiler task accesses live query state; killing an over-budget compiler subprocess leaves the guest and its already completed effects intact. |
| CA020 | Corrupt/truncated object/map/relocation, wrong CPU/helper/backend/contract and untrusted cache provenance never publish executable code. |
| CA021 | Reached guest-call inlining reconstructs source stack, frame limits, ownership and Drop order on each exit; changing the inlined body invalidates callers. |
| CA022 | Saturate candidate, compiler, variant, object-cache and code budgets; preserve active code until safe retirement, fall back visibly in auto, and never claim missing forced-tier execution passed. |


Additional encoding-amendment acceptance (all initially unexecuted):

| ID | Required setup and observation |
|---|---|
| NX029 | With jit=off, execute both initial scalar interpreter invoker classes and the general slot path against the same prepared targets; compare semantics, panic/fuel/cancellation, wrong-class rejection, allocations/copies, code size and paired timings. No measured benefit means retain slot selection, not an invented speedup. |
| NX030 | A resident container crosses many direct operations with zero codec calls; an explicitly requested managed/native conversion records its actual copy/allocation. Mixed direct/codec ownership stays singular, and unsupported direct storage never silently selects a codec. |
| CA023 | Snapshot, arbitrary effect bytes, checkpoint and compiler/object payloads round-trip in internal v4 raw chunks, including empty/1-byte/1-MiB boundaries and split UTF-8; exact payloads/digests agree with reference inputs. Counters show raw bytes plus headers, not hex/base64 expansion. |
| CA024 | Oversized/truncated/wrong-kind/reserved headers, duplicate or stale IDs, out-of-order chunks, wrong digest, cross-run transfer, cancellation and disconnect fail without publication or repeated effects. Queued and retained allocations remain within all byte/transfer quotas. |
| CA025 | Old internal framing and wrong-role/build handshakes reject before native load/effect entry; public schema-3 JSON still works. Matched macro-server traffic retains its own protocol and cannot be mistaken for internal v4. |
| CA026 | Profile macro/ABI/query record copies and decoding on cold, warm and invalidated inputs. Any optimized buffer/decoded-record reuse preserves token/hygiene/source identity, required procedural revalidation, authority and epochs; report regressions or no benefit honestly. |

Additional interpreter/tier-transfer amendment acceptance (all initially unexecuted; CA027 runs during interpreter-first work, CA028–CA033 only after interpreter acceptance):

| ID | Required setup and observation |
|---|---|
| CA027 | Repeated integer literals across bases, separators, suffixes, negative minima and overflow boundaries decode lexically once per prepared recipe; repeated executions retain independent unresolved bindings, exact diagnostics/fuel and cold-path laziness. Cover cache reuse/invalidation and stale decoded schema rejection. Use unchanged official rustc numeric/edition cases with decoding counters outside upstream files; report actual allocations/timings separately from correctness. |
| CA028 | Enter a live loop after several interpreted iterations, transfer between native tiers and return to the interpreter. Wrong PC/type/epoch/map or unavailable reservation rejects before ownership changes. Pending calls/copy/cleanup defer entry. Both contracts preserve exact effects, initialized/moved state, source stack and progress; precise fuel agrees, no iteration/call repeats, and repeated transfer keeps host stack bounded. |
| CA029 | A real background compiler finishes while a native loop is still running. An explicit test rendezvous controls completion timing without replaying guest work. Existing safepoints notice readiness within their guest-work polling bounds and transfer for validated installation; stale/cancelled/budget-limited results retain fallback. Verify actual later target entry, rejection reason, effect counts and compilation latency separately. |
| CA030 | Interpreter traversals make baseline eligible; subsequent completed native backedges/traversals alone make an optimizing job eligible. Flush counters at bounded polls and transfer/exit, with partial/faulted traversal excluded and completed edges counted once. Cache loads, entry guards and compiler activity never manufacture observations. |
| CA031 | Alternate matching shapes, an unrepresented fifth shape and a persistent internal guard miss. Verify direct variant selection, eight-failure threshold, 256-to-65,536 opportunity cooldown, bounded metadata, probation and 64-success reset; fallback makes progress with identical effects and no infinite retry. Forced-tier evidence cannot be satisfied by suppressed/fallback-only execution. |
| CA032 | Compare interpreter/Cranelift/LLVM under low live-byte, cumulative-handle and frame quotas with scalar-replaced aggregates and inlined calls. First failing logical operation and completed prefix agree. Pre-entry backing failure falls back unchanged; force materialization after a committed effect with allocation failure injection to prove it uses reserved backing, with no second logical charge, lost owner or repeated effect. Physical memory limits remain separately reported. |
| CA033 | A workload changes phase with unchanged source/dependencies. Deterministic injected cost history plus actual traversal counters triggers profitable reconsideration only after the 4,096/doubling evidence intervals; verify three-retry cap across variants, fresh-run reconsideration, no duplicate jobs and no retry of ABI defects. Nonprofitable fresh evidence submits no job; no timing assertion substitutes for native-entry evidence. |

Final contract-clarification acceptance (all initially unexecuted; CA037 is interpreter-first evidence):

| ID | Required setup and observation |
|---|---|
| CA034 | Scalar-replaced allocation/drop loops retain bounded live identity metadata without physical per-iteration tombstones. Force materialization with aliases and after an effect; fresh physical indexes bind once, old/stale handles remain invalid, logical quotas agree, and adopted arena backing survives native return until ordinary retirement. Failure injection checks reserved publication capacity and exact ownership/memory charges. |
| CA035 | While unit A loops, unit B completes a real compiler job large enough to exercise output backpressure. Bounded turns drain inactive/stale/cancelled outputs, service broker/cancellation and resume A without awaiting B. Saturate retained-output budgets, disconnect and duplicate IDs; verify compiler disposal and no cross-run adoption. Receiving/validated/published/selected counts remain distinct and effects occur once. |
| CA036 | SchedulingPolicyV1 defaults reproduce deterministic eligibility/cooldown/retry decisions. Inject one bounded alternative; settings/version/digest appear in results and cost history, malformed values reject, and both policies retain semantic/resource/authority limits. Code reuse is rejected only for changed semantic/code-generation identity, not scheduling statistics alone. |
| CA037 | Inventory all upstream files/revisions/configurations and classify runtime, diagnostic and compiler_tooling assertions, including mixed fixtures. Missing/unknown classification that can hide runtime work, unsupported runtime features, partial execution and oracle-only evidence block the runtime gate. The cold-body E0308 witness, static borrow diagnostics and MIR/rustdoc-only requirements stay outstanding separately without blocking it. Mixed runtime failures remain blockers; mixed runtime-only passes never become full-fixture passes. Reports reconcile inventory and category counts, retain platform deferrals, and cannot equate this harness control with full upstream runtime acceptance. |

<a id="section-59"></a>

## 59. Release, migration, and implementation audit

### 59.1 Mechanical migration from revisions 3 and 4

Invalidate live semantics caches from lrr-2024-v1, executable artifacts predating the new helper/map/contract identities, and preparation manifests before schema5. Reject public service schemas before 3 and all pre-v4 LRR internal framing, without fallback. The external macro-server tuple remains independently matched. The precise language semantics remain lrr-2024-v2; execution_contract is an additional mandatory identity. Migrate native configuration schema1 to schema2 explicitly, assigning codec-v1 to old codec bindings; do not reinterpret old bytes or claim they are Rust ABI entries. Reprepare native-generation schema2 with verified producer/transport/IR identities. No user-source rewriting is required. Existing component receipts remain historical evidence and cannot be relabeled revision-5 acceptance.

Remove the previous macro-unsupported dispatch path for valid user macro invocations, manual Copy rejection, bound-conditional-impl blanket rejection, actual-runtime-type-first generic member search, unknown-derive-only-affects-self rule, and metadata-only prohibition of live build/proc dependencies. Preserve genuine unsupported executable forms and raw/native safety boundaries. Every removed rejection is replaced by a positive regression test and an appropriate real-failure diagnostic.

### 59.2 Release commands to implement

Run these commands on macOS first. Their macOS results determine current task and milestone acceptance. Linux replay is deferred to the porting phase and must not be required to advance macOS implementation. A Linux-only fixture needs an equivalent real macOS acceptance fixture now, with its Linux replay tracked separately; this is not permission to skip the underlying behavior.

```sh
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- bootstrap-audit
cargo +1.99.0 run --locked -p xtask -- compile-time-tools-gate
cargo +1.99.0 run --locked -p xtask -- expansion-gate
cargo +1.99.0 run --locked -p xtask -- discovery-gate
cargo +1.99.0 run --locked -p xtask -- trait-goals-gate
cargo +1.99.0 run --locked -p xtask -- trait-runtime-gate
cargo +1.99.0 run --locked -p xtask -- frontend-cache-gate
cargo +1.99.0 run --locked -p xtask -- native-macro-gate
cargo +1.99.0 run --locked -p xtask -- direct-rust-abi-gate
cargo +1.99.0 run --locked -p xtask -- optimizing-jit-gate
cargo +1.99.0 run --locked -p xtask -- compiled-cache-gate
cargo +1.99.0 run --locked -p xtask -- verify-all
cargo +1.99.0 run --locked -p xtask -- release-audit
```

Each writes a machine-readable result with git revision, toolchain/adapter/server hashes, commands, fixture names, expected/observed values, exit statuses, and raw evidence paths. compile-time-tools-gate deliberately exercises authorized Cargo tool builds; no-compiler assertions apply to later live run orchestration, not outer host builds or explicit preparation. A disabled/missing fixture fails its gate.

### 59.3 Traceability requirements

Create docs/feature-traceability.md with one row per supported macro form, attribute behavior, source-generation mechanism, trait term/goal, language trait integration, native/world boundary, and authority. Columns: normative section, implementation module, task IDs, fixture IDs, and latest passing evidence. The audit requires every new diagnostic family in Section 56 to have a negative fixture and every new supported capability to have a positive fixture. Retain all common-engine and hybrid coverage; a new feature cannot erase an old ownership/JIT test.

Create docs/known-execution-boundaries.md from the actual feature manifest. Explicitly list async/closure/full-std restrictions, header-only region reasoning, native opaque boundaries, no global Rust validation, volatile procedural expansion results, and non-hardened native helpers. Include an example showing successful macro expansion followed by a reached unsupported operation, distinct from a macro error. Do not advertise complete arbitrary-crate compatibility.

### 59.4 Performance evidence and acceptance

Measure cold tool preparation; warm tool preparation; macro-free warm test; unchanged proc-heavy test with re-expansion; attribute body edit with unchanged headers; generated impl edit invalidating dispatch; conditional/projection-heavy loop; dynamic trait calls; native data crossing; and native-heavy generated test with fresh hybrid worker. Report median and tail latency from raw repeat samples, plus bytes/tokens/headers/goals/regions and cache statistics. State hardware and process lifecycle.

Correctness and absence of live compilation are hard gates. No numeric latency target is proof until measured. The conservative procedural-output policy can make a proc-heavy “no source edit” run slower than a macro-free one; report it. Only actual native-entry counters and Cargo freshness messages establish those respective reuse claims. Do not conflate metadata hits, expansion reuse, goal reuse, JIT code reuse, and optimized library reuse.

### 59.5 Final checklist

- All three procedural macro kinds and macro_rules work through hygienic tokens, not textual cargo expand output; definition editions and $crate remain correct.
- Build-script outputs, generated includes, cfg/env, aliases, and host/target/test units are captured under exact preparation identities without checking ordinary live runtime bodies.
- Scope, impl, and generated-test catalogs are complete before conclusions depending on absence/uniqueness; nested cold producers are handled without executing generated ordinary bodies.
- Generic and default-method calls preserve source member identity; only necessary concrete predicates/projections/dictionaries are resolved.
- Manual/derived policies, language operators, iterators, formatting, managed dyn/Box/opaque forms, and supported const/HRTB/GAT terms pass the declared fixture matrix.
- Query and expansion suspension never repeats guest/native effects; macro outputs revalidate per epoch; positive/negative/ambiguous selections invalidate correctly.
- Native optimized artifacts remain reusable through the explicit bridge; world mismatches do not trigger compilation or reflection; direct Rust ABI interchange occurs only for matched prepared native types.
- Tool/macro/native permissions and limitations are independently visible; no whole-request sandbox claim is made for native helpers.
- Direct producer ABI, resident ownership, both execution contracts, Cranelift optimized loops, required LLVM scalar/leaf imports, reconstruction and relocatable caches have actual macOS acceptance; unsupported broader IR classes are listed.
- Every T/H/M task including T085–T089 and every required MX/TR/NX/CA fixture has actual passing evidence; release notes distinguish implementation tests from the unbuilt prescriptions in this document.

### 59.6 Document validation versus runtime validation

This Markdown revision can be structurally checked without a compiler: headings/anchors, task/fixture identities, source references, fenced JSON examples, stale exclusions, and cross-section contracts. Such checks do not prove Rust source snippets compile, dependency pins resolve, the macro ABI matches, the solver matches Rust, or the JIT is correct. The implementation agent must execute the gates above and report failures directly. No runtime was built or benchmarked while producing this revision.

<a id="section-60"></a>

## 60. Primary sources for the expansion and trait revision

The algorithms, cache policy, limits, authority separation, and implementation tasks above are this specification's design decisions. Sources support the upstream interfaces and language constraints identified at their citations; they are not evidence that LRR has implemented those decisions. Source links to moving branches are explanatory; the build uses the exact recorded vendored/toolchain commits, not those branch tips. Sources were checked for this revision on October 4, 2026.

[^M01]: Cargo book, [compile-time-deps and unstable interfaces](https://doc.rust-lang.org/cargo/reference/unstable.html#compile-time-deps), and the [unit graph interface](https://doc.rust-lang.org/cargo/reference/unstable.html#unit-graph). Establishes the selected compile-time-only/graph interfaces and their unstable status; paired-toolchain behavior remains a capability gate.

[^M02]: Cargo book, [JSON messages and external tools](https://doc.rust-lang.org/cargo/reference/external-tools.html). Artifact freshness, build-script output messages, and the difference between cached reporting and actual execution.

[^M03]: Cargo book, [dependency resolver and feature contexts](https://doc.rust-lang.org/cargo/reference/resolver.html). Basis for exact unit/host/test feature distinctions rather than using a package-level feature union.

[^M04]: Cargo upstream, [check command implementation](https://raw.githubusercontent.com/rust-lang/cargo/master/src/bin/cargo/commands/check.rs), and rust-analyzer upstream, [build dependency integration](https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/project-model/src/build_dependencies.rs). Implementation evidence for command/test-intent/compile-time preparation behavior; pin and test it instead of importing private environment overrides.

[^M05]: Cargo book, [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) and [environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html). OUT_DIR, cfg/env directives, rebuild inputs, and per-compilation environment requirements.

[^M06]: rust-analyzer, [architecture](https://rust-analyzer.github.io/book/contributing/architecture.html), [procedural macro client source](https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/proc-macro-api/src/lib.rs), and [macro package dependencies](https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/mbe/Cargo.toml). Separate native macro server, private protocol, macro infrastructure, interner dependencies, and nondeterminism caveats.

[^M07]: Rust Reference, [procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html). Macro forms, token conversions, derives that emit arbitrary items, helper attributes, and compile-time native access.

[^M08]: Rust Reference, [method-call expressions](https://doc.rust-lang.org/reference/expressions/method-call-expr.html). Receiver ordering and type-parameter-bound lookup context. Runtime/source-view separation is LRR's implementation design for preserving those distinctions.

[^M09]: Rust Reference, [macros by example](https://doc.rust-lang.org/reference/macros-by-example.html), Rust edition guide, [2024 macro fragment specifiers](https://doc.rust-lang.org/edition-guide/rust-2024/macro-fragment-specifiers.html), and rust-analyzer [declarative expander source](https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/mbe/src/lib.rs). Matching, scope/hygiene, and definition-edition behavior.

[^M10]: cargo-expand project, [README and caveats](https://github.com/dtolnay/cargo-expand). Text expansion is a debugging aid, not a lossless executable interchange representation.

[^M11]: Rust compiler developer guide, [macro expansion](https://rustc-dev-guide.rust-lang.org/macro-expansion.html). Expansion/name-resolution/integration dependency model. LRR's conservative catalog seals are an independent design.

[^M12]: Rust Reference, [compound assignment expressions](https://doc.rust-lang.org/reference/expressions/operator-expr.html#compound-assignment-expressions). Primitive-before-substitution versus overloaded operand-order distinction.

[^M13]: Rust Reference, [associated items](https://doc.rust-lang.org/reference/items/associated-items.html). Associated type/const/method identities and generic associated type binder/predicate requirements.

[^M14]: Rust Reference, [trait and lifetime bounds](https://doc.rust-lang.org/reference/trait-bounds.html), and compiler developer guide, [trait resolution](https://rustc-dev-guide.rust-lang.org/traits/resolution.html) and [higher-ranked trait bounds](https://rustc-dev-guide.rust-lang.org/traits/hrtb.html). Header obligations and binder/leak-checking background; LRR does not claim the compiler's full borrow or well-formedness checking.

[^M15]: Rust Reference, [trait objects](https://doc.rust-lang.org/reference/types/trait-object.html) and [dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility). Object schema constraints; the checked guest representation is LRR-specific, not Rust native vtable layout.

[^M16]: Rust Reference, [impl Trait](https://doc.rust-lang.org/reference/types/impl-trait.html). Argument/return abstract identities and capture rules; the observed-path hidden-type check is LRR's explicitly relaxed execution policy.

[^M17]: Rust edition guide, [IntoIterator for arrays](https://doc.rust-lang.org/edition-guide/rust-2021/IntoIterator-for-arrays.html), [IntoIterator for boxed slices](https://doc.rust-lang.org/edition-guide/rust-2024/intoiterator-box-slice.html), and [match ergonomics](https://doc.rust-lang.org/edition-guide/rust-2024/match-ergonomics.html). Edition-sensitive behavior remains attached to source/token context rather than runtime type.

<a id="section-61"></a>

## 61. Compiler-described direct Rust ABI bindings

This chapter adopts the direct Rust ABI design as a release requirement. The default native transport is rust-abi-v1; codec-v1 is explicit. All direct calls use concrete prepared types and a matched compiler-produced function-pointer ABI. Native storage, leases, panic handling and exact-once effects are shared by both JIT backends and the interpreter. The scalar frame-view recipe below is a correct baseline; Section 62 additionally removes repeated canonical writes using reconstruction maps.

H005–H019 implement native configuration, preparation, ABI export, storage and call integration. T085–T089 implement the optimizing extension; NX011–NX030 and CA011–CA037 provide its required acceptance. Compatibility failures are preparation/runtime diagnostics with preserved evidence, never authority to select an unpinned backend or silently invoke rustc during a run.

### 61.0 Boundary and encoding inventory

Calling-convention lowering places typed values in registers/stack/return storage; it is not a serialization codec. Keep the following boundaries and their costs distinct. No transport switch occurs implicitly because a faster path is unavailable.

| ID | Boundary | Representation and when encoding occurs |
|---|---|---|
| B01 | JIT ↔ LRR helpers (Section 19) | In-process C ABI, fixed-width statuses/handles and trusted scratch/frame/map records. Baseline scalar staging copies bits; typed SSA removes redundant staging. No JSON or binary message codec per arithmetic operation. |
| B02 | JIT → native dependency (Sections 61.5–61.9) | Exact producer Rust pointer-call ABI or catch-adapter ABI. Register splitting, extension, aggregate copies and hidden returns follow that ABI. Native resident objects are not serialized; direct byte leases have no copyback. |
| B03 | Interpreter → native direct binding (Section 61.4) | Generated typed C slot invoker, or an accepted fixed scalar invoker selected from the prepared table. Specialization removes argument-slot arrays for measured common signatures; ownership/panic bookkeeping remains. No general request codec. |
| B04 | Explicit codec-v1 conversion binding (Section 39) | Retained only for requested conversion semantics; little-endian request/reply bytes and declared copies/writeback. Ordinary native operations use residents/direct calls. Opaque handles still address the common store. |
| B05 | Native registration/storage control (Section 61.4) | Versioned repr(C) records, bounded tables, handles and process-local pointers. Used at load, context/storage/lease/drop operations; these are typed control calls, not Rust-object serialization. |
| B06 | Producer ABI/layout sidecar (Section 61.3) | Address-free stable encoding during authorized preparation, validation and cache identity. Decode into a validated installed plan once; do not reparse metadata per call. |
| B07 | Public service and execution/effect IPC (Sections 25/42/56) | Public schema-3 JSONL; internal v4 headers with small JSON control and raw binary bulk frames. No internal hex/JSON expansion of effect or bulk bytes. IPC, authorization and bounded transactional publication remain. |
| B08 | Macro adapter/server (Sections 48–51) | Owned token/span/hygiene records plus the pinned external server protocol. Conversion/encoding occurs on expansion and revalidation, with separate authority; no token-string round-trip substitutes for it. |
| B09 | Identity/source/query persistence (Sections 6/41/51) | Stable hashed fields and explicit portable records, encoded at capture/export/import/invalidation. No raw query memory, local IDs, permissions or guest state is restored. |
| B10 | Optimizing compiler and compiled-code cache (Sections 22/62) | Bounded pointer-free compilation records, relocatable object bytes, maps and symbolic relocations. Encode at job/cache boundaries; bind process-local addresses only when installing executable code. |

B01–B05 are distinct in-process callable interfaces. B06–B10 are metadata or interprocess transports and must not be mistaken for the native function-call ABI. Report bytes/copies by boundary. Section 25.6 removes internal hex and JSON bulk expansion with its own protocol/authorization/size-equivalence acceptance. Section 61.15 requires boundary-specific measurements; direct-call adoption does not claim that process communication or real representation conversions vanished.

| Version domain | Revision-5 value | Compatibility rule |
|---|---|---|
| Public LRR service envelope | 3 | Reject 1/2; public JSON remains unchanged |
| LRR internal frame | 4 | Binary header/raw bulk; reject older peers; match role/build/handshake identities |
| Prepared project manifest | 5 | Explicit reprepare for older schemas |
| Native generation / native config | 2 / 2 | Reprepare generation; migrate old bindings explicitly to codec-v1 |
| Codec C control and byte wire | 1 / 1 | Preserved codec format; never interpret it as direct ABI metadata |
| Direct native control / producer ABI description | rust-abi-v1 / 1 | Exact producer and schema digest match |
| JIT helper and reconstruction-map contract | 2 | Old executable/map artifacts cannot load |
| Portable query checkpoint | lrr-portable-v2 | Reject v1 as a cache miss; require full build/profile/adapter/LRIR/schema compatibility |
| Macro server protocol | Matched pinned server tuple | Independent of the LRR service version; Section 48 owns admission |

### 61.1 Decisions and scope

| Area | Decision |
| --- | --- |
| Native compilation | Explicit preparation only; reuse Cargo-selected compatible artifacts and instantiate missing concrete generics in the generated preparation crate |
| Native entry | An address taken as an explicitly typed, monomorphic Rust function pointer; describe that pointer-call ABI |
| Discovery | Explicit native binding manifest; no arbitrary symbol lookup or discovery by demangled name |
| Compiler coupling | Exact producer compiler, target, configuration, exporter schema, and tested ABI lowering compatibility |
| Live frontend | Existing LRR frontend and queries; the compiler adapter runs only during native preparation |
| JIT backend | Cranelift 0.136.2 and producer-compatible pinned LLVM 23.1.1 under Section 62 |
| JIT policy | Section 20 baseline regions and Section 62 optimizing units, reconstruction maps and bounded concrete variants |
| Generic types | Concrete native types and primitive types; type and const arguments are fixed per prepared entry |
| Native objects | Opaque native-resident values with checked ownership; no general guest field access or construction from matching bytes |
| Raw direct calls | Generations compiled with `panic=abort`, within the admitted ABI subset |
| Recoverable panics | A prepared Rust-ABI catch adapter; no unwind through LRR JIT or host frames |
| Interpreter | Generated C-ABI slot invoker with measured fixed scalar alternatives; identical ownership and panic contracts |
| Loading | One immutable generation per worker; no unloading, hot patching, cross-generation objects, or persisted pointers |
| Initial platform | Rust 1.99.0 on `aarch64-apple-darwin`; each later platform needs its own acceptance |

Three different promises must remain visible:

1. A concrete specialization such as `dep::compute::<u64>` can have an ordinary native entry cost.
2. A specialization involving a prepared native `dep::Record` can use its real representation without serializing it, but ownership and borrows still need enforcement.
3. `dep::compute::<NewLiveRecord>` needs machine code and native trait implementations for that particular type. ABI metadata alone cannot supply them. This contract does not compile live units or equate live and native types.

### 61.2 Preparation and concrete specialization

The existing `.lrr/native.toml` remains the authority for bindings. Add a versioned `rust-abi-v1` binding transport. Existing codec bindings remain separately identifiable; selecting a direct binding never silently falls back to serialization.

Each direct declaration contains a stable binding name, dependency unit, fully qualified function or method path, concrete type and const arguments, argument/result native types and transfer modes. Effective panic behavior derives from the captured generation profile: abort selects the raw entry and unwind selects the catch adapter. There is no per-binding override that can make an unwind generation safe for a raw call. The restricted manifest grammar accepts the existing canonical type/path forms; it is not arbitrary Rust pasted into generated source. Rustc checks accessibility, generic arity, all required bounds, associated-type normalization, and signature agreement.

For example, a binding for `dep::mix::<u64>` with arguments `(u64, u64)` and result `u64` causes generated Rust equivalent to:

```rust
// Illustrative generated source; the actual symbol names are digest-derived.
type MixU64 = fn(u64, u64) -> u64;
static MIX_U64: MixU64 = dep::mix::<u64>;
```

Taking the typed address requests a callable specialization and makes its pointer-call contract explicit. The registration function references the static so that the linker must retain a callable target. Rustc may reuse an upstream instance, instantiate the generic downstream, or emit a required compiler shim. The exporter does not assume which one happened or reconstruct an internal symbol's calling convention.

The direct target for a raw binding is that function pointer's target, without an LRR wrapper body. If a compiler shim is required, its existence and cost belong to the generated-code report. Explicit associated paths select methods, including `<NativeType as dep::Trait>::method`; the managed trait dictionary is not passed to rustc-generated code.

Preparation follows this transaction:

1. Validate explicit build authority, native dependency closure, manifest syntax, owning editions, and concrete binding requests.
2. Generate the bridge crate, typed address declarations, interpreter invokers, native storage operations, and catch adapters where required. Embed a `BindingRequestDigest` covering the canonical requested bindings, generated template version, and expected exporter version. This digest is known before compilation.
3. Invoke Cargo with the existing matching profile and artifact-message workflow. An exact-version rustc-driver adapter wraps the generated bridge's compiler invocation. Other units use the ordinary selected compiler.
4. During that invocation, inspect the generated declarations after type checking and normalization. Query the function-pointer ABI and native type layouts; write a bounded canonical sidecar into a temporary generation directory. Continue normal code generation and linking.
5. Emit a receipt that ties the sidecar's ABI schema digest to the compiler invocation, generated input digest, and embedded `BindingRequestDigest`. Exporter output uses an invocation-specific temporary path and is accepted only if that exact compiler invocation and its link succeed. There is no second metadata-only compilation and no post-analysis modification of Rust constants.
6. After successful linking, bind the sidecar, request digest, library content hash, Cargo provenance, compiler/exporter identity, and input fingerprint in one manifest. Run the preparation validation probe in a disposable authorized process. Atomically publish the complete generation. The ABI digest is authenticated by this manifest and receipt; the library reports the earlier request digest, not a circular digest of its own compiled ABI or bytes.

An exporter/compiler failure publishes nothing. A fresh Cargo bridge artifact is reusable only with its matching exporter sidecar and unchanged exporter identity. Missing metadata requires explicit re-preparation of the generated crate, not inspection of stale incremental shards. An unchanged dependency `.rlib` can stay fresh even when new generic machine code is emitted in the generated crate. Report these separately.

Use a coordinator-selected Cargo compiler wrapper that recognizes only the generated bridge by its canonical manifest/input identity. It delegates every other compiler invocation unchanged to Cargo's selected compiler. For the bridge it runs the matching rustc-driver adapter with the captured arguments and configuration. Wrapper-induced Cargo fingerprint changes can reduce reuse and must appear in the artifact report; this design does not promise that every previously built dependency will remain fresh.

The adapter needs the matching compiler development libraries or a reproducibly built compiler adapter. The current minimal toolchain configuration does not establish their availability. An adapter mismatch is an actionable preparation failure, never permission to substitute a nearby compiler or decode `.rmeta` in the runtime.

#### 61.2.1 Why metadata must describe a function pointer

Internal optimized symbols can acquire compiler-specific details. A source `fn(A) -> R` signature is insufficient to call such a symbol. Address-taking produces a callable Rust function-pointer target, including rustc's necessary reification shim. Export the ABI for that exact normalized pointer type and require a differential test against a Rust caller using the very same pointer.

`#[track_caller]` needs particular care: pointer coercion can introduce a shim and change the visible caller location. The initial schema rejects bindings whose selected entry needs caller-location behavior; do not claim the JIT source location survives automatically. Native calls deeper inside an admitted function retain their ordinary compiled behavior. Supporting caller propagation later requires a separate explicit location contract and tests.

### 61.3 Artifact schema and identity

The on-disk ABI description contains no addresses. Use the project's explicit stable encodings: fixed-width little-endian numbers, explicit enum tags, checked lengths, sorted records, and domain-separated digests. Never persist rustc `Debug` output, `DefId`, `TypeId`, raw Rust structs, or serde layout as an ABI contract. JSON may render a human-readable inspection copy; it is not the canonical encoding.

Version 1 imposes a 16 MiB ABI-description limit, at most 16,384 bindings, 16,384 type records, 64 source arguments per binding, 256 lowered machine components per signature, and 64 levels of structural description. Arithmetic overflow, excessive lengths, unknown mandatory fields/tags, and duplicate identifiers reject the generation. Cyclic type references use stable type IDs; the encoding does not recursively expand pointers. Existing run memory/object limits also apply; these schema limits do not authorize larger native allocations.

| Record | Required contents |
| --- | --- |
| `ProducerKey` | Full rustc commit/build identity, compiler/sysroot hashes, backend identity, target specification digest, target triple, CPU and enabled features, pointer width, endianness, relevant Cargo/profile/rustflags inputs, panic strategy, exporter binary/schema identity |
| `NativeTypeKey` | Native owning unit identity and producer-canonical nominal definition identity with normalized concrete type/const substitutions; aliases normalize to the same key, and names alone never establish equivalence |
| `NativeTypeLayout` | Size, alignment, backend scalar/aggregate classification, scalar validity ranges where relevant, initialized component ranges, field/variant/niche facts needed by the admitted lowering, and required storage/drop entry IDs |
| `NativeEntrySchema` | Binding ID, normalized pointer signature, per-argument and result type IDs, transfer/borrow modes, panic entry kind, `FnAbi` translation, and all required target features |
| `MachineCallAbi` | Calling convention; ordered machine arguments/results; integer extension; split/cast component offsets and widths; ignored arguments; indirect argument rules; hidden return pointer; stack argument requirements; pointer address spaces; unwind policy |
| `NativeGenerationManifest` | Above schema digest, native library hash, prepared input fingerprint, dependency artifact provenance, schema version, and preparation receipt |
| `PreparedNativeCall` | Pure binding/type/ABI identities, argument evaluation and transfer recipe, result storage recipe, call-site origin, and normal/panic continuations; no address |
| `InstalledNativeCall` | Worker-local generation owner, validated target address, lowered signature, and prepared recipe; never portable |

Keep the LLVM/compiler ABI description distinct from the Cranelift lowering plan. Preserve correctness requirements even when Cranelift does not expose corresponding optimization attributes. Ignoring an optimization hint can be conservative; ignoring a register, extension, storage, or ownership requirement is an ABI bug.

Type keys do not contain the enclosing schema digest: that would create a hash cycle. Producer-scoped stable definition paths identify nominal types, including private types exposed through a public alias; runtime `DefId` indexes are never serialized. Layout records refer to type keys, and the enclosing schema digest binds their actual descriptions. Runtime type identity is the pair `(GenerationId, NativeTypeKey)`.

`GenerationId` is exactly the Section 38.1 GenerationIdentity digest: it binds linked bytes and stable producer/schema/input identities, excluding its own field, derived output paths and per-attempt observations. Exporter receipts separate stable provenance from temporary paths/times; the latter never perturb this identity. Source signature caches may survive a body-only native change, but installed calls, native object identities, and JIT artifacts always belong to one exact generation. A worker holding generation A can never install generation B.

### 61.4 Registration and loading

Keep a small C-ABI control surface for library registration and native storage management. Rust ABI calls are the execution surface. Registration returns bounded tables of entry IDs, process-local callable addresses, type operation addresses, and the embedded binding-request digest. Raw pointers in this table are trusted native capabilities admitted only by the loader; no guest operation can supply or construct them.

The loader validates the immutable generation manifest before opening the library, then validates the table count, IDs, digest, null entries, target compatibility, and correspondence to the admitted schemas. Loading itself may execute native initializers and therefore still requires native authority. Hashes establish artifact matching, not safety of arbitrary native code.

The runtime converts a registered callable address only under its matched signature. Function/data pointer representation assumptions are part of the macOS ARM64 acceptance; pointer bytes are never sent over the service protocol. Do not infer validity just from a plausible executable address or require that every target lies inside the bridge image: a legitimate retained pointer can refer to a linked dependency image.

JIT imports use an internal typed `NativeEntryId` registry tied to the generation. No source spelling becomes a symbol lookup string. Continue disabling all ambient symbol fallback. Where Cranelift's relocation needs a range-extension stub or an indirect call, record it; a direct source binding does not guarantee a single hardware direct-branch instruction.

#### 61.4.1 Control interface responsibilities

Use one versioned registration symbol, `lrr_native_rust_abi_v1`, with a C-compatible size/version-checked output table. This name and the new transport distinguish it from revision 4's codec interface. Stable C fields contain fixed-width integers, bounded byte pointers, opaque context pointers, and C function pointers. The Rust callable address is an opaque pointer in an entry record, never a C function pointer with a guessed signature.

| Control operation | Contract |
| --- | --- |
| Register | Return immutable entry/type-operation tables and binding-request digest; no guest values or effects |
| Create context | Establish run epoch, empty native slot bookkeeping, and one reusable panic state for synchronous calls |
| Reserve storage | Allocate uninitialized storage for one registered native type with checked size/alignment; return no initialized value |
| Interpreter invoker | Per-entry typed C function accepting trusted argument/output slots, with optional accepted fixed scalar class; construct typed arguments under the declared transfer contract and call the admitted raw or catch entry |
| Inspect caught state | Return a bounded C-compatible panic/status view; do not expose Rust `String`, enum or panic payload layout |
| Reset caught state | Destroy native payload resources under the contained-panic policy before reuse |
| Drop value | Consume an initialized registered type exactly once using native Drop; report a contained panic where the generation permits it |
| Deallocate storage | Free already consumed or uninitialized storage without invoking user Drop |
| Destroy context | Require no outstanding initialized values, leases, or pending panic resources; never substitute for ordered guest cleanup |

Argument slots for the interpreter contain pointers to typed native storage or validated scalar scratch and the declared transfer mode. They are constructed by the trusted runtime, not read from guest bytes. Each invoker has its concrete argument count and types compiled into its source. It uses ordinary typed Rust calls; there is no general runtime implementation of the Rust ABI in the C shell.

The reusable panic state is valid because calls are synchronous and native-to-guest callbacks are forbidden. State inspection/reset must complete before the next entered call. Any new reentrant execution design must replace this single-state contract. Abort scalar calls need no per-call native context operation or result allocation.

#### 61.4.2 Measured scalar interpreter invokers

Keep the typed slot invoker as the general interpreter path. Explicit native preparation may additionally emit bounded, fixed C-ABI scalar invoker classes for common concrete signatures. The initial candidates are U64BinaryV1 and F64BinaryV1: respectively two u64 or two f64 source arguments and the same scalar result. Their exact signatures are `extern "C" fn(context_ptr, u64, u64, out_u64_ptr) -> u32` and `extern "C" fn(context_ptr, f64, f64, out_f64_ptr) -> u32`. The context is opaque native control state; the output points to aligned trusted scalar storage and is initialized only on success. They use the contained outcome codes in Section 61.7: 0 success, 1 caught panic, 2 fatal failure; unexpected status is an infrastructure fault. Abort-mode panic still terminates the process.

The generated function calls its already prepared concrete Rust target/catch entry using ordinary typed Rust. The runtime selects only a registered class whose signature, producer/generation, binding and panic policy match; it never synthesizes arbitrary C/Rust signatures, loads an arbitrary symbol, or invokes a generic foreign-call engine. Validate context, scalar type/initialization and continuation before entering, with the same fuel, cancellation and exact-once transaction. These candidates eliminate the argument-slot array and its construction; they do not remove scalar extraction, the C call, output storage or panic handling. Output storage may be reused only after its previous result has been consumed.

The registration schema lists each optional scalar class/entry beside the mandatory slot entry. Its tagged signature and template identity participate in the native schema/generation digest; missing class means use the typed slot path, never an ABI guess. Adding a class requires explicit preparation and its own declaration/admission tests. Baseline and specialized invokers must call the same prepared target with the same inputs and semantics.

NX029 compares actual slot and scalar invocation on the interpreter (jit=off), including return values, floating-point bit behavior, wrong-class rejection, cancellation/fuel and abort/catch outcomes. Retain paired cold and steady-state time, allocation/copy counts and code-size measurements. Enable a class for production selection only when its measurements establish a benefit without semantic regression; otherwise keep the slot path and report the failed/inconclusive optimization. Investigating both initial classes and retaining the result is required; an unmeasured special path is not a required default or a speedup claim.

### 61.5 Admitted ABI subset

Version 1 admits ordinary synchronous Rust function pointers, monomorphic after native preparation, on macOS ARM64. The lowering must implement each accepted `PassMode` from the producer schema rather than choosing a convention from the spelling of the source type.

| ABI case | Version 1 treatment |
| --- | --- |
| Zero-sized argument/result | No machine argument where the producer says `Ignore`; ownership and Drop effects still occur |
| Integer through 64 bits, bool, char, f32, f64 | Producer-defined register type and extension; bool/char validity checked before admission |
| Scalar pair | Two producer-ordered components, with actual offsets and validity requirements |
| Sized aggregate cast | Implement explicit producer component offsets/widths and target classification; reject unsupported padding/register forms |
| Indirect sized argument | Implement its precise pointer/by-value/stack rule; never substitute a pointer merely because an aggregate is large |
| Indirect sized return | Reserve aligned native result storage and pass the producer-required hidden pointer |
| Native `&T` and `&mut T` | Leased initialized native storage, no escaping returned borrow |
| `&[u8]`, `&mut [u8]`, `&str` | Explicit slice/string contracts with real data/length ABI components and call-scoped backing leases |
| Nested borrowed values, borrowed results, unsized by-value arguments, arbitrary DSTs | Reject before entry |
| Trait objects, live closures, live function callbacks, async/future entry points | Reject; native implementations may use them internally |
| SIMD, i128/u128, unusual address spaces, variadic, special calling conventions, foreign exceptions | Reject in version 1 |

Rejecting an unsupported **signature** does not forbid a native function from using those features internally. A native library may use vectorization or i128 arithmetic behind an admitted `fn(u64) -> u64` entry.

Each lowering rule is checked against the producer rustc using identical test arguments and the same callable pointer. For aggregate padding, copy only ranges required by the ABI using operations that permit uninitialized padding; never materialize padding as a valid Rust scalar, hash it, or use it in a branch. If the pinned Cranelift API cannot express the required operation correctly, reject that ABI case until it can. Full arbitrary Rust ABI coverage is not inferred from passing scalar calls.

### 61.6 Native storage and ownership

NativeResident is the storage contract for ValueKind 14 (NativeObject), not a second value kind or nominal type universe. It describes an opaque, fully initialized value stored using the generation's layout and allocator. Its managed representation is an epoch-checked ownership handle. It is distinct from a managed struct and from the host runtime's own `String` or `Vec`.

Every slot records generation/type identity, storage pointer, size/alignment, initialization and ownership state, and active borrow state. Addresses remain stable while borrowed. The generation provides C-ABI allocation/deallocation and type-specific drop operations so that allocation and destruction remain paired with their originating native runtime. Raw storage allocation does not initialize a `T`.

The slot state machine is:

```text
ReservedUninit -> Live -> BorrowedShared(count) -> Live
                       -> BorrowedExclusive -> Live
                       -> Moving(call_id) -> Consumed
                       -> Dropping -> Dead
```

Borrow transitions are reversible only on normal call completion or an admitted catch result. Moving becomes committed immediately before native entry. The callee owns a by-value argument from that point, including during native unwinding. LRR must never drop the transferred value again. Source storage can be deallocated after the call without dropping its consumed contents. Uninitialized result storage becomes `Live` only after a successful return; reserve its handle and accounting before entry.

Version 1 treats opaque native values as non-Copy in the guest, even when rustc says their type is Copy. Copy/Clone requires an explicit native operation. Native constructors initialize slots; methods borrow them; by-value calls transfer them. No guest field writes, synthetic discriminants, transmute, or reconstruction from same-looking live structs are permitted. This gives efficient opaque generic containers without pretending LRR implements every native representation invariant.

Owned `String`, `Vec<T>`, enums, and generic structs can use this storage contract once their particular ABI forms pass acceptance. Their public methods are bound explicitly. Converting a managed string into native `String` is an explicit allocation/copy operation; repeated native operations can retain the converted object. Do not hide that conversion in a claimed zero-copy call.

Normal destruction removes ownership before invoking native Drop and occurs in the existing guest drop order. Drop uses the generation's panic policy, including double-panic termination. Fatal guest faults retain the current policy of terminating the worker without running further user Drop code. Storage destructors in host RAII must not bypass this rule.

#### 61.6.1 References and byte buffers

Before native entry, acquire leases in deterministic backing-object order. Shared native references may overlap. A mutable reference excludes overlapping shared or mutable access; disjoint byte ranges may receive separate mutable leases. Opaque native objects use whole-object leases in version 1, so field-disjoint borrowing is not guessed from layout metadata.

For managed byte buffers, pin contiguous initialized backing storage for the call. Check bounds, mutability, overflow, and UTF-8 for `&str`; no resizing, relocation, concurrent guest access, or guest callback is allowed while leased. Generate real references only after the leases exist. Native writes are already in the backing storage on return or caught panic, so no copyback is required. Invalid or unpinnable storage is a pre-entry diagnostic for a direct binding; any copying adapter is a separately declared binding.

Zero length does not remove Rust's pointer validity and alignment requirements. `&mut [u8]` may modify bytes without violating byte validity. General mutable typed guest slices remain unsupported because native code may replace elements whose validity, drop, and aliasing contracts LRR does not own.

References to native values are call-scoped and non-escaping. Require native resident types to satisfy `'static` as owned types; this does not make a temporary borrow of such a value `'static`. Unsafe native implementations remain trusted. No lifetime erasure, metadata hash, or alias guard proves arbitrary unsafe dependency code sound.

### 61.7 Panic policy and callable entries

#### 61.7.1 Abort generations

A generation built under a verified `panic=abort` profile uses the typed pointer to the specialization directly. The dependency closure must satisfy the selected compiler's linking/panic-strategy requirements. Rust panics terminate the worker and report native process failure; they cannot pass a guest expected-panic test.

The admitted bindings cannot introduce foreign unwinding through JIT frames. As with other native unsafety, unsafe native code violating that contract is outside managed guarantees. An ordinary function signature is not evidence that a function never panics, and an `unwind` generation cannot opt into this raw path merely because its author expects success.

#### 61.7.2 Unwind generations

Generate a monomorphic **Rust-ABI** adapter around each selected call. Its schematic signature is:

```rust
// Schema notation, not a source implementation.
fn caught<A, R>(state: *mut NativeCallState, out: *mut MaybeUninit<R>, arg: A) -> u32
```

The actual adapter has concrete arguments, and its own function-pointer ABI is exported. State and output storage are allocated/reserved before entry. The adapter catches panics from the target and owned argument cleanup inside the native generation, writes `R` only on success, and returns `0 = success`, `1 = caught panic`, or `2 = fatal native post-call failure`. All unexpected statuses terminate the worker. Scalar results also use reserved output storage in this first catch form; its cost is measured separately.

The panic state carries a bounded string/static-string description or a fixed opaque-payload marker. Payload destruction and adapter cleanup must themselves be contained or abort the process; no Rust unwind may escape the adapter. `AssertUnwindSafe` applies only to this audited state machine and does not imply application-level rollback. Mutated borrowed objects remain mutated. A result is never adopted on panic, and consumed arguments are never restored.

This adapter may inline the target during preparation. It still has status/output handling and exception machinery, so it is not labeled the raw direct path. Unwind tables and cleanups for arbitrary LRR JIT frames are deliberately unnecessary for this design.

### 61.8 Call execution and exactly once semantics

Both tiers use one semantic state machine:

```text
Resolve -> Evaluate -> Validate -> Reserve -> Lease -> Ready
                                                       |
                                                    Entered
                                                       |
                                   +-------------------+----------------+
                                   |                   |                |
                                 Success          CaughtPanic      ProcessFailure
                                   |                   |                |
                              AdoptResult        RecordPanic         terminate
                                   |                   |
                              ReleaseLeases       ReleaseLeases
                                   |                   |
                              PublishNextPC       GuestCleanup
```

Resolve the declared binding before argument evaluation under the existing missing-callee rules. Evaluate left-to-right exactly once. Validate concrete types, generation identity, ownership tickets, ranges, and borrows. Reserve result storage, result handles, continuation and panic bookkeeping before the effect. Check fuel/cancellation and commit the caller's canonical continuation, transfers, and `Entered(call_id)` before invoking native code.

Once `Entered` is set, no error or guard miss can re-execute this call. A post-return descriptor/continuation mismatch resumes at the post-call continuation only after publishing the already completed result, or terminates as an internal failure. It cannot deopt to the call opcode. A signal/timeout after entry reports `may_have_executed` and terminates the worker. Native calls remain atomic with respect to guest scheduling; cancellation does not interrupt and resume arbitrary native instructions.

Direct calls and interpreter invokers use the same transfer, lease, fuel, panic, and continuation contracts. The interpreter's generated `extern "C"` invoker takes validated slot descriptors, calls the statically typed native entry, and deposits its result into reserved native/scalar storage. It does not deserialize a general request. The runtime never uses a transmute to invoke an unknown Rust signature from Rust source; only generated typed code or accepted JIT lowering performs that invocation.

### 61.9 JIT integration and the scalar fast path

The canonical-cell/frame-view lowering below is the direct-call baseline. Optimizing units use Section 62 typed SSA and exit maps instead of mandatory per-op physical stores; both preserve this logical entry/ownership protocol.


Add `NativeCallOp` to LRIR with a pure `PreparedNativeCall` identity. The recorder may include a successfully completed synchronous native call and its subsequent same-body operations within a baseline 64-op region. Native calls count as one guest operation. Guest calls, control-flow exits and cleanup remain baseline boundaries; Section 62 governs reached-call inlining, loops and reconstruction in optimizing units. No native implementation body enters LRIR.

Promotion still requires real completed observations. Recording a successful call does not prove subsequent calls cannot panic or mutate state; generated code includes all defined return paths. No speculative calls, replay-based warmup or native callback to the guest is permitted. The optimizing tiers follow Section 62 and cannot prepare an unentered live path.

Two lowering forms share the state machine:

| Form | Entry and completion work | Intended use |
| --- | --- | --- |
| Checked direct call | Shared preflight helper acquires tickets/leases and reserves storage; JIT emits matched Rust ABI call; shared completion helper adopts result and releases leases | Owned objects, references, aggregates, catch adapters, unresolved scalar materialization |
| Scalar direct call | JIT emits bounded inline preflight and commit against a verified frame view, with no per-call helper | Initialized primitive scalar inputs/results in abort generations |

The checked form removes codec and dispatcher costs but retains helper calls. It must not be advertised as equivalent to a bare scalar call.

#### 61.9.1 Verified frame view

The scalar form requires an explicit extension to the current helper-only memory model. At region entry, a versioned helper validates the frame, semantic epoch, region recipe, generation, and expected slot indexes, then fills a trusted `NativeRegionView`. It supplies pointers only to the region's admitted canonical scalar cells and bounded execution-state fields. It never exposes an arena base for unchecked indexing.

The view schema includes a version/digest, scalar slot count and stable addresses, expected descriptors, fuel/cancellation access, canonical PC and pending-native-call fields, and a native-entry counter. Host/JIT sizes, alignments and offsets are mechanically checked. Cancellation uses the host's atomic representation and required ordering; a plain load from an atomic field is not an acceptable substitute.

Introduce `native_region_enter_v2(ctx, frame, region_id, view_out) -> status`, `native_call_prepare_v2(ctx, frame, op_id, scratch_out) -> status`, and `native_call_finish_v2(ctx, frame, op_id, outcome) -> status` in the closed helper registry. Their pointer parameters refer only to compiler-sized/aligned trusted stack storage. The first is the scalar-form entry contract; the latter two implement the checked form. `prepare` returns Continue only after publishing the canonical entered state and furnishing all machine inputs; generated code then invokes the one admitted target immediately. A non-Continue return never leaves an entered-but-uninvoked call resumable. `finish` accepts only the pending call's outcome and can never request retry.

The exact fixed-width field layouts of the view and scratch are versioned host/JIT schemas with golden offset tests, separate from the producer Rust ABI schema. They are not raw views of Rust's `VmContext` or arbitrary frame structs. Native dependency entries never receive these helper structures.

Frame scalar cells must have fixed, pinned backing storage while this view exists. Suspension, reallocation, frame replacement, or any helper that cannot preserve the view ends the region. Host Rust code must hold no overlapping active references while JIT stores through the view. The synchronous native dependency has no route to the VM; callbacks and concurrent guest access remain forbidden. The view is invalid on every region exit and is never cached across invocations.

For each scalar call, emit this sequence:

1. Guard the concrete binding/substitution and initialized scalar descriptors unless immutable region facts already prove them. Literal materialization and shape misses leave the opcode unentered for the checked path.
2. Read scalar bits from admitted canonical cells or reuse SSA values known equal to their current contents. Validate bool/char ranges when not already established. Apply producer-required extension/truncation rules.
3. Check cancellation and, in precise, available fuel at the prescribed pre-entry point. Charge once in precise; store the canonical continuation, entry state and native-entry counter. These stores cannot move after the call.
4. Call the fixed generation entry using its lowered ABI. Treat the call as effectful and as a memory clobber for native-visible state; never mark it pure based on its scalar signature.
5. Write the scalar result and descriptor into the canonical destination, clear pending entry state, and advance the PC. Subsequent baseline operations can reuse the result in SSA while canonical state stays current. Optimizing units use Section 62 reconstruction maps and may defer that physical store until an observable exit.

If a destination requires dropping an old owning value, or any required fact cannot be established, use the checked form. Do not make guarded native calls eliminate source-level ownership or effect checks. All existing resource charges must match the interpreter; no batching of fuel across an observable call.

Guard hoisting is limited to facts whose dependencies remain immutable for that region. The scalar baseline does not hoist a mutable borrow across another call, infer purity, or eliminate alias checks from type names. Section 62.6 permits an interval lease across accepted operations only with the required alias/lifetime proof. Native object fast paths beyond the checked form need a later accepted ownership proof and their own performance evidence.

This form removes per-call dispatcher and helper transitions for eligible scalar calls. It still performs required guards and canonical bookkeeping. If those dominate tiny functions, the end-to-end almost-zero-overhead objective is unmet even when the raw ABI probe passes. Section 62 supplies reconstruction and explicitly selected accounting contracts; benchmark both raw ABI and the complete execution path.

### 61.10 Generic behavior and user-visible limits

| Requested use | Behavior |
| --- | --- |
| `dep::f::<u64>` already declared | Resolve the prepared entry; hot calls carry no generic dictionary |
| New concrete native type/const combination | Produce an exact missing-specialization preparation request; no runtime compiler process |
| `dep::Container<NativeRecord>` | Native resident object with explicit constructors/methods/drop and accepted ABI cases |
| Associated types or native trait methods | Rustc resolves them during preparation; schema records concrete normalized signature and exact trait selection |
| Generic live caller reaches different concrete arguments | Select the matching prepared binding through checked dispatch; the baseline guards its admitted shape and Section 20 permits up to four optimized variants; a miss never calls the wrong specialization |
| Native algorithm wants a trait implementation defined only on a live type | `LRR-E-NATIVE-WORLD-MISMATCH` before entry |
| Native algorithm calls a live closure/comparator | Unsupported callback boundary; a native wrapper containing its own native closure may be bound |
| Live edits change only argument values or caller body | Reuse the immutable generation if its prepared inputs remain valid |
| Live edits introduce a new native specialization | Preserve prior effects and stop with the preparation request; no automatic prepare and replay |

There is no finite ahead-of-time artifact containing every possible instantiation of an arbitrary Rust generic. This design buys low repeated-call cost by paying concrete instantiation cost during explicit preparation. It does not turn a native generic declaration into a universal runtime implementation.

### 61.11 Diagnostics and telemetry

Retain existing missing-binding, world-mismatch, stale-generation and native-process-failure categories. Add typed outcomes for producer mismatch, unsupported ABI, unsupported signature, invalid native value, native borrow conflict, and unsupported panic entry. Distinguish exporter/schema incompatibility from a user type error and from a backend lowering bug.

Every reached failure includes binding identity, concrete type/const arguments, producer/generation identity, phase, whether entry occurred, and the exact supported preparation remedy. Do not print memory addresses or arbitrary native payloads as diagnostics.

Report counters separately: prepared specializations; reused dependency artifacts; raw Rust-ABI calls; caught-adapter calls; interpreter invoker calls; checked/scalar direct JIT calls; pre-entry guard misses; post-call exits; native-object transfers; borrow-lease acquisitions; explicit conversion bytes; and actual codec calls. A direct-call counter requires an actual completed machine-code path, not just CLIF generation. Entry counters separately record calls that fail to return.

### 61.12 Performance experiments and decision gates

Measure these paths with the same compiler profile, prepared generation, inputs, and result consumption:

| Baseline | What it isolates |
| --- | --- |
| Rust loop through a runtime-loaded typed function pointer | Ordinary out-of-line call to the same address; compiler must not devirtualize or inline it |
| Cranelift loop through that address | Raw ABI lowering, call sequence and register effects |
| Typed C entry for the same specialization | Whether Rust ABI materially improves on design 1 |
| Checked direct call in an actual LRR loop | Ownership/helper and managed execution costs |
| Scalar direct call in an actual LRR loop | Remaining fuel, guard, canonical-state and region-exit costs |
| Fully optimized Rust caller with inlining allowed | Lost inlining and caller/callee optimization opportunity |
| Existing codec path when implemented | Cost removed by this design |

Run tiny arithmetic generics, large computations, methods on `Container<u64>`, by-value aggregates, zero-copy slice kernels, and a prepared native trait-bound function. Include a many-call workload whose function does little work so batching cannot hide overhead. Run abort and catch paths separately. Vary arguments and consume results; use dependency chains for latency and independent calls for throughput. Inspect disassembly to confirm identical target identity, no constant-folded benchmark, actual call retention, no hidden codec, and expected stubs/spills.

Collect at least 50 independent warm sample batches per scenario, each long enough to avoid timer-resolution dominance. Randomize paired path order and record raw times, iterations, CPU/OS, compiler identities, profile, thermal/run conditions, median, p95 and uncertainty. Report cold preparation, loading, JIT compilation and steady execution separately. A bootstrap interval must resample paired independent batches, not treat every loop iteration as independent evidence.

The raw-ABI performance-claim acceptance target is at most 5% median overhead against the matched Rust pointer-call loop, with the upper 95% confidence bound also at most 5%. When measurement uncertainty prevents that conclusion, report inconclusive and improve the experiment; do not relabel it passed. This is a design target, not a prediction or a nanosecond guarantee.

Apply the same 5% target to a full LRR workload before describing that workload as almost overhead-free compared with native Rust. Report absolute ns/call and all failed cases, especially tiny calls. The raw ABI gate can pass while the integrated gate fails. If design 1 has equivalent results, retain it as an honest lower-maintenance comparator; if native ownership checks dominate, changing the calling convention again will not fix them.

### 61.13 Producer acceptance and provenance

The following compatibility gates are mandatory before publishing a callable artifact:

| Gate | Evidence required | If it fails |
| --- | --- | --- |
| Matching compiler adapter | Exact Rust 1.99.0 producer exports the pointer ABI and issues a sidecar receipt for the actual code-generation invocation | Preparation unavailable for that producer; no fallback compiler |
| Pinned backend compatibility | Cranelift 0.136.2 passes producer-matched ARM64 differential tests for every admitted ABI class | Restrict the explicit ABI subset or record a witnessed backend issue; never guess |
| Frame view soundness | Stable storage, no conflicting host references, correct atomics, canonical state, and invalidation tests | Keep checked direct form; do not claim scalar fast-path performance |
| Performance | Raw and integrated results against all named baselines | Report which objective failed; require a new explicit design decision before changing semantics |

On October 13, the installed compiler reports `rustc 1.99.0`, commit `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, LLVM 23.1.1, host `aarch64-apple-darwin`. The local reference checkout is at `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, and its `rustc_codegen_cranelift` manifest requests Cranelift 0.135.0; this project pins 0.136.2. These are not a matching producer/backend acceptance result.

The existing project JIT ABI file contains synthetic helper/scalar probes. The production native bridge and direct-call design are not established by those probes. Revision-5 ABI execution, safety and performance claims require new receipts; document review and historical bootstrap probes cannot satisfy them.

### 61.14 Source references

- [Native integration](lrr_rust2024_macros_traits_implementation_spec.md#section-35), [execution boundary](lrr_rust2024_macros_traits_implementation_spec.md#section-40), [JIT integration](lrr_rust2024_macros_traits_implementation_spec.md#section-41), and [native/live worlds](lrr_rust2024_macros_traits_implementation_spec.md#section-55) define the related normative contracts.
- [Local rustc ABI structures](rustc/rust/compiler/rustc_target/src/callconv/mod.rs), [ABI queries](rustc/rust/compiler/rustc_ty_utils/src/abi.rs), [Cranelift signature lowering](rustc/rust/compiler/rustc_codegen_cranelift/src/abi/mod.rs), and [pass-mode lowering](rustc/rust/compiler/rustc_codegen_cranelift/src/abi/pass_mode.rs) are implementation references at the checkout commit above, not the ABI specification of the installed compiler.
- [Rust compiler monomorphization guide](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html) explains concrete instantiation and downstream generic code generation.
- [Rust ABI reference](https://doc.rust-lang.org/reference/items/external-blocks.html#abi) states that the Rust ABI has no stability guarantee.
- [Rust type layout](https://doc.rust-lang.org/reference/type-layout.html) and [validity rules](https://doc.rust-lang.org/reference/behavior-considered-undefined.html) establish why matching size/alignment alone does not admit a native value or reference.
- [Function unwinding rules](https://doc.rust-lang.org/reference/items/functions.html#unwinding) establish why recoverable native panics need an explicit contained boundary.

### 61.15 Required encoding and copy reduction priorities

Within the fixed Section 57 task order, apply these priorities to the affected components; they do not authorize starting guest JIT before interpreter acceptance:

1. H005–H019: keep ordinary native values resident and use direct bindings. Expose conversion operations explicitly and count their allocations/bytes; do not add serialization to every call because one boundary needs conversion.
2. T093/H021/H023/M041/M045: implement internal v4 raw bulk transport before claiming optimized worker/effect/cache transfer. Public JSON and the matched macro-server protocol remain intact. Cover the Section 25.6 malformed, stale, authority, cancellation, quota and exactly-once controls.
3. H019/T111: measure and admit the fixed scalar interpreter invokers in Section 61.4.2. A lack of benefit retains the correct slot path; neither result starts guest JIT.
4. T087–T089: eliminate eligible native call boundaries using prepared LLVM IR as already required by Section 62. Count actual inlining separately from ABI calls and preserve profile/ownership/effect behavior.
5. M004–M016/H022/T111: profile macro/metadata copying before changing it. Reuse immutable owned token/span backing and decoded metadata within their valid identity/epoch; avoid rebuilding identical serialization buffers or reparsing ABI descriptions per call. Never retain pointers into a mutable upstream interner, omit hygiene, skip a required macro revalidation/invocation, infer authority from a cache hit, or equate epochs. The external server protocol stays pinned.

Report logical bytes, actual wire bytes (including headers), copied bytes, allocations and decode/encode time by B01–B10 boundary. Compare old internal hex transport only in an isolated benchmark/reference encoder, never as a production fallback. Use actual native-target invocation and controlled effect/macro outputs to verify equivalent work. NX029/NX030 and CA023–CA026 are pending acceptance requirements, not completed tests or guaranteed gains. These measurements supplement the native ABI/LLVM benchmarks rather than replacing them.

<a id="section-62"></a>

## 62. Optimizing execution and throughput contracts

This chapter is the required optimizing execution architecture. It extends the correct direct-call baseline in Section 61 with typed SSA, hot loop graphs, reconstruction maps, bounded specialization, an LLVM hot tier, asynchronous optimization and relocatable code reuse. The live frontend remains independent and reached-path lazy. No source-level type, ownership, fault, effect or capability guarantee may be inferred solely from a native representation.

The initial required LLVM import class is abort-mode scalar leaf functions with no state/allocator/TLS/unwind/address-identity dependencies. Broader native IR imports are explicitly outside initial release acceptance until separately specified and exercised; unsupported imports retain their valid direct-call binding. LLVM lowering of eligible reached LRR scalar units and this initial native import class are required, even if auto mode declines to promote an unprofitable workload.

### 62.1 Priority and expected benefit

Implement the capabilities below in the Section 57 task order. Relative payoff is measured, not assumed.

| Priority | Change | Cost it addresses | Main condition |
| --- | --- | --- | --- |
| 1 | Optimize typed IR and enable Cranelift speed optimization | Unoptimized code, redundant work and register spills | Existing semantics represented explicitly in IR |
| 2 | Compile complete hot loops with native backedges | Dispatcher/region entry on every iteration | Exit maps and correct interruption/ownership behavior |
| 3 | Keep values in registers and reconstruct interpreter state at exits | Repeated tagged-cell loads, stores and per-op helper calls | Complete state reconstruction maps |
| 4 | Specialize hot concrete type combinations and reached callees | Generic dispatch, repeated shape misses, indirect guest calls | Bounded variants and exact invalidation |
| 5 | Retain native storage and amortize ownership checks | Object conversion, allocation and repeated lease lookup | Proven ownership across the optimized interval |
| 6 | Optimize live callers together with prepared native IR | Function-call cost, lost constant propagation and vectorization | Compatible compiler IR and native identity preservation |
| 7 | Cache relocatable code and optimize asynchronously | Recompilation and pauses across the edit/run loop | Immutable compile inputs and validated cache publication |

Priorities 1–5 can use Cranelift and direct Rust ABI entries. Priority 6 motivates an LLVM optimizing tier. An LLVM tier alone will not repair an IR full of opaque helpers, unknown memory effects, tagged values, and forced state stores.

### 62.2 Required execution tiers

```text
Interpreter
    immediate execution and authoritative reached behavior
        |
        v
Cranelift optimized loops and reached regions
    concrete types, native backedges, direct dependency calls
        |
        v
LLVM optimized hot units where profitable
    prepared native IR imports, cross-call optimization, vectorization
```

The live source frontend remains LRR. Rustc runs only during explicit native preparation. The LLVM tier lowers LRR's typed optimized IR and consumes previously prepared native IR; it does not invoke rustc on a live crate or use rustc to diagnose cold live code.

Retain the interpreter as the semantic reference and fallback. Cranelift supplies quicker code generation; LLVM is justified only where its measured execution savings repay its extra cost. If the second native tier does not improve total workload time, keep it disabled. Backend selection may restrict a run to Cranelift or LLVM through --jit-tier; both backend implementations and their acceptance remain required.

Revision 5 adopts these tiering/backend changes. Preserve the interpreter-first priority before beginning guest JIT implementation. Unavailable LLVM or producer tooling is a recorded implementation prerequisite failure, not permission to mark its tasks complete.

### 62.3 Typed optimized IR before backend optimization

Create a common typed SSA representation between reached LRIR and the optimizing backends. Each value has a concrete representation class: primitive scalar, native resident object, checked managed handle, or unresolved dynamic value. Track initialization, ownership, effects, possible faults/panics, and native generation dependencies explicitly.

Native calls carry matched ABI/type identities and conservative effects. A scalar signature does not prove purity. Guest arithmetic retains its checked-overflow behavior even when the native dependency uses its own release arithmetic profile. Do not infer Rust alias guarantees for a managed reference merely because it is assigned a Rust-looking type.

Start with copy propagation, dead scalar computation elimination, common scalar expressions, constant propagation, redundant type-check elimination, and removal of provably unnecessary bounds checks. Operations that can fault, panic, allocate, drop, call, or perform an observable effect cannot be discarded or reordered without a proof under the selected execution contract. In particular, an unused checked arithmetic result may still panic.

Use Cranelift `opt_level="speed"` for this tier and keep verification enabled during acceptance. Its speed setting enables optimization; it does not establish correctness of LRR's input IR. Revalidate the exact pinned backend and register allocator settings instead of copying settings from a different release. [Cranelift settings documentation](https://docs.rs/cranelift-codegen/latest/cranelift_codegen/settings/struct.Flags.html#method.opt_level)

### 62.4 Native loops and state reconstruction

Replace the 64-op straight-line restriction in the optimizing tier with bounded compilation units that can contain loop backedges and reached branches. A loop enters native code once and executes many iterations before returning. Bound compiler work and generated size independently; a source-op limit should not force one dispatcher round trip per iteration.

Initially compile observed control-flow paths and their loop edges. A branch to a path whose semantics have not been prepared exits to the interpreter at that branch. It does not trigger speculative evaluation, compile a cold body, or replay the loop from its beginning. Once more paths are reached, a later optimized version can include them.

Replace per-op canonical writes with state reconstruction maps at exits and safepoints. Each map records:

- Source PC and inline call stack.
- Live locals and their locations in registers, stack slots or canonical storage.
- Concrete descriptors and logical initialization/move state.
- Outstanding ownership, drop obligations and active leases.
- Fuel/accounting state under the selected execution contract.
- Whether a native call is not yet entered, in progress, or completed with a result.

Every potential guard failure, panic, resource fault, cancellation exit and interpreter fallback needs a valid map. When an optimizing unit enters a baseline helper, materialize its required canonical inputs and continuation before the helper can inspect them; on return either rebuild valid SSA facts or exit. A map is a recipe, not evidence that stale canonical memory is safe to read. Native calls with observable effects retain a committed entry record. After a call completes, any exit resumes after it with its actual result and ownership state. No reconstruction path can cause a second call.

Guard failures before an operation restore the state before it. A checked-arithmetic overflow exits with the correct source origin and executed-prefix state. Object destructors run in the same logical order as in the interpreter. Optimized-away physical objects require a specified reconstruction recipe when their identity or ownership becomes observable.

Start with scalar-only loops to establish these invariants. Add native owned values and guest-call inlining only after exit maps can account for their lifetimes. Inlining a reached guest call also needs source stack and cleanup reconstruction; it is more than concatenating instructions.

#### 62.4.1 Entry maps and transfer of a running activation

Every optimized entry has a verified entry map paired with its reconstruction schema. It identifies the exact body/BoundPc/op index, source stack, live input locations and representations, initialization/move states, scope/drop obligations, leases, logical resource/accounting state and matching dependency/generation/contract identities. Entry maps and their ABI/schema digests are part of artifacts, compiler jobs and object-cache compatibility. An entry at a loop header imports the current iteration's state; it never restarts the function, condition or earlier iterations. Only previously reached/prepared PCs are eligible.

Initially allow transfer only at canonical opcode boundaries with no pending action, in-progress native call, aggregate-copy activation or cleanup activation. These activities finish or resume through their existing owner before another entry is considered. Completed native calls resume at their post-call PC with the actual result. Scalar entry guards and required scratch/reconstruction reservations run before changing canonical ownership. A rejected entry leaves canonical state unchanged and selects the checked fallback. Successful entry commits one activation owner; canonical slots that are stale while SSA owns a value cannot be read by helpers or treated as duplicate owners. Owned-value entry is admitted only after T086's maps account for every owner and lease.

TierTransfer reconstructs canonical state using the outgoing artifact, releases its interval leases and returns to the dispatcher. The dispatcher validates the target entry against that state and enters it, or continues through a valid lower tier/interpreter at the same PC. No active instruction or return address is patched, no host recursion is introduced by repeated transfers, and outgoing artifacts remain pinned under Section 22. Entry, rejection and transfer themselves execute no guest operation, consume no guest fuel and produce no completion observation. Failed entry must make progress through fallback rather than repeatedly trying the same target. CA028 verifies both directions and rejected transfer without replay.


### 62.5 Specialization and guard placement

A hot generic LRR unit can have several versions keyed by concrete runtime types, exact trait/member selection, native binding identities, generation, semantic mode, and source dependencies. Use the fixed four-variant limit plus baseline and the global code-memory budget in Section 62.11. An unrepresented combination executes through the checked fallback; it never reuses an incompatible specialization.

Guard at entry when a fact remains valid for the whole unit. Within a unit, typed SSA facts replace repeated descriptor checks. Stable callee identities permit direct calls or reached guest-body inlining. Guard failure must be uncommon for specialization to help; record misses and stop compiling versions that do not repay their code size and compile cost.

Select among installed variants by the full concrete key before invoking one; an unrepresented fifth shape goes directly to the checked fallback. Do not probe incompatible native variants in sequence. For each installed variant/entry, keep bounded counters for guard failures, successful completed traversals and cooldown; do not allocate an unbounded table for unseen shapes. After eight consecutive guard failures without a completed traversal, bypass that entry for 256 matching dispatch opportunities. A failed probationary entry doubles the cooldown up to 65,536 opportunities; 64 consecutive successful traversals reset the failure/backoff state. A probationary success clears the consecutive-failure count but retains the cooldown level until that reset threshold. Body/dependency replacement creates fresh state. Fallback executes normally during cooldown; suppression never rolls back effects or skips an operation. Force mode respects cooldown and must report any missing required native execution. CA031 tests alternating shapes, side-exit failures, recovery and the variant cap. Section 62.11 separately controls compilation retries.


Source changes invalidate all compiled callers that incorporated the changed body, constants, method selection, or native IR. Installing a new version never retargets an already active native stack to incompatible code. Retain old code until active frames have returned or explicitly deoptimized. This is compilation-version lifetime management within one native generation, not hot swapping native dependencies.

### 62.6 Ownership and data locality

Retain native containers across operations and pass their real storage to prepared methods. Avoid repeatedly materializing managed copies. A native constructor followed by many native operations and one final result conversion is the intended efficient pattern.

Where control-flow and ownership facts prove it safe, resolve a native handle and acquire its external lease once for an optimized interval. Keep the checked native pointer in a register. Release the lease on every normal and exceptional exit. The interval cannot cross a move, drop, conflicting alias, callback, suspension, or operation that invalidates the proof.

Do not cache a container's internal buffer pointer across operations that can reallocate it. A stable `Vec<T>` header does not imply a stable element buffer. Native methods may change interior state, and every call retains its declared memory effects. Opaque storage keeps Rust representation and invariants native; exposing an arbitrary live struct as native memory remains unsupported.

Scalar replacement and stack allocation can remove temporary allocations when neither identity nor address escapes and destruction semantics are preserved. Start with managed scalar aggregates and accepted native types with compiler-derived contracts. A native object's layout alone is not enough to prove that construction, allocation or Drop can be eliminated.

Logical managed resource accounting follows the canonical LRIR execution in both contracts, independently of physical scalar replacement. Charge cumulative logical handle/allocation counts and live logical bytes at the same operation, size and logical lifetime as the interpreter, and release live charges at the same semantic cleanup/transfer points. Eliminating an allocation, slot or inlined frame cannot eliminate its logical quota/frame-limit checks or change the first quota-failing operation. Virtual allocations carry checked logical identities and initialization/ownership records; materialization binds physical storage to those identities without charging a second logical allocation or resurrecting a retired handle. Native ambient allocation and physical RSS remain separately reported limitations, not deterministic managed quotas.

A virtual allocation has a typed logical identity `(run epoch, arena kind, logical allocation ordinal)` distinct from physical handle bits. Allocate/check its monotonically increasing ordinal at the canonical allocation point; report ordinal exhaustion through the same logical resource policy in every tier. Do not append a physical arena entry or tombstone for a virtual value that dies without materialization. Retain only live virtual records plus bounded accounting summaries, so a scalar-replaced allocate/drop loop does not grow a per-iteration identity table. Logical accounting applies to interpreter allocations too; physical indexes count only actual arena publication and remain subject to checked representation limits.

Materialization appends a fresh checked physical entry and records the logical-to-physical association for that live value. All aliases to that value bind to the same entry; existing physical values retain their original handles. Guest-observable identity, borrowing, address exposure or entry to a canonical helper requires materialization before exposure. Never reinterpret a logical ordinal as FrameId/CellId/ObjectId bits, recycle a retired physical index or re-materialize an already bound value as a second owner. Retirement removes the live association; the logical ordinal is never reused. Physical publication/index capacity, association storage and backing needed by a reconstruction are included in its pre-entry reservation.


Before entering an optimized interval, reserve bounded physical backing for its worst-case admitted reconstruction, including canonical cells, handles, scopes and owned values. An interval with unbounded reconstruction demand must materialize at bounded checkpoints or stay in the checked tier. Reservation failure before entry declines that optimization without a new guest fault, then lets the checked path apply its ordinary resource checks; do not precharge future logical allocations or move their failure before an earlier effect. A later safepoint may renew a reservation only while the existing reservation still suffices to reconstruct the current state. After an effect, reconstruct from the reserved backing without a fallible allocation; insufficient backing is an internal JIT defect, never permission to replay or drop completed work. Physical reservations count against worker memory. During reconstruction, backing consumed by live canonical cells/objects/frames transfers exactly once to their arena owner and stays allocated/charged until its ordinary retirement. Release only unused reservation capacity on exit; transferred storage must not be freed when the native activation returns. Transfer/retirement cannot double-charge logical quotas or physical memory. Physical reservations provide no claim of identical process-OOM behavior across tiers. CA034 verifies backing adoption and physical/logical identity separation. CA032 checks low logical quotas, eliminated objects, inlined frame limits and reconstruction after a completed effect.


### 62.7 Cross-boundary inlining with prepared LLVM IR

Calling a tiny dependency function through its native address can never expose its arithmetic to the caller's optimizer. To remove that call and optimize the combined loop, preparation must optionally retain compiler IR for selected concrete specializations in addition to their callable native addresses.

The native generation gains an IR bundle containing producer and LLVM identity, target/data layout/features, concrete entry identities, IR content hashes, allowed imports, and native state dependencies. Absence of such a bundle leaves a valid direct-call binding. Do not assume arbitrary existing Cargo artifacts contain usable bitcode: Cargo disables embedded bitcode where possible. [rustc bitcode options](https://doc.rust-lang.org/rustc/codegen-options/index.html#embed-bitcode)

Preparation obtains this IR from the actual compiler invocation for the concrete specialization. Generic declarations in metadata are not already-specialized LLVM bodies. Asking for an IR bundle can require additional preparation or rebuilding affected artifacts; retain actual Cargo reuse evidence.

The LLVM tier lowers the live hot unit to compatible LLVM IR, imports eligible prepared definitions, applies normal optimization/inlining passes, and emits machine code. LLVM ORC can provide the compilation and object-linking infrastructure; LRR still owns semantic lowering, optimization policy, safepoints and closed symbol resolution. ORC does not supply a Rust frontend or automatic correctness at this boundary. [LLVM ORC](https://llvm.org/docs/ORCv2.html#lljit-and-lllazyjit)

Initially import only abort-mode leaf functions with scalar arguments/results and no global, TLS, allocator, unwind, callback or address-identity dependencies. Eligibility is established from the verified concrete IR and transitive references, never from a scalar source signature alone. Reject residual calls or intrinsics that violate this leaf contract; the original direct binding remains available. This allows a clean experiment such as inlining a concrete `mix::<u64>` into a live loop. Expand only through separately accepted IR classes. Fully inlined native bodies contribute zero physical ABI entries; track logical native operations and native_ir_inlined_executions separately from actual transport/call counters. Preserve precise charges, native arithmetic semantics and exit origins even if optimization removes a logical call's physical instruction. Entry/return records around actual opaque native calls remain mandatory. A verified effect-free scalar leaf may use reconstruction/accounting maps instead of an opaque-call transaction; this exception cannot be inferred for an uninspected native call.

For later general imports:

1. Reuse the existing generation's statics, TLS, allocator, vtables and address-taken callable identities. Never create a second copy of mutable crate state or run initializers again.
2. Give imported bodies optimization-only definitions where appropriate and map residual calls to explicit registered native capabilities. No default process-symbol lookup is allowed.
3. Preserve required COMDAT/linkage, target-feature and semantic metadata. Unsupported intrinsics or unregistered references reject the import and retain the direct call.
4. Use exact compatible producer/LLVM configurations and cache identities. A nominally parseable bitcode file is not proof that all calling conventions, attributes and runtimes match.
5. Preserve the native function's arithmetic/panic semantics inside an otherwise managed caller. Do not use native IR attributes to invent guarantees about invalid managed input.

LLVM's cross-language LTO facilities establish that compatible IR can be optimized across boundaries, but the import/state rules above are additional LRR engineering. Cranelift cannot consume LLVM bitcode as if it were CLIF. Building a generic LLVM-to-CLIF translator would introduce another large compiler project, so it is not the recommended path. [rustc linker-plugin LTO](https://doc.rust-lang.org/rustc/linker-plugin-lto.html)

### 62.8 Execution contracts and runtime bookkeeping

Memory validity, exactly-once effects, native ownership and generation matching remain mandatory in every mode. Performance improvements eliminate checks only when facts prove them redundant.

Exact per-op accounting and immediate canonical state are separate choices. The optimizing tier can delay physical state writes while preserving exact behavior using reconstruction maps. Fuel for a fixed straight-line segment can also be checked once when enough remains, provided any earlier fault reports the exact consumed prefix and the low-fuel case executes the precise checked path. Never charge for operations that did not execute or move an external effect before its fuel check.

Implement the separately named `throughput` execution contract with no exact instruction fuel and cooperative cancellation at native-call boundaries and bounded loop-backedge intervals. Retain the default `precise` contract for current fuel/resource semantics and detailed tracing. The chosen contract belongs in results and cache keys. An optimization flag must not silently change the resource contract.

The throughput contract still has process deadlines, memory limits where already enforceable, and defined cancellation points. A backedge interval bounds loop iterations between polls, not wall time: one iteration or native call can take arbitrarily long. The coordinator must retain its worker termination policy. Native ambient allocations remain outside managed memory accounting, as already true in hybrid mode.

Batch profiling counters at exits or sample them after warmup. Detailed per-op tracing is a diagnostic configuration with its cost reported. Keep only the call-state records required for failure attribution and correctness on the critical path; do not confuse optional statistics with required resource behavior.

Publish checked per-unit/per-tier completed traversal/backedge deltas at existing safepoints and exits, at least within the Section 16.3 polling bounds. This lightweight promotion feedback remains enabled after baseline entry; optional detailed traces/timing samples may be reduced. Count only actually completed edges, exactly once across transfers and exceptional exits; cache hits and entry attempts do not increment them. Flush deltas before changing tiers so Cranelift executions can make LLVM work eligible without waiting for more interpreter traversals. Counters use stable source-unit and edge identities shared across tiers, with separate tier attribution. At a poll, a newly eligible candidate with available queue capacity may request TierTransfer to prepare/enqueue one bounded job at the dispatcher; native polling itself runs no query or compilation. Coalesce one outstanding request per unit/backend/key and honor cooldown/resource failures to avoid a transfer on every poll. CA030 requires promotion driven by native work, including a loop that does not normally return to the dispatcher.

At those same safepoints, check compiler-completion readiness without blocking. A pending completion for the active unit requests TierTransfer at an eligible mapped boundary; the execution owner then receives, validates, binds and publishes through bounded dispatcher work. Do not wait for compilation, copy a whole object or perform linking at a native backedge. Ineligible boundaries retain readiness until the next eligible boundary. Validated applicable code can enter at the current mapped PC; stale, rejected, budget-limited or inapplicable results retain a valid lower tier. Cancellation/deadline handling takes precedence. With no eligible promotion request or compiler transport work, polling adds no tier transfer; jit=off never requests one. Section 62.9.1 also services inactive/stale results through bounded dispatcher transfers so output draining cannot depend on current-unit applicability. CA029 requires a genuine mid-loop background completion to be noticed within the applicable polling bound, with actual later entry or an explicit rejection reason; no wall-time bound is inferred for native calls or linking.




The exact CLI/service defaults, limits and polling intervals are Sections 16.3 and 26. Numeric --fuel conflicts with throughput; queries and const evaluation retain bounded execution. Detailed tracing forces precise diagnostic instrumentation and is incompatible with claims of uninstrumented throughput. A benchmark must name its effective contract.

### 62.9 Compilation latency and reuse

#### 62.9.1 Compiler-result lifecycle and draining

The execution owner's event pump drains results for every submitted job, including inactive units and stale/cancelled jobs, independently of whether the current PC can use the result. Readiness polling is nonblocking; actual receiving proceeds at safe dispatcher boundaries in bounded turns. Drain at most one complete job or 1 MiB of payload per turn, whichever comes first, then service cancellation/broker events and resume guest work before another turn. A transport-ready completion for an inactive unit may request a coalesced service TierTransfer at an eligible mapped boundary; it need not be selected for execution. No native safepoint performs bulk copying, queries or linking. Retained output bytes share the existing bounded transfer/cache/worker-memory limits; when admission fails, consume and discard bounded chunks or cancel/reap the compiler session through supervision so its output cannot indefinitely block the sole compiler or guest. Never wait for a result while holding guest arena borrows.

Track job IDs through explicit states: submitted, receiving, received, validated, published, selected, and discarded/failed. Received means an entire bounded message and digest are present; validated means compatibility/provenance/map/relocation checks succeeded; published means executable memory is finalized and an immutable artifact is registered; selected means a matching entry actually executed. Each transition is owned once; receiving or publication is not native-entry evidence. Inactive valid results may remain bounded pending artifacts; publication/selection remains subject to source/generation/contract checks and code budgets. Stale outputs are drained/discarded without executable publication. Duplicate/unknown job IDs fail the compiler session as protocol defects and leave the proven guest tier intact in auto mode.

Cancellation/run end stops new submission, cancels pending jobs and drains/disposes results or reaps the owned compiler within the existing supervision deadline. A later run cannot adopt an earlier job merely because its unit name matches: it must satisfy an explicit retained-job identity policy; initially discard unfinished cross-run jobs. Remove every queue/buffer/artifact ownership reference when discarded, with no guest rollback or lost committed effect. CA035 saturates output/backpressure while another unit loops, checks cancellation/stale/duplicate paths, and separately observes receiving, validation, publication and selection.


Evaluate promotion using measured compile cost and expected remaining work:

```text
expected_remaining_executions * expected_time_saved_per_execution
    > compile_cost + expected_deoptimization_cost
```

These are estimates; retain the cheap tier when confidence is weak. Optimization also has a CPU-contention and code-memory cost, even when compilation runs in the background. A fixed traversal count alone is a poor policy for both one-shot tests and long computations.

The optimizing compiler worker is a supervised subprocess, not a thread that the runtime must somehow kill or meter independently. It consumes an immutable, fully prepared typed compilation unit with no live query pointers or permission to execute source/macro code. Runtime execution continues in its current tier. Before publishing completed code, the owner checks source dependencies, native generation, semantic contract, and compiler/ABI fingerprints. Stale results are discarded. No background compiler mutates the active query database or patches active instructions. Send pointer-free typed inputs and relocatable outputs using compiler-specific begin/chunk/commit message kinds over the internal v4 bounded framing and raw byte chunks in Section 25.6 (these are compiler jobs, not broker effect requests), with compiler-build, target, helper/map, contract and input digests in the handshake/job envelope. Compiler jobs use explicit stable symbolic IDs, never worker-local pointers. The compiler process loads its pinned backend libraries but does not load the native dependency generation or call its entries. It emits objects and symbolic relocation identities; the execution worker performs final binding and executable publication under its existing authority. Reaping an over-budget compiler cannot kill or corrupt a guest run; auto retains its lower tier and force reports failure. Treat compiler RSS and wall limits separately from execution-worker limits.

Store relocatable compiled objects with bounded metadata, reconstruction maps, dependency keys and approved relocation identities. On a fresh worker, validate the cache record, map native entries for that worker's generation, apply only admitted relocations, and finalize executable memory through the audited platform path. Do not persist raw executable addresses or run state. Keep fresh hybrid workers; code reuse does not require sharing native globals between requests.

Cache compatibility includes source and inline dependency digests, type specialization, native generation/IR bundle, target CPU/features, backend version/settings, helper ABI, execution contract, and reconstruction schema. Reusing code for a different CPU feature set or semantic contract is a correctness failure. Untrusted or malformed cached code is never executable merely because it has a plausible filename or self-reported hash.

### 62.10 Required integration experiments

Validate changes in independently attributable stages, retaining the direct-ABI acceptance suite:

1. Measure direct ABI calls and the full scalar LRR call path against the same native pointer target.
2. Add typed IR optimization and a single native loop backedge. Measure dispatcher entries, state loads/stores, helpers, code size, and actual wall time.
3. Add state reconstruction for scalar loops. Deliberately exercise every exit, overflow, cancellation and fuel boundary; compare exact effects and results to the interpreter.
4. Add bounded type/callee specialization, then owned-object exit maps and lease amortization. Include polymorphic and allocation-heavy regressions.
5. Prepare a tiny scalar LLVM IR bundle and demonstrate caller/callee inlining in assembly. Compare against both a direct-call loop and optimized native Rust. A loop with an unremovable large native call is the negative control.
6. Add asynchronous optimization and relocatable caches. Measure first run, steady state, body-only edit, invalidating edit, new specialization preparation and process restart separately.
Future expansion, outside initial release acceptance: add broader native IR classes only after a separately specified contract and mutable-state, ownership and residual-symbol controls. The six experiments above require only the initial scalar leaf import class.

Keep precise and throughput results separate. Compare equivalent arithmetic, panic, inputs and outputs; compare metering overhead explicitly when the native reference has no fuel instrumentation. Include compile/link/load time when reporting time to finish a test, and exclude it only in clearly labeled steady-state measurements.

The strongest supported claim would be workload-specific parity with optimized native Rust, including its inlining, for admitted programs. Neither a zero-codec ABI nor an optimizing backend alone supports that claim. The recommendation is to remove structural execution overhead first, then use compiler IR to remove the remaining optimization boundary where measurements show it matters.

### 62.11 Fixed compiler budgets and scheduling

Promotion traversal thresholds, guard-failure/cooldown lengths and profitability retry intervals/counts are initial tuning defaults, not measured optimal values or guest semantics. Freeze their initial values as `SchedulingPolicyV1`; record a policy version, explicit stable digest and effective settings in run/performance results and cost-history records. An explicit host configuration may select tested bounded values; report it, validate all positive thresholds/counts and checked sizes, and retain hard compiler/code/variant/queue limits and execution-contract polling bounds. Policy changes cannot relax authority, reached-path admission, exactly-once behavior, fuel/quota semantics, cache ABI compatibility or mandatory force-tier evidence. Policy identity governs reproducible decisions and cost-history interpretation; it need not invalidate machine code whose semantic/code-generation identity is otherwise identical. Test defaults plus an injected bounded alternative in CA036. Change defaults only with attributable measurements and a policy-version update, not an unreported performance tweak.

Baseline units retain the 64-op limit; optimizing units are bounded to 4,096 reached LRIR operations, 256 blocks and 1 MiB emitted code per artifact. At most four optimized variants plus the baseline exist per unit. The 64 MiB worker code budget includes both backends and retired versions. Oversized units retain the lower tier; force-mode acceptance must not claim an optimized entry for such a fallback.

One compiler worker runs at most one optimization job with one prepared job waiting. Each job has a 30-second wall limit and a 2 GiB compiler-worker RSS limit; cancellation/deadline supervision is separate from guest execution. A failed auto optimization retains the proven lower tier with a diagnostic counter; explicit force-tier failures fail the selected fixture. Immutable typed inputs, not live queries, cross the compiler-worker boundary. Compilation performs no guest/native call, macro execution, Cargo invocation or external linking process. ORC object linking is an in-process JIT operation with the closed registry.

After 32 completed traversals, baseline promotion follows Section 20. Optimize by the expected remaining work inequality above, using recorded per-unit/backend compile and execution costs. When no cost history exists, permit one bounded exploratory optimization after 1,024 completed traversals; force lowers that requirement to one actual observation. A nonprofitable result enters bounded reconsideration, not permanent exclusion. Keep its measured compile/execution/deoptimization costs. After at least 4,096 additional completed traversals since that attempt, fresh per-unit execution/shape/exit evidence may trigger reevaluation of the profitability inequality. Each subsequent retry doubles this evidence interval; allow at most three automatic retries after the initial optimization attempt per unit/backend/dependency/contract key per run, shared across that key's concrete variants. A nonprofitable inequality causes no job and consumes no retry, but a submitted retry does. New jobs still obey compiler, candidate, variant and code budgets. Persist cost history separately from run-local retry counters so fresh workers can reconsider changed workloads without discarding cost evidence. Changed dependencies form a new key. ABI/import audit failures remain defects and are not retried as profitability experiments; resource-limit and internal-codegen failure policies in Section 21.6 still apply. Force does not bypass authority, code lifetime or hard budgets. CA033 injects deterministic old/new cost evidence and executes real work to verify phase-change reconsideration and retry exhaustion. At equal estimated benefit use stable digest order. Tests inject recorded cost inputs to verify decisions; no timing-based assertion may manufacture native execution.

Prepared LLVM IR is limited to 64 MiB uncompressed per generation, 8 MiB per imported body and 4,096 imported definitions per optimized job. Verify module/target/schema and closed references before compilation. Disk object cache is capped at 512 MiB per project using deterministic last-use/digest eviction between requests. Objects include integrity/provenance receipts from the trusted compiler pipeline, bounded relocation/map records and exact compatibility identities; a matching self-reported checksum from an untrusted writer is insufficient authority to execute code.

LLVM acceptance initially supports the exact 23.1.1 producer-compatible build paired with the Rust 1.99.0 native preparation toolchain. T087 must record source/build/library hashes and ABI probes before it integrates. The current rustc source checkout's different commit and Cranelift 0.135.0 reference implementation cannot establish compatibility with the installed compiler and pinned Cranelift 0.136.2. If the exact LLVM/exporter distribution is unavailable, record that prerequisite and continue independent interpreter work; do not change the compiler or claim LLVM acceptance.

## Implementation errata for pinned dependencies

**E-M006-001 — Repetition cardinality.** The required pinned `mbe` source silently
truncates unequal repeated bindings in one transcription axis. The standalone
Rust 1.99.0 witness rejects that input, as MX008 requires. Apply the exact audited
`vendor/mbe-cardinality.patch` while retaining the original source archive and
inventory. Audit reconstructs the patch from its pinned preimage and verifies its
postimage; this corrects the transcriber to the existing language contract and
preserves the selected matcher, frontend and dependency pin. See
[the witness, correction and regression record](docs/errata/m006-repetition-cardinality.md).

**E-M006-002 — Repetition termination.** A real Rust 1.99.0 oracle accepts and
executes a 65,537-element repetition that the pinned transcriber rejects at its
65,536-iteration recovery cutoff, despite the output remaining within Section
50.6 limits. Extend the same audited patch to terminate from matched repetition
axis lengths and reject a repetition with no driving binding. Preserve the
original archive/inventory and validate the combined patch from that preimage.
See [the cutoff witness and regressions](docs/errata/m006-repetition-cardinality.md).
This repair does not establish preallocation output-budget enforcement.

**E-M006-003 — Explicit token preflight.** Integrate a count-only pass of the same
pinned transcription algorithm through an explicit bounded `mbe` entry point.
LRR must use that entry point; an oversized candidate is refused before creating
its output token builder, with actual materialization counters in acceptance
fixtures. Preserve original archived preimages and audit the combined transcriber
and supporting dispatch/restore-type patches. No alternate matcher or frontend
is introduced. See [the integration and remaining byte/scratch limits](docs/errata/m006-token-preflight.md).

**E-M006-004 — Exact schema2 byte preflight.** Extend the same pinned count-only
transcription pass with byte admission based on LRR's captured owned span/context
metadata and shared codec field writers. Count exact final wire bytes before
creating the main output builder; unknown spans are typed schema errors, and
resource failures cannot fall back to another arm. Exercise the real 32 MiB
boundary and byte+1, with actual zero-materialization rejection evidence. Keep
the existing audited original preimages. See [the byte accounting and remaining scratch requirement](docs/errata/m006-byte-preflight.md).

**E-M006-005 — Admitted repetition replay.** The counting pass records validated
repeat iteration counts and discards nested recipes from after-end trials.
Replay of the same immutable bindings/template skips zero/after-end bodies and
trailing separators. Validate recipe exhaustion and final counts; enforce the
checked admitted-token count plus the owned definition's 256-group temporary
wrapper bound before raw appends. Preserve actual leaf-append/peak-slot evidence
and the same audited original preimages. See [the scratch witness and depth-80 native regression](docs/errata/m006-scratch-replay.md).

**E-M006-006 — Stable ordinary macro definitions.** The pinned frontend accepts
nightly metavariable expressions that native Rust 1.99.0 rejects with E0658.
Keep its rule parser and add an ordinary stable-profile definition-demand check
over structured tokens for nightly headers, `$$` and `${...}` syntax. Retain the
error at the definition; do not scan literal contents or type-check ordinary
body syntax. Certified compiler-owned builtin treatment remains M011. Preserve
the original archived preimage and actual native/pinned witnesses. See
[the profile correction and regressions](docs/errata/m006-stable-profile.md).


**E-M007-001 — Pinned hygiene adapter API.** Expose the existing mark algorithm
with an explicitly resolved call-site context and a span-only Salsa database.
The original source-aware entry retains its opaque fast path and call-context
lookup, then delegates. Preserve the archived normalization/replay/interner
algorithm; prove its complete suffix unchanged apart from the helper database
trait. Keep expansion/context slots private and reconstruct checked owned
ancestor records without HIR source providers. This is a mechanical API
integration, not a language or hygiene algorithm change. See
[the audited API correction and pending integration](docs/errata/m007-hygiene-api.md).


**E-M007-002 — Marked span and ancestor admission.** Precompute actual mixed-site
span substitutions through the pinned hygiene store, retaining source/origin
identity and using explicit definition owner/edition. Preview that closed map
at the original transcriber marking sites without invoking the materialization
marker before admission. Count each reachable context ancestor once through
the shared schema2 field writer and roll back newly introduced metadata. Export
the complete reachable ancestor closure and require exact admitted/codec byte
equality, including the real 32 MiB boundary and byte+1. Preserve the original
matcher, transcriber, archive and combined-patch preimages. See
[marked transcription and the pending name-resolution integration](docs/errata/m007-marked-byte-preflight.md).


**E-M007-003 — Producer-complete reexport cycle correction.** The initial LRR
namespace index incorrectly diagnosed a complete `a -> b -> a` cycle without
an originating definition. Section 50.2 already requires a complete empty
result. Preserve the failing witness and regression; keep pending producer
input distinct and publish complete producer results atomically. Source/catalog
seals and final source-aware selection remain required before runtime negative
caching. See [the implementation correction](docs/errata/m007-complete-reexport-cycles.md).

**E-M007-004 — Owned Unicode macro identity.** Keep the pinned parser/lexer,
normalize validated owned macro names to NFC after removing the raw prefix,
and preserve source spellings/ranges. Section 3.2 records supplemental exact
`unicode-normalization = 0.1.25` (Unicode 17), without changing previous pins.
See `docs/errata/m007-unicode-names.md` for the failing witness, native comparison
and the separate pending macro token-input normalization requirement.

E-M007-004 token integration also normalizes identifier symbols at the owned/pinned
token boundary. Preserve raw flags and registered source spans, contexts, origins
and editions; do not normalize literal text. Exact marked byte admission must use
the canonical identifier bytes, including NFC expansions. The standalone native
edition comparisons prove this adapter component, not future procedural-server or
source-scheduler integration.

**E-M007-005 — Local publication source interval.** Generated local declaration
batches must fit between their producer and the next known statement/producer or
child-scope entry. Reject the whole out-of-interval batch before clearing demand
or changing names. The source scheduler supplies consistent logical ordinals;
never silently move known source frontiers. See
`docs/errata/m007-local-publication-order.md` and its retained failing witness.

**E-M007-006 — Emitted textual binding history.** Preserve every same-position
emitted binding until pinned name compatibility has been applied. A later
incompatible spelling must not erase an earlier compatible definition. Only
then apply textual shadowing and expansion-origin ambiguity. Preserve the
failing witness and regression in `docs/errata/m007-textual-binding-history.md`.

**E-M007-007 — Transforming scope-opening demand.** A scope's own transforming
attribute is part of its opening frontier and must suspend lookup in its
unexpanded contents. Captured parent visibility includes that frontier while
excluding subsequent source declarations. Unloaded external modules require
typed source input rather than empty scope publication. See
`docs/errata/m007-source-opening-frontier.md` for the witness and correction.

**E-M007-008 — Raw source name validation.** Retain the original AST name syntax
separately from semantic DefKey spelling when validating source macro names.
Stripping `r#` before source validation must not reject valid 2024 `r#gen`.
Preserve token/source spelling, then canonicalize the owned lookup name. See
`docs/errata/m007-raw-source-name.md` for the failing witness and native acceptance.

E-M007-004 original-source integration canonicalizes owned declaration names,
logical module paths and duplicate-name ordinals at the pinned AST boundary.
Prepared source contexts apply the same canonical module identity before
duplicate detection and publication. Preserve original header token spelling,
source spans and SourcePath identity. AST semantic names already stripped of a
raw prefix must not undergo invalid bare-keyword source revalidation. See
`docs/errata/m007-unicode-names.md` for the retained source-bucket witnesses.

**E-M007-009 — Entered-body macro frontiers.** The pinned MACRO_CALL kind may
cast as an item even in an expression initializer. Use actual statement
position to establish outward producer demand. Keep skipped nested executable
and item ranges opaque to lookup until their own explicit entry, rather than
resolving those positions through an enclosing entered frame. Preserve the
failing witnesses and native source-shadowing comparison in
`docs/errata/m007-entered-call-frontiers.md`.

**E-M007-010 — Captured source-chain metadata.** Validate every declaration
metadata row associated with an identity across captured source scopes before
name filtering or origin selection. A matching first row cannot hide a later
conflicting context/origin. Retain the minimal failing witness and regression
in `docs/errata/m007-source-chain-metadata.md`.

**E-M007-011 — Imported macro reexport visibility.** Caller-aware macro lookup
checks access from the importing declaration scope and checks that the target
binding's declared visibility contains the reexport's exposed visibility. A
public original macro does not permit widening a private imported alias. Preserve
the native Rust 1.99.0 macOS E0364 witness and source/native regression in
`docs/errata/m007-reexport-visibility.md`. Module-prefix visibility and membership
seals remain separate requirements; candidate-only lookup cannot authorize guest
dispatch.
