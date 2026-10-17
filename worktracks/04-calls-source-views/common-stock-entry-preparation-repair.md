# LOCAL-024 — stock nested statement macro entry join

Bounded T032/M013/M017/M018 prerequisite, WT04/WT08, October 16. E-R5-005,
MA004/MA005/MA006/MA008 apply; these are distinct from recurring modes 1–8.

Invariant: unchanged stock runtime witnesses must enter the same selected-body
source assembly/publication used by the standalone CLI. Generated declarations
join the ordinary syntax/index/type/call route. Never expand the held
`prepare_declarative_expression` literal/break recipe executor to handle this.

Minimal unchanged witness: official `tests/ui/macros/macro-nested_stmt_macros.rs`,
344 bytes, SHA256 abef2c55f007cf929838a1cae6e3e329f8bf4747979db3e78b11580d5b86eb49,
pinned upstream ae0fe511b15ab760d7e963b13389d1bc2ab42b18, run-pass owning2015/noaux.
V2 frozen source 3dad9485 compiles, but the whole main faults at actual grault!()
224..233 before any generic call (terminal4aae06/1, release57a2c9d5).
The exact failing key/profile conjunct is unobserved. This was a harness route
error: `run_source_preparation_tests::start` bypassed the existing production
`cli::retained_entry::prepare`; setup also uses a test compiler-version digest.
No claim that this proves a production macro request validator defect.

Actual common route: `cli::retained_entry::prepare` ->
`SourceDiscovery::prepare_body_syntax` -> actual `SourceSyntaxAssembly` receipts
and `PreparedSourceSyntax` -> `RetainedSyntaxInput::prepare` ->
`Database::publish_retained_syntax/publish_retained_body` -> same selected
SourceCallable -> `Database::prepare_source_local_plan` -> ordinary BodyIndex,
Thunk verifier and RunMachine. Original/generated output joins through retained
AST nodes, owning editions and admitted source/epoch/seal/origin closure.

Change: expose this existing entry adapter crate-wide, accept the actual selected
entry name rather than hardcoded main, and use it in the common runtime harness.
Standalone CLI passes main explicitly. Reuse its same generated-entry fallback
and explicit gaps; do not introduce fixture/source/macro-name conditions.
Preserve the V2 failed attempt and source. Future module/TypeArg/Q47 work remains
held. No removal of legacy runtime engines is claimed; this harness must no
longer enter them for completed statement-macro expansions.

Dependencies/authority: only sealed admitted outputs, no producer execution/query
side effects/new native authority. Selected function only; cold executable bodies
stay cold. Cold source assembly/indexing remains existing bounded work; no new
installed operation planning, global scan or deep BodyIndex clone. Direct actual
source/callee identities and currentness remain enforced by existing adapters.

Evidence: source review against all8 modes, exact frozen source; owning2015 whole
unchanged main first, report next common failure, all4 only after green. Inspect
actual retained TypeArg nodes and no macro-expression residue/cold generic code.
Passing preparation is not a runtime-contract/fullfixture pass. All-context
matrix, obsolete-route removal and milestone remain incomplete.
