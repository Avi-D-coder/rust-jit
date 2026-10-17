# Reached module direct source ownership — held source component

LOCAL-024, T032/M013/M017/M018 prerequisite; unchanged official `numbers-arithmetic/shift.rs`. Only `engine/run_module_source_preparation.rs` changes. The helper remains unhooked.

The missing-environment branch now uses the actual selected declaration context scope and existing `CompletedSourceEnvironment::scope_owner`. A unique actual source is borrowed for a one-element capture slice. Missing and ambiguous owners remain explicit; no name lookup, caller-source substitution or first ordinal selection is added. Existing canonical source/header/context/body/caller, environment/currentness and macro publication checks remain.

This removes O(S) source-reference collection and allocation on first module admission, replacing it with existing O(log scopes) map lookup plus fixed stack storage. Cold capture/currentness costs remain; no performance measurement is claimed.

Generated boundary export and actual selected-body macro readiness remain required. `shift.rs` reaches an `assert_eq!` inside `test_misc` before `test_expr`; module environment publication alone cannot establish common macro/full-fixture acceptance. Root owns runtime integration and guard recheck ordering. No Cargo/compiler/tests executed; no runtime/architecture/task/milestone gate closed.

Exact source-aware all-eight review: `worktracks/evidence/2026-10-16-reached-module-direct-source-owner-source-review.json`. Precode: matching `-precode.json`. Prior full component `/private/tmp/lrr-reached-module-source-v1` remains immutable; this is a single-leaf overlay, not a replacement snapshot.
