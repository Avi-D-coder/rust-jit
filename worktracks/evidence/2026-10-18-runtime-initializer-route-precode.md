# Remove reached item-value pure-evaluator fallback

Root, LOCAL-044, WT04/T032/M023, E-R5-007. Existing unchanged upstream const
arithmetic and const-fn-nested execute initializer calls; V7 actual i128 boundaries
expose that one runtime Type now works in the VM but cannot inhabit the pure
store-independent scalar wire. Never put arena handles in that wire or invent a
second wide evaluator for reached values.

Invariant: reached const/static initializers use the ordinary initializer-frame
path regardless of whether their expression happens to fit the pure evaluator.
Currently prepare_selected_item_value tries constant_item/static_cell ->
constant_item_data, catches any error and only then demands VM initialization.
This duplicates scalar/array runtime execution and hides real preparation errors.

Extend the existing cache-first initializer demand route for every reached item:
existing const/static run-long cells are reused; absent cells demand actual
CallResolver::initializer/ordinary signature/body/frame/continuation. Remove the
production item-value scalar/array pure fallback helpers constant_item,
constant_layout and static_cell. Keep preparation-time constant queries for
array lengths/const parameters and record remaining constant-pattern debt.
No new evaluator, macro identity, Type inference exception, namespace bypass or
static checker. Source identity/currentness and exact reached item remain owner.

Files engine/run_value_items.rs and documentation of run_initializers.rs only.
No runtime pattern/constructor edits conflict with the other workers. Common
CallResolver/local header extensions already scoped-pass V6/V7.

Cost: cache lookup at reached entry; ordinary VM initializer once for scalar or
shared-reference storage, existing fresh nonprimitive behavior remains held.
No whole-body/cold initializer preparation. No parser/planner added inside an
installed opcode. No resource accounting, fuel/time caps or sandbox.

Acceptance: existing nominal/static/promotion tests and unchanged whole const
fixtures on exact compiled source. Intermediate route repair does not close E007:
aggregate/nonCopy const run-once template instantiation and constant-pattern
preparation still need their own shared mechanism and actual evidence.

Required shared context join before validation: deleting scalar fast paths also
requires their real declared result to reach ordinary initializer root operations.
Extend Frame::definition_result_source/descriptor/materialize to reuse actual
GenericEnv normalized primitive proof or admitted written result, before existing
structural/reference cases. Always seed an entered initializer root through that
same DefinitionResult hint service, as ordinary function roots already do. No
width/macro/name guessing, no separate expression/type evaluator. Added files:
vm/frame.rs (only definition result methods) and vm/step.rs documentation. Record
constructor worker owns a different frame method and coordinates that shared file.

Independent review found another common result-service prerequisite before V8:
plain function pointer results were absent from both structural_result and the
nominal-only with_written_result capture. Generalize engine/run.rs closed declared
result capture to every supported closed WrittenType (the existing walker already
handles functions/pointers/primitive/aggregate/reference shapes), preserving actual
source proof and runtime generic bindings. Do not skip initializer hints for a
fixture or invent another Type converter. No new parser or evaluator.

Common HintRecipe service separation: GenericEnv::normalized_root_hint only
remaps syntactically printed primitive hints. Declaration-selected DefinitionResult
and an actually captured DefinitionWrittenResult delegate to their exact owning
Frame result service; they cannot be compared to a printed Primitive tag. Frame
root/definition checks and current admitted result source/descriptor remain. This
applies to every ordinary root, not initializer/macro names. V7 observed root
refusal also lacked u128 in syntax::thunks::hint, repaired through that common map.
