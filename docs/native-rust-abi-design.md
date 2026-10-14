# Direct Rust ABI calls for prepared Cargo dependencies

Supporting design rationale, October 13, 2026. Adopted by [LRR revision 5, Section 61](../lrr_rust2024_macros_traits_implementation_spec.md#section-61), with the optimizing extensions in Section 62 and task order in Section 57. The governing specification and ledger take precedence over this original proposal, including its narrower baseline tiering and implementation sequence. Adoption changes the specification, not runtime implementation or acceptance evidence. Interpreter-first priority remains in force.

The authorized encoding amendment is normative in specification Sections 25.6, 61.4.2 and 61.15: ordinary direct/resident bindings, explicit conversion-only codecs, internal v4 raw bulk frames with public JSON unchanged, measured scalar interpreter invokers, existing LLVM inlining, then profile-driven macro/metadata copy reduction. All new acceptance remains pending; the original proposal below is supporting rationale.

The design prepares concrete Rust specializations with Cargo and rustc, exports their compiler-described function-pointer ABIs, and binds reached JIT calls to those immutable entries. Native values retain native storage and ownership. Scalar calls can avoid serialization, allocation, a wrapper body, and a return to the dispatcher. Calls involving owned objects or recoverable panics retain explicit, measurable bookkeeping.

The performance objective is almost no **additional boundary cost relative to an ordinary out-of-line native call with the same semantics**. An optimized caller that inlines the whole generic implementation is a separate baseline. Managed execution still has fuel, canonical state, validity checks, and cancellation boundaries; eliminating a codec does not eliminate those costs.

The subsequent [throughput architecture proposal](native-throughput-design.md) extends this baseline after authorization to make broader changes. It proposes optimizing loops, state reconstruction, bounded specialization and a later LLVM tier for prepared-IR inlining. Its changed tiering/backend choices are explicit extensions, not claims about the baseline or current implementation.

## Decisions and scope

| Area | Decision |
| --- | --- |
| Native compilation | Explicit preparation only; reuse Cargo-selected compatible artifacts and instantiate missing concrete generics in the generated preparation crate |
| Native entry | An address taken as an explicitly typed, monomorphic Rust function pointer; describe that pointer-call ABI |
| Discovery | Explicit native binding manifest; no arbitrary symbol lookup or discovery by demangled name |
| Compiler coupling | Exact producer compiler, target, configuration, exporter schema, and tested ABI lowering compatibility |
| Live frontend | Existing LRR frontend and queries; the compiler adapter runs only during native preparation |
| JIT backend | Pinned Cranelift 0.136.2; no backend substitution |
| JIT policy | Existing observed regions, thresholds, one artifact per recipe, and 64-op bound; add a native-call operation inside an observed region |
| Generic types | Concrete native types and primitive types; type and const arguments are fixed per prepared entry |
| Native objects | Opaque native-resident values with checked ownership; no general guest field access or construction from matching bytes |
| Raw direct calls | Generations compiled with `panic=abort`, within the admitted ABI subset |
| Recoverable panics | A prepared Rust-ABI catch adapter; no unwind through LRR JIT or host frames |
| Interpreter | A generated C-ABI slot invoker for each binding; identical ownership and panic contracts |
| Loading | One immutable generation per worker; no unloading, hot patching, cross-generation objects, or persisted pointers |
| Initial platform | Rust 1.99.0 on `aarch64-apple-darwin`; each later platform needs its own acceptance |

Three different promises must remain visible:

1. A concrete specialization such as `dep::compute::<u64>` can have an ordinary native entry cost.
2. A specialization involving a prepared native `dep::Record` can use its real representation without serializing it, but ownership and borrows still need enforcement.
3. `dep::compute::<NewLiveRecord>` needs machine code and native trait implementations for that particular type. ABI metadata alone cannot supply them. This proposal does not compile live units or equate live and native types.

## Preparation and concrete specialization

The existing `.lrr/native.toml` remains the authority for bindings. Add a versioned `rust-abi-v1` binding transport. Existing codec bindings remain separately identifiable; selecting a direct binding never silently falls back to serialization.

Each direct declaration contains a stable binding name, dependency unit, fully qualified function or method path, concrete type and const arguments, argument/result native types, transfer modes, and requested panic behavior. The restricted manifest grammar accepts the existing canonical type/path forms; it is not arbitrary Rust pasted into generated source. Rustc checks accessibility, generic arity, all required bounds, associated-type normalization, and signature agreement.

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

### Why metadata must describe a function pointer

Internal optimized symbols can acquire compiler-specific details. A source `fn(A) -> R` signature is insufficient to call such a symbol. Address-taking produces a callable Rust function-pointer target, including rustc's necessary reification shim. Export the ABI for that exact normalized pointer type and require a differential test against a Rust caller using the very same pointer.

`#[track_caller]` needs particular care: pointer coercion can introduce a shim and change the visible caller location. The initial schema rejects bindings whose selected entry needs caller-location behavior; do not claim the JIT source location survives automatically. Native calls deeper inside an admitted function retain their ordinary compiled behavior. Supporting caller propagation later requires a separate explicit location contract and tests.

## Artifact schema and identity

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

`GenerationId` binds the linked bytes and the entire producer/schema/input manifest, excluding the `GenerationId` field itself. Source signature caches may survive a body-only native change, but installed calls, native object identities, and JIT artifacts always belong to one exact generation. A worker holding generation A can never install generation B.

## Registration and loading

Keep a small C-ABI control surface for library registration and native storage management. Rust ABI calls are the execution surface. Registration returns bounded tables of entry IDs, process-local callable addresses, type operation addresses, and the embedded binding-request digest. Raw pointers in this table are trusted native capabilities admitted only by the loader; no guest operation can supply or construct them.

The loader validates the immutable generation manifest before opening the library, then validates the table count, IDs, digest, null entries, target compatibility, and correspondence to the admitted schemas. Loading itself may execute native initializers and therefore still requires native authority. Hashes establish artifact matching, not safety of arbitrary native code.

The runtime converts a registered callable address only under its matched signature. Function/data pointer representation assumptions are part of the macOS ARM64 acceptance; pointer bytes are never sent over the service protocol. Do not infer validity just from a plausible executable address or require that every target lies inside the bridge image: a legitimate retained pointer can refer to a linked dependency image.

JIT imports use an internal typed `NativeEntryId` registry tied to the generation. No source spelling becomes a symbol lookup string. Continue disabling all ambient symbol fallback. Where Cranelift's relocation needs a range-extension stub or an indirect call, record it; a direct source binding does not guarantee a single hardware direct-branch instruction.

### Control interface responsibilities

Use one versioned registration symbol, `lrr_native_rust_abi_v1`, with a C-compatible size/version-checked output table. This name and the new transport distinguish it from revision 4's codec interface. Stable C fields contain fixed-width integers, bounded byte pointers, opaque context pointers, and C function pointers. The Rust callable address is an opaque pointer in an entry record, never a C function pointer with a guessed signature.

| Control operation | Contract |
| --- | --- |
| Register | Return immutable entry/type-operation tables and binding-request digest; no guest values or effects |
| Create context | Establish run epoch, empty native slot bookkeeping, and one reusable panic state for synchronous calls |
| Reserve storage | Allocate uninitialized storage for one registered native type with checked size/alignment; return no initialized value |
| Interpreter invoker | Per-entry typed C function accepting trusted argument/output slots; construct typed arguments under the declared transfer contract and call the admitted raw or catch entry |
| Inspect caught state | Return a bounded C-compatible panic/status view; do not expose Rust `String`, enum or panic payload layout |
| Reset caught state | Destroy native payload resources under the contained-panic policy before reuse |
| Drop value | Consume an initialized registered type exactly once using native Drop; report a contained panic where the generation permits it |
| Deallocate storage | Free already consumed or uninitialized storage without invoking user Drop |
| Destroy context | Require no outstanding initialized values, leases, or pending panic resources; never substitute for ordered guest cleanup |

Argument slots for the interpreter contain pointers to typed native storage or validated scalar scratch and the declared transfer mode. They are constructed by the trusted runtime, not read from guest bytes. Each invoker has its concrete argument count and types compiled into its source. It uses ordinary typed Rust calls; there is no general runtime implementation of the Rust ABI in the C shell.

The reusable panic state is valid because calls are synchronous and native-to-guest callbacks are forbidden. State inspection/reset must complete before the next entered call. Any new reentrant execution design must replace this single-state contract. Abort scalar calls need no per-call native context operation or result allocation.

## Admitted ABI subset

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

## Native storage and ownership

`NativeResident` is a new managed descriptor for an opaque, fully initialized value stored using the generation's layout and allocator. Its managed representation is an epoch-checked ownership handle. It is distinct from a managed struct and from the host runtime's own `String` or `Vec`.

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

### References and byte buffers

Before native entry, acquire leases in deterministic backing-object order. Shared native references may overlap. A mutable reference excludes overlapping shared or mutable access; disjoint byte ranges may receive separate mutable leases. Opaque native objects use whole-object leases in version 1, so field-disjoint borrowing is not guessed from layout metadata.

For managed byte buffers, pin contiguous initialized backing storage for the call. Check bounds, mutability, overflow, and UTF-8 for `&str`; no resizing, relocation, concurrent guest access, or guest callback is allowed while leased. Generate real references only after the leases exist. Native writes are already in the backing storage on return or caught panic, so no copyback is required. Invalid or unpinnable storage is a pre-entry diagnostic for a direct binding; any copying adapter is a separately declared binding.

Zero length does not remove Rust's pointer validity and alignment requirements. `&mut [u8]` may modify bytes without violating byte validity. General mutable typed guest slices remain unsupported because native code may replace elements whose validity, drop, and aliasing contracts LRR does not own.

References to native values are call-scoped and non-escaping. Require native resident types to satisfy `'static` as owned types; this does not make a temporary borrow of such a value `'static`. Unsafe native implementations remain trusted. No lifetime erasure, metadata hash, or alias guard proves arbitrary unsafe dependency code sound.

## Panic policy and callable entries

### Abort generations

A generation built under a verified `panic=abort` profile uses the typed pointer to the specialization directly. The dependency closure must satisfy the selected compiler's linking/panic-strategy requirements. Rust panics terminate the worker and report native process failure; they cannot pass a guest expected-panic test.

The admitted bindings cannot introduce foreign unwinding through JIT frames. As with other native unsafety, unsafe native code violating that contract is outside managed guarantees. An ordinary function signature is not evidence that a function never panics, and an `unwind` generation cannot opt into this raw path merely because its author expects success.

### Unwind generations

Generate a monomorphic **Rust-ABI** adapter around each selected call. Its schematic signature is:

```rust
// Schema notation, not a source implementation.
fn caught<A, R>(state: *mut NativeCallState, out: *mut MaybeUninit<R>, arg: A) -> u32
```

The actual adapter has concrete arguments, and its own function-pointer ABI is exported. State and output storage are allocated/reserved before entry. The adapter catches panics from the target and owned argument cleanup inside the native generation, writes `R` only on success, and returns `0 = success`, `1 = caught panic`, or `2 = fatal native post-call failure`. All unexpected statuses terminate the worker. Scalar results also use reserved output storage in this first catch form; its cost is measured separately.

The panic state carries a bounded string/static-string description or a fixed opaque-payload marker. Payload destruction and adapter cleanup must themselves be contained or abort the process; no Rust unwind may escape the adapter. `AssertUnwindSafe` applies only to this audited state machine and does not imply application-level rollback. Mutated borrowed objects remain mutated. A result is never adopted on panic, and consumed arguments are never restored.

This adapter may inline the target during preparation. It still has status/output handling and exception machinery, so it is not labeled the raw direct path. Unwind tables and cleanups for arbitrary LRR JIT frames are deliberately unnecessary for this design.

## Call execution and exactly once semantics

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

## JIT integration and the scalar fast path

Add `NativeCallOp` to LRIR with a pure `PreparedNativeCall` identity. The recorder may include a successfully completed synchronous native call and its subsequent same-body operations within the existing 64-op region. Native calls count as one guest operation. Guest calls, control-flow exits, cleanup, and the other existing boundaries remain boundaries. No native implementation body enters LRIR.

Promotion still requires real completed observations. Recording a successful call does not prove subsequent calls cannot panic or mutate state; generated code includes all defined return paths. No speculative calls, replay-based warmup, native callback to the guest, whole-function compilation, or extra tier is introduced.

Two lowering forms share the state machine:

| Form | Entry and completion work | Intended use |
| --- | --- | --- |
| Checked direct call | Shared preflight helper acquires tickets/leases and reserves storage; JIT emits matched Rust ABI call; shared completion helper adopts result and releases leases | Owned objects, references, aggregates, catch adapters, unresolved scalar materialization |
| Scalar direct call | JIT emits bounded inline preflight and commit against a verified frame view, with no per-call helper | Initialized primitive scalar inputs/results in abort generations |

The checked form removes codec and dispatcher costs but retains helper calls. It must not be advertised as equivalent to a bare scalar call.

### Verified frame view

The scalar form requires an explicit extension to the current helper-only memory model. At region entry, a versioned helper validates the frame, semantic epoch, region recipe, generation, and expected slot indexes, then fills a trusted `NativeRegionView`. It supplies pointers only to the region's admitted canonical scalar cells and bounded execution-state fields. It never exposes an arena base for unchecked indexing.

The view schema includes a version/digest, scalar slot count and stable addresses, expected descriptors, fuel/cancellation access, canonical PC and pending-native-call fields, and a native-entry counter. Host/JIT sizes, alignments and offsets are mechanically checked. Cancellation uses the host's atomic representation and required ordering; a plain load from an atomic field is not an acceptable substitute.

Introduce `native_region_enter_v2(ctx, frame, region_id, view_out) -> status`, `native_call_prepare_v2(ctx, frame, op_id, scratch_out) -> status`, and `native_call_finish_v2(ctx, frame, op_id, outcome) -> status` in the closed helper registry. Their pointer parameters refer only to compiler-sized/aligned trusted stack storage. The first is the scalar-form entry contract; the latter two implement the checked form. `prepare` returns Continue only after publishing the canonical entered state and furnishing all machine inputs; generated code then invokes the one admitted target immediately. A non-Continue return never leaves an entered-but-uninvoked call resumable. `finish` accepts only the pending call's outcome and can never request retry.

The exact fixed-width field layouts of the view and scratch are versioned host/JIT schemas with golden offset tests, separate from the producer Rust ABI schema. They are not raw views of Rust's `VmContext` or arbitrary frame structs. Native dependency entries never receive these helper structures.

Frame scalar cells must have fixed, pinned backing storage while this view exists. Suspension, reallocation, frame replacement, or any helper that cannot preserve the view ends the region. Host Rust code must hold no overlapping active references while JIT stores through the view. The synchronous native dependency has no route to the VM; callbacks and concurrent guest access remain forbidden. The view is invalid on every region exit and is never cached across invocations.

For each scalar call, emit this sequence:

1. Guard the concrete binding/substitution and initialized scalar descriptors unless immutable region facts already prove them. Literal materialization and shape misses leave the opcode unentered for the checked path.
2. Read scalar bits from admitted canonical cells or reuse SSA values known equal to their current contents. Validate bool/char ranges when not already established. Apply producer-required extension/truncation rules.
3. Check cancellation and available fuel at the prescribed pre-entry point. Charge once; store the canonical continuation, entry state and native-entry counter. These stores cannot move after the call.
4. Call the fixed generation entry using its lowered ABI. Treat the call as effectful and as a memory clobber for native-visible state; never mark it pure based on its scalar signature.
5. Write the scalar result and descriptor into the canonical destination, clear pending entry state, and advance the PC. Subsequent operations can reuse the result in SSA while canonical state stays current.

If a destination requires dropping an old owning value, or any required fact cannot be established, use the checked form. Do not make guarded native calls eliminate source-level ownership or effect checks. All existing resource charges must match the interpreter; no batching of fuel across an observable call.

Guard hoisting is limited to facts whose dependencies remain immutable for that region. The design does not hoist a mutable borrow across another call, infer purity, or eliminate alias checks from type names. Native object fast paths beyond the checked form need a later accepted ownership proof and their own performance evidence.

This form removes per-call dispatcher and helper transitions for eligible scalar calls. It still performs required guards and canonical bookkeeping. If those dominate tiny functions, the end-to-end almost-zero-overhead objective is unmet even when the raw ABI probe passes. Removing them would require a further semantic/optimization proposal; this document does not hide that tradeoff.

## Generic behavior and user-visible limits

| Requested use | Behavior |
| --- | --- |
| `dep::f::<u64>` already declared | Resolve the prepared entry; hot calls carry no generic dictionary |
| New concrete native type/const combination | Produce an exact missing-specialization preparation request; no runtime compiler process |
| `dep::Container<NativeRecord>` | Native resident object with explicit constructors/methods/drop and accepted ABI cases |
| Associated types or native trait methods | Rustc resolves them during preparation; schema records concrete normalized signature and exact trait selection |
| Generic live caller reaches different concrete arguments | Select the matching prepared binding through checked dispatch; the one compiled region guards its first admitted shape and falls back on mismatch without compiling a second artifact |
| Native algorithm wants a trait implementation defined only on a live type | `LRR-E-NATIVE-WORLD-MISMATCH` before entry |
| Native algorithm calls a live closure/comparator | Unsupported callback boundary; a native wrapper containing its own native closure may be bound |
| Live edits change only argument values or caller body | Reuse the immutable generation if its prepared inputs remain valid |
| Live edits introduce a new native specialization | Preserve prior effects and stop with the preparation request; no automatic prepare and replay |

There is no finite ahead-of-time artifact containing every possible instantiation of an arbitrary Rust generic. This design buys low repeated-call cost by paying concrete instantiation cost during explicit preparation. It does not turn a native generic declaration into a universal runtime implementation.

## Diagnostics and telemetry

Retain existing missing-binding, world-mismatch, stale-generation and native-process-failure categories. Add typed outcomes for producer mismatch, unsupported ABI, unsupported signature, invalid native value, native borrow conflict, and unsupported panic entry. Distinguish exporter/schema incompatibility from a user type error and from a backend lowering bug.

Every reached failure includes binding identity, concrete type/const arguments, producer/generation identity, phase, whether entry occurred, and the exact supported preparation remedy. Do not print memory addresses or arbitrary native payloads as diagnostics.

Report counters separately: prepared specializations; reused dependency artifacts; raw Rust-ABI calls; caught-adapter calls; interpreter invoker calls; checked/scalar direct JIT calls; pre-entry guard misses; post-call exits; native-object transfers; borrow-lease acquisitions; explicit conversion bytes; and actual codec calls. A direct-call counter requires an actual completed machine-code path, not just CLIF generation. Entry counters separately record calls that fail to return.

## Original specification amendment rationale

This was an authorized design change from revision 4, now adopted in revision 5. It is not a discovered contradiction or an implementation erratum against the old codec design. The table records the original amendment rationale; Section 62 further extends tiering and code reuse.

| Existing rule | Required amendment |
| --- | --- |
| D11 and Sections 19/21 prohibit native guest-object addresses in JIT | Admit verified scalar frame views and leased native storage only under the direct-call contract; keep ordinary managed objects checked |
| D22/D23 and Sections 20/41.2 terminate or dispatch calls | Permit observed synchronous native-call ops inside a region; retain guest-call dispatch and fixed tiering |
| Section 19 fixes five helpers and scratch-only loads | Version the helper ABI, add region-view/preflight/completion contracts, and permit the exact audited loads/stores described above |
| Section 21 closes imports to five helpers | Extend the registry with immutable generation-bound entry IDs; retain zero ambient lookup |
| Sections 35/36/39 fix codec-only C execution | Add Rust-ABI binding transport, native resident types, a compiler exporter, and C control/interpreter surfaces |
| Sections 37/38 preparation and freshness | Include exporter binary, compiler ABI schema, retained callable table and probe receipt in atomic generation publication |
| Section 40 staged buffers and caught panic arms | Add lease-based direct memory, native storage transfers, abort direct entries, and Rust-ABI catch adapters |
| Sections 41/42 cache and service contracts | Add pure ABI descriptors and worker-local installed entries; preserve no serialized pointers and explicit authority |
| Section 43 performance evidence | Add ordinary-call and inlined baselines, assembly evidence, conversion accounting and full guest measurements |
| Sections 44/57 tasks | Extend affected tasks with direct-ABI acceptance and integration dependencies while retaining the cross-ledger sequence and interpreter-first priority |
| Section 55 native/live world separation | Retain nominal separation and no compilation of selected live runtime units |

Runtime unsafe remains confined to `jit/abi.rs`, `jit/helpers.rs`, `jit/module_cache.rs`, `native/loader.rs`, and `native/ffi.rs`. New pure schema and lowering modules contain no Rust `unsafe` blocks. Generated native shells are separately audited preparation products. Cranelift emission still belongs to the trusted compiler surface even when its Rust implementation uses safe APIs.

## Implementation sequence and acceptance

The rows below are original work packages, not completed T/H/M cards. Their adopted assignments and integration requirements are in revision 5 Sections 44/57 and docs/tasks.json. A standalone ABI experiment cannot mark guest JIT, native runtime, or platform support complete.

| Package | Deliverable and invariant | Required acceptance before integration |
| --- | --- | --- |
| D3A producer contract | Exact-version preparation adapter and bounded canonical schema | Same-invocation sidecar/receipt matches the embedded request digest and final library hash; deliberate compiler/target/schema mismatch rejects; no runtime builds |
| D3B scalar ABI experiment | Prepared generic scalar pointer and pinned-Cranelift caller on macOS | Same-pointer Rust/JIT differential tests, signed widths, bool/char, floats including NaNs and signed zero; retained machine-code and compiler identity evidence |
| D3C ABI coverage | Explicit Pair/Cast/Indirect/Ignore lowering | ZST with Drop, tuple/enum niches, mixed int/float aggregates, hidden returns, stack spills/alignment and unsupported cases; each supported class actually runs |
| D3D storage | Native resident slots, allocation/drop and transfer state | Constructor/move/replace/drop exactly once; stale/wrong-type/wrong-generation rejection; non-Copy duplicate use fails before entry |
| D3E borrows | Native-object and zero-copy byte leases | Overlap rejection, shared overlap, disjoint mutable slices, zero length, bounds/UTF-8, no copyback, panic mutations visible, no relocation or escape |
| D3F panic and interpreter | Catch adapters and typed C invokers | Native panic cleanup, argument/result Drop counts, exotic payload failure, abort process attribution, equal interpreter behavior |
| D3G checked JIT call | Native call inside observed regions | Tier equivalence, exact effects/fuel, shape miss before entry, failure after entry never replayed, cancellation at the correct boundary |
| D3H scalar JIT call | Verified frame view and inline call bookkeeping | Pinned storage/alias audit, canonical-state checks at every exit, descriptor misses, atomic cancellation checks, source invalidation, no per-call helper in accepted assembly |
| D3I release evidence | Reproducible artifacts, diagnostics, benchmarks | All required controls and full guest tests run; macOS evidence separate from unexecuted Linux cases; implementation ledger updated honestly |

For ABI and language cases, select unchanged official fixtures from `rustc/rust/tests` incrementally, with auxiliary crates, revisions, directives, expected files, commit and hashes. Keep custom pointer-call differential harnesses, ABI mutations, and effect counters outside the copied upstream directory. A successful custom harness is not a full stock rustc fixture pass.

The negative matrix must include malformed schema lengths/tags, truncation, stale sidecar, altered library hash, unsupported target feature, swapped entry ID, wrong signature, invalid bool/char, generation mismatch, double move/drop, lease conflicts, missing specialization, non-native type, pending macro authority, and an attempted implicit compiler invocation. Tests intentionally involving process aborts run in sacrificial workers and assert the actual process result. Nothing catches arbitrary failure as success.

## Performance experiments and decision gates

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

The proposed raw-ABI acceptance target is at most 5% median overhead against the matched Rust pointer-call loop, with the upper 95% confidence bound also at most 5%. When measurement uncertainty prevents that conclusion, report inconclusive and improve the experiment; do not relabel it passed. This is a design target, not a prediction or a nanosecond guarantee.

Apply the same 5% target to a full LRR workload before describing that workload as almost overhead-free compared with native Rust. Report absolute ns/call and all failed cases, especially tiny calls. The raw ABI gate can pass while the integrated gate fails. If design 1 has equivalent results, retain it as an honest lower-maintenance comparator; if native ownership checks dominate, changing the calling convention again will not fix them.

## Feasibility gates and current evidence

The design choices above are fixed for this proposal; these are implementation feasibility questions, with fail-closed outcomes:

| Gate | Evidence required | If it fails |
| --- | --- | --- |
| Matching compiler adapter | Exact Rust 1.99.0 producer exports the pointer ABI and issues a sidecar receipt for the actual code-generation invocation | Preparation unavailable for that producer; no fallback compiler |
| Pinned backend compatibility | Cranelift 0.136.2 passes producer-matched ARM64 differential tests for every admitted ABI class | Restrict the explicit ABI subset or record a witnessed backend issue; never guess |
| Frame view soundness | Stable storage, no conflicting host references, correct atomics, canonical state, and invalidation tests | Keep checked direct form; do not claim scalar fast-path performance |
| Performance | Raw and integrated results against all named baselines | Report which objective failed; require a new explicit design decision before changing semantics |

On October 13, the installed compiler reports `rustc 1.99.0`, commit `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, LLVM 23.1.1, host `aarch64-apple-darwin`. The local reference checkout is at `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, and its `rustc_codegen_cranelift` manifest requests Cranelift 0.135.0; this project pins 0.136.2. These are not a matching producer/backend acceptance result.

The existing project JIT ABI file contains synthetic helper/scalar probes. The production native bridge and direct-call design are not established by those probes. This proposal has no ABI execution, safety, or performance acceptance receipts. Its review checks cover the written contracts and links only.

## Source references

- [Native integration](../lrr_rust2024_macros_traits_implementation_spec.md#section-35), [execution boundary](../lrr_rust2024_macros_traits_implementation_spec.md#section-40), [JIT integration](../lrr_rust2024_macros_traits_implementation_spec.md#section-41), and [native/live worlds](../lrr_rust2024_macros_traits_implementation_spec.md#section-55) establish the current contracts this proposal changes or retains.
- [Local rustc ABI structures](../rustc/rust/compiler/rustc_target/src/callconv/mod.rs), [ABI queries](../rustc/rust/compiler/rustc_ty_utils/src/abi.rs), [Cranelift signature lowering](../rustc/rust/compiler/rustc_codegen_cranelift/src/abi/mod.rs), and [pass-mode lowering](../rustc/rust/compiler/rustc_codegen_cranelift/src/abi/pass_mode.rs) are implementation references at the checkout commit above, not the ABI specification of the installed compiler.
- [Rust compiler monomorphization guide](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html) explains concrete instantiation and downstream generic code generation.
- [Rust ABI reference](https://doc.rust-lang.org/reference/items/external-blocks.html#abi) states that the Rust ABI has no stability guarantee.
- [Rust type layout](https://doc.rust-lang.org/reference/type-layout.html) and [validity rules](https://doc.rust-lang.org/reference/behavior-considered-undefined.html) establish why matching size/alignment alone does not admit a native value or reference.
- [Function unwinding rules](https://doc.rust-lang.org/reference/items/functions.html#unwinding) establish why recoverable native panics need an explicit contained boundary.
