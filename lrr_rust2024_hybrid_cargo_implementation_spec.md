# LRR: Rust 2024 interpreter/JIT with optimized Cargo artifact reuse

**Document revision:** 3.0  
**Specification date:** October 4, 2026  
**Runtime release to build:** `lrr 0.1.0`  
**Guest semantics profile:** `lrr-2024-v1`  
**Execution modes:** `managed` (default) and explicit `hybrid-native` using the same interpreter/JIT  
**Status:** implementation contract; not an implemented or benchmarked runtime  
**Supersedes:** revisions 1 and 2, including the earlier edition-2021 single-architecture specification. This is the complete replacement document. The live-code engine remains one architecture; trusted Cargo-native islands are an additional call target, not another interpreter or JIT.

**Execution route:** Read Sections 1–5 and 34–36 first. Sections 6–26 define the common engine; Section 34 fixes Rust 2024 semantics; Sections 35–43 specify the native integration; Section 44 orders the new tasks and gates; Section 45 gives the artifact-reuse probe; Section 46 records additional primary sources. Apply H001–H003 during the original task ledger and run the H004 reuse probe after bootstrap; complete the common engine, then H005–H028 and the mandatory hybrid gates in Section 44. There is no alternative backend or arbitrary native-symbol loader.

**Revision 3 contract:** Live Rust 2024 code is still prepared only when reached. Native islands are ordinary, fully checked Rust crates, prepared explicitly by Cargo. Compatible existing artifacts are reused by Cargo; LRR does not interpret Cargo incremental-cache internals. Test runs never invoke Cargo or rustc, and native cache misses never trigger a hidden build or replay a partially executed test. Hybrid execution is explicitly unsandboxed. Its worker is fresh per run request; only portable source/query work, not native heap state or executable pointers, crosses that process boundary.

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
34. [Rust 2024: required edition semantics and migration gates](#section-34)
35. [Native integration: locked design and exact reuse boundary](#section-35)
36. [Binding manifest, native types, and native closure selection](#section-36)
37. [Cargo bridge preparation and existing artifact adoption](#section-37)
38. [Input fingerprints, immutable native generations, and freshness](#section-38)
39. [Generated native ABI, codecs, and object storage](#section-39)
40. [Boundary execution machine, aliases, panic, and semantics](#section-40)
41. [Query/JIT integration, process lifecycle, and warm reuse](#section-41)
42. [CLI, service messages, permissions, and result reporting](#section-42)
43. [Required hybrid evidence, diagnostics, and performance gates](#section-43)
44. [Ordered edition/hybrid implementation tasks and gates](#section-44)
45. [Standalone Cargo reuse probe and first implementation experiment](#section-45)
46. [Additional primary sources for Rust 2024 and native reuse](#section-46)

<a id="section-1"></a>

## 1. Mandate and interpretation of this document

Build one execution system for a deliberately specified subset of Rust source. It must start with interpretation, prepare executable operations only when reached, retain source-derived work between edits, and promote observed straight-line execution regions to native code using Cranelift. The live-code execution path never requires guest borrow checking, whole-body type inference, trait-obligation solving, coherence checking, LLVM, linking a guest executable, or compiling the selected live crate with rustc. Explicit native preparation separately uses Cargo/rustc to check and link only the declared native closure and generated bridge. This distinction is mandatory in telemetry and documentation.

The implementation language is Rust. Building the runtime itself uses rustc and Cargo. These host build steps are not guest compilation. Cargo is also used explicitly to prepare package metadata and, in trusted hybrid preparation, a native bridge. No run/test execution launches either tool. A native dependency may internally use LLVM-generated optimized code without making the live frontend depend on LLVM.

The language remains eager: function arguments, arithmetic operands, and effects execute in the specified order. “Lazy” describes preparation, resolution, and code generation, not call-by-need evaluation. Never postpone an effect or arithmetic error to obtain type information from a later statement.

This is an execution profile, not an alternative implementation of every Rust language rule. Passing a test means that this test passed under `lrr-2024-v1`; it does not mean rustc would accept the source. In particular, programs with invalid Rust aliasing can have defined, checked behavior here. Never optimize using static Rust guarantees that this runtime has not established.

### 1.1 How an implementation agent must use the specification

Read Sections 1–5 and 34–36 before creating files. Implement the core task cards with H001–H003 applied, run H004 after bootstrap, and follow Section 44’s dependency order for hybrid work. Treat each task's acceptance tests as requirements, not illustrations. Definitions in the main specification govern implementation details; fixtures govern observable behavior. A later correction to a discovered contradiction must be recorded in `docs/spec-errata.md`, accompanied by a regression test, and must preserve the mandate above.

Do not select another backend, frontend, query framework, ownership model, or language profile. Do not introduce a whole-function compiler, Wasm runtime, tracing optimizer, rustc frontend integration track, arbitrary native adapter plugin system, asynchronous compiler, or alternate numeric mode. These are excluded deliverables, not branches that the implementation agent should explore. The generated, closed native bridge specified in Sections 35–43 is required and is the sole exception to revision 2’s prohibition on native adapters.

An implementation obstacle is handled by reducing the failing case to a test and fixing the assigned component. It is not permission to silently change guest semantics. API spelling errors in this document may be repaired against the pinned dependency source without changing architecture. A missing external package or incompatible toolchain is a bootstrap failure with recorded evidence; do not silently select `latest`.

### 1.2 What “complete” means here

Completion means all specified supported features, task gates, and release checks work. It does not mean accepting arbitrary crates.io packages. Unsupported features have a prescribed error and prescribed point of failure. They must not become silent no-ops, implicit native fallbacks, or indefinitely postponed research tasks.

The document includes Rust-shaped interface definitions and algorithms. Except for explicitly labeled bootstrap snippets and guest fixtures, these are normative interface specifications, not a precompiled codebase. No claim is made that dependency combinations, generated native code, latency targets, or security properties have already passed the specified tests. The implementation must produce that evidence.

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
    | observed-region Cranelift JIT        |
    | managed guest heap and cleanup       |
    +--------------------------------------+
                  |
       brokered, bounded effect messages
                  |
      explicitly granted host resources
```

The diagram above shows managed mode; hybrid mode uses the same core in a fresh worker per run request plus the explicit Cargo-native bridge (Sections 35 and 41). There is one interpreter, one Cranelift JIT, one value representation, one call dispatcher, one cleanup mechanism, and one managed effect protocol. Interpretation and native execution are tiers of the same machine, not two independently defined languages.

<a id="section-2"></a>

## 2. Locked design decisions and accepted costs

This section records the trade-off evaluation as decisions. It is not a menu.

| ID | Determined choice | Why this choice wins for this project | Cost deliberately accepted |
|---|---|---|---|
| D01 | Independent execution frontend over `ra_ap_syntax` | Controls exactly when executable meaning is requested | We implement resolution and runtime semantics ourselves |
| D02 | No rustc HIR, THIR, or MIR on the guest execution path | Typed compiler IR is downstream of analysis we are intentionally omitting | No automatic compatibility with the native Rust standard library |
| D03 | Rust 2024 live guest profile only, including the specified let-chain subset | Targets the requested edition with explicit drop/pattern semantics | Older-edition source is native-only, not silently reinterpreted as 2024 |
| D04 | Linux x86-64, little-endian, 64-bit guest target only | One ABI and target configuration eliminate cross-platform ambiguity | macOS, Windows, and non-x86-64 builds are not release targets |
| D05 | Parse loaded source files; semantically prepare only reached executable operations | Reuses a maintained parser without confusing parsing with checking | Cold startup reads and parses more bytes than it executes |
| D06 | Index declaration headers outside the execution trace when lookup needs them | Correct method/name selection requires a complete candidate set | “Untouched” means no body semantics, not no inspection of declarations |
| D07 | Custom single-threaded dependency-tracked memo database | The required query set is small and its invalidation rules can be explicit | We implement dependency validation, cycles, and cache accounting |
| D08 | No Salsa dependency | Avoids learning and exposing another evolving framework in an agent-built first release | We do not inherit Salsa's parallelism or mature query tooling |
| D09 | Immutable snapshots; one active run per worker | Prevents mixed-revision execution and simplifies memory/code lifetime | No live patching of active frames and no parallel tests in one worker |
| D10 | Fresh managed state per test; fresh hybrid worker per run request | Retains managed caches while resetting native globals between repeated invocations | Hybrid machine code is not retained across worker processes; globals may be shared by tests within one batch |
| D11 | Fixed-width tagged values and checked guest handles | Invalid guest Rust cannot manufacture invalid host Rust references | More memory and dynamic dispatch than native Rust values |
| D12 | Explicit moves, initialization state, and logical drops | Ownership has observable execution consequences even without a borrow checker | A real cleanup subsystem is required |
| D13 | No general dynamic borrow checker; call-scoped guards at native boundaries only | Omits borrowing proofs while honoring native reference validity | Aliases tolerated by managed execution can fail when simultaneously passed to a native call |
| D14 | Concrete call contracts plus operation guards | Supplies enough information to execute safely without analyzing cold expressions | Some otherwise valid Rust needs annotations |
| D15 | Preserve unresolved literals; reject width-sensitive arithmetic without context | Prevents silent false test results caused by guessing a numeric type | Unannotated arithmetic can require a suffix or local annotation |
| D16 | Restricted nominal method dispatch; no obligation solver | Runs concrete and unbounded-generic methods without proving unrelated bounds | Conditional impl selection and associated-type inference are unsupported |
| D17 | Closed runtime facade for selected `core`, `alloc`, and `std` APIs | Produces useful tests without interpreting unsafe standard-library internals | Every supported API must be implemented and tested explicitly |
| D18 | Built-in macros and derives only on the live side; native expansion belongs to explicit Cargo preparation | Avoids macro hygiene and arbitrary compile-time execution blocking the runtime | User `macro_rules!` invocations and procedural macros are not executed |
| D19 | No guest closures in this release | Capture inference and closure ownership are independent language projects | Tests must use named functions and explicit data |
| D20 | One lazy LRIR machine from the beginning | Prevents divergence between a temporary AST interpreter and production semantics | The first scalar interpreter must already use explicit PCs and continuations |
| D21 | Native compilation units are observed straight-line regions | Never emits native code for an unvisited branch or uncalled callee | Frequent exits to the dispatcher and smaller artifacts |
| D22 | Generic helper lowering plus guarded primitive fast paths in the same JIT | Gives a simple correct baseline and a fixed useful optimization scope | No inlining, trace stitching, vectorization, or heap-load elimination |
| D23 | Dispatcher-owned guest calls | Keeps recursion, lazy entry, and cleanup off the native call stack | Guest-call overhead remains larger than native calls |
| D24 | Synchronous compilation at dispatcher boundaries | Eliminates races and unfinished artifacts | A hot region incurs a synchronous compilation pause; the worker watchdog bounds failures |
| D25 | One compiled artifact per observed region recipe | Bounds specialization and removes variant-selection complexity | A changed runtime shape takes the shared helper slow path |
| D26 | In-memory native cache only | Avoids executable-file trust, relocations across processes, and cache poisoning | Native code is lost when a worker is restarted |
| D27 | Module-arena retirement only between runs | Avoids calling freed native pointers | Old code occupies space until a safe run boundary |
| D28 | Capability broker from the first output operation | Keeps host effects behind a replaceable boundary | Some operations require an IPC round trip |
| D29 | Capability-mediated execution plus crash-isolated worker, not a hardened hostile-code sandbox | Delivers the execution system without pretending a JIT is an isolation mechanism | Do not run adversarial code or claim containment against JIT/runtime bugs |
| D30 | Read/write directory grants; injected environment; no networking or subprocess support | A finite, testable permission system covers initial test workloads | Network/process APIs remain unsupported even when users request a grant |
| D31 | Explicit offline Cargo preparation; generated workspace-member cdylib in hybrid mode | Cargo itself validates and reuses compatible artifacts | Initial bridge build/link is required; live runs never launch Cargo |
| D32 | No build/proc-macro execution in managed preparation or live execution; authorized native preparation delegates these to Cargo | Native dependencies can use their ordinary build ecosystem | Native preparation has ambient authority, even with Cargo offline |
| D33 | Deterministic serial test scheduling and structured results | Makes replay, debugging, and agent integration straightforward | Lower total throughput for independent large test suites |
| D34 | Native Rust is an oracle only for well-typed supported fixtures | Separates intended relaxed behavior from implementation bugs | Two categories of semantic tests must be maintained |
| D35 | Correctness, laziness, and measured JIT use are release gates | Prevents a fast but incorrect interpreter-only demonstration | Throughput targets cannot be met by silently skipping work or tests |

The external premise for D02 is specific: rustc describes THIR as typed and MIR construction as operating on THIR. The decision is architectural, not a claim that borrow-checking flags are unavailable.[^R01] Rust-analyzer documents a syntax layer distinct from semantic analysis; LRR reuses that syntax boundary, not its inference engine.[^R02] Query memoization and input/derived-fact separation are established compiler techniques; the small implementation below is our design, not a claim to reproduce all of rustc or Salsa.[^R03]

### 2.1 Non-negotiable invariants

Assign these identifiers to tests and review comments:

| Invariant | Requirement |
|---|---|
| I-LAZY | On the live side, no semantic operation preparation, expression-macro expansion, or Cranelift generation for an unentered executable path; explicit Cargo preparation is a separate fully compiled native island |
| I-ORDER | Every guest-visible operation happens once and in the prescribed order |
| I-SNAPSHOT | Every active frame, query, and installed entry used by a run belongs to its immutable semantic environment |
| I-MEMORY | Guest bytes, tags, references, and callable identities never become unchecked host pointers |
| I-OWNER | Every initialized non-Copy owned value has exactly one logical owner or one explicitly tracked transfer ticket |
| I-DROP | Normal completion and guest panic perform specified cleanup once; fatal runtime faults do not execute more guest code |
| I-EFFECT | Every managed effect is broker-authorized for that run; hybrid native effects require explicit whole-island authority and invalidate any whole-run sandbox claim |
| I-CACHE | Reuse is conditional on all positive and negative semantic dependencies |
| I-JIT | A native guard miss resumes at the uncommitted operation, never at the beginning of an already executed region |
| I-REPORT | Report actual backend entries, validation status, limitations, and effective security strength |

<a id="section-3"></a>

## 3. Build target, dependency lock, and bootstrap

### 3.1 Fixed host and guest environment

Build and test on `x86_64-unknown-linux-gnu`. Use Rust **1.99.0** for the host runtime and native oracle. The release announcement identifies this version; the syntax package manifest examined for this plan requires Rust 1.98, so the selected host toolchain is above that declared minimum.[^R04][^R05]

Guest pointer width is 64 bits. Guest integer overflow checks are always enabled. Guest floating point uses ordinary IEEE operations without fast-math. Guest `debug_assertions` is true. Guest `target_os="linux"`, `target_arch="x86_64"`, `target_endian="little"`, `target_pointer_width="64"`, and `unix` are true. No target CPU feature is exposed to guest `cfg`; native code generation may use detected host instructions, but generated artifacts never leave that worker.

Host source, live guest source, and generated bridge source use edition 2024. Native dependency editions remain those in their own manifests. The host runtime stays pinned below; the native build uses the explicitly recorded project toolchain, which may differ because only the defined C bridge ABI crosses the boundary.

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.99.0"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

### 3.2 Exact direct dependencies

Use these exact direct versions. Commit the resolved transitive `Cargo.lock`; all subsequent host builds use `--locked`. The versions below were inspected in current upstream package documentation. Their joint build is a required bootstrap test, not a claim of a completed build.[^R05][^R06][^R07]

```toml
[workspace]
members = ["crates/lrr", "xtask"]
resolver = "3"

[workspace.package]
version = "0.1.0"
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

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"

[profile.dev]
panic = "abort"
```

The TOML package's published version includes build metadata; the exact `=1.1.6` requirement selects that base version, and `Cargo.lock` records the complete resolution. Do not add Tokio, Wasmtime, Salsa, LLVM, `rustc_driver`, `hir_ty`, a GC library, a general-purpose plugin framework, or a filesystem-watcher dependency. `libloading` is confined to the explicit hybrid loader, and `toml_edit` to transactional bridge-workspace setup; their versioned APIs were checked separately.[^H14][^H15]

Use the standard library for process management, channels, deterministic `BTreeMap` indexes, timers, and file polling. Use `proptest` only in host tests. Runtime-owned macros and derives in the host code are unrelated to guest macro support.

### 3.3 First commands

These commands build the future repository; they do not refer to a preinstalled `lrr` executable:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
cargo +1.99.0 generate-lockfile
cargo +1.99.0 test --workspace --locked
cargo +1.99.0 run --locked -p xtask -- bootstrap-audit
cargo +1.99.0 build --release --locked -p lrr
```

`bootstrap-audit` must print the toolchain, direct dependency versions, pointer width, OS, parser smoke result, and native JIT smoke result. It fails on a nonmatching target. It also writes `artifacts/bootstrap.json`. The actual runtime must reject unsupported host targets at startup with `LRR-E-HOST-TARGET`; do not accidentally ship interpreter-only behavior as a completed native release.

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

Use one library crate with three binary entry points. Do not begin with fifteen mutually dependent crates. Rust module visibility enforces boundaries; `xtask architecture-audit` enforces forbidden imports. This removes package wiring work without weakening ownership boundaries.

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
    src/frontend/{mod,names,types,signatures,dispatch,thunks,constants}.rs
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
  artifacts/.gitkeep
```

Files are created when their numbered task begins. Do not fill the tree with `todo!()` stubs. Every public function returns the explicitly defined result/error type; do not use panics to represent unsupported guest behavior.

### 4.1 Import rules

`core` imports only standard library, Serde, and hashing support. `syntax` is the only module importing `ra_ap_syntax`. All syntax nodes stay on the worker's owner thread; expose owned IDs, ranges, and summaries elsewhere. The upstream AST type is not `Send`/`Sync` in the inspected API, and LRR has no reason to force it across threads.[^R05]

`db` and `frontend` do not import `effects::broker`, process creation, networking, filesystem reads, or the native JIT. `runtime` has no syntax or compiler dependency. `vm` consumes a small `Resolver` interface and LRIR; it does not create Cranelift objects. `jit` may import Cranelift and runtime ABI types; code generation consumes an immutable region plan, not the source database. `engine` composes these modules. `service::coordinator` owns host filesystem authority and subprocess invocation. The worker's only intended host communication is its framed protocol.

Only `jit::abi`, `jit::helpers`, and `jit::module_cache` may contain `unsafe`. The architecture audit rejects other local `unsafe` blocks. The runtime does not use unsafe Rust to store guest aggregates or references.

### 4.2 Stable internal interfaces

Expose these top-level operations:

```rust
// Result types and IDs are LRR-owned, not upstream parser types.
trait Resolver {
    fn thunk(&mut self, key: PcKey) -> Result<RecipeId, Diagnostic>;
    fn call_plan(&mut self, request: CallRequest) -> Result<CallPlan, Diagnostic>;
    fn descriptor(&mut self, key: TypeKey) -> Result<TypeId, Diagnostic>;
    fn drop_plan(&mut self, ty: TypeId) -> Result<DropPlan, Diagnostic>;
}

trait BrokerClient {
    fn request(&mut self, request: EffectRequest) -> Result<EffectReply, Diagnostic>;
}

// These interfaces are synchronous and owned by a single execution thread.
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
| Guest edition | 2024; exact supported semantics in Section 34 | Other live target editions: preparation error; Cargo-native dependencies retain their editions |
| Scalars | `()`, `bool`, `char`, `i8/i16/i32/i64/isize`, `u8/u16/u32/u64/usize`, `f32/f64` | `i128/u128`, SIMD: reached type/operation error |
| Literals | Integer bases, suffixes, decimal floats, Unicode chars, UTF-8 strings, byte strings | C strings and unsupported literal forms: reached error |
| Functions | Named functions, simple identifier/`_` parameters, explicit parameter types, recursion, function items/pointers with concrete signatures | Variadics, foreign ABI, async, generators: reached call error |
| Control flow | Blocks, `if`, `if let`, `while`, `while let`, top-level `&&` let chains in if/while, `loop`, labels, `break`, `continue`, `return`, `&&`, `||` | Pattern guards/let chains in match guards and async suspension: reached error |
| Local bindings | `let`, `mut`, annotations, tuple/struct destructuring, `ref`/`ref mut`, `let-else` | Complex unsupported patterns: error when tested/bound |
| Aggregates | Structs, tuple structs, enums, tuples, arrays, slices through references | Unions, arbitrary layout attributes, DST struct tails: reached schema error |
| Ownership | Runtime Copy/move, partial moves, replacement, guest references, logical Drop | No borrowing/lifetime proof, no raw pointers, no transmute |
| Generics | Concrete explicit/contextual type arguments; structural substitution; lifetime erasure | Const generics, higher-ranked types, associated-type projections, `impl Trait`/`dyn Trait`: reached contract error |
| Methods | Inherent and explicit trait methods under restricted dispatch rules | Bound-conditional selection, custom Deref/Index/operator traits: reached dispatch error |
| Constants | Pure primitive/tuple/array constant expressions and pure named calls under a restricted constant VM | Static variables, effects in constants, heap-owning constants: reached error |
| Patterns | Wildcards, bindings, literals, tuple/struct/enum patterns, `ref`, `&`, or-patterns with consistent bindings, guards | Range/slice-rest patterns and complex binding ambiguity: reached error |
| Macros | Exact built-in inventory in Section 18 | User declarative/procedural invocation: reached or scope-preparation error |
| Derives | Built-in `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq` | Other derives block use of affected generated behavior; unknown transforming attributes block affected scope |
| Closures | None | Creation of a closure expression: reached unsupported error; cold closures remain untouched |
| Collections | Fixed `Vec`, `String`, slices, range iterator, `Option`, `Result` facade | `HashMap`, `Rc`, `Arc`, `Box`, custom allocators, arbitrary adapters: reached error |
| Iteration | `for` over supported ranges, arrays, Vec, slices; explicit built-in iterator `next` | User-defined Iterator/IntoIterator and closure combinators: reached error |
| Error flow | Guest panic, built-in assertions, `?` for exact Option/Result types, expected-panic tests | `catch_unwind`, custom Try/From residual conversion: reached error |
| Effects | Managed: captured output, granted files, injected environment. Hybrid: native calls additionally have ambient worker authority | Managed network/process/clock/random/native APIs remain unsupported; native effects are not mediated |
| Cargo | Managed metadata subset plus explicit hybrid native bridge preparation and artifact reuse | Unsupported live macros/build outputs remain errors; native dependencies are compiled by Cargo, never interpreted |
| JIT | Observed regions, helper operations, guarded primitive fast paths | Whole-function codegen, traces spanning branches, guest inlining, SIMD: not implemented |
| Security (managed) | Capability mediation, managed values, supervised worker, bounded resources | Hardened hostile-code containment: not claimed; required-hardening request fails closed |

A syntactically valid `unsafe { ... }` block itself is a transparent lexical block. It grants no authority. Executed supported safe operations inside it behave normally; raw pointer/FFI/assembly operations still fail. An `unsafe fn` may be called under the same checked runtime conventions; its signature does not enable native access.

### 5.1 Compatibility reporting

Every result contains:

```json
{
  "runtime_version": "0.1.0",
  "semantics_profile": "lrr-2024-v1",
  "rust_validation": "live_not_performed",
  "sandbox_strength": "capability_mediation",
  "guest_target": "x86_64-unknown-linux-gnu",
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

Each digest domain starts with `lrr/v1/`, for example `lrr/v1/body`. Length-prefix each component so concatenation cannot confuse `ab,c` with `a,bc`. For input content, use the exact bytes. For normalized declaration summaries, encode token kinds and text without nonsemantic whitespace; retain literal text, attributes, visibility, generic syntax, and supported cfg information.

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
    guest_edition: u16,       // exactly 2024
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

No worker-local ID or Cranelift memory image is loaded from disk. Stored snapshots and portable query records contain stable keys and explicitly encoded data; intern/remap those keys in the current worker. A Cargo-native generation is a separately authorized, validated linked library, not a deserialized JIT/query memory image (Sections 38–41).

<a id="section-7"></a>

## 7. Source capture and immutable runs

### 7.1 Coordinator responsibilities

The coordinator is the only component that discovers or reads host project files. It captures source bytes, manifests, the lockfile, explicit environment inputs, and the supported crate graph. It gives the worker an immutable bundle. A worker query cannot open a path, ask for an arbitrary file, or read the ambient environment.

For standalone execution, the source root is the directory containing the root `.rs` file. Discover external `mod` declarations using syntax. Resolve `name.rs` and `name/mod.rs` according to supported module rules; both existing is `LRR-E-MODULE-AMBIGUOUS`. `#[path]` is accepted only when its literal relative path stays within the approved root. Source symlinks escaping the approved root are refused. Runtime file-read permission is distinct from permission to load source.

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

`lrr watch` polls known source/manifest paths every 100 milliseconds, using metadata to identify candidates and content hashing before publication. A changed Cargo configuration file, package manifest, or lockfile invalidates preparation and stops automatic running with `LRR-E-PREPARE-STALE`. It does not run Cargo automatically. Source-body edits publish a new snapshot and rerun the specified filter.

Only one test request is active. When another source edit arrives, request cancellation and queue the newest snapshot. The supervisor may terminate a stuck worker. This is application behavior being implemented; it does not require the implementation agent to work asynchronously.

<a id="section-8"></a>

## 8. Syntax adapter, declaration completeness, and lookup

### 8.1 Adapter API

Expose these owned structures from `syntax`:

```rust
struct ParsedFile {
    content: Digest,
    edition: u16,
    errors: Vec<ParseDiagnostic>,
    // The upstream syntax tree remains private to this module.
}

struct ItemOutline {
    def_key: DefKey,
    header: HeaderRecipe,
    body: Option<BodyHandle>,
    attributes: Vec<AttributeRecipe>,
    visibility: Visibility,
    declaration_span: SourceSpan,
}

struct BodyHandle { def_key: Digest, body_hash: Digest, range: SourceRange }
```

The parser may parse an entire file, including unused function syntax. Outlining extracts headers and opaque body handles. It must not resolve a type name in an unused parameter, check a body, expand an expression macro, or request a callee.

A parse diagnostic blocks execution only if it prevents reliable identification of the selected declaration or the entered syntax node. An invalid cold expression that can be delimited is stored as an opaque node with a delayed diagnostic. Unmatched braces or an unterminated literal may make the selected body structurally unavailable; return `LRR-E-PARSE-BLOCKS-ENTRY` rather than guessing boundaries.

### 8.2 Attribute policy

Classify every attribute during outline construction:

| Class | Handling |
|---|---|
| `doc`, `allow`, `warn`, `deny`, `forbid`, `inline`, `cold`, `must_use`, `deprecated` | Retain for diagnostics; no execution effect |
| `cfg`, `cfg_attr` | Evaluate the supported syntax against captured cfg values; structurally remove disabled items |
| `test`, `ignore`, `should_panic` | Record for test discovery |
| Supported derives | Record a lazy descriptor/operation policy; do not expand a body |
| `repr` | Record; reached nominal schema accepts default representation only and rejects other requested representations |
| `path` on a module | Source-loader rule, bounded to approved roots |
| Unknown item-transforming attribute | Mark the affected declaration/scope incomplete |
| Tool attributes such as `rustfmt::skip` | Ignore only the exact documented allowlist: `rustfmt::skip`, `clippy::*` lint forms |

Unknown attributes are not assumed harmless. An unresolved transformation can create, delete, or change names. Before using the affected declaration or requiring completeness of its scope, report `LRR-E-ATTRIBUTE-UNSUPPORTED`.

### 8.3 Namespace indexes

Maintain separate value, type, and macro namespaces. In module scopes, collect item headers before executable lookup. Within an entered block, nested item declarations are visible according to lexical item scope even if written after a call; collect their headers structurally without entering their bodies.

Local `let` bindings are different: a binding becomes active only after its initializer and pattern binding succeed. Shadowing creates another `LocalId`. A reference to a local before activation must not silently bind to that inactive local; resolution continues to the prior lexical binding or other permitted namespace.

Store each name bucket as a sorted candidate vector plus a completeness fingerprint. A bucket's fingerprint includes absence. An empty bucket is a real dependency. Adding a declaration must invalidate a memoized failed lookup.

### 8.4 Supported path lookup algorithm

For a reached value/type path:

1. Determine its namespace from operation context, not from spelling.
2. Handle `crate`, `self`, repeated `super`, and an external crate alias using the selected crate instance.
3. For an unqualified first segment, search active locals, nested item scope, the enclosing module's direct declarations/imports, and the supported prelude in that order.
4. Resolve explicit imports and re-exports using per-name buckets. Explicit local declarations shadow glob-derived candidates; multiple different glob candidates are ambiguous.
5. Traverse subsequent path segments through module/type namespaces as appropriate.
6. Apply visibility at the reached use. Support private, `pub`, `pub(crate)`, and `pub(super)`; `pub(in path)` is a reached visibility diagnostic in v1.
7. Return exactly one identity, a missing-name diagnostic, or an ambiguity diagnostic containing sorted candidates.

Import graph resolution uses a worklist to a fixed point. The maximum number of import edges processed is 100,000 per revision. Cyclic re-exports without an originating definition produce no candidate; recursive traversal is not implemented through unbounded host recursion. A still-incomplete scope cannot be treated as an empty scope.

### 8.5 Macros and completeness

A `macro_rules!` definition can be indexed without expanding or validating its body. Its invocation is unsupported. A reached expression invocation returns `LRR-E-MACRO-UNSUPPORTED`. An item-position invocation marks that item's enclosing name scope incomplete; a requested lookup requiring that scope returns `LRR-E-SCOPE-PREPARATION`. This may happen before any guest statement because the macro could define a required name.

Before resolving a method, complete the impl-header universe for the selected guest crate graph. An unexpanded item macro that could add impls blocks this universe. Do not pick a method based on whichever files happened to execute first. No method bodies are entered during this completion step.

### 8.6 Called-body structural index

On the first actual call, scan that function's syntax to build a structural index: node kinds, parent/child roles, lexical scopes, local binding declarations, loop labels, source spans, and fixed temporary slot numbers. Do not resolve executable names, evaluate constants, expand macros, or diagnose unsupported expressions during this scan.

This is permitted structural work on an entered body. The index may include syntax from untaken branches, but those nodes have no executable recipes until reached. An opaque unsupported node gets a node ID and span; it is not rejected during indexing. Nested function/closure bodies remain separate opaque body handles.

Allocate node IDs by preorder and assign each expression a value temporary slot and a place temporary slot. These are logical register numbers, not guest memory allocations. Actual cells are allocated when the expression executes. Function parameters and local bindings have independent local slots. Enforce a maximum of 100,000 nodes and 200,000 temporary slots per entered function.

<a id="section-9"></a>

## 9. Query database: exact implementation

### 9.1 Database ownership and purity

`SourceDb` lives in the worker and is accessed only by its execution thread. It owns parsed source, declaration indexes, body structures, and pure recipes. It does not own mutable guest state or native entry pointers. Runtime executions, profile counters, permissions, and native publication are not queries.

Query providers may read only explicit input records or other queries. They cannot call `std::fs`, `std::env`, `SystemTime`, process creation, the effect broker, or the ordinary VM. Constant evaluation is a small pure evaluator owned by `frontend::constants`, not a call back into the effectful guest engine.

### 9.2 Input records

Each input record is identified by `InputKey` and contains an immutable value plus an output fingerprint. Inputs are `SourceBlob`, `DefinitionHeader`, `DefinitionBody`, `ScopeMembers`, `ImportEdges`, `ImplHeaders`, `CrateCfg`, `DependencyAliases`, `FacadeSchema`, and `CompileEnv`. Missing input is represented explicitly and fingerprinted. Revision publication replaces changed input values and increments a database revision counter.

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
| `Dispatch` | Method request, receiver shape key, lexical traits, impl-universe digest | Target identity and explicit receiver adjustment |
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

A successful path lookup records every searched bucket up to and including the winning scope. A failed lookup records all searched buckets. Method dispatch records the complete applicable impl-header bucket, including candidates rejected structurally or requiring unsupported obligations. The fingerprint includes the fact that scope expansion is complete.

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

`TypeDescriptor` contains the key, shape category, an optional deferred field recipe, Copy status, an optional destructor identity, and a representation version. Lifetimes are erased. A reference to a nominal type does not force its field schema. A value construction, field operation, or reached drop may force it. An alias expands on demand with an active-alias stack; a cycle reports `LRR-E-TYPE-ALIAS-CYCLE`.

Generic type parameters map to concrete `TypeId`s in a frame's `GenericEnv`. An unused parameter can remain unbound until an operation needs it. Merely reading a signature does not solve trait obligations or traverse generic bounds. Const generic parameters, higher-ranked types, `dyn Trait`, `impl Trait`, and associated-type projections are outside the profile and fail only when their concrete meaning is required.

### 11.2 Expected-context propagation

Use `Hint = Unknown | Concrete(TypeId) | Tuple(Box<[Hint]>) | Array(Box<Hint>, u32)`. Hints are frame state attached to expression slots, not global inference variables. They travel from a reached parent into the child about to execute. They never inspect future statements or untaken branches.

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

### 11.3 Unsuffixed literals: never silently pick a width

Represent an unsuffixed integer as `NumericLiteral { sign, magnitude: u128, spelling, origin }`; the parser rejects magnitudes beyond this internal range. This internal u128 is a literal parser representation, not a supported guest integer type. An unsuffixed float stores its decimal spelling until a reached f32/f64 context exists. Do not execute arithmetic in a “universal number” representation and narrow later.

Materialization rules are deterministic:

1. A suffix supplies the type immediately. Validate representability; negative minimum values receive the unary-negation treatment below.
2. A concrete numeric hint supplies the type, with a checked literal conversion.
3. A concrete other operand supplies the type when an operation requires matching primitive operands.
4. A reached typed argument, field, return, index, or assignment supplies context.
5. If an executed arithmetic/comparison/cast-sensitive operation still lacks a concrete type, stop with `LRR-E-TYPE-CONTEXT` before performing that operation. Include a suggested explicit suffix or local annotation in the diagnostic.

An unresolved literal payload is a checked LiteralId in a run-owned arena. Its record contains the numeric spelling/magnitude and a one-time optional concrete TypeId binding. Copies of that unresolved literal share the record, so materializing a later copy fixes the same numeric variable for prior aliases; conflicting later context fails rather than reinterpreting it. Source recipes only store immutable spelling data and allocate a fresh literal record on each evaluation. Numeric materialization validates range and reserves its result before setting the binding.

A literal may cross a local binding while unresolved. A literal cannot become a generic type argument simply because the compiler would conventionally default it to i32. `identity(1)` requires context; `identity(1_i64)` works. `Vec::new()` requires a concrete expected `Vec<T>` or explicit `Vec::<T>::new()`. Empty generic containers do not retrospectively acquire a type from a later push.

These examples are normative:

```rust
fn use64(x: u64) -> u64 { x }
fn works() -> u64 { let x = 3; use64(x) }
fn works2() -> u64 { 3 + 4 }          // Return context reaches arithmetic.
fn works3() { let x = 3; assert_eq!(x, 3_u64); }
fn fails() { let x = 3 + 4; use64(x); } // No future-use inference.
```

`works3` is valid because the assert equality operation materializes an unresolved operand using the other operand's concrete descriptor. Formatting an unresolved number without type-sensitive arithmetic prints its source numeric value and does not materialize the original cell. This is limited to diagnostic/display formatting; it must not influence dispatch or arithmetic.

### 11.4 Arithmetic contracts

Implement all arithmetic in `vm::numeric` before native lowering. The interpreter and JIT must agree on these rules. The Rust Reference documents several non-obvious overflow cases, including signed division overflow and shift counts; use it as the oracle for supported concrete primitive operations.[^R08]

- Integer `+`, `-`, `*`, unary `-`, `/`, `%`, shifts, and compound assignments are checked. Overflow produces a guest panic, not a host panic. Use `checked_*` and explicit shift count checks. Signed minimum divided or remaindered by -1 is overflow. Division/remainder by zero panics.
- For a signed, explicitly suffixed minimum literal such as `-128_i8`, recognize the unary-negation-plus-literal syntactic pair and construct the minimum directly. Do not first reject `128_i8` as an independently evaluated positive literal. Negating an already materialized minimum value still overflows.
- Shift operands need not have the same integer type. Require an integer right operand, reject a negative count, and require the count to be less than the left width. Unsuffixed shift counts receive `u32` context. This choice is part of the execution profile.
- Bitwise operators operate on concrete same-type integers, or bool/bool where Rust permits it. `&&`/`||` operate only on bool and short-circuit. No user operator traits are dispatched.
- Ordering and equality are implemented for primitive same-type values, strings/slices under the facade rules, and supported derived comparisons. Float comparisons preserve NaN behavior. No fast-math, reassociation, or fused multiply-add substitution is allowed.
- Integer `as` conversions implement Rust's truncation/sign-extension rules. Float-to-integer conversion saturates according to Rust's primitive cast semantics. Float-to-float and integer-to-float use the host primitive cast with matched source/destination types. `char` casts are limited to supported valid primitive casts; integer-to-char is only from u8. Ref-to-pointer, pointer-to-int, and representation casts fail.
- `usize`/`isize` are 64-bit guest types but retain separate identities from u64/i64. No implicit conversion between them.
- f32 calculations round at each guest primitive operation, not after a chain calculated in f64. Test raw bit patterns where meaningful, with explicit treatment of unspecified NaN payloads.

The operation dispatch table must enumerate every `(operator, left kind, right kind)` implementation. A missing entry produces `LRR-E-OP-TYPE`, never an `unreachable!()` reached through guest input.

### 11.5 Standard coercions

The only implicit coercions are literal materialization, shared reborrowing, `&mut T` to `&T`, and array-reference to same-element slice-reference. Function items coerce to a supported concrete function-pointer signature after parameter/result descriptor comparison. There is no closure coercion, trait-object coercion, arbitrary deref coercion, integer widening, string allocation, or error conversion.

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

Assert size 16 and alignment 8 in tests. `kind` discriminants are: 0 empty/internal, 1 unit, 2 bool, 3 char, 4 signed integer, 5 unsigned integer, 6 f32 bits, 7 f64 bits, 8 object handle, 9 reference handle, 10 function item, 11 unresolved integer, 12 unresolved float, 13 built-in iterator, 14 opaque native object handle (hybrid-only). Kind 14 payload is a checked run-local NativeObjectRecord arena ID, never an address or foreign handle accepted from the public protocol; its descriptor is opaque and non-Copy (Section 39). Store sign-extended signed integers in payload; validate the declared width at construction. Bool payload is 0 or 1. Char payload is a Unicode scalar value. Validate these invariants at all host entry points and debug-check internal producers.

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

Every access checks epoch, nonzero index, bounds, and occupied entry. Return `LRR-E-STALE-HANDLE` on failure. Handle arithmetic is never available to the guest. Cap cumulative handle allocations as well as live bytes; a loop that allocates and drops cannot force unlimited tombstones. All quota failures are explicit runtime faults.

Keep separate typed arenas for cells, objects, references, frames, iterators, unresolved-literal records, transfer tickets, and hybrid NativeObjectRecords. Typed Rust wrappers prevent passing a cell handle to an object lookup even if the bit pattern happens to exist in both arenas.

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

Sized primitives, supported function pointers/items, and shared references are Copy. Unsized str/slice descriptors cannot be read as standalone owning scalar values. Mutable references are not Copy. Tuples and arrays are Copy only if every element type is Copy. A nominal type is Copy only if it has the supported `#[derive(Copy)]` marker and all reached field descriptors are Copy; an explicit manual `impl Copy` is rejected when Copy policy is needed. `Clone` is not Copy. Vec and String are not Copy.

Computing Copy policy is a pure, lazy descriptor query. It may examine fields of the used type, not methods or unrelated types. Recursive owning aggregates are invalid without indirection; references terminate descriptor recursion.

Read operands use `copy_from` for Copy types and `move_from` otherwise, except in borrow/display/comparison contexts explicitly defined to peek or reborrow. Never infer that a bitwise duplicated `ValueRepr` is a legal guest copy.

A type with a user Drop method cannot have an owning non-Copy field moved out by ordinary projection. Report `LRR-E-PARTIAL-MOVE-DROP`. This prevents executing its Drop body over an incompletely initialized self without implementing additional Rust restrictions. Types without user Drop support partial moves and drop only their remaining live fields.

### 13.4 Assignment order and replacement

Simple assignment evaluates the right-hand expression before evaluating the destination place, matching Rust's specified assignment operand order.[^R08] Store the RHS in a transfer ticket. Resolve the LHS and validate the destination. Replace atomically, then schedule dropping the replaced old value before proceeding to the next source operation.

Compound primitive assignment evaluates the destination place and its current scalar, then the RHS, then performs one checked primitive operation and replacement. Support compound assignment only for primitive scalar destinations and supported primitive operators. User `AddAssign` and similar overloads are excluded. A failed arithmetic operation leaves the destination unchanged.

### 13.5 Lexical cleanup and temporary scopes

Each frame has a stack of `ScopeRecord`s containing locals in successful initialization order, temporary cells, and a parent scope ID. A scope exit schedules locals in reverse initialization order; fields of an aggregate are scheduled in declaration order. Function parameters are registered in parameter order and therefore cleaned up in reverse order. Array and Vec elements are cleaned up in increasing index order under the LRR profile.

For supported Rust 2024 syntax, implement the exact scope rules in Section 34. A full expression statement's non-extended temporaries end with that statement. Non-extended block-tail temporaries end after evaluation of the tail, before the block's locals are destroyed. A failed if-let/if-let-chain cleans its pattern-condition temporaries and bindings before entering else; successful pattern-condition storage lives through the consequent. Ordinary boolean operands have their own shorter temporary scopes. Direct syntactically extending let borrows still extend their operands to the containing block; propagation is specified in Section 18.4. A match scrutinee is not itself a temporary boundary. Do not implement the edition change by naive match desugaring.[^R09][^H01][^H02]

Create dedicated native differential fixtures for each supported extension form and for a borrowed temporary passed through a function. If a syntactic temporary-extension form is not implemented, reject that reached borrow with `LRR-E-TEMPORARY-SCOPE`; do not silently choose a shorter or longer lifetime. The release feature manifest must enumerate the forms above, and all must pass before release.

### 13.6 Drop machine

Destruction is a VM state machine, never Rust host stack recursion:

1. Mark a live owner `Dropping`, preventing duplicate guest drop.
2. If its concrete type has a supported user `Drop::drop`, create a mutable self reference valid for this destructor activation and push a normal guest call frame.
3. After that call returns, traverse still-live fields in the specified order and push their drop obligations.
4. Release facade-owned buffers after their live elements finish dropping.
5. Mark the owner dropped, retire its storage, and continue the pending control transfer.

For a user Drop signature, require exactly `fn drop(&mut self)` returning unit. Select the unambiguous structurally matching impl without proving bounds. A bounded plausible impl triggers `LRR-E-DISPATCH-OBLIGATION` when destruction actually reaches that type. A cold Drop body is not prepared until called.

DropRecipe queries store immediate child descriptor/field references without recursively expanding every child DropRecipe. Recursive container types therefore produce finite recipes; the cleanup machine follows only the finite live object graph.

Cleanup tasks store concrete owner IDs and a next-field index. A yield, effect, or native exit in a destructor preserves this state. Do not invoke one destructor twice on resumption. Locals not successfully initialized are not registered for cleanup; partially initialized aggregate construction holds each completed field in a construction guard until the final aggregate commits.

### 13.7 Panic, fault, and leak distinctions

`GuestPanic` is explicit guest control flow carrying a bounded message and source origin. It unwinds guest frames and executes the cleanup machine. A second guest panic while already unwinding terminates the test immediately with `LRR-E-DOUBLE-PANIC`. `#[should_panic]` recognizes only the first ordinary guest panic after successful cleanup; double panic fails.

`RuntimeFault` means execution cannot proceed under this profile: missing name, unsupported construct, stale reference, resource exhaustion, incompatible call shape, or denied operation classified as fatal. It stops guest execution immediately and does not run guest destructors. The run is marked failed, all physical arenas/resources are reclaimed, and already completed output/effects remain recorded. This avoids executing arbitrary extra guest code after an invariant failure. Clearly distinguish this from a Rust panic in diagnostics.

`mem::forget` transfers a managed owner to a run-lifetime leak list; it does not execute guest Drop. End-of-run physical reclamation still frees its managed representation. An opaque native value instead follows the explicit process-lifetime forgotten-store policy in Section 44.7; never physically free it through the worker allocator or invoke native Drop as a substitute for forgetting. No forgotten value can be recovered through an ownership path, though still-valid guest references may read its intentionally live storage until the run ends.

Host code must not panic for guest input. Release host panic aborts the worker. The coordinator reports `LRR-E-WORKER-CRASH` and invalidates all worker-local cache telemetry for that process.

<a id="section-14"></a>

## 14. Function calls, generics, and method selection

### 14.1 Call packet and boundary checks

```rust
pub struct CallPacket {
    pub target: CallableId,
    pub signature: SignatureId,
    pub args: Vec<TransferId>,
    pub generic_env: GenericEnv,
    pub result_hint: Hint,
    pub continuation: ReturnContinuation,
    pub origin: Origin,
}
```

No packet stores an unchecked native function pointer. A callable is a named source definition, supported function item/pointer, fixed facade ID, or NativeBindingKey from the prepared native schema. Cranelift region pointers remain private execution artifacts. NativeBinding targets use the explicit boundary machine in Section 40; they never create a guest frame for a native body. Steps 6–8 below describe source callees; native targets instead reserve/stage/enter/decode through that boundary machine after the same argument evaluation and contract preparation.

Call execution order is fixed:

1. Evaluate the callee expression or method receiver. Preserve a receiver that is a place as PlaceOrValue without moving it; inspect its descriptor, select autoref/consuming adjustment, and perform that adjustment before arguments. A temporary owning receiver is retained until the call's temporary scope ends. Resolve a direct named callee's header; a missing callable fails here, before arguments. This is an explicit behavior for otherwise invalid Rust.
2. Instantiate whatever parameter hints are known from explicit type arguments, receiver type, and expected result. Do not inspect the callee body or its callees.
3. Evaluate arguments left-to-right, each exactly once, into transfer tickets. An excess argument is still evaluated; argument-count failure occurs after all written arguments have evaluated.
4. Bind remaining generic parameters structurally against concrete actual descriptors. Materialize unresolved literals only from known concrete parameter context. Never infer a generic from a default numeric width.
5. Apply the closed coercion list. Check actual arity and each concrete parameter contract. Do not prove where-clauses or bounds.
6. Reserve the entire callee frame and parameter cells. Transfer arguments in source parameter order. On reservation failure, preserve and then physically reclaim owned temporaries under fault handling without replay.
7. Publish a guest call event, set the caller continuation, push the frame, and dispatch its first PC. The body becomes a semantic query only at this point.
8. On normal return, evaluate the returned value under the instantiated return hint, apply permitted coercions, and verify the concrete return contract. Then perform frame cleanup and transfer the result to the caller's result cell.

For an unbound generic occurring only in a return contract, the actual returned descriptor may bind it at the reached return. A caller cannot use an unbound result as a concrete descriptor before the value exists. Conflicting bindings fail with `LRR-E-GENERIC-BINDING`.

### 14.2 Generic substitution algorithm

Use recursive structural unification of a **signature recipe with an actual runtime descriptor**, not an expression inference engine:

- `Param(T)` binds or compares an environment entry.
- A primitive recipe requires exact identity.
- `Vec<T>`, tuple, array of known length, reference, and nominal constructors compare the outer constructor then recurse into type arguments.
- A lifetime argument is discarded.
- A trait bound is stored for diagnostics, not visited as an obligation.
- An associated projection or opaque type reports unsupported only if this boundary requires its concrete descriptor.

Limit substitution depth to 128. Preserve actual nominal identities: equal field layouts do not make two structs interchangeable. Do not instantiate machine code per generic argument list; the baseline IR is shape-polymorphic and uses per-frame descriptors. Primitive native guards specialize observed operations without creating a Rust monomorphization pipeline.

### 14.3 Return and tail contexts

All paths can be malformed except the one actually returning. A function declared `-> i64` may have a cold branch returning a String; calling a path that returns i64 succeeds. Taking the String path reaches `LRR-E-CALL-RETURN-TYPE` at the return boundary. This behavior is central to the product and needs a dedicated fixture.

Falling off a body without a value produces unit and is checked against the return contract. `return` without expression produces unit. `!` is supported only as the declared result of built-in panic/unreachable operations; a user never-type signature is excluded when a boundary requires it.

### 14.4 Receiver candidates and dispatch completeness

Rust method lookup has ordered dereference/autoref receiver candidates; the order matters, including cases where an `&self` method is selected before a different mutable receiver candidate.[^R10] Implement this restricted ordered sequence:

1. Start with the actual receiver descriptor.
2. Repeatedly peel only built-in guest reference descriptors, recording each dereferenced type. Do not invoke user `Deref`.
3. For each recorded type T, try T, `&T`, and `&mut T` in that order, omitting inaccessible mutable candidates but not reordering the rest.
4. At each receiver candidate, inspect inherent methods first. If none are applicable, inspect methods from explicitly visible traits and a generic parameter's declared trait names. Read all relevant impl headers and completeness tokens before declaring uniqueness.
5. At the first candidate rank with applicable methods, require exactly one. Stop searching lower ranks. A plausible bounded candidate whose applicability would need trait proof makes the rank unresolved and returns `LRR-E-DISPATCH-OBLIGATION`; it cannot simply be discarded to choose another method.

A candidate is structurally applicable if its receiver/impl type constructors match the actual descriptor under generic substitution and its method name matches. Arbitrary where-clauses, blanket impl conditions, specialization, negative impl reasoning, and associated-type equalities are not solved.

An exact, unbounded trait impl is supported. An unbounded generic impl such as `impl<T> Wrapper<T>` is supported through structural substitution. An impl with a bound remains unsupported for automatic selection when that bound would determine applicability, even if a human can see it is satisfied.

At a selected generic function body, a bound such as `T: Describe` authorizes the *trait name* for lookup; it does not certify that the actual T implements it. The first reached `x.describe()` selects a concrete structurally matching impl or fails. An unused bound has zero dispatch cost and is not validated.

### 14.5 Explicit calls and trait defaults

`Type::method(...)` and `<Type as Trait>::method(...)` identify a narrower namespace. Explicit qualification eliminates unrelated name candidates but does not waive receiver or argument contracts. Resolve the named impl structurally; a bounded required impl still reports the obligation diagnostic.

For a selected trait implementation lacking a method override, use the trait's default body if present. Its `Self` binds to the concrete receiver type. Do not check whether every required method in the trait is implemented. Missing methods fail only when called. Default bodies themselves are lazy like ordinary bodies.

Function pointer equality, pointer arithmetic, C ABI signatures, native symbol names, variadics, and guest `extern` calls are unsupported. A same-source `extern` declaration can be indexed cold but fails on invocation. Do not search the host process for a matching symbol.
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

The `Op` enum has exactly the following initial variants. Add no variant without updating the interpreter, verifier, effect classification, recorder, native lowering table, and an opcode fixture.

| Opcode | Inputs / output | Required behavior |
|---|---|---|
| `Literal` | literal recipe, hint → slot | Allocate/materialize literal, or make a shared unresolved-literal record. |
| `Unit` | → slot | Initialize unit without allocation of an owning object. |
| `ResolveValue` | name recipe → value/place slot | Resolve only this reached name in the correct namespace. |
| `MakePlace` | local or temporary identity → place slot | Produce checked location, not value read. |
| `ProjectField` | place/value, field recipe → place | Force only the used schema; validate field. |
| `ProjectIndex` | base, usize slot → place | Check index and create fixed projection. |
| `Deref` | ref value → place | Validate lease before exposing place. |
| `Read` | place, read mode → value slot | Copy/move/peek according to the selected context. |
| `Borrow` | place, access → reference slot | Make checked lease, no alias counting. |
| `SetHint` | child slot, hint recipe | Set context before entering the child. |
| `Unary` | operator, value slot → slot | Execute closed primitive table. |
| `Binary` | operator, two value slots → slot | Execute closed primitive table or supported derived equality. |
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
| `TryUnwrap` | Option/Result value → value/control | Success payload or early return with exact error type. |
| `FacadePure` | fixed facade ID, arguments → slot | Bounded no-IO primitive/container operation. |
| `EffectPrepare` | fixed effect ID, arguments | Validate guest arguments and create pending broker request. |
| `PanicPrepare` | message recipe | Prepare an explicit guest panic record; the control exit activates it. |
| `Poll` | no guest value | Check fuel/cancel/deadline epoch. |
| `DropStart` | owner | Schedule explicit cleanup work. |
| `Unsupported` | diagnostic template | Reached profile fault. |

Place slots, value slots, packet slots, and guard slots are distinct `SlotKind`s. A structural slot can be reused on the next loop iteration only after its previous scope incarnation has retired or transferred all ownership. The verifier catches a recipe addressing the wrong slot kind; runtime checks catch dynamic lifecycle mistakes.

### 15.2 Recipe verifier

Validate immediately after pure construction and before installation. Check: nonempty bounded operation list; valid node/phase keys; all slot IDs within the body's structural slot count; allowed operand/output kinds; each output written at most once per recipe unless the opcode explicitly mutates a place; terminal exit exactly once; valid `Origin`; no direct native address; no host filesystem path; no unknown facade ID; no call or effect hidden in an opcode marked pure.

The verifier does not type-check guest expressions, prove branch conditions, inspect successor recipes, or resolve a missing name. An `Unsupported` operation is a valid recipe that produces a guest-visible fault only when executed. Structural verifier failure is `LRR-E-INTERNAL-IR`, a runtime implementation defect.

### 15.3 Expression phase machines

Use the following phase templates. Every named phase is a deterministic `Phase` enum value or indexed value. Child enumeration reads syntax only; asking for a phase recipe does not ask for a child's recipe.

**Literal/path.** `Enter` creates a literal or resolves one value, then `Finish` transfers the result to the parent's structurally determined continuation. Paths in place context return a place without prematurely copying/moving the value.

**Ordinary binary.** `Enter` sets left hint and enters left. `AfterLeft` stores the left result, derives right hint where possible, and enters right. `Apply` invokes one primitive operation after both have completed. `Finish` transfers the result. For a left unresolved literal and right concrete value, materialize the left only at Apply. The left and right expressions are never rerun.

**Short circuit.** `Enter` evaluates the left under bool context. `Select` validates bool. For `false && rhs` or `true || rhs`, it produces the known bool and finishes. Otherwise it enters RHS under bool context. No RHS semantic recipe exists until selected.

**Block.** `Enter` activates its scope. For each statement i, `Statement(i)` enters only that statement; `StatementCleanup(i)` cleans that statement's temporary scope then advances. `Tail` enters the actual tail under the block's result hint or produces unit. `Exit` transfers the tail result out before leaving the block's local scope. Block-tail temporary ownership follows its separately annotated temporary scope.

**Let.** `Enter` prepares only the written annotation and pattern header needed for binding hints, without looking at later statements. `AfterInit` stores the initializer; `Bind` matches the irrefutable pattern. A failed ordinary let pattern reports `LRR-E-PATTERN-REFUTABLE` when reached. A let-else failure enters the else block; normal completion of that else block is `LRR-E-LET-ELSE-DIVERGE` because the profile requires a control transfer. Register successful bindings in left-to-right pattern order.

**If.** `Enter` evaluates condition under bool. `Select` chooses one arm or unit if no else. `Join` transfers selected arm result. Do not unify arm result types. The surrounding reached storage/call contract checks the selected result.

**If let.** Evaluate the scrutinee once; `Test` runs non-consuming pattern matching. On success commit bindings into a condition scope and enter the consequent. On failure destroy that condition scope before entering else. On success destroy consequent-local state, then condition bindings/remaining scrutinee temporaries, before Join. The owned result is transferred out first. Let chains generalize this state machine in Section 34; the previous edition’s keep-alive-through-else behavior is forbidden.

**While/while let.** `Enter` activates loop control record. Each `PollCondition` creates fresh condition temporaries; evaluate condition/scrutinee; `SelectBody` chooses body or exit. `AfterBody` cleans iteration scope and returns to PollCondition. A while-let scrutinee lives through that iteration body. Normal loop result is unit.

**Loop.** `Enter` activates a loop record and enters body. `AfterBody` cleans iteration locals, polls, and starts again. A reached `break value` transfers value to the loop's result destination after cleaning intervening scopes. Value-bearing breaks from while/for are rejected. `continue` never skips cleanup.

**Assignment.** `Enter` executes RHS first. `EvaluateDestination` resolves/evaluates LHS place. `Commit` replaces and enters old-value cleanup. `AfterDrop` returns unit. A fault resolving LHS leaves RHS ownership in a temporary until physical fault cleanup.

**Function/method call.** `Enter` evaluates target/receiver. `ResolveHeader` performs CallPrepare. For each argument i, `Argument(i)` enters that argument with available hint; `AfterArgument(i)` transfers it into the packet. `Invoke` seals the packet and returns a call exit to the dispatcher. `Resume` consumes the completed result. Method receiver adjustment is frozen before argument evaluation; do not re-evaluate an effectful receiver.

**Struct/tuple/array constructor.** Begin construction guard, evaluate written initializers left-to-right in source order, transfer each successful value into its field slot, then commit. Struct field declaration order controls storage/drop; written order controls evaluation. Struct update `..base` is excluded. Repeated array `[e; N]` evaluates e once, then requires Copy or a supported constant scalar recipe; it does not clone non-Copy values. N is a reached bounded pure constant. Empty arrays require element context.

**Match.** `Enter` evaluates scrutinee once. `TestArm(i)` prepares only that arm's pattern. A mismatch advances to the next arm. A match creates pending bindings, evaluates guard if present, and commits/enters the body only on success. A false guard discards pending bindings and advances according to the or-pattern rule below. `Join` transfers selected result and cleans scrutinee at its enclosing temporary scope. No exhaustiveness analysis. Falling off all arms is `LRR-E-MATCH-NO-ARM`.

**For.** Evaluate source once; IteratorStart supports only the inventory in Section 17. Each iteration invokes IteratorNext, matches the loop pattern, enters the body, and cleans iteration scope. Drop the iterator on break or return. An owning iterator drops its remaining elements in increasing index order.

**Question mark.** Evaluate operand once. An Ok/Some extracts and transfers payload. An Err/None begins an early return using the current function's concrete matching Result/Option contract. No `From`, `Try`, or residual trait solving. Returning Err with a different error descriptor is a boundary fault.

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

Treat zero-op semantic phases as one Unit body operation followed by an exit, so verifier assumptions remain uniform. The completion hook records the exact before/after bound PC for both ordinary and exit ops. A guard miss resumes at that exact operation_index. The 64-op JIT limit counts synthetic exits too; a recipe boundary alone does not end a region if its observed exit is an unconditional Next.

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
    if native entry is installed and this entry is not suppressed:
        invoke region; process its explicit exit; continue
    execute exactly one recipe through interpret_one
    record completed operation observations from that real execution
    process recipe exit: next, branch, call, return, effect, panic, fault, yield
    consider one eligible native promotion at this safe boundary
```

The authoritative order inside one opcode is: validate non-mutating preconditions, check/charge its fuel unit, reserve needed resources, perform the guest operation, commit its result/next-op PC, append a completion observation. An error event is recorded with its opcode identity but is not a successful completion. A guest panic may occur after an earlier committed side effect, which remains visible.

### 16.3 Fuel semantics

Charge one unit per LRIR opcode and one unit per processed element for aggregate helpers. Structural recipe installation and source queries do not consume guest fuel; they consume preparation budgets and wall time. Interpreter and JIT use the same opcode charging function. Poll adds one opcode charge and also checks cancellation. The default is 10,000,000 units per test. The Cranelift tier must not execute an unbounded managed helper loop inside one opcode without per-element checks. A trusted Cargo-native call is explicitly outside managed fuel accounting; its only enforced execution timeout is the supervisor wall deadline.

Cancellation is checked at least every 256 guest opcode charges and at every call/effect/backedge/cleanup boundary. Because native regions have at most 64 guest opcodes, returning from a native region bounds cancellation latency in guest work, not wall time. The coordinator's process deadline handles a stuck compiler/helper or blocked OS request.

### 16.4 Pending actions and effects

CallSeal and EffectPrepare retain sealed/prepared data in frame slots but do not publish an externally active action. The explicit recipe exit publishes that action.

A pending action is one of `CallPacket` (including NativeBinding targets), `ReturnPacket`, `EffectRequest`, `PanicRecord`, `FaultRecord`, or `YieldRecord`. It contains the next continuation and a monotonically increasing per-run action ID. Publishing requires `pending == None`. The dispatcher consumes it and clears it exactly once. A native helper must not overwrite an outstanding pending action.

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

Create synthetic crates `core`, `alloc`, and `std` with stable facade IDs and exports. They are not parsed from the Rust toolchain's source and not linked as native guest libraries. The edition-2024 prelude uses the reservation/name-resolution rules in Section 34; async-related additions are recognized as unsupported identities rather than executed or silently omitted. A user item may shadow a prelude name through normal lookup. Qualified `std::...` uses the synthetic crate unless the project manifest illegally aliases another dependency as std, which preparation rejects.

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
| `Result<T,E>` | `Ok`, `Err`, `is_ok`, `is_err`, `unwrap`, `expect(&str)`, `unwrap_err`, `as_ref` | Error formatting on panic uses supported Debug. No From conversion. |
| Ranges | `a..b`, `a..=b`, `..b`, `a..`, `..` | Iteration only concrete integer bounded ranges. Other forms only slicing. |
| Iterators | built-in `next`, `enumerate`, `count`, `collect::<Vec<T>>` | Only facade iterator families; no closures, map/filter, or user Iterator impl dispatch. |
| Scalar methods | `wrapping_add`, `wrapping_sub`, `wrapping_mul`, `checked_add`, `checked_sub`, `checked_mul`, `abs` on signed integers; `min`, `max` on integers | Exact concrete same-type contracts. Abs(min) panics. No floating min/max ambiguity. |
| `mem` | `drop`, `replace`, `forget` | Shared ownership/drop machine; no raw native drop. |
| Clone | `.clone()` on Copy values, String, Vec of Clone elements, supported derived Clone aggregates | Recursive checked clone with construction guard and per-element fuel. |
| Comparison | primitive and string equality/order; Vec/slice/array/tuple equality; supported derived PartialEq/Eq | Compare left-to-right, short-circuit; equality does not move operands. |
| Debug / Display | primitives, strings, listed containers, supported derived Debug | Bounded formatter; no user formatting trait callbacks. |
| `std::fs` | `read_to_string(&str)`, `read(&str)`, `write(&str, bytes-or-str)` | Broker mediated. `read` produces Vec<u8>; errors return Result. |
| `std::env` | `var(&str)` | Reads only explicitly injected map; no ambient host environment. |
| IO error | `Error::kind`; display/debug of errors; fixed ErrorKind variants | No raw OS handles or arbitrary native error objects. |

Manual implementations of facade Clone/Copy/Debug/Display/PartialEq/Eq are unsupported when their standard policy is needed; the built-in derives and inventory are the only standard policies. User Drop is the explicit exception. An ordinary user-defined trait or inherent method with the same spelling has its own nominal identity and follows ordinary dispatch, but does not become a facade policy.

For method-looking intrinsic entries, resolution still respects inherent/trait namespace rules. The registry recognizes the exact facade type identity, not any user type with the same spelling. `std::string::String` and prelude String have the same synthetic nominal identity.

### 17.3 Iterator mechanics

`IteratorState` variants are integer range, shared/mutable slice, owning Vec, UTF-8 chars, bytes, enumerate, and count/collect driver state. A range stores concrete integer type, next value, bound, and inclusive/exhausted bit; check inclusive maximum without overflowing when stepping past the final item. An owning Vec iterator owns the buffer; yielded cells transfer ownership out and become moved. A borrowed iterator stores a lease and validates it on each next.

`enumerate` owns its inner iterator and a checked usize counter. `count` consumes an iterator and checks fuel every item. `collect` requires an explicitly known Vec element descriptor, moves each yielded item into a growing Vec, and preserves cleanup if an item fails. These drivers run as resumable VM facade operations, not uninterruptible host loops. User-defined iterator types are rejected on IteratorStart or `.next()` dispatch; do not half-implement the trait protocol.

### 17.4 Formatting

Support format strings with literal text, `{{`/`}}`, positional `{}`, positional `{:?}`, and indexed `{0}` / `{0:?}`. Support implicit captured identifiers such as `{name}` by resolving each named argument once at the reached macro invocation. Reject alignment, width, precision, hexadecimal, scientific, and custom format specifications with `LRR-E-FORMAT-SPEC` when formatting is reached.

Parse a format string into a cached immutable format recipe keyed by its literal contents and macro origin. Evaluate explicitly written arguments left-to-right once. Formatting reads/borrows values and does not move them. Limit nesting to 64 and output to the run output budget. Container formatting checks fuel per element. Cyclic reference display emits `<cycle>` after encountering the same active reference/object identity; it does not recurse indefinitely.

Supported equality on references compares checked referent values under the same finite equality policy, not reference-handle identity. Mutable references are only read for comparison, never copied as owning values. Cyclic referent comparison remains bounded by fuel/depth checks.

This formatter is not a generic Rust formatting-trait implementation. User `impl Display/Debug` is outside the profile, except supported derived Debug. A non-formattable value faults only when its formatting is actually required.

<a id="section-18"></a>

## 18. Built-in macros, derives, constants, and temporary annotations

### 18.1 Macro inventory and lazy expansion

Recognize these exact macro families after normal macro-name resolution: `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `print!`, `println!`, `eprint!`, `eprintln!`, `format!`, `vec!`, `cfg!`, `stringify!`, `concat!`, `line!`, `column!`, `file!`, `module_path!`, and `include_str!` / `include_bytes!` under the source-authority rule below.

Lower these macros to synthetic AST-free recipes, not to Rust source reparsed through a general macro expander. Each synthetic child has a deterministic origin combining invocation node ID and child ordinal. Macro arguments remain executable thunks. An untaken assert failure message or cold macro invocation does not resolve its argument names.

- `assert!(cond, ...)` evaluates cond once. Prepare formatting arguments only on false.
- `assert_eq!`/`assert_ne!` evaluate each operand once into shared views, compare without moving them, and prepare additional failure-message expressions only when comparison fails. Base operand Debug formatting is also deferred until failure.
- `panic!`/`todo!`/`unimplemented!`/`unreachable!` create guest panics with bounded formatted messages. They are not unsupported-feature faults.
- Debug assertions are enabled and behave like assertions.
- Output macros format then submit one bounded output effect. Empty println emits one newline.
- `vec![a,b,c]` evaluates each item once in order and infers element type only from context or a concrete encountered first value; later values must match. `vec![e; n]` evaluates e then n once, requires concrete usize n, clones e n times through the supported Clone mechanism, and consumes the original appropriately. n=0 still evaluates and then drops e.
- `cfg!` evaluates cfg syntax from the snapshot and never visits code as though it were `#[cfg]`. `stringify!` returns normalized token text under a documented single-space token join, not necessarily byte-for-byte rustc formatting. Do not include stringify in native text-equality oracle tests.
- `concat!` accepts only literal tokens and emits a string constant. No guest expressions are evaluated.
- `line!`, `column!`, `file!`, and `module_path!` read the active source map/definition context at execution. Source-position edits must invalidate or parameterize these values even when native code is reused.
- `include_str!` and `include_bytes!` accept one literal path. The coordinator includes that file in the source snapshot under source-root authority before publishing the snapshot when the invocation is structurally indexed. Runtime use instantiates those captured bytes. The included file is a source dependency, not a guest filesystem permission. Unreadable includes are stored as deferred source-input errors and surface only on reached invocation. An escaping path is denied at capture and likewise deferred unless needed for declaration completeness.

User `macro_rules!` bodies may remain opaque cold definitions. Calling one is `LRR-E-MACRO-USER`. Item-position invocations that might introduce declarations block completeness of the affected scope; do not pretend their names are absent. Live procedural attribute/derive macros are never executed. The independent native build closure may execute its own procedural macros only during authorized Cargo preparation.

### 18.2 Built-in derives

Recognize unqualified or core-qualified built-in Copy, Clone, Debug, PartialEq, Eq in `#[derive(...)]`. Synthesize descriptor policies and dispatch entries lazily; do not generate guest source. Generic derived policies examine actual instantiated field descriptors, not all theoretical T values. A reached derived Copy type containing String reports `LRR-E-DERIVE-FIELD`; the cold declaration is not rejected globally.

Derived Clone recursively clones fields in declaration order using construction guards. Derived PartialEq compares discriminant then fields in declaration order, short-circuiting on inequality. Derived Eq marks the supported equality policy; it must reject concrete float-containing fields when its Eq contract is needed. Derived Debug prints declaration/variant name and ordered fields with bounded recursion.

An unknown derive can affect impl completeness and therefore blocks operations requiring that type's generated impl universe with `LRR-E-MACRO-DERIVE`. It does not prevent executing a separate unrelated function in a scope whose callable names are already complete.

### 18.3 Pure constant evaluator

Implement a separate small evaluator over immutable `ConstValue`, not the live VM. This is necessary because query results cannot contain guest side effects or guest handles. Supported values: concrete primitive scalars, immutable strings/bytes, tuples, and arrays. Supported syntax: literals, primitive unary/binary/casts, block locals, if, loops with finite fuel, return, named constants, and calls to named `const fn` whose reached operations remain in this subset.

The evaluator uses the same primitive arithmetic functions but its own local stack of immutable/mutable local scalar slots. No nominal Drop, reference mutation, facade allocation, IO, panic unwinding, macros with effects, or ordinary non-const function call is permitted. `panic!` encountered during const evaluation returns `LRR-E-CONST-PANIC`. A cold branch containing an effect remains untouched.

`Constant` query key contains DefKey, expected concrete type, cfg, and explicit pure arguments for a const-call memo only when all arguments have stable ConstValue encodings. Limit evaluation to 100,000 steps, 128 frames, and 1 MiB serialized result. Active constant cycles report `LRR-E-CONST-CYCLE`; ordinary repeated recursive const calls with changing arguments are legal until budget/depth limits.

An array length requires a nonnegative usize-compatible constant within the element/allocation limits. A named `const X: T` gives its initializer T context. On each runtime use, instantiate fresh owning guest tuple/array cells from immutable ConstValue; sharing the memo does not share mutable guest state. A const string's bytes may be physically shared read-only, with fresh checked guest leases.

### 18.4 Exact temporary-scope annotation procedure

The body structural index produces `TempScopeId` and `ExtendedTo` annotations without evaluating expressions. For the supported 2024 profile, choose the nearest temporary boundary from: statement, complete function, loop/if/else body, ordinary bool condition or bool condition-chain operand, ordinary bool match guard, lazy boolean operand, if-pattern-condition-plus-consequent scope, while-pattern-condition-plus-iteration scope, match arm body, and a block's entire tail expression. Pattern condition chains have nested clause scopes; their cleanup is specified in Section 34. A match scrutinee uses its enclosing boundary, not the selected arm's scope. Apply syntactic lifetime-extension annotations after computing these default boundaries; an explicit extension wins over the default narrowed scope.

For each let initializer, mark an extending-expression frontier. Start at the initializer; propagate through parentheses, tuple/array element constructors (not repeat operands), braced struct fields, tuple-struct/variant constructor arguments, supported casts, block tails, selected-when-executed if branch tails, and match arm expressions. Do not propagate through ordinary function/method calls, arithmetic, or a closure. A borrow expression on that frontier extends its operand's temporary to the containing block. Propagate that extension down borrow/deref/field/tuple-index bases and index bases, but not index expressions.

An extending pattern is a by-reference binding, or a tuple/struct/array/or-pattern containing an extending direct subpattern. An explicit reference-pattern wrapper is not itself an extending wrapper. An extending let pattern extends the initializer's temporary; then propagate through the base operations just listed. The rule acts on structural annotations even if only one branch later executes; only executed temporaries are allocated and registered.

Source compiler behavior in this area is edition-sensitive. The conformance suite pins 2024 and records native 1.99 outputs for supported examples. An observed discrepancy must be fixed in this annotation pass; do not compensate with ad hoc lifetime extension inside the reference helper.
<a id="section-19"></a>

## 19. Native ABI and helper contracts

This is the private **Cranelift-to-LRR helper ABI**, not the Cargo-native plugin ABI. The separately versioned C bridge in Section 39 never accepts ValueRepr, VmContext, frame handles, or these internal helper pointers.

### 19.1 Uniform entry signature

All guest native regions have this one host ABI:

```rust
pub type RegionFn = unsafe extern "C" fn(*mut VmContext, u64) -> u32;
// Arguments: pinned host context address, checked guest FrameId bits.
```

Use the native System V calling convention selected by the pinned JIT module on Linux x86-64. The second argument is an integer handle, not a host frame pointer. Generated code receives no direct guest-object address. It cannot return a Rust object, borrow a Rust slice, unwind through host frames, or call a guest function directly.

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
| 8 | `Deopt` | Current opcode has not committed; resume its interpreter/helper path once. |

A helper that returns Call/Return/Effect/Panic/Fault/Yield must have filled the corresponding canonical VM state. A region must immediately return any nonzero helper result. Returning zero from a region, returning an unknown number, or returning a status inconsistent with pending state is `LRR-E-INTERNAL-JIT-ABI`.

### 19.3 Fixed helper registry

The release imports exactly these symbols. Their ABI signatures use pointer-width integer for the context, I64 for frame/op/region IDs and payloads, I32 for status/selector values, and pointer-width integer for the one trusted scratch pointer.

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

Assert size 32, alignment 8, and field offsets. Allocate a 32-byte, 8-byte-aligned Cranelift explicit stack slot for it. The only guest-related native loads in this release read that stack slot after stage_scalar returned Continue. No JIT load directly reads an arena vector, object, cell, or unchecked guest reference.

The scratch pointer is a trusted compiler-generated stack address. The helper must not accept such a pointer from any public API or guest value. Put its unsafe write in one audited function with the precondition: caller is verified generated LRR code, scratch slot is live and correctly aligned for this helper call, and no native reference escapes. Use `write` on the raw pointer only after the context/frame/op have been validated. Record this exact safety argument in the code comment.

The native pointer-to-RegionFn conversion occurs once in `jit::abi` after successful finalization. Check target pointer width and entry alignment. Store the function pointer only inside an artifact owned by its live module arena. Do not cast arbitrary bytes or a guest function-item handle to RegionFn.

### 19.5 Guard miss and exactly-once execution

On Deopt, the dispatcher sets `suppress_native_once = (frame, current_pc, current_op_index)`, then executes the authoritative interpreter opcode once. Clear suppression after that opcode completes or produces a terminal action. Do not re-enter the same region before executing the missed operation, and do not restart at the region entry.

Every completed opcode has already committed its values, ownership, fuel, output, and continuation. Native SSA temporaries are discarded on exit; they contain no uncommitted owning guest object. This baseline has no general deoptimization map because canonical VM slots are current at every opcode boundary. The cost is extra helper traffic; that is an accepted D22 cost, not a reason to invent an optimizing tier.

A native stage guard can inspect values before fuel is charged because staging is pure. If fuel is exhausted, commit reports Fault with the same resource policy as the interpreter before changing guest state. A guard miss consumes no fuel; the one interpreter execution performs the one charge. Dedicated tests must compare exact fuel usage across tiers.

<a id="section-20"></a>

## 20. Observation, region formation, and promotion

### 20.1 Observation record

Append observations only from the shared successful-op completion hook:

```rust
pub struct Observation {
    pub body: BodyKey,
    pub pc_before: BoundPc,
    pub op: BoundOpId,
    pub pc_after: BoundPc,
    pub shape: ScalarShapeOrGeneric,
    pub boundary: BoundaryKind,
}
```

The record contains descriptor IDs, never guest payload values, secrets read from files, or guest handles. Native completion hooks can increment execution counts but must not recursively request compilation. Recorders belong to the engine, not to pure queries.

### 20.2 Region formation algorithm

A candidate region begins at a function entry, a branch target, a loop iteration entry, a return-from-call continuation, a post-effect continuation, a post-cleanup continuation, or the PC after a 64-op split. These starts are announced by the dispatcher when actual control reaches them.

Collect the actually completed opcode sequence until the first of: 64 opcodes; a branch exit; guest call; guest return; effect; panic/fault; cleanup transition; yield; or source-body boundary. The branch/call/effect preparation opcode may be the final opcode if it completed or published a valid pending action; its successor is not part of this region. An errored opcode is not compiled from a failed observation. A candidate with zero completed opcodes is discarded. The synthetic recipe-exit operations in Section 15.5 are included; a branch/call/effect region ends after that observed exit, never before a hidden unrecorded terminator.

Normalize the record to a `RegionRecipe { start_pc, ordered_ops, end_kind }`. Deduplicate by symbolic operation/PC sequence, not by actual values. It is illegal to append an opcode merely because the syntax graph suggests it will execute next. No cold target is queried while forming or verifying a region.

### 20.3 One artifact and fixed promotion policy

Normal `--jit=auto` promotes a region after **32 completed interpreter traversals** of that same sequence. A traversal is counted when the region closes, not each time an internal opcode executes. `--jit=force` uses threshold one but still requires one real completed observation; native code can be entered only on a later visit. `--jit=off` never creates guest native artifacts.

Select at most one eligible region after each dispatcher boundary. Choose the highest completed-traversal count, breaking ties by stable region digest. Compile synchronously. Maximum candidate length is 64 guest opcodes. Maximum queued eligible candidates is 1,024; drop lowest-count candidates deterministically when full. No background compiler thread, speculative compilation, whole-function fallback, or cache disk loader is permitted.

For each region recipe there is at most one live native artifact. For primitive fast paths, freeze the concrete scalar descriptors observed in the first qualifying traversal. If future values have another shape, the guard misses and that opcode executes through the common helper/interpreter; it does not create another specialized artifact. Repeated shape misses do not mark source invalid and do not trigger recompilation. Record miss rates for diagnosis.

### 20.4 Generic regions are real native code

A region with no eligible scalar fast path still emits a Cranelift function containing the ordered helper calls and status branches. This removes interpreter opcode selection for that sequence but retains runtime helpers. It is a valid native artifact, not a claim of high throughput. Telemetry must distinguish native entries, generic-helper opcodes executed under native dispatch, and directly computed scalar opcodes.

A one-shot test may never enter native code, even with force mode, because observing a path is not permission to rerun it. Such a test still succeeds in the interpreter and reports zero native entries. Native acceptance fixtures deliberately contain repeated calls/loops so their native-entry assertions are meaningful.

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

The release helper symbol table contains exactly the five helper functions in Section 19.3. Do not register libc, allocator, filesystem, process, random, clock, Rust std symbols, or guest-exported names. Imported symbol names in generated CLIF must come from a fixed HelperId enum, never string concatenation with source names.

If the backend inserts an unregistered libcall, compilation/finalization fails closed. Mark the region compile attempt failed and keep executing its interpreter form in auto mode; force mode turns the compile failure into an explicit test failure. Do not restore ambient lookup to make a libcall resolve. The chosen initial fast-path subset avoids operations requiring floating-point or large-stack libcalls.

### 21.2 Builder settings

Create `JITBuilder::with_flags` with `opt_level="none"` and `enable_verifier="true"`, plus the pinned module's default libcall-name translator. Preserve the constructor's target-specific relocation/PIC settings; do not overwrite them from an old blog example. The inspected 0.136.2 constructor enables x86-64 PIC for its relocation scheme.[^R12] Clear fallback lookup, register the exact helpers, then create JITModule.

Use the module's target pointer type and default calling convention to construct all signatures. Guest frame/op IDs remain I64 even though they are not pointers. Use I32 for all statuses. Helper declarations use `Linkage::Import`; region definitions use local linkage and opaque generated names such as `lrr_region_17`. Source names appear only in diagnostic metadata.

### 21.3 Compile algorithm

1. Confirm the RegionRecipe contains only successful observed operations, has 1–64 opcodes, and its inputs remain valid for the active snapshot.
2. Create `RegionPlan` with per-op generic or eligible guarded-scalar lowering and a complete dependency fingerprint. Pin every referenced BoundOp, descriptor, and recipe for artifact lifetime.
3. Declare the region function with the uniform ABI. Create Cranelift Context and FunctionBuilderContext owned by the compiler; clear/reuse them after each completed compilation.
4. Create an entry block, append function parameters, and emit region_enter. Branch to a return block on any nonzero status.
5. For each generic op, emit step(ctx, frame, op_id). If status is Continue, continue to the next op. Otherwise return that status immediately.
6. For each eligible scalar op, emit stage_scalar with the explicit stack slot. On nonzero status return. Load the staged scalar bits, compute the whitelisted native operation, and emit commit_scalar with result/outcome. On nonzero status return.
7. Emit region_leave after the final successful op. Return its Dispatch status. A terminator may already have returned Call/Effect/etc.; make CFG predecessors explicit and seal every block.
8. Run Cranelift's verifier, then the local import/ABI/stack-slot audit. Every imported function must match a registered helper's full signature. Every memory access must be to the defined scratch stack slot with statically checked offset/width. No indirect call or native guest pointer arithmetic is allowed.
9. Define the function, finalize the module definitions, retrieve the finalized function address, and convert it in the single audited ABI function.
10. Publish the artifact to the entry table only after all steps succeed. Record code size, compilation wall time, op count, helper count, scalar count, and content keys. A failure publishes no callable address.

Cranelift JITModule requires finalized referenced definitions and imposes explicit code-memory lifetime requirements; the module arena is designed around those constraints.[^R06]

### 21.4 Whitelisted direct scalar lowering

The required native fast paths are deliberately finite: i32/i64/u32/u64 `+`, `-`, bitwise `& | ^`, integer comparisons, bool `!`, and bool equality/inequality. isize/usize reuse 64-bit machine operations but guards retain their distinct guest TypeIds. All multiplication, division, remainder, shifts, floating-point operations, casts, aggregate operations, and reference operations use step in this release.

For unsigned add, compute result and detect overflow by checking whether the result is less than the left operand, additionally checking the width bound for u32. For unsigned subtraction, overflow means left is less than right. For signed 32-bit add/sub, compute in I64 and compare against the signed 32-bit min/max. For signed 64-bit add, overflow occurs when the operands have the same sign and the result's sign differs. For signed 64-bit subtraction, overflow occurs when the operands' signs differ and the result's sign differs from the left operand. Implement these with Cranelift integer operations/compares, not host undefined signed arithmetic.

On overflow call commit_scalar with outcome 1, not an unchecked wrapped result. The helper produces the same guest panic origin and fuel charge as the interpreter. Comparisons use signed or unsigned condition codes according to the guarded type and commit a bool value. Bitwise outputs are normalized to the guest width/sign representation before commit.

Every fast-path operation must have boundary fixtures: zero, one, minimum, maximum, negative one where valid, equal values, opposite signs, and overflow. Property tests compare thousands of random bit-pattern pairs to the shared primitive helper. No fast path may be enabled solely because its happy-path sample works.

### 21.5 No native speculative operations

The optimizer level is none, but correctness must not rely on that alone. Never emit a helper with a side effect before its source-order position. Never read a referenced guest object directly in CLIF. Do not fuse two guest arithmetic operations across a potential overflow boundary. Do not hoist permission checks, allocation, call contracts, or fuel charges across control flow. Scalar staging is the only permitted read-only precheck, and it covers one immediately following operation.

### 21.6 Compile failure policy

Classify failures as resource limit, known unsupported backend operation, ABI/import audit failure, or internal codegen failure. A known resource limit stops promotion for that run and continues interpretation in auto mode. An ABI/import audit failure invalidates that region and reports an internal JIT defect; release tests must fail. An internal codegen failure records the CLIF dump and bounded diagnostics, disables that artifact for its recipe key, and continues interpretation only in auto mode with `jit_degraded=true` in the result.

Force-mode fixture runs fail on any attempted compilation failure. Release gates require zero degraded fixtures. Do not hide failures by reporting the interpreter fallback as a successful JIT execution.

<a id="section-22"></a>

## 22. Artifact cache, edits, and code-memory lifetime

This section concerns worker-local Cranelift artifacts. It applies across tests in a live worker; only managed mode retains that worker across run requests. Hybrid workers transfer portable pure facts, never these executable pointers, and separately reuse the immutable Cargo-native generation (Section 41).

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
    pub cranelift_build: Digest,
}
```

Do not key every artifact by the entire snapshot digest: that would throw away unrelated reuse. Do not key only by source text: signature, method-universe, cfg, facade, and ABI changes can alter meaning. The semantic dependency digest includes both positive and negative facts used in bound recipes. A region containing a dynamic call dispatcher does not depend on the callee's body unless an operation actually embedded such a body-derived fact; this release never inlines a callee.

Literal addresses, guest handles, permissions, and actual input values are not in the artifact. Permissions are checked on every effect. Origin mappings are reattached to the active snapshot; `line!` and related values use active source-map helpers rather than stale embedded positions.

### 22.2 Installation across runs

At a new run boundary, clear the active-PC-to-native-entry table but retain artifact storage. When a reached PC requests installation, revalidate its recipe dependencies through the current query revision. Obtain its currently valid region/semantic key and look up a matching artifact. On hit, install an entry labeled with the active semantic epoch; on miss, run the interpreter and accumulate a new candidate. Do not label old artifacts valid merely because the body bytes match.

A body edit invalidates every region owned by that BodyKey, even when only a cold branch inside the same function changed. Invalidation is body-granular; execution/preparation is path-lazy. This deliberately avoids a second expression-diff identity system. The cold-body reuse guarantee concerns a different unchanged reached body, not unaffected expressions within an edited body.

A method body edit preserves callers whose dispatch target/signature are unchanged, but the edited body's own BodyKey changes. Adding a competing impl changes the relevant candidate-set fingerprint and invalidates affected dispatch results. Adding a previously missing name invalidates a negative lookup. Editing a comment before a function changes its displayed line locations but not its body key when the function bytes remain identical.

The first reached execution of a previously cached region in a new snapshot must therefore validate/install once. Hot entries in the same run do not query the database again.

### 22.3 Arena policy

A `ModuleArena` owns one JITModule, a set of artifacts, pinned bound metadata, and its accounting. Seal it after 128 regions or 4 MiB of emitted native code, whichever comes first. Create another arena for later compilations. Total native code budget is 64 MiB per worker; include allocation slack in process RSS monitoring because emitted byte count is not exact mapped memory size.

Between runs, evict sealed arenas in least-recently-entered order until within budget. Before freeing an arena: clear every native entry referencing it, remove its artifacts from the lookup map, drop all stored RegionFn values, assert no active native invocation, and then call `unsafe free_memory`. Release its metadata pins afterward. Code is never freed during a guest run, destructor activation, or helper call. An open arena may be sealed at a run boundary to make it evictable.

If promotion would exceed the code budget during a run, stop promoting and interpret the remaining work. Do not evict executing code or restart the test. Set `promotion_stopped=code_budget` in telemetry. This is not a semantic error.

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

In managed mode, one persistent worker serves one coordinator. Hybrid mode uses a fresh worker per run request as required by Section 41. It executes one test at a time. Compiler tasks run in that same worker thread at safe points. Coordinator IO/monitoring threads use std channels so cancellation and deadlines remain observable while the worker is busy.

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

## 24. Managed Cargo-compatible project preparation

### 24.1 Compatibility level

This section is the `managed` preparation path. The separately authorized `hybrid-native` preparation path is specified in Sections 35–38 and deliberately permits Cargo-native artifacts. Do not apply the managed native-dependency ban to hybrid preparation, and do not weaken managed preparation because hybrid exists.

LRR is a Cargo project consumer, not a replacement implementation of Cargo's build graph. Use Cargo metadata to obtain package IDs, manifests, source roots, dependency aliases, and resolved features. Cargo documents that metadata interface separately from compilation.[^R13] Execution never calls `cargo build`, `cargo check`, `cargo test`, rustc, build.rs, or a proc-macro executable.

The accepted preparation subset is intentionally closed: Rust 2024 packages; library, binary, and integration-test targets; normal/dev dependencies for a selected target; local/path or already cached registry source; no build dependencies, custom-build targets, proc-macro targets, target-specific dependency tables, optional dependency activation, dependency-feature forwarding, alternate registries, patch/replace sections, Cargo plugins, or custom target JSON. Feature definitions may enable other local features and ordinary `cfg(feature=...)`; they may not contain `dep:`, `dependency/feature`, or weak dependency forwarding syntax.

Reject unsupported manifest features during preparation, because selecting an incorrect dependency/feature graph would invalidate all later name resolution. Do not silently ignore them. Source-level unsupported constructs in dependencies remain lazy according to the normal profile once the project graph is accepted.

### 24.2 Commands and no-network rule

Preparation is an explicit command:

```sh
lrr prepare --manifest-path Cargo.toml --package demo --target lib
lrr prepare --manifest-path Cargo.toml --package demo --target test:integration
cargo lrr prepare --manifest-path Cargo.toml --package demo --target bin:demo
```

For the selected feature flags, invoke the pinned Cargo metadata command with `--format-version 1 --locked --offline --manifest-path ABSOLUTE_MANIFEST --filter-platform x86_64-unknown-linux-gnu`, adding `--no-default-features` and a comma-separated `--features` list when requested. Package selection is performed on the returned metadata; do not invent a metadata `--package` flag. Cargo may internally resolve metadata, but the runtime must never run guest build steps.

A missing lockfile, missing cached registry package, or required network fetch fails `LRR-E-PREPARE-OFFLINE`. LRR does not fetch dependencies automatically. The user may manage their Cargo cache outside LRR; that is not hidden runtime work.

### 24.3 Manifest and configuration audit

Before invoking Cargo, inspect workspace/package manifests and relevant `.cargo/config` / `.cargo/config.toml` files under the workspace and selected Cargo home. Permit registry cache location and documented no-execution configuration only; reject source replacement, custom runners, rustc wrappers, target runners, aliases relevant to invoked commands, and unstable configuration. Clear wrapper/toolchain injection environment variables from the metadata subprocess. Use the pinned rustup toolchain explicitly.

This audit is a trust reduction for local development inputs, not a proof that running Cargo on a malicious workspace is sandboxed. A user invoking prepare authorizes reading that workspace and running the installed Cargo executable. Since metadata behavior can evolve, bootstrap/integration tests must use a sentinel build.rs and proc-macro package and confirm neither executable runs; preparation rejects both as soon as identified.

### 24.4 Unit mapping

Create `PreparedUnit` records containing package ID, canonical manifest/source root, crate name, dependency-alias map, selected feature set, edition, target kind, and cfg values. Build a root-target dependency graph from metadata and the manifest subset. Include dev dependencies only for root unit/integration tests; dependency libraries are not compiled/interpreted with `cfg(test)`. An integration-test target is its own root crate with access to the package library through its Cargo crate alias.

The selected root library's unit-test mode enables `cfg(test)` in that library. A normal run disables it. Test selection does not enable `cfg(test)` for all dependencies. Package binaries and integration tests require an explicit target when more than one target of that kind exists; preparation fails with the sorted target list instead of choosing arbitrarily.

Cargo.lock resolver metadata and each package's local feature closure are captured together. Workspace packages outside the selected target's reachable graph cannot contribute features to its effective profile; if Cargo metadata reports such unaccounted unification, preparation fails rather than using a guessed graph.

Because the accepted manifest subset forbids conditional/optional dependency activation and feature forwarding, metadata's selected feature closure has a finite direct interpretation here. Preserve exactly the local feature sets returned for the prepared graph and record them in the manifest. Reject a graph whose metadata feature result cannot be attributed to the allowed local-feature rules; do not approximate Cargo resolver behavior.

### 24.5 Prepared manifest

Write `target/lrr/prepared/<prepare-id>.json` atomically through a temporary file and rename. Fields: schema_version=1, lrr_profile, host_toolchain, cargo_version, target, root_package, units, selected_features, cfg_by_unit, dependency_aliases, manifest_digests, lockfile_digest, relevant_config_digests, package_source_locations, and preparation warnings. The prepare ID is the stable hash of semantic configuration and audited manifest inputs.

Store source locations, not compiled objects or rlibs. On run, check recorded manifest/lock/config digests before source capture. Any mismatch produces `LRR-E-PREPARE-STALE`. Source body edits do not require prepare; they create new source snapshots. Registry source bytes are read from the prepared location and included by digest, not assumed immutable just because Cargo normally caches them.

Managed preparation does not accept prebuilt rlibs, native dynamic libraries, or build-script OUT_DIR artifacts. Native reuse is available only through the explicit hybrid preparation record and versioned bridge in Sections 35–43; directory contents alone never authorize loading.

### 24.6 Built-in cfg and attributes

Supported cfg predicates are `all`, `any`, `not`, boolean names, and string key/value predicates for feature, target_os, target_arch, target_endian, target_pointer_width, unix, debug_assertions, and test. Unknown cfg names evaluate false and are recorded in warnings; syntactically malformed cfg is a source-configuration error when determining affected declaration membership. `cfg_attr` is expanded structurally only into supported attributes. A true cfg_attr producing an unknown transforming attribute follows the completeness policy in Section 8.

`#[cfg(false)]`-equivalent excluded items are absent from indexes and need no executable syntax traversal. Do not implement `cfg!` as an item exclusion mechanism: it is a bool expression, so ordinary lazy branch execution decides which body is touched.

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
lrr prepare --manifest-path PATH --package NAME --target TARGET [feature flags]
lrr watch FILE.rs [--test] [FILTER]
lrr serve --stdio
lrr explain DIAGNOSTIC_CODE
lrr doctor
```

Common run/test/watch flags: `--jit`, `--allow-read`, `--allow-write`, `--env`, `--fuel`, `--timeout-ms`, `--memory-mib`, `--output-limit-kib`, `--json`, `--nocapture`, `--trace-queries PATH`, `--diagnostic-dir PATH`, and `--require-hardened-sandbox`. Only the bounded resource overrides in Section 26 are accepted. Guest command-line arguments and std::env::args are outside this release: reject any nonempty trailing argument list or service args field with `LRR-E-ARGS-UNSUPPORTED`; do not advertise guest argument parsing. Main must take no parameters and return unit or a supported Result<(), E>, handled like the corresponding test return without test attributes.

`cargo-lrr` strips exactly the initial `lrr` argument supplied by Cargo subcommand invocation, then invokes the same CLI parser in-process. It does not shell out to a different runtime binary. Cargo-style package/target selection is handled through explicit prepared manifests, not by mimicking every cargo test flag.

### 25.2 Test discovery

Outline `#[test]` functions in the selected root's loaded modules without preparing their bodies. Each test has fully qualified module name, DefKey, source origin, ignored flag, and optional should-panic expectation. Sort names lexicographically by UTF-8 bytes. Duplicate full test names are a discovery error.

A test must have zero runtime parameters and no unbound generic parameters. Its supported result is unit or `Result<(), E>` where E has supported Debug. The runner inspects only a selected test's signature when about to execute it. Invalid signatures in unselected tests remain discovery metadata, not a reason to fail a different selected test.

A bare filter is a substring match; `--exact` is equality. `--ignored` selects only ignored tests; `--include-ignored` selects both; using both flags is a CLI error. Default ignores ignored tests. Zero selected tests is exit 5 with `LRR-E-TEST-NO-MATCH`, not silent success. `#[should_panic]` with no argument accepts ordinary guest panic; `expected="text"` requires a substring in the panic message. Unsupported attribute syntax fails only when selecting that test.

### 25.3 Per-test execution

Before each test: reset guest arenas/counters/output/permissions, increment run epoch, retain valid source/query/native caches, install root signature, and call with no arguments. After normal completion and cleanup, unit or Ok(()) passes; Err(E) fails with bounded Debug output. Ordinary guest panic passes only under matching should_panic. Profile faults, resource faults, double panic, codegen failures in force mode, and worker crashes always fail.

Tests run serially. Do not run multiple tests in one guest heap or persist managed guest statics. Managed source/Cranelift caches may be warm; managed guest state may not. In hybrid mode, native process statics can persist across tests inside one request, but a fresh process resets them for the next request; native contexts and managed arenas are fresh per test (Section 41). This distinction must appear in benchmark reports.

### 25.4 Result schema

Emit one JSON object per test, plus one final summary object. Include these required fields:

```json
{
  "schema_version": 1,
  "event": "test_result",
  "test": "tests::sum",
  "status": "passed",
  "profile": "lrr-2024-v1",
  "rust_validation": "live_not_performed",
  "sandbox_strength": "capability_mediation",
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

Use newline-delimited JSON on public stdin/stdout. Each request has schema_version=1, request_id, method, and params; unknown fields fail validation. Supported methods are `open`, `apply_changes`, `list_tests`, `run`, `cancel`, `stats`, and `shutdown`. Public messages never carry host pointers, guest handle IDs intended for execution, arbitrary native bytes, or a callable ABI. Hybrid requests name only a previously prepared native generation; its callable table is internal.

- `open` selects standalone source root or a prepared project and captures a snapshot. Return snapshot ID and discovered targets/tests.
- `apply_changes` supplies base_snapshot and a batch of `{relative_path, new_utf8_text}` replacements/deletions. Validate all paths/size limits first, then publish the batch atomically. It changes the service's in-memory source overlay, not disk files. A stale base fails with current snapshot ID.
- `list_tests` returns metadata only for a named snapshot.
- `run` names snapshot, selected tests/main, mode, and policy. Return accepted, then ordered progress/result events. Only one active run is allowed; queue at most two newer requests and coalesce only requests explicitly marked replace_pending=true.
- `cancel` targets a run/request ID and requests interruption; it does not silently cancel a different run.
- `stats` returns real counters and current cache sizes without guest execution.
- `shutdown` cancels active execution, performs bounded worker termination, and exits.

The coordinator must continue reading cancel/shutdown while the worker executes. Use one reader thread and a main event loop receiving std channels. Every public response includes the originating request ID. Do not interleave human log text on protocol stdout; use stderr for bounded runtime logs.

### 25.6 Internal worker wire protocol

Use a 4-byte little-endian length prefix followed by UTF-8 JSON, with a 16 MiB maximum frame. Large snapshots are sent as BeginSnapshot, SourceChunk messages of at most 1 MiB of source text, and CommitSnapshot; the worker publishes nothing until commit validates all digests. The maximum applies to encoded JSON length, not just source length. Chunk by UTF-8 boundaries and account for JSON escaping.

Handshake includes protocol version, profile digest, helper ABI digest, host target, and worker build ID. Reject mismatches. Message kinds are handshake, snapshot begin/chunk/commit, run, cancel flag update, effect request/reply, telemetry, result, and shutdown. The worker's execution thread checks a cancellation atomic updated by a small protocol-reader thread; that reader may enqueue immutable messages but never access the query DB or VM. Thus the compiler/VM/query owner remains single-threaded.

For effect byte payloads, use BeginEffect/EffectChunk/CommitEffect and corresponding reply frames when the payload exceeds 64 KiB. Each byte chunk contains at most 1 MiB raw data encoded as lowercase hexadecimal; validate even length and hex alphabet, then decode under the declared total-length quota. Small byte payloads use the same hex representation inline. Text output chunks remain UTF-8 JSON strings. Authorize a request before accepting its payload, and do not perform a file write or expose a read result until the full payload, digest, and commit have validated. Every chunk carries run_id, action_id, and consecutive chunk_index. Duplicate/missing/out-of-order chunks fail the action without a retry.

Bound each channel to 64 messages and 32 MiB of total queued encoded data, whichever fills first, and enforce backpressure. A closed pipe terminates the run. A malformed or oversized frame terminates the worker connection without trying to recover at an unknown byte boundary. Never use deserialization that constructs arbitrary Rust types from untrusted tag names.

### 25.7 Exact service request examples

The following objects define the accepted parameter names; apply deny_unknown_fields to each tagged request type. `source.kind` is file or prepared, not a request to infer a mode from a path extension. All digest strings are 64 lowercase hexadecimal characters. `base_snapshot` must match the coordinator's current overlay base.

```json
{"schema_version":1,"request_id":1,"method":"open","params":{"source":{"kind":"file","path":"/work/demo/main.rs"}}}
{"schema_version":1,"request_id":2,"method":"list_tests","params":{"snapshot":"<snapshot-digest>"}}
{"schema_version":1,"request_id":3,"method":"run","params":{"snapshot":"<snapshot-digest>","selection":{"kind":"tests","filter":"case","exact":true,"ignored":"exclude"},"jit":"auto","policy":{"read":[],"write":[],"env":{}},"limits":{"fuel":10000000,"timeout_ms":10000,"memory_mib":256,"output_limit_kib":4096},"stream_output":false,"replace_pending":false}}
{"schema_version":1,"request_id":4,"method":"apply_changes","params":{"base_snapshot":"<snapshot-digest>","changes":[{"path":"main.rs","text":"#[test] fn case() { assert_eq!(1_i64, 1_i64); }"}]}}
{"schema_version":1,"request_id":5,"method":"cancel","params":{"run_id":1}}
{"schema_version":1,"request_id":6,"method":"stats","params":{}}
{"schema_version":1,"request_id":7,"method":"shutdown","params":{}}
```

Angle-bracket digest values above are explanatory placeholders; the actual parser rejects them. A prepared open uses `source={"kind":"prepared","prepare_id":"<digest>"}`. A main selection uses `selection={"kind":"main"}`. Tests selection supports ignored values exclude, only, and include. A change with text=null deletes that overlay file; deleting the selected root is rejected before publication. All fields shown for run are required in protocol v1; the CLI fills defaults explicitly. The accepted response supplies the assigned run_id. Each result/error echoes request_id and the actual snapshot.

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
| Guest fuel/test | 10,000,000 units | 1,000,000,000 | Runtime resource fault |
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
| Public/internal JSON frame | 16 MiB encoded | Fixed | Protocol error |

Resource overrides may reduce values or increase them up to the stated ceilings; they cannot disable checks. Convert all MiB/KiB using powers of 1024 and check arithmetic overflow while parsing flags. Log requested and effective limits without silently clamping an excessive request.

### 26.3 Failure records and reproducibility

On internal defects, write a bounded artifact bundle only when the user supplied `--diagnostic-dir PATH` or a host test explicitly enables it. The directory is coordinator authority, not a guest write grant. Include source digests, reduced LRIR/CLIF, version hashes, opcode/region IDs, and counters; redact environment values and runtime file contents by default. Do not upload anything.

A reproducible test seed and reduced fixture are required before marking an internal defect fixed. Semantic source snippets may themselves contain sensitive user code; warn in the manifest that diagnostic bundles are local confidential artifacts.
<a id="section-27"></a>

## 27. Ordered implementation task ledger

### 27.1 Task discipline

Implement the tasks in the order below. A task is complete only when its named acceptance tests exist, pass, and are included in the corresponding gate. Maintain `docs/tasks.json` with task ID, status (`not_started`, `in_progress`, `complete`), commit ID, acceptance-test names, and artifact paths. A task may not be marked complete because a later feature will supposedly make it work.

Every task must leave the host repository building. During early tasks, unsupported release functionality returns an explicit `LRR-E-NOT-YET-IMPLEMENTED` from a development-only command path. The release verifier rejects that code anywhere in the selected feature inventory or fixtures. Do not replace missing work with passing placeholders, ignored tests, expected crashes, or hard-coded fixture outputs.

A task's prerequisites are the preceding tasks within its milestone plus all earlier milestone gates, unless a stricter dependency is stated. There is no parallel implementation requirement. Keeping one integration branch is the determined execution strategy for a low-autonomy implementation agent.

### 27.2 Gate command contract

Implement `cargo run --locked -p xtask -- gate NN`. It runs the listed host unit tests, fixture groups, and structural audits for milestone NN, writes `artifacts/gates/NN.json`, and returns nonzero on a failure, skip, missing test, malformed artifact, or stale fixture expectation. The file records the real command lines, exit codes, git revision, toolchain, and elapsed times.

`xtask verify-all` runs gates 00–12, architecture/vendor/manifest audits, the full fixture matrix, property suites, and benchmark collection. Benchmark targets are reported as targets, not fabricated passes. Functional release acceptance requires all mandatory correctness/security-boundary/laziness/native-use gates, and complete measured benchmark results.

### Milestone 00 — Reproducible repository and toolchain

**T000 — Create workspace and execution instructions.** Files: root Cargo.toml, rust-toolchain.toml, crates/lrr/Cargo.toml, xtask/Cargo.toml, AGENTS.md, docs/tasks.json. Use Sections 3–4 exactly. Create module roots and three bin entry points; no guest execution stubs are exposed as successful commands. Acceptance: host `cargo test --workspace --locked` succeeds; task ledger validates; `workspace_has_exact_members` and `toolchain_is_pinned` pass. AGENTS.md contains the operating rules in Section 32.

**T001 — Freeze dependencies and patch JIT symbol lookup.** Files: Cargo.lock, vendor/cranelift-jit, vendor/UPSTREAM.toml, patch diff. Resolve the exact direct versions, add only the Section 21 patch, and capture archive checksums. Acceptance: `vendor_patch_is_exact`, `direct_versions_match_spec`, and dependency license inventory pass. No git/unversioned direct dependency is permitted. A package-resolution failure blocks the gate with evidence; it is not permission to upgrade silently.

**T002 — Parser and native smoke tests.** Files: syntax/parse.rs, jit/abi.rs test module, xtask/bootstrap.rs. Implement the two smoke tests in Section 3, including one registered helper call and a deliberately unresolved native import. Acceptance: `parser_counts_cold_function_without_resolving`, `jit_adds_seven`, `jit_calls_registered_helper`, and `jit_does_not_resolve_ambient_symbol` pass. Run the bad-import case in a child test process so upstream error/panic behavior cannot compromise the host test runner.

**T003 — Gate harness and architecture audit.** Files: xtask/gates.rs, xtask/audit.rs, compat/features.toml, compat/diagnostics.toml. Add checks for forbidden imports, unsafe module locations, and missing diagnostic/feature fixture references. Acceptance: inject a forbidden `std::fs` use into a worker-module test sample and verify the audit fails; restore the real source and gate 00 passes. The audit scans parsed Rust paths where possible, not only fragile substring matches.

**Gate 00:** reproducible host build, pinned parser/JIT smoke, closed symbol table, functioning audit and ledger.

### Milestone 01 — Source snapshots and pure query core

**T010 — Stable keys and encodings.** Files: core/ids.rs, core/digest.rs, source/manifest.rs. Implement typed IDs and explicit StableEncode tags/lengths. Acceptance: maps with different insertion order have identical hashes; field/order/version changes alter expected hashes; distinct key families cannot collide through identical byte concatenation; source-location fields are excluded from semantic keys where specified.

**T011 — Source loader and immutable snapshots.** Files: source/loader.rs, source/snapshot.rs, source/changes.rs. Capture standalone roots/modules, UTF-8 content, and approved includes with size/race limits. Acceptance: `snapshot_is_immutable_after_disk_edit`, `module_path_escape_denied`, `module_rs_ambiguity_is_error`, `source_batch_is_atomic`, and `racing_capture_reports_limit` pass using controlled temporary directories.

**T012 — Syntax adapter and outline split.** Files: syntax/ast_adapter.rs, syntax/outline.rs, syntax/spans.rs. Extract item headers, nested body handles, source-relative spans, and parse diagnostics without semantic resolution. Acceptance: edit a function body and keep its header/bucket fingerprints unchanged; insert lines before an unchanged function and update its displayed location; cold missing-name and unsupported-expression fixtures outline successfully.

**T013 — Query fetch, dependency validation, and cycles.** Files: db/key.rs, db/value.rs, db/memo.rs, db/provider.rs. Implement Section 9's fetch algorithm with explicit QueryValue variants and no Any/downcast escape hatch. Acceptance: synthetic diamond dependency graph runs each provider once per invalidating change; unchanged-output recomputation keeps downstream reuse; active cycle returns a typed non-cacheable error; breaking that cycle by an edit succeeds immediately; repeated ordinary function identity lookup is not a cycle.

**T014 — Fine-grained input publication and negative dependencies.** Files: db/dependency.rs, db/trace.rs, source/changes.rs. Refresh all changed outlines before publishing header/body/scope inputs. Acceptance: adding a previously absent name invalidates cached missing lookup; body-only edits do not invalidate name buckets; duplicate candidates are deterministic; a failed query with unchanged dependencies is cached. Trace includes provider execute/reuse/invalidated events with real counts.

**T015 — Cache budgets and source-map attachment.** Files: db/memo.rs, syntax/spans.rs, engine/telemetry.rs. Implement between-run LRU eviction, metadata accounting, and span reattachment. Acceptance: constrained synthetic cache tests evict deterministic victims; a live parse/source-map handle is pinned; an edited line offset cannot produce an old line number. Gate 01 records provider counts for the edit matrix's initial rows.

**Gate 01:** immutable revisions, header/body separation, correct memo reuse including negative answers, bounded caches.

### Milestone 02 — Name resolution, body structure, and first execution

**T020 — Namespaces and imports.** Files: frontend/names.rs, project/cfg.rs. Implement local/module/prelude/type/macro buckets, import fixed point, aliases, visibility, and cfg membership. Acceptance: lexical shadowing, explicit-vs-glob resolution, missing/ambiguous imports, cyclic reexports, and inactive cfg items have dedicated deterministic tests. A macro-blocked namespace cannot be reported complete.

**T021 — Called-body structural index.** Files: syntax/body_index.rs, ir/pc.rs. Assign node, phase, slot, lexical scope, and loop-label identities without semantic evaluation. Acceptance: an entered function containing a cold invalid branch indexes without resolving that branch; nested function bodies are opaque; node/slot budgets fail only when that body is reached. Repeat indexing yields identical keys.

**T022 — Minimal LRIR types and verifier.** Files: ir/op.rs, ir/recipe.rs, ir/validate.rs, frontend/thunks.rs. Implement Literal, Unit, SetHint, ResultTransfer, ScopeEnter, ScopeLeave, and recipe exits first. Acceptance: malformed slot kinds and missing terminal exits fail structural verification; a legal Unsupported recipe passes structural verification but faults on execution. No successor thunk is requested by a branch recipe.

**T023 — Primitive values and checked arenas.** Files: runtime/value.rs, runtime/arena.rs, runtime/cell.rs, runtime/shape.rs. Implement scalar descriptors, handle epochs, cell states, transfer tickets, and literal records. Acceptance: stale/wrong-arena/zero/out-of-range handles fail; bit-copy cannot bypass guest ownership; ValueRepr layout assertions pass. A run reset invalidates every previous guest handle.

**T024 — First VM path.** Files: vm/frame.rs, vm/step.rs, engine/run.rs. Execute `fn main() { let x: i64 = 40_i64; x; }` through LRIR with explicit PCs/scopes. Acceptance: no AST evaluator exists; opcode counters are nonzero and accurate; a following cold function is not semantically queried. The CLI prints a structured actual result, not a hard-coded success.

**T025 — Numeric operations and contexts.** Files: vm/numeric.rs, frontend/types.rs. Implement Section 11, starting with every supported primitive and the strict unresolved-literal rule. Acceptance: exhaustive i8/u8 arithmetic boundary loops; random wider arithmetic/cast comparisons; works/fails context examples; signed-min literal; distinct usize/u64 contracts. Gate rejects silent i32 defaulting.

**Gate 02:** real LRIR scalar execution, checked handles, deferred cold semantics, explicit numeric context behavior.

### Milestone 03 — Control flow, calls, and basic tests

**T030 — Blocks, let, assignment, and shadowing.** Files: frontend/thunks.rs, vm/control.rs, vm/step.rs. Implement block phase templates and runtime storage contracts. Acceptance: RHS-before-LHS assignment trace; new binding initializer sees the old shadowed binding; immutable reassignment faults; loop-local reincarnations do not reuse stale refs or leak ownership. Source invalidity after an early return remains untouched.

**T031 — If, short circuit, while, and loop.** Files: frontend/thunks.rs, vm/control.rs. Implement branch-only selected thunk fetch, labels, break values, continue cleanup, and backedge polling. Acceptance: cold undefined branch and short-circuit RHS have zero thunk/query events; hot condition loops terminate correctly; nested labeled break/continue choose correct scopes. Fuel exhaustion is deterministic.

**T032 — Function signatures and call packets.** Files: frontend/signatures.rs, vm/call.rs. Implement concrete boundaries, generic structural substitution, ordered arguments, and return checks. Acceptance: uncalled invalid function signature/body does not fail another call; an unused generic bound is ignored; incompatible actual parameter/return descriptors fault only at reached boundaries; effectful arguments execute once.

**T033 — Dispatcher-owned recursion and interruption.** Files: engine/dispatch.rs, vm/frame.rs. Keep guest recursion on explicit frames and enforce frame limits. Acceptance: recursive factorial with concrete types; deep recursion hits guest frame limit rather than host stack overflow; cancellation/fuel produce the specified status and no duplicated output. Host call stack depth stays bounded in a 2,000-frame fixture.

**T034 — Minimal test harness and builtin assertions.** Files: cli/args.rs, engine/run.rs, facade/macros.rs. Discover/select zero-argument tests, implement assert and panic through guest outcomes, and isolate each test heap. Acceptance: one selected test passes despite another broken body; exact/substr/no-match behavior works; should_panic cannot accept an unsupported-feature fault. Add the fixture manifest/harness format from Section 28.

**Gate 03:** useful selected scalar tests, recursive calls, lazy branches, ordered effects through a fake broker, no host stack recursion.

### Milestone 04 — Aggregate ownership, references, and cleanup

**T040 — Nominal/tuple/array descriptors and construction.** Files: frontend/types.rs, runtime/object.rs, vm/step.rs. Implement deferred schemas and construction guards. Acceptance: named-field written evaluation order differs correctly from declaration-order storage; duplicate/missing field faults occur at reached construction; partial constructor panic drops completed fields once; cold unused schema errors are not forced.

**T041 — Places and leases.** Files: runtime/place.rs, runtime/reference.rs. Implement field/index/deref projection and checked reference leases. Acceptance: out-of-bounds/index-type faults; same-location scalar assignment remains visible through a ref; whole move invalidates inline refs; shared writes fail; two mutable guest aliases serialize without a host RefCell panic.

**T042 — Copy, move, and partial initialization.** Files: runtime/cell.rs, runtime/shape.rs, vm/step.rs. Implement lazy Copy policies, field moves, assignment transfer, and type-with-Drop restriction. Acceptance: String move/use-after-move; Copy scalar repeat reads; partial tuple/struct moves preserve other fields; double-owner property tests fail the buggy model and pass the implementation.

**T043 — Lexical scopes and temporary annotations.** Files: syntax/body_index.rs, vm/control.rs, vm/cleanup.rs. Implement Sections 13.5 and 18.4 explicitly. Acceptance: direct extending borrow, tuple/struct extension, extending pattern, non-extending call, Rust 2024 if-let else, block tail, and short-circuit temporary lifetimes match the recorded supported oracle cases.

**T044 — User Drop and explicit cleanup machine.** Files: frontend/dispatch.rs, frontend/types.rs, vm/cleanup.rs. Invoke concrete unbounded Drop impls as guest frames; clean live fields, locals, construction guards, and facade payloads in order. Acceptance: normal/drop/replace/early-return paths drop once; cold Drop body is prepared only on drop; partial moves skip moved fields; bounded ambiguous Drop selection faults when reached.

**T045 — Guest panic unwinding and fatal faults.** Files: vm/control.rs, vm/cleanup.rs, cli/render.rs. Implement ordinary panic cleanup, double-panic termination, and no guest cleanup after fatal profile/memory/resource faults. Acceptance: separate fixtures assert each difference through drop output; worker crash is not should_panic success; hostpanic injection terminates only the worker process in integration tests.

**Gate 04:** ownership and cleanup invariants pass, references remain host-safe, panic/fault distinctions are observable and tested.

### Milestone 05 — Patterns, dispatch, and finite language completion

**T050 — Transactional pattern engine.** Files: vm/pattern.rs, frontend/thunks.rs. Implement the closed pattern forms, match ergonomics, pending bindings, and commit. Acceptance: failed arms preserve scrutinee ownership; true arms move once; conflicting moves fault before commit; or-pattern names/modes and binding order validate; overlapping alternatives can rerun false guards without replaying scrutinee evaluation.

**T051 — Match, if-let, while-let, let-else, and question mark.** Files: frontend/thunks.rs, vm/control.rs. Connect the pattern engine to phase templates and exact Option/Result early-return rules. Acceptance: unvisited arms have no semantic thunks; no-match fault is reached only after all mismatches; let-else normal fallthrough faults; question-mark error type mismatch fails at return boundary without From resolution.

**T052 — Inherent and restricted trait dispatch.** Files: frontend/dispatch.rs, frontend/names.rs. Implement ordered receiver candidates, complete impl-header buckets, explicit qualification, default trait bodies, and obligation/ambiguity errors. Acceptance: receiver-rank precedence; unused bound succeeds; reached missing method fails; new competing impl invalidates cached dispatch; bounded candidate is not silently discarded.

**T053 — Named function values.** Files: runtime/value.rs, frontend/signatures.rs, vm/call.rs. Support named function-item values and concrete fn-pointer coercion with uniform guest dispatch. Acceptance: call a selected function through a local variable; incompatible fn signature faults; a numeric payload cannot become a callable; closure creation receives the specific unsupported diagnostic only when reached.

**T054 — Pure constants.** Files: frontend/constants.rs. Implement immutable ConstValue evaluator, memo dependencies, budgets, and runtime instantiation. Acceptance: array length constant, recursive bounded const fn, cycle, cold effect branch, reached forbidden effect, and two uses with independently mutable guest arrays. Constant memo contains no guest handles.

**Gate 05:** every supported core-language row in the feature manifest has positive and negative/reached-lazy fixtures.

### Milestone 06 — Facade, built-ins, and real capability broker

**T060 — Vec and slice operations.** Files: facade/collections.rs, runtime/object.rs, runtime/reference.rs. Implement exact Section 17 inventory and deterministic growth. Acceptance: push/pop/get/get_mut/index/slice, reallocation invalidation, nonreallocating push preservation, pop invalidating only removed element, consuming iteration drops remaining elements, quota reservation preserves prior ownership.

**T061 — String and formatting.** Files: facade/text.rs, facade/macros.rs. Implement UTF-8 operations, chars/bytes iteration, bounded formatter, and source-order argument handling. Acceptance: multibyte len/char/slice cases; invalid boundary panic/error; formatter unsupported spec; capture-name lookup; formatting never moves String; failure-only assert message remains lazy.

**T062 — Option, Result, scalar methods, and iterators.** Files: facade/option_result.rs, facade/collections.rs, facade/registry.rs. Implement all remaining finite inventory entries and resumable iterator drivers. Acceptance: empty Option/Result explicit context; unwrap/expect panics; `collect::<Vec<i64>>`, enumerate/count, inclusive max integer range terminates without overflow; user iterator fails explicitly.

**T063 — Macros and derives.** Files: facade/macros.rs, facade/derive.rs, frontend/types.rs. Implement the complete Section 18 inventory and lazy synthetic children. Acceptance: vec list/repeat including zero count evaluation; Debug/Copy/Clone/PartialEq/Eq field rules; unknown derive completeness; source-position intrinsics after edit; includes are source dependencies with deferred errors; user macros never execute.

**T064 — Worker protocol and fake-broker equivalence.** Files: service/wire.rs, engine/worker.rs, effects/protocol.rs. Move execution into the persistent worker; implement bounded framing, snapshots, run/reset, effect requests, and cancellation atomic. Acceptance: malformed/oversized/truncated frames fail closed; duplicate replies do not replay effects; 100 sequential tests retain caches but no guest state.

**T065 — Real broker permissions.** Files: effects/policy.rs, effects/broker.rs, effects/files.rs, effects/output.rs. Implement cap-std grants, captured output, injected environment, and fixed errors. Acceptance: default FS deny, read-only grant, write-only grant, prefix confusion, `..`, absolute path, escaping symlink, oversized read, and reused native policy scenario through the fake artifact stub until JIT arrives. No worker std::fs/env/process operation is introduced.

**Gate 06:** finite facade complete, real tests can read approved fixtures, no ambient runtime authority, bounded worker IPC.

### Milestone 07 — Baseline observed-region JIT

**T070 — Freeze ABI and helper registration.** Files: jit/abi.rs, jit/helpers.rs, jit/verify.rs. Implement Section 19 with layout/signature assertions and helper effect classifications. Acceptance: wrong ABI status, wrong frame ID, stale semantic epoch, unlisted symbol, and wrong scratch layout fail safely. The three permitted unsafe modules contain documented preconditions at every unsafe block.

**T071 — Record real executions and form regions.** Files: jit/record.rs, jit/plan.rs. Build candidates from successful completion hooks, using exact boundaries and 64-op cap. Acceptance: skipped branch/callee never enters a region; errored operation is excluded; effect/call splits occur; repeated traversal count is separate from opcode count; no guest value payload enters an observation.

**T072 — Generic Cranelift lowering.** Files: jit/lower.rs, jit/module_cache.rs. Emit helper sequence/status exits and closed imports; finalize before publication. Acceptance: force-mode repeated loop has actual native entries and helper-op counts; recursive guest calls still use dispatcher frames; a failed helper exits before following helper; interpreter/native transcript and fuel match.

**T073 — Precise exits and once-only resumption.** Files: engine/dispatch.rs, jit/helpers.rs. Implement canonical op PC, pending actions, and suppression on Deopt. Acceptance: inject guard misses after zero, one, and multiple completed operations; earlier writes/output/ownership transfers occur once; the missed operation executes once; later operations are not skipped. Artificial ABI inconsistency fails as an internal defect.

**T074 — Promotion policy and compilation failures.** Files: engine/install.rs, jit/plan.rs. Implement thresholds off/auto/force, deterministic eligibility, synchronous promotion, and degraded-mode reporting. Acceptance: auto threshold32 exact; force observes before compiling; one-shot force fixture reports zero native entries without replay; auto compile-resource fallback is visible; force compile failure fails its fixture.

**Gate 07:** a real generic native tier executes only observed code with identical semantics and precise failure/effect exits.

### Milestone 08 — Guarded scalar native execution and bounded code cache

**T080 — Scalar staging and commit.** Files: jit/helpers.rs, jit/abi.rs. Implement read-only staged operands and exact fuel/result commit. Acceptance: unresolved literal, different type, moved slot, or changed descriptor causes Deopt before commit; failure uses interpreter diagnosis; output slot ownership is reserved before mutation; scratch access never escapes.

**T081 — Whitelisted fast paths.** Files: jit/lower.rs. Implement only Section 21.4 operations, checked overflow, and proper primitive result identities. Acceptance: boundary tables and 10,000 random pairs per arithmetic/comparison type match the shared helper; native_scalar_ops is positive in hot-add/compare fixtures; multiplication/float fixtures execute via helpers, not invented unsupported fast paths.

**T082 — Artifact provenance and edit reuse.** Files: engine/install.rs, jit/module_cache.rs. Implement full keys, dependency validation, active epoch labels, and metadata pins. Acceptance: every Section 22.5 edit matrix row; permission changes do not recompile but deny effects; source-position intrinsics reflect new lines; changed shape triggers a guard miss without a second artifact for the same region recipe.

**T083 — Module arena retirement.** Files: jit/module_cache.rs. Implement region/code limits, safe boundary eviction, entry clearing, and metadata reclamation. Acceptance: tiny test budgets repeatedly create/evict arenas, no stale pointer is callable, active invocation prevents free, dropped entry cannot be relabeled as another artifact, and RSS stress does not grow without bound across source revisions.

**T084 — Native audit and reproducible diagnostics.** Files: jit/verify.rs, service/coordinator.rs, xtask/audit.rs. Audit CLIF import signatures, scratch loads, indirect calls, source-origin metadata, and closed lookup patch. Acceptance: deliberately malformed native plan is rejected before publication; diagnostic bundle is bounded/redacted; real CLIF dumps correspond to regions that entered native execution.

**Gate 08:** primitive native execution, dependency-correct warm reuse, safe code lifetime, no hidden symbol fallback.

### Milestone 09 — Cargo preparation, CLI, and agent workflow

**T090 — Managed Cargo manifest subset audit.** Files: project/cargo.rs, project/prepare.rs. Implement forbidden manifest/config detection and sanitized offline metadata invocation. Acceptance: ordinary two-package workspace accepted; optional/target-specific/build/proc-macro dependencies rejected; missing lock/cache fails offline; sentinel build.rs and proc-macro never execute.

**T091 — Prepared units and staleness.** Files: project/units.rs, source/manifest.rs. Implement target-specific cfg(test), dependency aliases, features, prepared IDs, and hash checks. Acceptance: unit vs integration vs run cfg fixtures; duplicate target ambiguity; manifest/lock/config edit blocks execution until prepare; source body edit does not require prepare. No rlib or generated-output loading path exists.

**T092 — Complete CLI and test result schema.** Files: cli/args.rs, cli/render.rs, cli/exit_code.rs, bin/cargo-lrr.rs. Implement Section 25 flags, JSON/human separation, exit priority, selected-signature checks, and bounded traces. Acceptance: golden CLI parser cases; ignored/should_panic/no-match semantics; real counters serialized; unsupported sandbox request exits before source execution.

**T093 — Stdio service and atomic overlays.** Files: service/requests.rs, service/coordinator.rs. Implement public NDJSON requests, progress/results, edit batching, stale-base failures, cancellation, and bounded queue. Acceptance: one client edits called function and reruns with cache reuse; concurrent cancel is processed; malformed request does not mutate snapshot; guest/protocol bytes cannot inject stdout log records.

**T094 — Watch, shutdown, and worker recovery.** Files: service/watch.rs, engine/worker.rs. Implement 100 ms polling, coalescing, snapshot capture, manifest-stale behavior, deadline/grace/kill, and safe restart between runs. Acceptance: repeated rapid source changes run only committed snapshots; no mixed revisions; crash resets native cache claims; shutdown reaps worker and releases broker resources.

**Gate 09:** end-to-end standalone and prepared Cargo workflows, structured agent service, actual warm edit/test loop.

### Milestone 10 — Differential, property, mutation, and robustness evidence

**T100 — Native Rust oracle harness.** Files: xtask/fixtures.rs, fixtures/native. Build only valid supported fixtures using pinned rustc, edition2024, opt0, overflow checks, debug assertions, and a fresh temp executable path. Acceptance: oracle cache keys include all source/helper/compiler inputs; unsupported-relaxed fixtures are never sent to rustc as expected-pass cases; outputs/drop order/results compare with explicit normalizations.

**T101 — Three-mode equivalence harness.** Files: xtask/fixtures.rs, fixtures/runtime. Run each fixture with off/auto/force on fresh guest state using scripted broker transcripts. Acceptance: same value/panic/fault, source origin, output, ownership/drop log, and fuel; mode-specific native counters differ as expected. Every force-native fixture must assert actual entries and direct scalar entries where applicable.

**T102 — Generators and reducer.** Files: crates/lrr/tests/property_*.rs, xtask/reduce.rs. Implement Section 29's expression/control/state model generators and deterministic seed replay. Acceptance: injected arithmetic, ownership, stale-cache, and replay bugs are caught; reducer produces a smaller failing fixture preserving expected failure class. Persist seeds and reduced cases as regression tests.

**T103 — Protocol, parser, and quota stress.** Files: crates/lrr/tests/stress_*.rs. Exercise malformed UTF-8/JSON, oversized lengths, parser recovery, module cycles, handle exhaustion, deep refs/types, and cancellation during compile/effect/cleanup. Acceptance: no unbounded allocation, hang, host panic on guest input, or unauthorized effect; timeouts are classified and subprocesses reaped.

**T104 — Full feature/diagnostic coverage audit.** Files: compat/*.toml, xtask/audit.rs. Require positive/reached-negative/cold-lazy coverage for every feature and exact inventory coverage for every opcode/facade/diagnostic. Acceptance: deleting a referenced fixture or leaving a release TODO/not-yet-implemented code makes the gate fail. Coverage is about actual exercised paths, not only file presence.

**Gate 10:** reproducible semantic, laziness, safety-boundary, cache, and native-use evidence for the entire declared profile.

### Milestone 11 — Measurements and bounded optimization

**T110 — Benchmark driver and datasets.** Files: xtask/bench.rs, benches/cases. Implement Section 30 cold/warm/edited/scalar/facade/preparation datasets with raw sample storage and process separation. Acceptance: setup time is separated correctly; native run includes compilation when promised; rustc/cargo baseline commands and cache states are recorded; source changes are verified, not simulated counters.

**T111 — Profile actual overhead.** Files: engine/telemetry.rs, docs/performance.md. Measure query validation, bound-op dispatch, helper calls, slot/arena lookup, scalar staging, compilation, broker, and cleanup. Acceptance: totals/overlaps are documented; pure interpreter vs generic native vs direct-scalar counters explain the observed results; no unsupported speedup claim appears.

**T112 — Implement only local in-architecture improvements.** Files: existing hot modules only. Permitted work is eliminating redundant hash/map lookups within one stable run, prebinding slot/op IDs, avoiding unnecessary host allocations, and batching already ordered output capture. Do not add a new tier, broaden speculative compilation, remove guards, or change guest semantics. Acceptance: all gates rerun unchanged; benchmark raw results show the change's effect. A nonimproving patch is reverted, not rationalized.

**Gate 11:** complete measurements, bounded memory across edits, documented performance limitations, no speed claims without raw results.

### Milestone 12 — Release handoff

**T120 — User and agent documentation.** Files: README.md, docs/profile.md, docs/security.md, docs/performance.md, AGENTS.md. Include supported examples, explicit Cargo subset, runtime vs Rust validation distinction, default deny examples, numeric annotation limitation, and no-hardened-sandbox warning. Acceptance: every documented command is exercised in a clean integration test; screenshots or examples do not imply unimplemented capabilities.

**T121 — Reproduce from clean checkout.** Run the Section 31 release runbook in a clean Linux x86-64 environment with the exact toolchain and dependency lock. Acceptance: all mandatory gates pass without untracked source files; fixture/benchmark artifacts are attached; version/license/vendor manifest complete; no ignored selected fixture, stale fixture expectation, or degraded JIT run remains.

**T122 — Final acceptance demonstration.** Execute the two-package demo: list/select a test; pass despite broken cold code; run a hot primitive loop through native code; process Vec/String data; deny then permit a fixture read; edit a called body through stdio; show unchanged-body/code reuse; revoke permission without recompilation; run native Rust validation separately on a valid version. Acceptance: a recorded JSON transcript and command script reproduce every assertion. This is the completion artifact, not a narrative claim.

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

**L004 — Early return skips later semantics.** Expect value1 and no preparation of marker.

```rust
fn answer() -> i64 {
    return 1_i64;
    /*lrr-marker:after_return*/ unknown!();
}
#[test] fn case() { assert_eq!(answer(), 1_i64); }
```

**L005 — Unused trait bound.** Expect pass; no trait obligation query exists.

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

**L007 — Cold unsupported closure/user macro.** A function containing closure creation and another containing a user macro invocation remain uncalled. The selected scalar test passes. Companion fixtures call each and assert the distinct closure/macro diagnostic. An item-position invocation whose expansion is needed for a reached name must instead report scope incompleteness; include that third case.

### 28.3 Numeric context and arithmetic fixtures

**N001 — Context from a reached call.** Expect pass; x's literal record becomes u64 at use64's argument boundary.

```rust
fn use64(x: u64) -> u64 { x }
#[test] fn case() { let x = 3; assert_eq!(use64(x), 3_u64); }
```

**N002 — No future-use inference.** Expect `LRR-E-TYPE-CONTEXT` at addition before use64 is called.

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

**J002 — Same generic body, different scalar shape.** Call a generic addition body repeatedly with i64 to promote, then with u64. Because bounds are not checked and custom operators are excluded, the reached primitive operation dispatches by actual scalar type. Expect a guard miss followed by correct helper execution, one artifact for the region recipe, and no duplicated preceding print/effect. Put the print in a separate earlier statement inside the body; compare its exact count.

**J003 — Injected guard miss after effectful prefix.** Host harness builds a valid observed region whose earlier generic op mutates a guest cell, then causes scalar staging to miss on a later op. Assert the earlier mutation's version/counter increases once, not twice. This is a VM unit fixture, not arbitrary source codegen. Run misses at every eligible op index.

**J004 — Call/return recursion under native regions.** A repeated recursive function reaches native regions, yet maximum native guest-call recursion remains one region invocation. Guest frame count records the actual recursion depth. No guest function is linked as a direct native callee.

**J005 — No native one-shot replay.** A test prints X once and returns, with force enabled. Expect stdout X, native_entries=0, and any compiled candidate must remain unentered until a future legitimate execution. Compiling an observed region is permitted; executing it solely for benchmarking is forbidden.

**J006 — Native overflow parity.** Warm a valid checked addition region, then pass maximum and one on the same guarded type. Expect the shared overflow panic, the same origin and fuel as interpreter, and no succeeding output. A wrapping result is a test failure even if the runtime later panics elsewhere.

**J007 — Closed imports and arena retirement.** Host-only negative native plans attempt an unknown symbol, an indirect call, and a load from a guest-derived address. All fail before publication. Tiny arena-budget runs repeatedly retire code, then revisit old PCs through fresh valid installation without a stale-pointer call.

### 28.6 Cache/edit fixtures

**E001 — Cold body edit.** Run J001 plus an uncalled function. Modify only that cold body's bytes through apply_changes. Repeat the test. Expect unchanged reached BodyKeys, zero Thunk recomputation for unchanged reached phases, native artifact cache hits, and no semantic preparation of the cold body in either revision. Parse/outline work for the changed file is allowed and recorded separately.

**E002 — Called body edit.** A main test calls helper returning1; edit helper to return2 and edit the test assertion appropriately, leaving a separate hot support function unchanged. Expect changed helper/test bodies reprepare and unchanged support native code reused. A second variant edits helper body only while test checks a property valid for both values, isolating caller reuse.

**E003 — Negative lookup becomes positive.** First run reaches missing function and faults. Add its declaration, keep caller body bytes unchanged, rerun. Expect negative lookup invalidation and successful execution, not a cached failure. Conversely removing a used function must invalidate a cached success.

**E004 — Candidate-set change.** Add a competing unbounded impl relevant to a reached method name. Expect deterministic dispatch ambiguity and no reuse of old selected-target fact. A separate unrelated impl edit must not invalidate a disjoint method bucket.

**E005 — Schema and location changes.** Change a used struct field descriptor and assert dependent TypeId/artifact invalidation. Separately insert newlines before an unchanged function with line!; output line changes while position-independent native region code remains reusable. Diagnostic spans must point to the new line too.

**E006 — Guest reset.** Execute the same test100 times with new run epochs. Captured output, variables, cells, and broker resources are fresh; source/query/native caches reuse. Host test deliberately attempts an old handle through a test-only checked API and receives stale-handle failure.

### 28.7 Permissions, Cargo, and protocol fixtures

**P001 — Deny then grant.** Fixture file contains `hello`. First run without grant calls read_to_string and sees PermissionDenied. Second run with read grant returns hello. Third run revokes grant while reusing code and again denies. File contents must never appear in an unauthorized result or trace.

**P002 — Escape attempts.** Separate cases for `../secret`, absolute path, `fixtures2` vs granted `fixtures`, nested escaping symlink, encoded NUL, and a write through a read-only grant. Expect denied errors and unchanged outside files. Test normal in-root symlinks according to the pinned cap-std behavior; only successful confined resolution is accepted.

**P003 — Environment/output limits.** A host environment secret is absent from guest var unless explicitly injected. stdout exceeds quota and produces the specified resource fault. Trace logs redact injected values. --nocapture cannot bypass the quota.

**P004 — Managed Cargo subset.** A two-package path-dependency workspace with a renamed alias and local feature prepares/runs. Integration tests see their package library and root dev dependency; dependency cfg(test) is false. Build.rs/proc-macro/optional/target-specific-dependency variants fail preparation without executing their sentinels.

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

Build the host runtime in release mode once. Record CPU model, core count, RAM, kernel, filesystem, runtime/compiler versions, build flags, source bytes, functions, packages, and selected test. Run benchmarks without concurrent compilation jobs. Preserve raw measurements rather than only an average.

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

The first three and memory correctness are mandatory architectural gates. The latency/throughput figures are hypotheses and nonblocking performance targets for 0.1; measured misses must be published prominently. Shipping a useful correct prototype with an accurately reported missed throughput target is preferable to claiming a speedup obtained by skipping work. No number in this document is a measured performance result.

### 30.5 Permitted optimization process

When a target is missed, use profiling to identify the dominant component, change only its local representation/access path within this architecture, and rerun all equivalence/laziness tests. Candidate local changes are prebinding worker-local IDs, reserving host Vec capacity from known bounded structure, avoiding repeated type-name formatting, using a direct active-run PC table, and sharing immutable source/descriptor storage.

Do not remove dynamic memory/capability checks, adopt unchecked native Rust references, suppress arithmetic overflow, precompile cold blocks, mutate live source snapshots, run tests on persistent guest state, or add a second JIT architecture. Those would change the problem rather than optimize this implementation.

<a id="section-31"></a>

## 31. Release runbook and completion evidence

### 31.1 Required generated artifacts

A completed repository contains: `Cargo.lock`; vendor provenance/license files; task ledger with all tasks complete; compat manifests with all fixture links; native-oracle fixture results; off/auto/force equivalence results; query/edit transcripts; permission/protocol stress results; actual JIT entry/CLIF evidence; raw benchmarks plus a summarized report; and the final two-package demo transcript.

Generated artifacts belong under `artifacts/` and include a version/build hash. Do not commit binaries, native code pointers, guest secrets, or large unbounded logs. Keep small deterministic fixture expectations and regression seeds under version control. Large run artifacts can be archived separately, but a release manifest must identify their digest and location.

### 31.2 Clean build sequence

Run from the repository root on the fixed Linux x86-64 target:

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

Do not advertise “runs any Rust crate,” “zero compilation,” “borrow-checker compatible,” “fully sandboxed,” or a speedup without scoped evidence. Accurate description: **a lazy Rust 2024 interpreter and observed-region Cranelift JIT with a default capability-mediated mode and an explicitly unsandboxed Cargo-native integration, designed for repeated live-code tests without recompiling the live crate.** Revision 3 additionally requires every edition/native preparation/ABI/hybrid gate in Section 44, all HN fixtures, and the profile/freshness/reset labels. Passing the original managed gates alone does not complete this release.

<a id="section-32"></a>

## 32. Instructions to the implementation agent

Copy the following operating rules into AGENTS.md, with a link to the checked-in specification:

> Implement the fixed LRR architecture and task ledger in order. Do not choose a different frontend, query framework, native backend, language profile, sandbox backend, or tiering strategy. Preserve explicit interpreter/JIT/capability semantics. Before changing code, identify the task and invariant being implemented. Add its acceptance test before marking the task complete. Run the current milestone gate after each integrated task. Never mark a selected fixture passed by skipping it, catching all errors as success, or hard-coding an expected output. Never load arbitrary native symbols or invoke guest build/procedural-macro code during live execution. Only explicit authorized native preparation may run Cargo-native build code, and only its immutable schema-checked generation may load in an authorized hybrid worker. Use pinned dependency source to repair mechanical API mismatches. Record an actual contradiction as a minimal witness plus regression and erratum, not an undocumented design fork. Finish with reproducible release artifacts and an honest feature/performance/security report.

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

[^R03]: Rust compiler developer guide, [Queries](https://rustc-dev-guide.rust-lang.org/query.html), and Salsa, [Overview](https://salsa-rs.github.io/salsa/overview.html). Background for input/derived-fact memoization. LRR deliberately implements the smaller algorithm specified here and does not depend on Salsa.

[^R04]: Rust project, [Announcing Rust 1.99.0](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/). Toolchain version/date reference; actual package compatibility is a bootstrap gate.

[^R05]: `ra_ap_syntax`, [package manifest](https://docs.rs/crate/ra_ap_syntax/0.0.356/source/Cargo.toml) and [SourceFile API](https://docs.rs/ra_ap_syntax/0.0.356/ra_ap_syntax/ast/struct.SourceFile.html). Parser version, required toolchain/edition metadata, parse API, and syntax-node ownership boundary.

[^R06]: `cranelift-jit` 0.136.2, [JITModule API](https://docs.rs/cranelift-jit/0.136.2/cranelift_jit/struct.JITModule.html). Native definition/finalization and memory-lifetime constraints.

[^R07]: Versioned package documentation for [serde](https://docs.rs/serde/1.0.229/serde/), [serde_json](https://docs.rs/serde_json/1.0.151/serde_json/), [blake3](https://docs.rs/blake3/1.8.7/blake3/), [clap](https://docs.rs/clap/4.6.7/clap/), [toml](https://docs.rs/crate/toml/1.1.6+spec-1.1.0), [cap-std](https://docs.rs/cap-std/4.0.3/cap_std/), and [proptest](https://docs.rs/proptest/1.11.0/proptest/). Individual version provenance, not evidence that the complete dependency set was built together.

[^R08]: Rust Reference, [Operator expressions](https://doc.rust-lang.org/reference/expressions/operator-expr.html) and [Expressions](https://doc.rust-lang.org/reference/expressions.html). Primitive arithmetic, cast, overflow, and operand-order reference for the supported subset. LRR's additional dynamic guards and strict inference policy are its own profile rules.

[^R09]: Rust Reference, [Destructors](https://doc.rust-lang.org/reference/destructors.html). Source for specified field/drop order and edition-sensitive temporary behavior. The document freezes an explicit supported 2024 annotation algorithm and requires native differential tests rather than assuming all current language behavior is implemented.

[^R10]: Rust Reference, [Method-call expressions](https://doc.rust-lang.org/reference/expressions/method-call-expr.html). Receiver-candidate ordering reference. LRR omits the full obligation/coherence system and explicitly faults when its restricted dispatch cannot establish a unique callable.

[^R11]: `cranelift-jit` 0.136.2, [JITBuilder API](https://docs.rs/cranelift-jit/0.136.2/cranelift_jit/struct.JITBuilder.html). Explicit symbols, platform fallback, and additive callback ordering. Motivates the narrow vendored clear-lookup patch rather than an ineffective None-returning callback.

[^R12]: `cranelift-jit` 0.136.2, [backend source](https://docs.rs/cranelift-jit/0.136.2/src/cranelift_jit/backend.rs.html). Inspected builder lookup storage and x86-64 relocation/PIC setup. The local patch changes only fallback lookup availability.

[^R13]: Cargo book, [cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) and [Dependency resolution](https://doc.rust-lang.org/cargo/reference/resolver.html). Metadata interface and the reason project compatibility must declare feature/unit limitations instead of treating the package graph as a complete executable build plan.

[^R14]: `cap-std` 4.0.3, [Dir API](https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html). Directory-capability operations used by the host broker. The application still needs correct grant selection, path validation, quotas, and integration tests.

[^R15]: Deno documentation, [Security and permissions](https://docs.deno.com/runtime/fundamentals/security/). Permission-interface inspiration only; LRR does not claim Deno-equivalent implementation or containment.

---

<a id="section-34"></a>

## 34. Rust 2024: required edition semantics and migration gates

### 34.1 Edition identity is part of every executable contract

`lrr-2024-v1` is the only live-source profile. Pass `Edition::Edition2024` for standalone sources and live Cargo units. Set generated packages to `edition = "2024"`. Every parse key, crate key, temporary-scope recipe, pattern recipe, portable cache header, bridge schema, and oracle fixture includes the edition/profile digest. Reject revision-2 cache records; never relabel their cached recipes as 2024. The edition's initial release was Rust 1.85; that is a minimum edition support fact, not a promise that every newer feature is in the live subset.[^H03]

A native dependency retains its declared edition. Cargo may compile an older-edition crate into an island used by a live 2024 test. LRR does not parse that crate as 2024 and does not apply its managed reference model inside that crate. An older-edition unit selected for live interpretation fails `LRR-E-LIVE-EDITION`; the diagnostic identifies the unit, not every dependency in the package graph.

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

Guest macro_rules expansion remains excluded. Therefore the 2024 `expr` versus `expr_2021` macro-fragment distinction is delegated to Cargo inside native dependencies, not partially implemented by a regex expander. A cold macro definition remains outline-only; a required item macro still prevents completeness as already specified.[^H08]

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

<a id="section-35"></a>

## 35. Native integration: locked design and exact reuse boundary

### 35.1 One engine, two authorized sources of callable code

The final execution graph is:

```text
live Rust 2024 tests and application bodies
                    |
         demand-driven LRR frontend
                    |
        interpreter / Cranelift regions
                    |
             shared call dispatcher
               /             \
       live CallPacket    NativeCallPacket
             |                  |
        lazy callee       boundary preflight
                                |
                       generated C-ABI entry
                                |
                   one Cargo-linked native island
                                |
                 optimized Rust/native dependencies
```

Managed execution has no native island. Hybrid-native execution uses the same LRR values, managed guards, query database, call continuations, and JIT. The only additional callable kind is `NativeBinding`. There is no direct import of arbitrary Rust symbols into Cranelift and no attempt to make the guest heap share native Rust layout.

The native side is a **dependency-closed set of Cargo library units**, compiled and linked together into exactly one generated `cdylib` for a preparation generation. A unit identifies a package, library target, feature selection, compilation target, and actual Cargo build configuration. Do not confuse a package with all of its units: an integration-test unit may be live while the same package's ordinary library unit is native.

### 35.2 What existing artifacts actually provide

An `.rlib` is a compiler-interpreted intermediate library, not a dynamic plugin. An `.rmeta` supplies metadata, not an executable implementation. Rust's library metadata and generic-code handling belong to rustc; LRR delegates that handling to Cargo and rustc rather than reading it as a runtime API.[^H10][^H11] The generated bridge provides explicit externally callable entry points and lets rustc select correct Rust calling conventions, layouts, monomorphizations, and link inputs.

The implementation reuses **compatible Cargo build products**, not arbitrary bytes found under a directory named `target`. Cargo's build directory can be separately configured, and its intermediate layout is internal. Use Cargo's artifact messages for actual paths.[^H12] Never require the path to literally contain `target/release/deps`, and never choose the newest-looking hash-suffixed library by filename.

Cargo incremental state remains Cargo/rustc-owned. There is no loader for `incremental/` object shards, query caches, dependency graphs, or work-product indexes. LRR benefits when Cargo subsequently uses those caches during native preparation. This is distinct from reusing an unchanged compiled dependency without invoking its compiler at all.

### 35.3 The selected decisions

| Decision | Fixed implementation | Accepted cost |
|---|---|---|
| Reuse authority | Cargo builds a generated member in the original workspace and reports which artifacts were fresh | Initial preparation may rebuild units whose configuration does not match existing artifacts |
| Native call boundary | Generated versioned C ABI, with explicit value codecs and native-owned handles | Not every Rust signature is automatically bridgeable |
| Native granularity | Dependency-closed library units, not unchanged functions inside a mutable crate | Editing native source requires preparation; no arbitrary hot patch of optimized internal calls |
| Code-generation reuse | Existing optimized dependency code plus cached generated bridge | New generic instantiations or newly bound functions may need wrapper compilation |
| Loading | One immutable cdylib per worker process, loaded explicitly and never unloaded while the process lives | A new native generation requires a new worker |
| Execution permission | Explicit `--execution hybrid-native --allow-native` and preparation authorization | Whole-run sandbox guarantees are withdrawn |
| Managed memory rules | Keep all existing managed guards and add call-scoped native alias/shape guards | Borrow-invalid live programs can fail at a native boundary even if managed operations tolerated them |
| State reset | Fresh hybrid worker for each run request; serial selected tests share that process | Hybrid JIT machine code is not retained between requests |
| Incremental frontend reuse | Validated portable source/query checkpoints across hybrid workers | Checkpoint import/remapping adds some startup work |
| Cache miss | Reached diagnostic with an exact preparation request; never compile or replay implicitly | A first use of an undeclared API requires explicit preparation |
| Public API discovery | Explicit deterministic binding manifest, compiled for validation | No magical reflection over every public function in an arbitrary rlib |
| Native-to-live callbacks | Prohibited in this release | Closure-heavy callback APIs need a native wrapper that completes the whole operation on the native side |

These decisions replace revision 2's blanket ban on native artifacts. They do not reopen frontend/backend selection.

### 35.4 The fast loop that must work

For a live application library depending on native `numeric_kernel`:

1. Run explicit preparation once. Cargo may report `numeric_kernel` as fresh from an existing optimized build. It compiles/links the generated bridge if needed.
2. Capture a live test and its called source bodies without rustc. Run the test. Calls into `numeric_kernel` cross the generated bridge and execute the compiled implementation.
3. Edit only a live function body or test assertion. Capture a new live snapshot, reuse portable source/query work, and execute with the same native-generation file. **No Cargo process, no rustc process, and no linker process may start during the run.**
4. When a test changes only its inputs, the already-bound native function remains available. When it reaches a previously unbound native specialization, stop with the prescribed missing-binding diagnostic; do not re-execute prior test effects after preparing it.
5. An edit to a native package, relevant manifest, or known build input invalidates preparation. Run explicit native prepare, let Cargo reuse what it can, then create a new generation and a new worker.

For an integration-test-only edit, the application's unchanged compiled library can itself be native. For a unit test of edited private implementation details, that library remains live. The compiled release library does not expose private unit-test functions and normally lacks `cfg(test)` code; never promise to obtain either from an ordinary `.rlib`.

### 35.5 No invisible split inside an optimized crate

A native function may contain inlined copies of other functions, constant-folded facts, and direct calls that do not use LRR's dispatcher. Consequently, changing a live function cannot update an optimized native caller merely by changing an entry table. The implementation refuses any native dependency path that reaches a unit designated live. Native inputs are frozen as a closure, and their behavior is tied to the generation.

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
schema = 1
profile = "release"
native_toolchain = "project"
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

Each binding has a deterministic `BindingKey`: ABI version, native schema version, dependency alias identity, public guest path, canonical Rust target path, concrete generic arguments, argument codecs/modes, result codec, and descriptor identities. Sort bindings by key bytes; assign operation IDs starting at 1. Reserve operation 0 for the generated native-handle drop operation. Additions can renumber IDs, so every call recipe includes the generation and schema digest. Never persist an operation number without those digests.

The manifest is authoritative for **what can be called through LRR**, not a claim to enumerate all methods in the native crate. A missing method on a native type is a reached `LRR-E-NATIVE-BINDING-MISSING`; LRR does not inspect native source and guess another method. Methods/traits authored on the live side for a native opaque type are not supported in this release. Explicit conversions to live scalar/data types remain possible through declared bindings.

### 36.4 Required codecs

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

Version 1 does not bridge arbitrary live structs, Rust references into the guest heap, trait objects, native function pointers, closures, iterators, futures, borrowed returns, raw pointers, or arbitrary `Vec<T>`. A concrete native collection can be stored as an opaque type with explicit operations. This is a useful execution boundary, not Rust ABI reflection.

### 36.5 Closure validation and source ownership

Compute the live-unit set from selected targets plus explicitly live source dependencies. Compute each native root's resolved normal target-dependency closure through Cargo preparation. Build scripts and proc macros belong to the native build closure, not executable guest units. Require the native runtime closure to be disjoint from live runtime units except the integration-test/package-library distinction above.

If a native crate depends on a crate designated live, fail with a deterministic shortest dependency path. Do not build the native crate against the last compiled version of that live crate. If a package appears in both graphs as different feature/configuration units, treat those as distinct nominal worlds and refuse guest-value interchange unless they meet through an explicit data codec. Do not equate native type identities by their text name.

Live source snapshots include no native function bodies. Native source is captured for freshness/provenance hashing, not semantic indexing. Native macro expansion, trait solving, const evaluation, and borrow checking happen only in explicit native preparation when Cargo considers the relevant unit dirty. Running a selected live test does not reperform them.

<a id="section-37"></a>

## 37. Cargo bridge preparation and existing artifact adoption

### 37.1 Separate init, prepare, and run

`native init` modifies configuration and generated workspace membership, but does not execute native code. `native prepare --allow-native-build` invokes Cargo and its authorized native build tools, emits a bridge, validates it in a probe process, and publishes a generation. `test/run --execution hybrid-native --allow-native` consumes that published generation without invoking a build tool.

Native authorization is not inferred from finding `.rlib` or `.so` files. `--allow-native-build` authorizes build scripts, procedural macros, configured compiler wrappers, linkers, and other native build-time execution within the declared policy. `--allow-native` authorizes the generated library and its transitive native runtime code. Neither flag is implied by a file read grant.

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

`native_toolchain="project"` means resolve the Cargo/rustc toolchain that applies in the original workspace under its rustup override and explicit preparation environment. Record absolute tool locations, `cargo --version`, `rustc -vV`, target specification identity, and relevant configuration before building. An explicitly named installed toolchain is also a configuration input; there is no automatic fallback to a different version. Generated source requires native edition-2024 support, so reject a compiler older than 1.85 at preparation.

Preserve the project's existing compilation target mode. For `target_mode="host"`, do not inject `--target`, even if the host triple is known; explicit target mode can change Cargo build sharing. A declared target triple must equal the supported Linux x86-64 execution target. Record whether it was explicit. Respect existing target-dir/build-dir configuration and pass a directory override only if it was explicitly recorded in native configuration.

Do not add or replace RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, CPU flags, linker flags, or a rustc wrapper merely to gather telemetry. Record those inputs and allow Cargo to determine their cache effects. Reject `-C prefer-dynamic`, Rust dylib dependencies, linker-plugin-LTO-only artifacts, and unsupported custom link scripts in this first release; they complicate the single-artifact linkage contract. Ordinary native system-library dependencies are allowed and recorded. A failed shared-library link caused by non-PIC inputs is a native preparation error, never a cue to force flags and rebuild everything silently.

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
    schema_version: u32,              // 1
    generation: Digest,
    bridge_abi: Digest,
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

The digest includes compiler/configuration identity, selected dependency artifact hashes, all binding/codec schemas, generated source, relevant manifests/lockfile/configuration, source-input digests, declared extra build inputs, and the bridge bytes. It does not include wall-clock timestamps, Cargo fresh/rebuilt flags, elapsed times, temporary staging directory names, the live test body hash, or the user's run-time filesystem grants. Compute it from an explicit GenerationIdentity record that also excludes the generation field itself and the derived content-addressed output path; never hash a self-referential serialized manifest. Put per-prepare observations, raw Cargo messages, fresh flags, and timings in a separate attempt report referencing the generation. An unchanged native prepare can therefore reuse an identical generation while reporting different freshness observations.

Do not put a bare `.so` path in the prepared live manifest. Store a generation identity plus an immutable content-addressed location and expected hash. The user-visible trust decision names that generation and the workspace from which it was prepared.

### 38.2 Native source input inventory

Before invoking Cargo, hash native package source-tree membership/content, declared external inputs, and manifests/configuration, excluding generated output directories. After the build, compare those same original source inputs to reject a source race; use Cargo dep-info to refine the published future invalidation inventory. Generated OUT_DIR files are expected to change during the build: capture them after Cargo finishes and verify a second stable read before publication. No post-build-only snapshot may be used to claim the source stayed unchanged throughout compilation.

Use public dep-info as a source-input aid, not Cargo's private fingerprint database. Cargo documents `.d` dependency files for external build systems.[^H12] Discover candidate `.d` files only in directories of artifacts actually reported by Cargo; parse their Make-style rules and associate them by matching a rule's target to a reported artifact path. Do not guess the dependency file's basename, or assume that stripping `lib` from an rlib name is a stable API.

The parser must handle escaped spaces, backslashes, line continuations, multiple targets, and empty phony dependency rules on the supported Linux target. Canonicalize paths relative to the recorded compilation/workspace location. Reject malformed or ambiguous matches. A `.d` file is not executable metadata, and its contents never determine a function address.

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
3. Inspect the shared-object metadata without executing it to check machine type, ELF class, exported entry name, and declared dynamic dependencies. Use a bounded standard system-tool invocation in preparation (`readelf`, recorded by version) for this first Linux release; do not execute `ldd` on an untrusted library.
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

## 39. Generated native ABI, codecs, and object storage

### 39.1 Ownership of this ABI

The ABI in this section is **LRR's own C-compatible interface**. It is not Rust's internal ABI and does not expose `ValueRepr`. The runtime and generated bridge each compile their own copy of `native_abi_v1.h`/its Rust representation. A bootstrap test compares sizes, alignment, field offsets, discriminants, and encoded golden messages. No Rust allocation, reference, trait object, enum layout, exception, or `std::any::TypeId` crosses this boundary. Rust's FFI and layout documentation explain why an explicit representation and ownership contract are necessary.[^H18][^H19]

Only these categories cross as C parameters: fixed-width integers, a pointer to immutable request bytes plus a length, a pointer to an output structure, and an opaque native-context pointer. A pointer is valid only during the specified call or reply lease. None is a guest value. Raw pointers are confined to `native/ffi.rs`, `native/loader.rs`, and the generated FFI shell; everything behind the shell uses checked Rust containers and typed native calls.

The generated bridge does not depend on the LRR runtime crate. It contains generated schema constants, codec/object-store support copied from versioned templates, and ordinary Cargo dependencies on the declared native packages. This prevents linking the interpreter and its Cranelift dependencies into every bridge and permits a different project Rust toolchain. Changes to a copied template change the bridge source digest and require preparation.

### 39.2 Entry table

Export exactly one application entry symbol, `lrr_native_v1`, with `#[unsafe(no_mangle)]`. Runtime/system symbols emitted by the toolchain are not interpreted as bindings. Its signature is:

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

On x86-64 the API table is 104 bytes aligned to 8; the reply is 32 bytes aligned to 8. Assert these values and every field offset on both sides. Never cast a shorter or differently versioned table to this structure. The entry checks non-null output, correct alignment, and sufficient size before writing; returns `ABI_MISMATCH` otherwise. The loader allocates `MaybeUninit<NativeApiV1>`, calls the entry, and assumes initialization only after success. It verifies every header field and schema digest before using a function pointer. All five function entries must be Some; nullable function-pointer fields permit rejecting missing entries without constructing an invalid Rust function-pointer value. The pointer cannot prove a hostile library safe; authorization is the trust boundary.

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

For declared concrete native types generate a closed enum:

```rust
// Concrete variants are generated from the binding manifest.
enum NativeValue {
    Plan(Box<kernel::Plan>),
    // One variant per declared type. No trait object crossing the ABI.
}
struct NativeSlot {
    type_ordinal: u32,
    value: std::cell::RefCell<Option<NativeValue>>,
}
```

The context owns an append-only vector of slots. Slot zero is unused. Native handle high 32 bits are the run epoch, low 32 bits the slot index; slots are never reused in that context. Preflight all ordinals and generations. Use `try_borrow`/`try_borrow_mut`, never panicking `borrow` helpers, and retain guards for the duration of a binding. An owned argument takes the slot's value only after all argument validation and output reservations pass. Multiple ownership uses or owned-and-borrowed uses of one handle fail before taking any value. The generated typed match arms produce the actual `T`, `&T`, or `&mut T` arguments; no transmute or unchecked downcast is needed.

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

No whole-program alias pass is added. Existing managed aliases can remain as dormant checked handles while a synchronous native call runs because there are no native-to-guest callbacks and no concurrent live execution. The worker marks involved guest locations locked for that call. Overlap between arguments used concurrently by native code is rejected. A resumed guest access sees completed writeback; stale/moved handles still fail the normal guards. Do not confuse dormant managed handle existence with creating a native Rust reference for that dormant handle.

A native function can create internal threads using its own Rust code and owned data. The supported borrowing wrappers cannot let a non-'static borrow escape into an unjoined thread without unsafe code. Unsafe native implementations remain trusted; the runtime cannot prove them safe. A returned object can encapsulate synchronization or interior mutability; access still goes through its explicitly bound methods.

### 40.2 Native call state machine

Implement `NativeCallState` with these states and no backward transitions after Entered:

```text
Resolve -> EvaluateArguments -> Validate -> Reserve -> Stage -> Ready
                                                          |
                                                       Entered
                                                          |
                              +---------------------------+-------------------+
                              |                           |                   |
                        DecodeSuccess                 DecodePanic        ProcessFailure
                              |                           |                   |
                          WriteBack                   WriteBack           terminate
                              |                           |
                           Adopt                   PublishGuestPanic
                              |
                       PublishResult -> ResumeCaller
```

Before EvaluateArguments, resolve the explicit binding ID and concrete schema. An undeclared direct binding produces `LRR-E-NATIVE-BINDING-MISSING` without evaluating that nonexistent callee's arguments, matching the live missing-name policy. For an already evaluated function value or receiver, preserve prior effects and never replay them. Generic arguments must select one prepared concrete binding; no bridge is generated on a first live call.

Evaluate arguments left-to-right using existing transfer tickets. The worker Validate phase checks concrete shapes, numeric ranges/materialization, ownership, native generation/type identity, borrow bounds, alias groups, UTF-8, request limits, and mutable destination accessibility. Native preflight repeats wire and handle checks defensively. Reserve phase allocates all known request, writeback, and continuation storage and reserves output-handle bookkeeping. Stage serializes or copies values. Owned guest values become call transfers under the existing move semantics; after a fatal preflight fault there is no subsequent guest execution regardless.

Set the caller's return continuation and canonical frame state before Ready. Commit Entered once and increment `native_binding_entries` immediately before the FFI call. A worker signal/crash/timeout after this point reports `effect_state="may_have_executed"`. Never jump back to Ready, reinvoke on malformed reply, switch to source fallback, or rerun the test automatically.

On success: validate the entire reply, write back mutable groups, prepare/adopt output ownership, clear call locks, then publish the result. On caught native panic: validate/write back groups, release the reply without adopting ordinary results, clear locks, and enter the existing guest panic cleanup machine. On `POSTCALL_FAILURE`: preserve any valid returned writeback, report entered/fatal failure, and terminate the worker without executing more guest code. If writeback itself cannot be trusted, report `writeback_state="unknown"`; do not manufacture a completed state.

### 40.3 Generated binding body recipe

For every manifest binding the generator emits exactly one typed arm:

1. Match the ordinal and decode against that arm's static schema.
2. Validate all slot identities, access modes, and non-overlap before taking any owned slot.
3. Acquire checked object and staging guards in ascending slot/group order.
4. Decode owned strings/bytes to Rust-owned allocations and take owned object variants. No arbitrary receiver method lookup happens here: the path is explicit in generated source.
5. Mark entry and invoke the ordinary safe Rust expression. Write the concrete return type in a local annotation to make compiler errors local to this binding.
6. Drop temporary argument borrows, but retain buffers containing mutable writeback.
7. Encode the result and mutable groups; allocate/record pending output handles.
8. Return a leased reply. Any native unwind from the call or its argument cleanup is caught by the arm's native unwind boundary, then encoded with writeback.

A binding to `kernel::sum::<u64>` literally calls that path with decoded values. A method binding uses either the explicitly declared inherent associated path or fully qualified native trait path supplied in the manifest grammar. It does not ask LRR's method resolver to implement the native crate's trait system. The schema's live names and types become an opaque native declaration namespace; unlisted native methods remain missing even if the underlying Rust crate implements them.

Generate a diagnostic source map from each arm back to manifest line and binding ID. A private function, unsafe direct call, wrong generic arity, inaccessible type, or unsatisfied bound is a native preparation compile failure. Report the rustc message and generated arm path; do not weaken the generated code or fall back to unchecked ABI calls to make it compile.

### 40.4 Panic and process failure

Generated bridges built with `panic=unwind` catch Rust unwinding **within the bridge**, before the extern C function returns. They encode a bounded panic description and keep response/writeback storage outside the catch closure. Use `AssertUnwindSafe` only around the explicitly audited state machine; after a panic, a partially consumed native object is either absent or marked poisoned, never silently restored. A native type's application-level invariants after panic are those of that native implementation; LRR does not promise rollback.

A bridge built under a Cargo profile with `panic=abort` is allowed to preserve reuse. Its table reports that mode. Any abort, segmentation fault, illegal instruction, process exit, allocator abort, or cross-boundary unwind violation is `native_process_failure`, never a guest panic and never a passing `#[should_panic]` test. Rust's ABI/unwinding rules do not permit treating arbitrary unwinds across a non-unwinding C boundary as normal return values.[^H20]

Install no process-global panic hook that calls into the worker. The bridge may leave the native hook intact; stderr is captured as native process output. Expected-panic matching uses the encoded payload, not a heuristic parse of stderr. Only string and static-string payloads get their text; other payloads become a fixed opaque-payload description. Destroying an exotic payload can itself panic: catch that cleanup inside the bridge where possible and otherwise terminate as infrastructure failure. Native panics from `Drop` during an already active guest panic are double-panic termination under the existing cleanup rules.

The host runtime remains built with panic=abort. It does not try to catch bridge panics in the interpreter or Cranelift helpers. The generation's separate Rust standard library and panic strategy belong to the native side of the C ABI.

### 40.5 Drops and fatal faults

Add native object destruction as an explicit action in the common drop machine. Normal scope exit, assignment replacement, moved argument cleanup, and ordinary guest panic schedule each live native handle exactly once in guest drop order. `drop_handle` removes ownership from its slot before invoking native Drop, so an unwinding destructor cannot be called a second time. Bridge-owned fields are dropped by ordinary native Rust, not reinterpreted by LRR.

Per-test success/panic cleanup must leave the native object table empty and no reply leased before context destroy. Remaining unexplained entries are a runtime ownership bug and fail the test infrastructure; explicitly forgotten values follow Section 44.7 instead. Native process statics are not entries in this table and do not reset here.

On a fatal managed runtime fault during a hybrid run, stop guest cleanup as specified in Section 13 and terminate that worker **without** dropping a context containing native objects. Deliberately leak that context until process exit; do not let a convenient Rust RAII wrapper invoke arbitrary native Drop after the fatal fault. Managed-only physical allocations may be reclaimed normally. Future requests use a new worker.

### 40.6 Semantic profile and result meaning

Live code retains checked arithmetic and `debug_assertions=true`. Native code retains the exact recorded Cargo profile. In particular, an ordinary release profile can have unchecked overflow and disabled debug assertions; do not claim the combined run is equivalent to ordinary debug `cargo test`. Preserve and display those settings rather than silently rebuilding optimized artifacts with different flags.[^H13]

A hybrid result establishes that the selected live path passed under `lrr-2024-v1` against the recorded native generation, subject to the listed boundary semantics. It does not certify that the live crate compiles with rustc or that a different native profile behaves identically. Keep separate CI tasks for normal Cargo tests/validation. This separation is not optional product wording: it prevents a fast test loop from being mistaken for Rust acceptance.

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

Extend `CallableId` with `NativeBinding(NativeBindingKey)`. Extend the prepared call operation with a `NativeCallRecipeId`. Call resolution, argument expression preparation, coercion hints, and source-origin tracking remain common. Native schema lookup happens only when a call/name is actually reached; listing bindings at prepare time does not prepare their live callers.

The dispatcher routes a native target through Section 40 after argument evaluation. It does not push a guest body frame or ask for a MIR/thunk for the native body. Native calls are atomic external execution steps relative to the live interpreter; their internal native calls are not lazy queries. A large fully optimized native function executes its ordinary compiled control flow.

The baseline Cranelift recorder terminates the observed region **before** a native call. Spill all live values into canonical frame cells, emit the ordinary return-to-dispatch status, and let the shared dispatcher perform the call. The continuation after the native call can start a separate observed region. This fixed design prevents native ABI objects from entering Cranelift IR, makes panic/ownership handling identical in both tiers, and avoids side-effect replay on a guard miss. Do not initially inline bridge codecs or directly call plugin function pointers from generated machine code.

Consequently, a hot loop with one tiny native call per iteration may be dominated by crossings. The prescribed remedy is a coarser explicitly bound native batch operation, not an unreviewed direct Rust ABI optimization. Benchmark this cost separately. An agent may add a new binding to a user native wrapper, but it must not silently transform loop effects or reorder calls.

### 41.3 Three cache domains

Keep three independently owned cache domains:

1. **Cargo cache:** existing dependency artifacts and rustc incremental data. Cargo alone determines compiler reuse during preparation.
2. **LRR native-generation cache:** immutable linked bridge plus provenance. It survives process restart; a valid generation can be loaded without running Cargo.
3. **LRR live-code cache:** pure source/query facts and reached LRIR recipes; worker-local Cranelift regions remain executable-memory objects, not serializable native library entries.

Give them separate hit/miss counters. A Cargo dependency hit does not imply a Cranelift hit; a bridge load is not JIT compilation; a portable query import is not native code restoration. Never print one undifferentiated “incremental cache hit” counter.

### 41.4 Hybrid worker lifetime

Each `run`, `test`, or service `run` request in hybrid mode starts a fresh `lrr-worker` process. The coordinator checks generation freshness and authorization first, spawns with the recorded native loader environment and current working directory, then sends the immutable live snapshot and portable checkpoint. The worker loads one generation, validates the table, and runs the request's selected tests serially. After results/checkpoint transfer, it exits. No fork of a multithreaded interpreter or native-loaded process is used.

Create a fresh native context and managed heap for each test. Drop those at normal completion before the next test. Native process-global variables, TLS, allocator state, once-cells, and threads may persist between tests **inside that same request**, as they can in a native test executable. The result includes `native_global_reset="per_run_request"` and the selected test order. Users requiring a process boundary between two tests submit separate run requests; this is use of the same fixed lifecycle, not a second worker policy.

On the next edit/run request, the old worker is gone and all native process state starts fresh. Native effects outside the process, such as files, network writes, subprocesses, and shared memory, are not undone or reset. Process isolation here provides lifecycle/crash handling, not a hostile-code sandbox.

Cranelift code from a previous hybrid worker is intentionally **not** restored. Warm tests initially interpret or re-JIT their reached live regions while continuing to use the already optimized native island. The persistent managed mode retains its existing in-memory JIT behavior. Report this trade-off; do not claim cross-request executable-code reuse that has not been implemented.

### 41.5 Portable source/query checkpoint

Implement the portable checkpoint before calling the hybrid edit loop incremental. The worker exports pure artifacts to the coordinator after a normally completed run request, and the coordinator supplies them to the next worker. The coordinator owns at most 64 MiB per open project; no checkpoint is accepted directly from a service client. Use the existing framed chunk protocol with an independent declared length/digest. A failed/crashed worker contributes no new checkpoint; the last validated checkpoint is retained.

Checkpoint header:

```text
format = "lrr-portable-v1"
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

The payload uses explicit serde records, not memory dumps. A record contains stable query key, stable symbolic result, its output fingerprint, ordered dependency stamps with stable keys/fingerprints, estimated bytes, and usage rank. Export only the following whitelist in v1: `Outline`, `BodyStructure`, `Signature`, `ScopeBucket`, `LookupPath`, `Thunk`, `NativeExportBucket`, `NativeSignature`, and `NativeCallRecipe`. Export only record variants whose full payload has a defined portable encoding. Omit an ineligible record; do not discard checks to increase hit rates.

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

Keep public protocol version 1 and add explicitly documented optional fields with defaults to its schemas; old managed requests remain valid. The old rejection of unknown fields still applies after the schema includes these fields. Add `execution` (default managed), `allow_native` (default false), and `native_generation` (absent by default) to service open/run request definitions and the CLI watch options. A hybrid request must supply a generation matching its prepared project. Arbitrary paths, dynamic library bytes, function addresses, and custom binding bodies are never run parameters.

Add coordinator methods `native_init_preview`, `native_init_apply`, `native_prepare`, and `native_status`. `native_init_apply` requires the preview's input digest and `allow_workspace_edit=true`; stale previews fail. `native_prepare` requires `allow_native_build=true` and the bindings/config digest. These operations are permitted only while no run is active. A run that discovers stale preparation returns a structured failure; it does not enqueue native_prepare or authorize it on the user's behalf.

Example run request (the repeated-digit digests illustrate encoding; actual requests must name existing prepared/snapshot identities):

```json
{
  "schema_version": 1,
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
    "replace_pending": false
  }
}
```

The canonical JIT field is `jit`; do not introduce a second `mode` spelling. Managed/hybrid-native execution and off/auto/force JIT selection are independent dimensions. Old required run fields stay required; only the documented hybrid extension fields default to managed/false/absent.

Internal worker handshake adds execution mode, generation digest, and portable-cache version. The worker reports library handshake completion before any test starts. Prepare probe and execution worker have distinct request types; a probe cannot accidentally execute test bindings.

### 42.4 Hybrid test result additions

Retain all common result fields and add these required hybrid fields:

```json
{
  "execution": "hybrid-native",
  "profile": "lrr-2024-v1",
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
  "fuel_coverage": "live_only",
  "memory_limit_coverage": "managed_and_bridge_buffers"
}
```

These zero-valued counters illustrate field types, not a completed run. Keep the existing `native_entries` counter defined as Cranelift region entries for backward compatibility; add an explicit `jit_region_entries` alias carrying the same value and document it. `native_binding_entries` counts calls into Cargo-native code and must never be inferred from Cranelift entries. Report compiler/Cargo absence using process-invocation hooks and acceptance-test supervision, not a hardcoded zero in every result.

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

Measure five separate scenarios on the same pinned Linux machine, with at least 30 timed samples after setup: first bridge preparation from compatible pre-existing artifacts; unchanged explicit prepare; fresh-process warm-generation test; live-only edit followed by test; and native dependency edit followed by prepare/test. Publish medians and p95 with source/project sizes, CPU, storage, toolchains, profiles, LTO settings, test count, process startup, and whether OS file caches were warm.

Use four workloads: one scalar native call; one native byte-buffer batch; an opaque-object lifecycle with many method calls; and a larger dependency dominated by native execution. Compare supported cases with normal `cargo test --release` and managed-only LRR where the same code is in scope. Do not compare against `cargo clean` unless the experiment is explicitly a cold-build comparison. Include a deliberate unfavorable tiny-call loop to quantify marshalling/dispatcher overhead.

Correctness/reuse gates are hard requirements: HN001 proves dependency freshness in the controlled matching fixture; HN003 proves zero compiler/linker invocations after live edits; every hybrid mode preserves result/effect semantics; native entry and process reset counters are actual. Latency targets are **unmeasured engineering targets**, not product facts: under 100 ms median for a small fresh-worker warm-generation live-only test after capture on the reference machine, and native bulk compute within 20% of the same wrapper in a minimal native harness when boundary costs are amortized. Missing these targets requires a profiling report and a concrete local optimization issue; never remove guards, reuse native global state, skip tests, or hardcode a success to hit them.

Profiles with LTO, very large generated monomorphizations, heavy constructors, large source snapshots, or frequent fine-grained native crossings may miss the targets. Publish that evidence. “Instant” means the user-visible goal of no repeated compiler/linker on the common edit loop, not a promise of zero startup or zero work.

<a id="section-44"></a>

## 44. Ordered edition/hybrid implementation tasks and gates

### 44.1 Execution order

Apply H001–H003 while implementing the original core tasks, before considering the Rust 2024 edition ready. Build the standalone Cargo probe H004 immediately after bootstrap; it validates the reuse premise without waiting for the whole interpreter. Finish the common engine gates with 2024 semantics. Then implement H005–H028 in order. Each task is required; the probe is not a substitute for hybrid runtime integration.

Keep an implementation ledger with task ID, commit, commands actually run, fixture IDs, artifact paths, and pass/fail evidence. A source file existing is not an acceptance result. Each gate is a script under `xtask`, exits nonzero on skipped/missing tests, and writes a machine-readable report with the runtime build ID.

### 44.2 Edition gate

**H001 — Edition identities and parser.** Files: core/profile.rs, syntax/parse.rs, project/units.rs, fixtures/edition2024. Replace live edition/profile constants with 2024, include edition in every affected cache key and checkpoint header, update CLI/help/result fixtures, and leave native dependency editions opaque. Acceptance: parser smoke uses Edition2024; E2409/E2411 pass; selected older-edition live target rejects while a native dependency does not.

**H002 — 2024 temporary lifetime machinery.** Files: syntax/body_index.rs, vm/control.rs, vm/cleanup.rs. Implement the explicit success/failure boundaries, tail temporary drain, and let-chain clause scopes in Section 34. Add native-oracle output fixtures before implementation. Acceptance: E2401–E2407/E2410 pass under off/auto/force with equal drop transcripts and no preparation of skipped clauses.

**H003 — Pattern ergonomics and reserved namespace.** Files: vm/pattern.rs, frontend/names.rs, facade/prelude.rs, compat/features.toml. Thread DefaultBindingMode through matching, reject only the reached prohibited modifier, and add unsupported async-prelude identities and edition tokens. Acceptance: E2408 plus shadowing/explicit-reference fixtures; full prior pattern suite rerun. Gate `xtask edition2024-gate` requires H001–H003 and all E24 fixtures except native-dependent E2412, which joins the final hybrid gate.

### 44.3 Native reuse proof and preparation gate

**H004 — Artifact reuse probe.** Files: tools/cargo-reuse-probe, artifacts/probe. Run the standalone procedure in Section 45 with a matching release baseline. Acceptance: Cargo reports native dependency fresh when first compiling the bridge; bridge changes do not compile live source; native source edit changes returned value and marks the relevant dependency dirty. Record raw JSON and toolchain. A failure must be reduced before native-loader implementation.

**H005 — Configuration/parser.** Files: native/schema.rs, project/native_config.rs, compat/native-schema.json. Parse the closed TOML grammar, canonicalize paths/type ASTs, reject source injection and unsupported codecs, deterministically sort/assign bindings, and hash schema. Acceptance: golden canonical outputs, duplicate path/type failures, malformed types, explicit generics, and arbitrary code strings rejected.

**H006 — Native/live partition.** Files: project/native_units.rs, project/cargo.rs. Resolve package/target aliases with Cargo metadata, form runtime/build closures, distinguish lib-tests from integration-test units, and reject native dependence on live units. Acceptance: HN010–HN012; preparation logs show no live-unit build command.

**H007 — Workspace transaction.** Files: project/native_workspace.rs. Implement preview/apply backups, marker ownership, member/default-member preservation, package-specific profile overrides, compare-and-swap writes, and safe rollback. Acceptance: no writes in preview; repeated init is byte-identical; user comments preserved where toml_edit supports them; HN007/HN009 workspace cases and concurrent-edit fixtures pass.

**H008 — Compiler/configuration provenance.** Files: project/native_toolchain.rs, project/native_build.rs. Record project toolchain, flags, target mode, manifest/lock/config inputs, and build authority. Reject unsupported compiler/target/linkage cases. Acceptance: compiler older than 1.85 rejected for generated source; host vs explicit-target invocations preserved; no wrapper inserted merely for telemetry.

**H009 — Cargo message collector.** Files: project/native_build.rs, project/native_messages.rs. Build only the generated bridge package, consume bounded JSON lines, retain non-JSON output as diagnostics, find exact cdylib artifact, and track fresh/rebuilt units. Acceptance: HN001/HN002/HN008/HN009/HN024; cached build-script output is not falsely counted as a script execution.

**H010 — Native input inventory.** Files: project/native_inputs.rs, project/dep_info.rs. Implement dep-info lexer/rules, source-directory membership, conservative fallback inventory, external file/env entries, generated OUT_DIR capture, and pre/post build source-race checks. Acceptance: escaped-path unit tests, HN029/HN030, native-source overlay change, and integration-test-only change that does not invalidate normal library when inputs permit.

**H011 — Generation publication.** Files: native/generation.rs, native/probe.rs, project/native_prepare.rs. Stage/copy/hash artifact; inspect ELF safely with readelf; record dynamic dependencies; authorize and run loader probe in its own process; atomically publish immutable library and manifest. Acceptance: HN025/HN027/HN031/HN035, no coordinator dlopen, no publication on source/probe failure.

Gate `xtask native-prepare-gate` requires H004–H011, the original bootstrap gate, and HN001/HN002/HN007–HN012/HN024/HN025/HN027/HN029–HN031/HN035 preparation assertions. No interpreter changes can bypass this gate by loading an arbitrary test `.so` path.

### 44.4 ABI and boundary gate

**H012 — ABI table and safe wire codec.** Files: native/abi.rs, native/codec.rs, templates/native-bridge/ffi.rs, native_abi_v1.h. Implement exact size/offset assertions, schema-driven checked decoder/encoder, replies, limits, and immutable table handshake. Acceptance: C/Rust layout comparison, golden byte strings, malformed/oversized-input property tests, and no foreign pointer accepted as a guest value.

**H013 — Typed bridge generator.** Files: native/generate.rs, templates/native-bridge. Emit one safe typed call arm per binding, compile-time concrete return annotations, manifest-source mappings, and schema/type ordinals. Acceptance: scalar/generic/associated/native-trait wrappers compile; private/unsafe/wrong-signature wrappers fail prepare with the right origin; generated call expressions contain no unchecked transmute.

**H014 — Native object store and reply lease.** Files: templates/native-bridge/store.rs, native/object.rs. Implement epoch/index slots, checked borrowing, non-Copy worker handles, take/drop semantics, pending-result adoption, and bridge-owned buffer release. Acceptance: HN013/HN014; fuzzed stale/wrong-type IDs do not enter binding; allocator/free ownership tracked by fixture counters.

**H015 — Call-scoped borrows and writeback.** Files: native/borrow.rs, native/call.rs, templates/native-bridge/staging.rs. Build alias groups, preserve overlapping shared byte views, reject mutable conflicts, reserve/copy/write back exact-length data, and lock guest locations for the synchronous call. Acceptance: HN015/HN016; staged UTF-8 boundary/range tests; native mutation visible after return.

**H016 — Entry transaction and panic.** Files: native/call.rs, native/panic.rs, templates/native-bridge/entry.rs. Implement Section 40's forward-only state machine, caught-unwind replies, postcall failure classification, and at-most-once sequence counters. Acceptance: HN017/HN019; panic=abort handled by supervisor fixture; no worker catch_unwind around foreign calls.

**H017 — Cleanup integration.** Files: vm/cleanup.rs, runtime/value.rs, runtime/shape.rs, native/object.rs. Add kind14 descriptors, explicit native drop actions, native non-Copy/opaque behavior, per-test empty-context assertion, and fatal no-user-Drop teardown. Acceptance: HN014/HN020, no native value enters a managed struct-layout operation, and mem::forget policy is defined as Section 44.7 prescribes.

Gate `xtask native-abi-gate` requires H012–H017 and HN013–HN020 against the controlled native workspace; use a narrow ABI driver before integrating the full JIT. A mock FFI function alone cannot satisfy it.

### 44.5 Execution, incrementality, and release gate

**H018 — Native namespace/query records.** Files: db/key.rs, db/provider.rs, frontend/names.rs, frontend/dispatch.rs, native/schema.rs. Implement the four pure native queries and complete explicit native namespace. Acceptance: HN006/HN028; no native call or loader runs in a query provider; native source bodies absent from live parse telemetry.

**H019 — Shared dispatcher and JIT barrier.** Files: vm/call.rs, engine/dispatch.rs, jit/record.rs, jit/lower.rs. Add NativeBinding call target, common call preparation, stop regions before native entry, and canonical continuation after return. Acceptance: HN003/HN004/HN033; a native-call event occurs once under all tiers; interpreter and JIT identical ownership/writeback behavior.

**H020 — Fresh hybrid worker and supervision.** Files: service/coordinator.rs, engine/worker.rs, native/loader.rs. Spawn one process per hybrid run request, one native generation per process, one object context per test; retain no loaded library in coordinator; implement process-group deadline handling and no-dlclose lifetime. Acceptance: HN018/HN021/HN025/HN036; repeat request resets a native static; tests in one batch demonstrate declared shared globals.

**H021 — Dedicated output/control channels.** Files: service/wire.rs, service/coordinator.rs, cli/render.rs. Separate native stdout/stderr from framed internal socket; bounded draining/capture, origin tagging, and missing-result infrastructure errors. Acceptance: HN026 and native exit(0) fixture; parser never treats native JSON-like logs as protocol messages.

**H022 — Portable checkpoint encoding and import.** Files: cache/portable.rs, cache/remap.rs, db/memo.rs, source/snapshot.rs. Implement whitelist, stable symbolic records, version checks, deterministic identity remapping, import with unverified revision, and current-input dependency validation. Acceptance: HN003/HN027/HN028/HN034; corrupt checkpoint cold-falls back; stale negative lookup and body indexes never execute.

**H023 — Checkpoint transport and bounds.** Files: service/wire.rs, service/coordinator.rs, cache/portable.rs. Export after completed request, validate framed/digested payload, cap at 64 MiB, retain only last good checkpoint, and drop executable/runtime state. Acceptance: fresh hybrid worker imports source work but reports zero restored JIT pointers; crashed worker cannot poison the prior checkpoint; quota failure changes performance only.

**H024 — CLI/service authority and freshness.** Files: cli/args.rs, service/requests.rs, native/generation.rs. Add explicit mode/authorization methods, mandatory pre-run native freshness checks, watch stale-stop, and mutually exclusive hardened sandbox request. Acceptance: HN003/HN005/HN022/HN023/HN024; a service native miss cannot trigger prepare; generation changes require newly scoped authorization.

**H025 — Telemetry and diagnostics.** Files: engine/telemetry.rs, cli/render.rs, compat/diagnostics.toml. Separate Cargo/native/JIT/query cache counters, settings/freshness/security/reset labels, effect-state reports, and actual invocation accounting. Acceptance: result schema fixtures, HN007/HN019/HN032, and test harness fails if any counter is fabricated or double-counted.

**H026 — Full hybrid conformance.** Files: xtask/hybrid.rs, fixtures/hybrid, fixtures/edition2024. Run every HN fixture in every applicable live tier; compile native oracle only for intended valid-profile comparisons. Acceptance: HN001–HN036 and E2412 pass; skipped required fixture is failure; controlled native crashes do not stop the whole report.

**H027 — Performance report.** Files: xtask/bench_hybrid.rs, artifacts/hybrid-performance.json, docs/performance.md. Execute Section43.4 scenarios, include unfavorable crossings and reset overhead, and identify measured misses. Acceptance: raw sample CSV/JSON, generated summary, reproducible commands, no “instant” claim unsupported by data.

**H028 — Release audit and handoff.** Files: xtask/release.rs, docs/security.md, docs/hybrid-native.md, AGENTS.md. Run original release gate plus edition/prepare/ABI/hybrid gates, audit unsafe/import boundaries, verify no Cargo run path, and publish supported codecs/partition/profile limitations. Acceptance: all required behavior proven; security label none_native tested; no remaining unconditional ban or silent native fallback contradicts this revision.

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

Native constants and statics are not directly imported as addresses. Expose a getter/setter binding explicitly. Native public enum fields or struct construction require explicit constructor/accessor bindings or owned data conversion; reflection through layout is excluded. Native function pointers are not guest function values; bind a named safe wrapper instead.

A live function returning or accepting a native type may mention its opaque schema in its signature. That is a concrete call-boundary contract, not permission to materialize its fields. A live nominal type cannot be declared Copy merely because its only field is a native opaque handle; the existing Copy recipe rejects that reached copy.

Native `Result::Err` values that a test wants to print must use a supported owned Debug representation, such as String, or an explicitly bound conversion. Do not call native formatting automatically from diagnostics. Rust-standard-library APIs on the native side are compiled ordinary code; their presence does not expand the live std facade.

<a id="section-45"></a>

## 45. Standalone Cargo reuse probe and first implementation experiment

The accompanying `lrr_cargo_reuse_probe` source bundle contains `probe.py` and README.md. It is a small executable experiment for the artifact-reuse premise, not an implementation of LRR. It uses only the Rust standard library and Python's standard library, and creates all Cargo packages in a new or empty directory. The source was syntax-checked as Python while preparing this document; **Cargo/rustc were unavailable, so the Rust builds, cache-hit expectations, and dynamic-library calls have not been run here**.

Execute on the supported Linux x86-64 development machine:

```sh
python3 probe.py --allow-native-build --work-dir /tmp/lrr-reuse-check
```

The path must be new or empty. The explicit flag authorizes native build tools and generated library execution. The script retains the workspace and per-command stdout/stderr under its `evidence` directory. It fails rather than quietly discarding an unexpected artifact-reuse result.

The probe performs this exact sequence:

1. Create an edition-2024 workspace with a `native_math` library and a live package whose cold function is deliberately ill-typed. The live package must never be compiled.
2. Build `native_math` under the ordinary release profile and record the actual `.rlib` path, content hash, Cargo messages, and tool version. There is no pre-existing LRR sidecar or bridge.
3. Add a generated-style cdylib workspace member that calls `native_math::scale` through ordinary Rust and exports a scalar C entry. Add only its package-specific optimization/incremental override and update the lockfile offline.
4. Build only that bridge package. Require Cargo's native_math artifact to be fresh and byte-identical to the baseline while the new bridge is compiled.
5. Load the reported `.so` in a disposable Python child using ctypes and verify the native computation. This child is intentionally unsandboxed and handles no LRR protocol.
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

## Revision 3 completion checklist

A release conforming to this document has a Rust 2024 live frontend with the specified edition semantics; one lazy interpreter/observed-region Cranelift engine; an explicit managed mode and explicitly unsandboxed native mode; a Cargo-built, immutable, typed bridge reusing compatible optimized artifacts; no compiler/linker in a prepared edit/test run; call-scoped safety/ownership and no effect replay; truthful profile/freshness/state-reset labels; portable incremental source/query reuse across fresh hybrid workers; and passing original plus edition/hybrid gates. The accompanying probe is evidence-producing source only until its recorded execution succeeds.
