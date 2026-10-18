# Ordinary intrinsic-Copy constants retain run-long storage

Root, LOCAL-044/WT04/T032, E-R5-007 and E-R5-008. The unchanged issue-17718
root and real auxiliary declare C3: fn() and both call and later read it.
Current resume_initializer caches only Primitive or shared Ref keys; ordinary
runtime shape metadata and intrinsic_read_mode already declare Function and RawPtr
Copy. Their reached initializers are therefore incorrectly re-entered per use,
contrary to the run-once initialization contract. This does not establish a
runtime result failure in issue-17718; initializer exact-once is its separate
LRR correctness obligation. No new stock pass is claimed.

Extend the existing resume_initializer storage choice using actual prepared
TypeDescriptor CopyStatus::Copy plus its nonaggregate scalar/reference/function/
pointer shape. Keep the same run_long_cell, const_cells and Frame ItemValue::ConstCell
copy_from route for every intrinsically copied representation. Nominal/array/tuple/
closure templates remain held until common immutable template instantiation exists;
never move the cached original or duplicate mutable storage. Update the stale
frame_item_values module comment which still describes the deleted pure evaluator
runtime route. No scalar kind reinterpretation, function name match, new evaluator,
new source query, parser, permission, compiler or static rule.

Actual source routes: RunMachine::enter_initializer -> ordinary CallResolver/body/
Frame DefinitionResult -> admit_initializer_return -> resume_initializer; then
prepare_selected_item_value -> Frame::execute_item_value -> GuestStore::copy_from.
TypeRegistry::intern and GuestStore::intrinsic_read_mode own intrinsic Copy facts.
The change reads one immutable descriptor at initializer return only; installed
reads remain their existing copy operation. No additional namespace walk, source
planning, BodyIndex copy or cache dependency is introduced. Static storage behavior
and nonCopy per-use construction remain unchanged pending E007 template work.

All eight recurring architecture modes checked: no parallel evaluator (1-2), no
context/macro/name inference (3-4), no patterns/cold executable preparation (5),
no planning in execution (6), no namespace scan/dependency widening (7), no
immutable body duplication (8). Evidence needed: exact new build and affected
existing initializer/value controls, unchanged issue-17718 with real auxiliary;
full nonCopy/generic-cache/promotion/suspension/origin matrix remains open.
Source review before integration must retain exact fingerprints. This is a bounded
source correction, not full E007 or interpreter/JIT gate completion.

Source-review snapshot:
- `crates/lrr/src/engine/run_initializers.rs`: `0b2f1d71256de037cb66ffd2454059d08c749ffb1eb2af0f5e994279dbad7494`
- `crates/lrr/src/vm/frame_item_values.rs`: `3c37f7bdd0acb7aa91595b47e4ea941d3f102c6903311f0e0b19dd289d977e73`
