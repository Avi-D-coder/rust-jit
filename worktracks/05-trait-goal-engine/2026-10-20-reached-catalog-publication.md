# M020 / LOCAL049: reached immutable catalog metadata publication

Bounded precode, 2026-10-20. Actual unchanged repr128 execution needs the genuine
ImplCatalog requested by its reached call. Root owns CLI/runtime resumption and
the actual input demand; this slice repairs the common metadata publication
boundary only, connected directly to that existing SourceDiscovery publication.
No isolated helper/control acceptance or source-only language pass is claimed.

Actual hazards: configured_catalog::publish_cold_catalog_declarations invokes
publish_captured_declarations and publish_cold_declaration_names; both reject an
active run. The first already shares an exact immutable row kernel with entered
publication but its bulk path mutates per row, so a later conflict can follow an
earlier mutation. The second can replace DeclarationNames(scope), and its
identity includes catalog seal and publication nonce. Later publish_discovery
verification does not retroactively validate these earlier mutations. Removing
run_active guards would permit overwriting current name leases.

Owned production files: db/local_declaration_inputs.rs and
db/cold_declaration_name_inputs.rs, configured_catalog.rs actual callsite and
one impl_catalog.rs reexport. Existing cold API guards remain. A privately
constructed catalog-scope receipt binds actual completed SourceDiscovery,
ImplCatalog plan/seal, actual source ordinal, parent declaration index, captured
scope and real completions. Reached DB APIs accept only that receipt; they check
coordinator/no-query, current scope/source, catalog key/seal, compiler context and
epoch. The declaration row kernel stages a full batch and checks actual existing
and staged row equality before a single derived revision commit. Reached name
publication stages every scope slot first and, while active, rejects any
different existing slot; exact reusable values/no-op and absent insertions alone
are allowed. BodyEnvironment, active caller source/body attachments, VM slots and
runtime namespace handles are not replaced. A different seal/nonce name record
is a hold/conflict, not permission to overwrite it or refresh a caller's proof.

The ordinary/generated join remains real captured declarations -> existing
header/owner/context metadata kernel -> actual catalog publication -> shared
source queries. No parser, evaluator, trait matcher, static checker, root proof
or second interpreter is introduced. The alternate unconditional active cold
publication remains refused; only authenticated catalog metadata gains the new
bounded entry. Cold work is existing scope/header/name preparation plus keyed
staging; hot VM dispatch does no source planning or global namespace comparison.
No memory accounting/cap is added. Required source review covers all eight modes:
no separate evaluator, macro-only route, method/numeric selection, origin
exclusion, duplicate patterns/cold executable checking, repeated planning, global
dispatch census or immutable BodyIndex copy.

Root's next actual interpreter run of entire unchanged repr128, static alias and
other real regressions owns acceptance. Authentic missing module/procedural
configuration, new source name contexts and runtime continuation/commit holds
stay explicit. No Cargo/Git, mock database/source copy, new isolated test or
native guest evaluation is authorized for this worker.

Implemented source candidate (not runtime acceptance):

- `CatalogDeclarationPublication` has private fields and no caller constructor.
  Only `SourceDiscovery::publish_cold_catalog_declarations` borrows the actual
  sealed catalog, source ordinal, nested declaration parent, scope and actual
  completion map. Its currentness check requires the exact parent/seal references,
  actual source publication, ImplCatalog plan domain/key, completed producer/import
  frontier, compiler/procedural publication and current source/epoch/compiler
  input stamps. It grants neither a DiscoverySeal nor a root/body entry proof.
- `Database::publish_catalog_declarations` and
  `publish_catalog_declaration_names` consume that borrowed receipt. Existing
  cold APIs retain their active-run refusals. The common declaration row kernel
  stages against both existing and earlier staged actual rows, and the bulk
  publishers commit one derived revision only after their entire scope batch
  validates. Missing annotation producers hold before the reached batch commits.
- Name publication reuses existing `ColdDeclarationNames::reusable`, including
  actual source/scope/context/seal/completion checks. During an active run an
  existing slot must be the exact retained reused Arc and have the same
  fingerprint. Newly prepared nonce/seal/name rows cannot replace it. Every slot
  is preflighted before one commit; validated empty/no-change scopes return
  without a revision. The two DB APIs are individually staged, not one atomic
  transaction spanning every source scope and final `publish_discovery`.

Separate own source review, against the hashes below: modes 1–4 add no evaluator,
macro-only execution, method/numeric policy or generated-source exclusion; the
actual original catalog uses its existing ordinary metadata publisher and guards.
Mode 5 changes immutable declaration metadata only, with no pattern/body lowering
or cold executable preparation. Mode 6 retains the existing scope plan and name
reuse; receipt currentness is coordinator preparation (including existing catalog
frontier and observed dependency checks), never installed dispatch. Mode 7 adds
no run namespace comparison or unrelated invalidation: mutations are keyed
immutable insertions and reject replacements; catalog frontier scans remain cold
publication work. Mode 8 borrows source/index/seal handles and reuses name Arcs,
with no BodyIndex clone or new whole-file syntax search. The existing row kernel
still checks direct actual declaration syntax/header attachments. Cold preparation
cost includes a scope row walk and staged keyed lookups; receipt checks repeat
the actual catalog frontier/compiler dependencies per API. No performance claim
is established by this inspection.

Exact source fingerprints (SHA256):

```
5eeb15dc5b2d3adab44b395a0f375a52e30e72dfbbb42b13b9ef1db47a9c7e65  crates/lrr/src/db/local_declaration_inputs.rs
66b5e9f6ece0c84252796ab056ba6e6dc9481cb6f89e99f3214e728431140a68  crates/lrr/src/db/cold_declaration_name_inputs.rs
9bdec52c5e19967412d5051f3ebb14126f3b9b04ddce938678dcaf8a71f31d30  crates/lrr/src/expand/source_discovery/configured_catalog.rs
18f61fc87a2fcf459b2bb4b25eafae81eca7a06e53d5bf47a83afb10c42d984c  crates/lrr/src/expand/source_discovery/impl_catalog.rs
```

Pinned `rustfmt --edition 2024 --emit stdout` parsed these four current files,
terminal 0; this is syntax-only evidence. No Cargo, runtime or currentness-control
execution was performed by this worker. Root owns the one combined actual
unchanged eight-main validation and its terminals; no architecture or M020 gate
is closed by this source handoff.

## Independent final bounded source review

Read-only review before combined eight-program validation, October 20. Exact SHA-256: `db/local_declaration_inputs.rs` `5eeb15dc5b2d3adab44b395a0f375a52e30e72dfbbb42b13b9ef1db47a9c7e65`; `db/cold_declaration_name_inputs.rs` `66b5e9f6ece0c84252796ab056ba6e6dc9481cb6f89e99f3214e728431140a68`; `expand/source_discovery/configured_catalog.rs` `9bdec52c5e19967412d5051f3ebb14126f3b9b04ddce938678dcaf8a71f31d30`; reexport `expand/source_discovery/impl_catalog.rs` `18f61fc87a2fcf459b2bb4b25eafae81eca7a06e53d5bf47a83afb10c42d984c`. The names successor was specifically rechecked: active reuse now requires the same retained ColdDeclarationNames Arc as well as fingerprint, rather than relying only on a hash. No actionable source issue found in this bounded slice. No tests/Cargo or source edits by reviewer; only this documentation stamp.

The publication receipt has private construction in the actual sealed publisher, retaining exact SourceDiscovery/ImplCatalog plan, source ordinal, captured scope, actual parent index, seal and annotation completions. require_current checks coordinator/no active pure query, actual sealed key/domain/epoch, plan/current source dependencies, compile environment/engine, complete actual source and argument indexes, absent remaining producers/import demands, observed compiler input stamps and procedural publication state, then actual captured scope currentness. No caller-provided ready flag, raw replacement rows or executable authority can construct the receipt.

New DB entry points require that receipt. Existing cold entry run_active refusals remain. Declaration metadata stages the whole requested scope batch using the same existing exact syntax/header/owner/context/body kernel, compares actual old and staged rows as well as fingerprints, then commits one derived revision. Identical original rows retain their existing input Arcs; no source_revision replacement occurs. Name publication validates/stages all requested scope slots before commit. During an active run it permits only absent insertions or exact reusable retained Arc/fingerprint identity; any changed seal/nonce/record rejects. BodyEnvironment, entered caller/body attachments, VM cells and immutable runtime namespace handles are not replaced. Separate scopes can publish valid insertions before a later scope fails; this is not a whole-catalog transaction or completion proof, and final discovery publication still owns completion.

Eight-mode assessment: (1)-(3) existing declaration metadata publication only, no expression/macro evaluator, member solver or primitive rule; (4) genuine captured ordinary/generated source receipts retain origin boundaries; (5) no executable body preparation or duplicated pattern checks; (6) existing prepared source scopes and metadata kernel reused, with no planning added to installed VM dispatch; (7) full catalog completeness/currentness checks are coordinator-side demanded cold preparation, not a per-operation world namespace census, and old source/body leases remain; (8) retained prepared source/index handles, no BodyIndex deep copy or per-phase whole-file executable search. Bulk metadata staging is proportional to the actual demanded source scope's declaration rows. Receipt validation inspects its actual completed catalog frontier; no measured performance claim is made.

Required acceptance remains actual combined lib/CLI/digester build and all eight unchanged programs through the maintained runtime harness, preserving V534 baseline failures. Source review does not prove active publication, suspended continuation correctness, unchanged repr128 success, origin/currentness matrix, complete interpreter/full-fixture or JIT acceptance. No isolated mock/control has been added.
