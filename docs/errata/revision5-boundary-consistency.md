# E-R5-001 — Revision-5 boundary and integration consistency

Superseding user decision: [the runtime-semantics policy](revision5-runtime-acceptance.md) replaces this record's all-category pre-JIT prerequisite. Static type/borrow checking and compiler/tooling acceptance do not block guest JIT. The earlier scope discussion below is historical; the complete inventory remains required.

October 13, 2026. This corrects contradictions and missing integration rules left when the authorized direct Rust ABI and optimizing design was incorporated into revision 5. It changes specification and acceptance requirements, not runtime implementation. No T/H/M task or native fixture is completed by this review.

## Minimal witnesses and corrections

| Witness | Incompatible requirements before this correction | Resolution and regression |
|---|---|---|
| Producer | Section 3 permitted a different compiler “because only the defined C bridge ABI crosses the boundary”; Sections 37/61 require a matched Rust ABI producer. | Independent producer compatibility applies only to codec-only generations. Direct bindings require the accepted exact producer/exporter. `producer_contract`. |
| Calling convention | Section 21 used the default convention “to construct all signatures”; Section 61 requires producer-described Rust signatures. | Default C convention is for RegionFn/helpers. Native/catch entries use MachineCallAbi. `call_convention`. |
| Baseline versus optimizing | Section 21 required multiplication, floating point, casts and references to “use step in this release”; Section 62 requires typed optimized lowering. | Scope the whitelist and helper algorithm to the baseline. Optimizing lowering retains its own semantic acceptance. `scalar_scope`. |
| Canonical state | Section 19's step/stage/commit helpers read canonical slots; Section 62 permits registers to hold newer values. | Materialize every canonical input/continuation needed by a helper before calling it. A map alone does not update memory. Deopt resumes the reconstructed uncommitted operation, never a completed call. Required runtime coverage remains CA011/CA012/CA021. |
| Service version/fields | Section 42 said protocol version2 while its example and Sections 25/56 used 3; Section 25 named a run `mode` while examples prohibited that spelling. | Use version3 and the canonical `jit` field. Align apply_changes with `{path,text}` examples. `service_version`, `run_field`, and parsed example controls. |
| Native ownership | Section 39 stored codec objects in `RefCell<Option<NativeValue>>`; Section 61 introduced separate NativeResident storage. The Section 36 direct constructor followed by a codec method had no common handle/store contract. | One context, slot registry, type identity, ValueKind14 and ownership state across both transports. Codec object handles access resident storage without copying fields. NX015/H014 explicitly exercise direct-new/codec-method/direct-drop. `old_native_store`, `resident_identity` and required shared-storage clauses. |
| Borrowing/panic | An unqualified guest-reference prohibition obscured direct byte leases; baseline borrow-hoisting restrictions obscured accepted interval leases; panic wording could imply catch behavior for abort generations. | Explicit byte/string exception only; Section 62 proof required for interval leases; generation profile determines raw versus catch entries. No per-binding override weakens the unwind boundary. Runtime evidence remains NX017/NX019/NX020. |
| Generation identity | Section 61 hashed the “entire” manifest while Section 38 excluded derived paths and per-attempt observations. Repeated prepares could therefore get different identities. | Both use Section 38's GenerationIdentity. Split stable receipt provenance from temporary paths/timings. `generation_identity`; runtime reuse evidence remains HN002/NX011/NX012. |
| Compiler ownership | Section 25 said the compiler/VM/query owner is single-threaded while Section 62 requires background compilation with independent RSS/termination limits. | One supervised compiler subprocess consumes pointer-free jobs and emits relocatable outputs. Final address binding occurs in the execution worker. `compiler_owner`, required compiler-process clause; CA019/T088 cover termination and stale jobs. |
| Cache/tier telemetry | Historical result text made all JIT entries aliases of Cranelift entries; inlined native work could be counted as a physical call; a cache-restoration example lacked a dynamic meaning. | Preserve legacy Cranelift counter, count total/per-tier entries separately, distinguish logical/inlined native work from physical ABI calls, and report actual validated object installation. CA017/CA020 and required inlined-accounting clause. |
| Fuel | Baseline call recipes unconditionally demanded fuel although throughput explicitly has none. | Runtime fuel clauses apply to precise. Query/const/compiler budgets remain in both. Numeric fuel in a throughput request is rejected by the example regression and remains CA018 runtime acceptance. |
| Editions | T100 explicitly selected edition2024 and Section 18's conformance suite pinned only 2024, despite the all-editions policy. | Oracle invocations retain each root/dependency edition; temporary-scope cases run edition pairs. `oracle_editions`, `edition_conformance`. |
| Bounds outcome | T041 called out-of-bounds indexing a fault; Section 11 requires GuestPanic and ordinary cleanup. | Distinguish bounds GuestPanic from index-type contract fault in both card and ledger. `index_failure_class`. No runtime repair is claimed. |
| Initial LLVM scope | Section 62 required broader IR expansion as step 7 of mandatory experiments, despite limiting initial release to scalar leaves. | Six initial experiments; broader imports require a later separately specified contract. `mandatory_future_import`. |

Section 61.0 now inventories ten encoding/call boundaries and their distinct version domains. It explicitly identifies effect hex/JSON transport, macro token protocol, metadata and compiled-object serialization as remaining costs; none is silently removed by direct Rust ABI adoption. Backend-specific code disposal and forced-tier budget outcomes are also made explicit.

The subsequent user-authorized encoding amendment supersedes the retained-hex decision described above: Section 25.6 now requires internal v4 raw bulk frames, public JSON remains v3, and portable query payloads use lrr-portable-v2. Sections 61.4.2/61.15 add measured scalar interpreter invokers and copy-reduction priorities. This amendment is a design change; the original contradiction witnesses remain historical.

## Reproduction

Requires Python 3.11+ for standard-library TOML parsing:

```sh
python3 tools/spec_consistency_audit.py
python3 -m unittest discover -s tools -p test_spec_consistency_audit.py -v
cargo +1.99.0 test --locked --offline -p xtask --bin xtask tasks:: -- --test-threads=1
```

The document audit checks witnessed obsolete clauses, required boundary rules, section links, exact task/ordering coverage, version agreement and JSON/TOML examples. Mutation controls reintroduce the old clauses and corrupt example versions/fuel or task order; the audit must reject each. The retained before/after reports and terminal test logs are in `artifacts/spec-revision5-consistency/`.

These are specification regressions, not compiled ABI probes, compiler compatibility acceptance, proof of all textual consistency, performance results or a full runtime milestone. Required runtime cases stay unexecuted/pending.

## Interpreter acceptance scope clarification

The earlier review left the full-suite prerequisite's measurement scope unresolved. The user-authorized final-contract amendment now defines a complete pinned tests/ inventory, revision/configuration identities, category and platform accounting, required compatibility backlog and conservative gate in Section 1.2.1. It does not narrow the objective to an in-profile subset. Current exclusions and conflicting compiler-diagnostic expectations remain explicit unsupported/contract-blocked cases until implemented or resolved by an authorized specification decision; they cannot become passes. This clarifies accounting without claiming the language/diagnostic contradictions have already been implemented away. See [E-R5-003](revision5-final-contracts.md).
