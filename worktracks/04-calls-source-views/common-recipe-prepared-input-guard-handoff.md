# Common recipe prepared-input currentness — LOCAL-024

Precode October16, T032/T033/M017/M018, specification sections14/32.2/57,
E-R5-005 and MA005/MA007/MA008. Actual unchanged numeric suffix main reaches
id_i8(j) then fails CALL-ARG-TYPE (2a3855/1). Root's ordinary source constraint
query40 needs to retain its exact prepared positive/negative dependencies after
cold lowering, without requerying source at execution. This component supplies
currentness only; it cannot select a name, Type, callable or executable body.

Existing route: actual admitted body/header/hygiene -> source member and parameter
goal38 -> root binding constraint40 -> ordinary parsed/indexed lowering -> verifier
VerifiedRecipe -> InstalledRecipes::prepare_install/install_ready/get_current ->
ordinary VM operations/continuation/cleanup. This slice extends VerifiedRecipe's
existing process-local currentness sidecar. It does not alter lowering, opcodes,
verifier semantics or VM evaluation. Root owns the cold query40/41 capture and
with_shared_prepared_inputs callsite; no integration/runtime acceptance until that join
and actual main evidence exist. Legacy namespace witness remains enforced when
present and is never replaced by a relevant guard or fabricated absence.

Production API: with_shared_prepared_inputs(self,Arc<PreparedInputGuard>) ->
Result<Self,Diagnostic> retains one immutable Arc of the existing DB-minted input closure and rejects
any duplicate/replacement, including an attempt to refresh a stale recipe.
The cfg(test) owned convenience with_prepared_inputs delegates to that service;
production phases retain the same cold query41 guard without another closure walk.
No dead owned API or lint suppression is added to production.
Actual guard contains only bounded owned input keys/fingerprints, no source
parser/index/query-root Arcs. Guard metadata plus Arc control is checked against
existing64MiB recipe admission. VerifiedRecipe.clone shares guard allocation;
conservative full payload accounting per recipe remains intentional, avoiding
undercharge after outer query retirement without a new registry. Normal query
and installed recipe metadata paths already use recipe metadata_bytes.

Private cold guard capture encodes actual sorted input stamps under explicit
domain and caches the digest in the guard. Both complete and missing-descendant
repair paths account the additional Digest in their actual struct-size bounds.
Shared attachment and VerifiedRecipe's fingerprint accessor read this cached
identity O(1); no supplied digest or redundant scalar authority. QueryValue::Thunk fingerprint
adds the optional digest to the existing tag7 envelope, so identical operations
with different dependency envelopes have distinct memo results. Raw Recipe IR
stable domain/opcode schema remains unchanged: currentness is a process-local
sidecar, not portable source/execution authority. Query fingerprint migration is
explicit; historical no-sidecar Thunk fingerprints are predecessors.

revalidate_namespace retains its existing witness check and additionally invokes
guard.require_current(db). Production installed install/get_current/resume paths
already call it before VM work. Guard comparison makes D relevant published-stamp lookups (O(D log I) for the
existing I-row input BTree), bounded100k/32MiB by existing capture, read-only published positive/negative stamp
checks, no provider/parse/reconstruction/global namespace census. Existing coarse
namespace-witness and installed source_revision checks remain explicit mode7
holds. Direct low-level vm::step has no Database and has never provided namespace
currentness; it remains a structural VM API, not promoted to source authority.

Controls use genuine DB pure provider/input observations and unchanged official
suffix source for actual ordinary recipe structure. Outside originals they cover
positive and negative stamp replacement, unrelated inputs, same operations with
changed guard envelope, clone sharing/duplicate rejection/checked metadata and
retained memo payload lifetime where existing owner accounting permits. No test
mutator/fake name/Type/procedural authority. NeedInput remains the existing cold
prepare_input_guard contract; attachment accepts a completed genuine guard only.
No guest effect or source planning occurs while checking it.

All8 planned review:1 no evaluator/assertion compiler;2 no generated literal/break
route;3 no context-specific Type/numeric rule;4 same opaque guard for all admitted
origins;5 no pattern machinery/eager child demands;6 capture/fingerprint cold once,
readonly installed checks;7 relevant actual input stamps only (existing broad
witness held);8 Arc guard sharing, no immutable body/index copy or node search.
Source-only intermediate; exact fingerprint/reviewer/terminals follow freeze.
No Cargo/native tests by implementation worker; sole validator owns execution.

Exact frozen guard5: worktracks/evidence/2026-10-16-recipe-prepared-input-guard.json
SHA256 `3b1b2e6e13860e71fee98c18256e722ceb01e6027cd567b88b4ae824a6d0c377`,
source fingerprint `68d78e2d516d5b0a6e853c60167e6cf12ad66133dc45d36674573de224c511da`.
Payload `/private/tmp/lrr-recipe-prepared-input-guard-v1/overlay` has four exact
owned leaves plus a Type38-baseline db/value override containing only the Thunk
optional guard fingerprint hook. Shared query36/39/40/41 additions are excluded
from that override and must be composed independently. No tests of guard5 run.

Distinct source-only root41 phase/lifetime review is recorded in
worktracks/evidence/2026-10-16-prepared-binding-phase-source-review.json
SHA256 `a8a634112ec4483c4b472c4a5ad4fdc43631096d36a1a82409a235f740fed7b3`,
inspected source fingerprint `666a3a100cd062d53b39713bb873efba4b84ad0cce292df27cfb285d0afc0d09`.
No fatal source finding. Runtime currently invokes get_current several times
per dispatcher pass, so these relevant checks may repeat; single-check/O(1)
dispatch is not claimed. Actual query40 semantic review belongs to the existing
pattern worker. FunctionSignature optional callee DefinitionBody observation was
identified separately as a genuine negative-dependency bug: header Type output
needs actual Fn/header/context only; root owns its provider repair.
