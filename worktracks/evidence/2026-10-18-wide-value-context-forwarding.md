# Full-width primitive value context uses ordinary forwarding

Root, LOCAL-043, T023/T025/T032, E-R5-005. Unchanged original i128 main on
V15 reaches Iterator::sum -> Sum::sum(self). Actual failure diagnostic retains
GenericEnv result Param bound to TypeId(17), but its tail result slot has no hint.
Frame::numeric_hint_value forwards builtin parent descriptors only IDs 2..=15;
i128/u128 are the reserved primitive IDs 17/18, already selected by the same
primitive type registry and all ordinary arithmetic operations. A block tail,
parenthesis, selected branch or numeric operand thus loses its existing width.

Extend only that builtin parent-type guard to include both wide IDs. No new
inference, result template fallback, numeric default, Sum name logic, macro
logic, source evaluator or static checker. Existing NumericHint lowering and
Frame::numeric_hint/preflight_branch_hint share this function. Existing numeric
comparison and shift independence remain unchanged. Cold branches are untouched;
the selected branch executes the same hint operation and continuation.

Costs: one existing constant-time reserved ID match; no parse/query/source plan,
namespace scan, extra cache key, allocation or immutable body copy. No change to
source/epoch guards or BodyEnvironment. All-eight review: same ordinary operation
(1-4), no pattern or cold arm prep (5), no planning (6), no namespace work (7),
no immutable data duplication (8). Exact source review and unchanged whole i128/
u128 captures plus existing literal/numeric controls are required; no pass is
claimed before actual validation. Wider result/reference/function forwarding,
other trait-goal gaps and full numeric/interpreter/JIT acceptance remain open.

Source snapshot: `crates/lrr/src/vm/frame.rs` SHA256 `5578022b20a5c32bb290bb3dfa38ea0520ee4d33bf4ad07b0c96c4babae8fa03`. Only numeric_hint_value differs from exact V15 Frame; worker record methods stay byte-identical.
