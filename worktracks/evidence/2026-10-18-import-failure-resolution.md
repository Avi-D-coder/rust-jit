# Ordinary import failure evidence

Root, WT04/WT05, T008/M025 dependency, LOCAL-045. The unchanged owning-2015
`tests/ui/consts/issue-17718.rs` with its real auxiliary source fails V9 before
execution on the import `std::sync::atomic::Ordering`. The facade contains the
enum, while another unchanged witness imports its SeqCst variant successfully.
Neither observation identifies the actual stalled namespace/producer; do not
guess an Ordering-specific alias or change the fixture.

Extend only failure-path evidence first. Existing NameNamespaces ordinary import
table retains the actual SourceImport, edition and identity. The shared
lookup_import_target service can report each namespace's actual current outcome
without mutating the graph or executing a query provider. Add a diagnostic-only
summary method and use it when attach_incomplete_source_origin selects an
ordinary blocked import. Keep the existing stable import spelling and error code.
No alternate resolver, native call, source access grant, static checker, macro
case, cached-plan modification or import success assertion is introduced.

Source paths: frontend/names.rs and expand/source_discovery.rs. On the failure
path only, three existing relevant path lookups plus formatting; normal dispatch,
source preparation and successful discovery are unchanged. All eight recurring
failure modes reviewed: no evaluator, macro-special syntax, numeric/member
exception, origin rejection, pattern/cold-arm traversal, installed replan, global
namespace scan or immutable body copy. This is diagnosis, not semantic repair.
Acceptance requires the sole validator's next exact-build unchanged fixture
capture; retain the existing preexecution failure until a shared repair and actual
runtime receipt establish progress. Source fingerprint and terminal still pending.

## Exact source review before validation

- `crates/lrr/src/frontend/names.rs`: `3696885418bb72b7aaa10cf01678ceb942354ec7d0bc2b1e5004dfa99be76593`
- `crates/lrr/src/expand/source_discovery.rs`: `cb9b1236f4b49b914ca51cfbee0cfca31bb8e49dce4e696bc8467cf427e743f2`

Independent source review receipt SHA-256 `fc6955cebe94b9098f8bbc9032fe5595c4f3b04f0da2ee6a98e100c90b8ebf85`. No source blocker; runtime validation remains pending.
The preliminary review found that propagating an evidence lookup error could replace the primary incomplete-discovery result. The current helper instead records each lookup error in the suffix and preserves the original diagnostic/blocked ledger. The failed preliminary review is retained locally.
