# Optimizing execution beyond direct Rust ABI calls

October 13 tier-transfer amendment: the authoritative specification now defines decoded literal preparation (§11.3), live entry maps (§62.4.1), shape selection/cooldown (§62.5), logical quotas and reserved reconstruction (§62.6), native profiling/completion polling (§62.8), and bounded phase-change reconsideration (§62.11). These rules supersede the less explicit descriptions below. CA027–CA033 are pending acceptance; no implementation or performance pass is implied. See [the amendment record](errata/revision5-tier-transfer.md).

Supporting design rationale, October 13, 2026. Adopted by [LRR revision 5, Section 62](../lrr_rust2024_macros_traits_implementation_spec.md#section-62), extending [the direct Rust ABI design](native-rust-abi-design.md). The governing specification and ledger define the required contracts, fixed limits, task order and acceptance. Production implementation is unchanged by adoption; interpreter-first implementation priority remains in force.

The authorized encoding amendment is normative in specification Sections 25.6, 61.4.2 and 61.15: ordinary direct/resident bindings, explicit conversion-only codecs, internal v4 raw bulk frames with public JSON unchanged, measured scalar interpreter invokers, existing LLVM inlining, then profile-driven macro/metadata copy reduction. All new acceptance remains pending; the original proposal below is supporting rationale.

Direct ABI calls remove boundary conversion. High throughput additionally requires keeping loops in machine code, retaining typed values in registers, eliminating proven-redundant checks, and making small dependency bodies available to an optimizer. The target is a fast initial execution path plus an optimizing tier for sustained work. No measured speedup is claimed here.

## Priority and expected benefit

The ordering below is an engineering hypothesis to test with attributed profiles, not a benchmark result.

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

## Recommended execution tiers

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

Retain the interpreter as the semantic reference and fallback. Cranelift supplies quicker code generation; LLVM is justified only where its measured execution savings repay its extra cost. If the second native tier does not improve total workload time, keep it disabled. If startup latency becomes irrelevant, an interpreter-to-LLVM configuration is a valid later simplification; two native tiers are not intrinsically faster than one.

Revision 5 explicitly adopts this tiering/backend change from revision 4 and the narrower direct-ABI baseline. Implementation remains gated by interpreter-first priority and the Section 57 task order.

## Typed optimized IR before backend optimization

Create a common typed SSA representation between reached LRIR and the optimizing backends. Each value has a concrete representation class: primitive scalar, native resident object, checked managed handle, or unresolved dynamic value. Track initialization, ownership, effects, possible faults/panics, and native generation dependencies explicitly.

Native calls carry matched ABI/type identities and conservative effects. A scalar signature does not prove purity. Guest arithmetic retains its checked-overflow behavior even when the native dependency uses its own release arithmetic profile. Do not infer Rust alias guarantees for a managed reference merely because it is assigned a Rust-looking type.

Start with copy propagation, dead scalar computation elimination, common scalar expressions, constant propagation, redundant type-check elimination, and removal of provably unnecessary bounds checks. Operations that can fault, panic, allocate, drop, call, or perform an observable effect cannot be discarded or reordered without a proof under the selected execution contract. In particular, an unused checked arithmetic result may still panic.

Use Cranelift `opt_level="speed"` for this tier and keep verification enabled during acceptance. Its speed setting enables optimization; it does not establish correctness of LRR's input IR. Revalidate the exact pinned backend and register allocator settings instead of copying settings from a different release. [Cranelift settings documentation](https://docs.rs/cranelift-codegen/latest/cranelift_codegen/settings/struct.Flags.html#method.opt_level)

## Native loops and state reconstruction

Replace the 64-op straight-line restriction in the optimizing tier with bounded compilation units that can contain loop backedges and reached branches. A loop enters native code once and executes many iterations before returning. Bound compiler work and generated size independently; a source-op limit should not force one dispatcher round trip per iteration.

Initially compile observed control-flow paths and their loop edges. A branch to a path whose semantics have not been prepared exits to the interpreter at that branch. It does not trigger speculative evaluation, compile a cold body, or replay the loop from its beginning. Once more paths are reached, a later optimized version can include them.

Replace per-op canonical writes with state reconstruction maps at exits and safepoints. Each map records:

- Source PC and inline call stack.
- Live locals and their locations in registers, stack slots or canonical storage.
- Concrete descriptors and logical initialization/move state.
- Outstanding ownership, drop obligations and active leases.
- Fuel/accounting state under the selected execution contract.
- Whether a native call is not yet entered, in progress, or completed with a result.

Every potential guard failure, panic, resource fault, cancellation exit and interpreter fallback needs a valid map. Native calls with observable effects retain a committed entry record. After a call completes, any exit resumes after it with its actual result and ownership state. No reconstruction path can cause a second call.

Guard failures before an operation restore the state before it. A checked-arithmetic overflow exits with the correct source origin and executed-prefix state. Object destructors run in the same logical order as in the interpreter. Optimized-away physical objects require a specified reconstruction recipe when their identity or ownership becomes observable.

Start with scalar-only loops to establish these invariants. Add native owned values and guest-call inlining only after exit maps can account for their lifetimes. Inlining a reached guest call also needs source stack and cleanup reconstruction; it is more than concatenating instructions.

## Specialization and guard placement

A hot generic LRR unit can have several versions keyed by concrete runtime types, exact trait/member selection, native binding identities, generation, semantic mode, and source dependencies. Begin with a bounded small polymorphic cache, such as four variants per unit, plus an explicit global code-memory budget. An unrepresented combination executes through the checked fallback; it never reuses an incompatible specialization.

Guard at entry when a fact remains valid for the whole unit. Within a unit, typed SSA facts replace repeated descriptor checks. Stable callee identities permit direct calls or reached guest-body inlining. Guard failure must be uncommon for specialization to help; record misses and stop compiling versions that do not repay their code size and compile cost.

Source changes invalidate all compiled callers that incorporated the changed body, constants, method selection, or native IR. Installing a new version never retargets an already active native stack to incompatible code. Retain old code until active frames have returned or explicitly deoptimized. This is compilation-version lifetime management within one native generation, not hot swapping native dependencies.

## Ownership and data locality

Retain native containers across operations and pass their real storage to prepared methods. Avoid repeatedly materializing managed copies. A native constructor followed by many native operations and one final result conversion is the intended efficient pattern.

Where control-flow and ownership facts prove it safe, resolve a native handle and acquire its external lease once for an optimized interval. Keep the checked native pointer in a register. Release the lease on every normal and exceptional exit. The interval cannot cross a move, drop, conflicting alias, callback, suspension, or operation that invalidates the proof.

Do not cache a container's internal buffer pointer across operations that can reallocate it. A stable `Vec<T>` header does not imply a stable element buffer. Native methods may change interior state, and every call retains its declared memory effects. Opaque storage keeps Rust representation and invariants native; exposing an arbitrary live struct as native memory remains unsupported.

Scalar replacement and stack allocation can remove temporary allocations when neither identity nor address escapes and destruction semantics are preserved. Start with managed scalar aggregates and accepted native types with compiler-derived contracts. A native object's layout alone is not enough to prove that construction, allocation or Drop can be eliminated.

## Cross-boundary inlining with prepared LLVM IR

Calling a tiny dependency function through its native address can never expose its arithmetic to the caller's optimizer. To remove that call and optimize the combined loop, preparation must optionally retain compiler IR for selected concrete specializations in addition to their callable native addresses.

The native generation gains an IR bundle containing producer and LLVM identity, target/data layout/features, concrete entry identities, IR content hashes, allowed imports, and native state dependencies. Absence of such a bundle leaves a valid direct-call binding. Do not assume arbitrary existing Cargo artifacts contain usable bitcode: Cargo disables embedded bitcode where possible. [rustc bitcode options](https://doc.rust-lang.org/rustc/codegen-options/index.html#embed-bitcode)

Preparation obtains this IR from the actual compiler invocation for the concrete specialization. Generic declarations in metadata are not already-specialized LLVM bodies. Asking for an IR bundle can require additional preparation or rebuilding affected artifacts; retain actual Cargo reuse evidence.

The LLVM tier lowers the live hot unit to compatible LLVM IR, imports eligible prepared definitions, applies normal optimization/inlining passes, and emits machine code. LLVM ORC can provide the compilation and object-linking infrastructure; LRR still owns semantic lowering, optimization policy, safepoints and closed symbol resolution. ORC does not supply a Rust frontend or automatic correctness at this boundary. [LLVM ORC](https://llvm.org/docs/ORCv2.html#lljit-and-lllazyjit)

Initially import only abort-mode leaf functions with scalar arguments/results and no global, TLS, allocator, unwind, callback or address-identity dependencies. This allows a clean experiment such as inlining a concrete `mix::<u64>` into a live loop. Expand only through separately accepted IR classes.

For later general imports:

1. Reuse the existing generation's statics, TLS, allocator, vtables and address-taken callable identities. Never create a second copy of mutable crate state or run initializers again.
2. Give imported bodies optimization-only definitions where appropriate and map residual calls to explicit registered native capabilities. No default process-symbol lookup is allowed.
3. Preserve required COMDAT/linkage, target-feature and semantic metadata. Unsupported intrinsics or unregistered references reject the import and retain the direct call.
4. Use exact compatible producer/LLVM configurations and cache identities. A nominally parseable bitcode file is not proof that all calling conventions, attributes and runtimes match.
5. Preserve the native function's arithmetic/panic semantics inside an otherwise managed caller. Do not use native IR attributes to invent guarantees about invalid managed input.

LLVM's cross-language LTO facilities establish that compatible IR can be optimized across boundaries, but the import/state rules above are additional LRR engineering. Cranelift cannot consume LLVM bitcode as if it were CLIF. Building a generic LLVM-to-CLIF translator would introduce another large compiler project, so it is not the recommended path. [rustc linker-plugin LTO](https://doc.rust-lang.org/rustc/linker-plugin-lto.html)

## Execution contracts and runtime bookkeeping

Memory validity, exactly-once effects, native ownership and generation matching remain mandatory in every mode. Performance improvements eliminate checks only when facts prove them redundant.

Exact per-op accounting and immediate canonical state are separate choices. The optimizing tier can delay physical state writes while preserving exact behavior using reconstruction maps. Fuel for a fixed straight-line segment can also be checked once when enough remains, provided any earlier fault reports the exact consumed prefix and the low-fuel case executes the precise checked path. Never charge for operations that did not execute or move an external effect before its fuel check.

For maximum throughput, propose a separately named `throughput` execution contract with no exact instruction fuel and cooperative cancellation at native-call boundaries and bounded loop-backedge intervals. Retain a `precise` contract for current fuel/resource semantics and detailed tracing. The chosen contract belongs in results and cache keys. An optimization flag must not silently change the resource contract.

The throughput contract still has process deadlines, memory limits where already enforceable, and defined cancellation points. A backedge interval bounds loop iterations between polls, not wall time: one iteration or native call can take arbitrarily long. The coordinator must retain its worker termination policy. Native ambient allocations remain outside managed memory accounting, as already true in hybrid mode.

Batch profiling counters at exits or sample them after warmup. Detailed per-op tracing is a diagnostic configuration with its cost reported. Keep only the call-state records required for failure attribution and correctness on the critical path; do not confuse optional statistics with required resource behavior.

## Compilation latency and reuse

Evaluate promotion using measured compile cost and expected remaining work:

```text
expected_remaining_executions * expected_time_saved_per_execution
    > compile_cost + expected_deoptimization_cost
```

These are estimates; retain the cheap tier when confidence is weak. Optimization also has a CPU-contention and code-memory cost, even when compilation runs in the background. A fixed traversal count alone is a poor policy for both one-shot tests and long computations.

A compiler worker consumes an immutable, fully prepared typed compilation unit with no live query pointers or permission to execute source/macro code. Runtime execution continues in its current tier. Before publishing completed code, the owner checks source dependencies, native generation, semantic contract, and compiler/ABI fingerprints. Stale results are discarded. No background compiler mutates the active query database or patches active instructions.

Store relocatable compiled objects with bounded metadata, reconstruction maps, dependency keys and approved relocation identities. On a fresh worker, validate the cache record, map native entries for that worker's generation, apply only admitted relocations, and finalize executable memory through the audited platform path. Do not persist raw executable addresses or run state. Keep fresh hybrid workers; code reuse does not require sharing native globals between requests.

Cache compatibility includes source and inline dependency digests, type specialization, native generation/IR bundle, target CPU/features, backend version/settings, helper ABI, execution contract, and reconstruction schema. Reusing code for a different CPU feature set or semantic contract is a correctness failure. Untrusted or malformed cached code is never executable merely because it has a plausible filename or self-reported hash.

## Experiments and adoption order

Adopt changes in independently attributable stages, retaining the direct-ABI acceptance suite:

1. Measure direct ABI calls and the full scalar LRR call path against the same native pointer target.
2. Add typed IR optimization and a single native loop backedge. Measure dispatcher entries, state loads/stores, helpers, code size, and actual wall time.
3. Add state reconstruction for scalar loops. Deliberately exercise every exit, overflow, cancellation and fuel boundary; compare exact effects and results to the interpreter.
4. Add bounded type/callee specialization, then owned-object exit maps and lease amortization. Include polymorphic and allocation-heavy regressions.
5. Prepare a tiny scalar LLVM IR bundle and demonstrate caller/callee inlining in assembly. Compare against both a direct-call loop and optimized native Rust. A loop with an unremovable large native call is the negative control.
6. Add asynchronous optimization and relocatable caches. Measure first run, steady state, body-only edit, invalidating edit, new specialization preparation and process restart separately.
7. Expand admitted native IR classes only when mutable state identity, ownership and residual symbol resolution pass their controls.

Keep precise and throughput results separate. Compare equivalent arithmetic, panic, inputs and outputs; compare metering overhead explicitly when the native reference has no fuel instrumentation. Include compile/link/load time when reporting time to finish a test, and exclude it only in clearly labeled steady-state measurements.

The strongest supported claim would be workload-specific parity with optimized native Rust, including its inlining, for admitted programs. Neither a zero-codec ABI nor an optimizing backend alone supports that claim. The recommendation is to remove structural execution overhead first, then use compiler IR to remove the remaining optimization boundary where measurements show it matters.

Final contract amendment: Sections 1.2.1, 12.2, 62.6, 62.9.1 and 62.11 now require full upstream accounting, separate virtual/physical identities, adoption of materialized backing, all-job result draining and SchedulingPolicyV1 reporting. CA034–CA037 remain pending. See [the contract record](errata/revision5-final-contracts.md).
