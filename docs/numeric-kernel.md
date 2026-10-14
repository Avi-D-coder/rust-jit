# T025 numeric operations — work in progress

Current integration: the pinned syntax frontend decodes char/byte escapes and
normalizes decimal float data into owned literal recipes, while retaining source
spellings and spans. Recipe admission rejects missing or mismatched decoded
payloads; their stable encoding affects recipe identity. The VM materializes
floats directly at a reached f32/f64 width, rejects nonrepresentable literals
before binding, and prints actual scalar results including negative zero, NaN
and infinity. Arithmetic may produce nonfinite results; literal overflow fails.
Explicit returns carry the primitive return hint and stop before later statements.
Reads of a previously bound literal alias yield its concrete scalar descriptor.

Six new acceptance controls cover source float rounding/casts and no defaulting,
failed-range retry without binding, escaped chars/bytes, explicit return contexts,
actual CLI execution and decoded recipe admission/identity. Gate02 now runs the
existing source/query gate and checks actual passes for all registered T020–T025
fixtures. Its guard rejects missing task inventories and ignored selected tests.
T025 remains in progress pending the final acceptance audit. Generated bodies,
symbolic aliases, compound assignment/control/call contexts, trait operators,
complete cleanup and JIT/release integration remain assigned cross-ledger work.

The following progress notes describe earlier checkpoints; pending statements
in them are historical and superseded by the current integration above.

The concrete primitive kernel enumerates checked integer arithmetic, shifts,
bitwise operations, comparisons and unary negation/not, plus f32/f64 arithmetic
and IEEE comparisons. Fixed-width native checked operations run at the matched
guest width; isize/i64 and usize/u64 retain distinct descriptor IDs. Shift counts
can have another integer descriptor but must be nonnegative and below the left
width. Left shifts discard shifted-out bits as Rust's checked shift operations
do; count violations are guest panics. Signed minimum divided/remaindered by -1
is overflow, including the remainder case whose mathematical result is zero.

Arithmetic overflow, zero division/remainder and invalid shift counts return a
typed GuestPanic(PanicKind). Wrong operand descriptors/unsupported pairs and
unresolved widths return RuntimeFault diagnostics. No host panic or global
numeric-width default implements these guest outcomes. This kernel reports a
panic reason; the executing opcode must attach source origin and the dispatcher
must run ordinary panic cleanup. That source/cleanup integration is still pending.
Short-circuit boolean operations remain control-flow phases, not this binary
kernel. Nonprimitive/user-operator dispatch remains the trait/facade tasks.

Six concrete-kernel tests exercise 655,360 exhaustive i8/u8 arithmetic pairs
against independent widened mathematical/range oracles; 40,000 seeded wider
arithmetic cases across signed/unsigned 16/32/64-bit and distinct word descriptors;
all i8 values with negative/boundary mixed-type shift counts; unary minima,
boolean bitwise operations, f32 per-operation rounding, NaN comparisons and
runtime-fault descriptor/context controls. The wider i128/u128 calculations
exist only in the test oracle; guest arithmetic never uses a universal number.

A seventh storage/context test verifies result-ticket reservation before shared
literal binding. materialize_ticket validates range and prior binding, checks
and reserves its result, allocates the concrete ticket, then fixes the shared
record. The VM's existing suffix/context conversion now uses it. A quota failure
preserves the original unresolved ticket and permits a later different width;
a successful conversion fixes the width for every alias. The existing scalar
execution suite must remain green after this integration.

Remaining T025 acceptance: source arithmetic/negation/cast phase templates,
reached contextual hint propagation without future-use inference, complete
primitive casts and float/char literal parsing, direct construction of the
suffixed signed-minimum syntactic pair, GuestPanic opcode/dispatcher origin and
cleanup, and Gate02. No T025 task completion, source arithmetic support, JIT
agreement, full resource/security/performance release or Linux support is claimed
by these kernel tests. Ordinary/hygienic/symbolic/generated-source integrations
remain the later revision-4 requirements as well.

The concrete primitive cast matrix is now implemented. Integer conversions
preserve/truncate the low bits and extend according to source signedness, while
retaining the destination descriptor. Float-to-integer conversion occurs directly
at the destination width, with truncation toward zero and saturation. Integer to
float and float-width conversions use matched native source/destination operations;
u64-to-f32 has no f64 intermediary. Bool/char convert to integers, only u8 converts
to char, and scalar identity conversions preserve bits. Pointer/reference and
representation casts remain denied. Unresolved literals still require reached
context before a cast. These rules follow the
[Rust Reference cast contracts](https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions)
within the prescribed live-profile exclusions.

Four further kernel tests cover a 1,200-case integer conversion matrix, all u8
code points and invalid cast classes, destination-width float saturation and
fraction truncation, negative zero/NaNs, and an integer-to-f32 double-rounding
witness. The pinned Rust1.99.0 native compiler control in
artifacts/macos-first/t025-cast-oracle/identity.rs confirms bool/char/unit identity
casts; only metadata is produced and no selected live runtime unit is compiled.
Source cast recipes, reached contexts and source-level numeric acceptance remain
pending. Earlier remaining-work text's concrete primitive cast kernel requirement
is now exercised; its source integration and all other listed requirements remain.

The verified LRIR inventory now contains NumericBinary, NumericUnary and
PrimitiveCast. Their operator enums are owned IR data with explicit stable tags;
VM execution validates/read-checks scalar operands, prepares destination storage,
invokes the concrete kernel and commits a checked ticket. Admission rejects
wrong/missing slots and output/operand aliases. Three additional opcode-counter
columns preserve separate started/completed accounting.

Guest arithmetic panics return StepOutcome::GuestPanic with bounded PanicKind,
current PC and immutable recipe Origin. The failed operation has started but has
not completed; neither later ordinary operations nor the recipe exit execute.
Panicked frames retain live storage until explicit cleanup. Scalar panic cleanup
retires active scopes once and reaches PanicCleaned. RuntimeFault instead enters
Faulted, leaves storage live and cannot invoke panic cleanup. Four actual boundary
tests exercise those distinctions, three arithmetic panic classes, real unary/
cast outputs and malformed recipe admission.

RunMachine now routes the typed panic through scalar cleanup to a distinct
Panicked report; the CLI has a developmental panic JSON branch with nonzero exit
status. These dispatcher/CLI branches are wired but their source-level arithmetic
panic path has not yet been exercised: the source adapter still needs numeric
phase/context lowering. The kernel/step cleanup evidence must not be called
complete source panic, user Drop or should_panic acceptance. Source origins in
these boundary tests use explicitly constructed verified recipes. Full generated
origins, user destructor activation order, cleanup failures/double panic and
frame-stack interruption remain the assigned later integrations.

Original source integer arithmetic, comparisons, bitwise operations, unary
operators, grouped expressions and primitive casts now lower into entered PC
phases. Binary Entry supplies a reached operand hint and selects the left PC;
Operand(0) prepares the right hint from the evaluated left descriptor and selects
the right PC; Operand(1) invokes the numeric opcode. No source child is evaluated
by the query provider. Explicit primitive return annotations supply root/tail
context; local annotations and already evaluated concrete operands also supply
context. A bool comparison-result hint never supplies an integer operand width.
A cast target never defaults its unresolved source width.

NumericHint is owned IR data referring to logical slots. VM execution treats
numeric operand hints as advisory materialization contexts, while declared
storage contracts remain strict. Explicit suffixes and distinct shift-count types
retain their own descriptors. An unresolved local read can bind its shared literal
record through a reached context; subsequent aliases retain that fixed width.
A syntactic integer negation/literal pair, including parentheses, constructs
negative magnitude directly and accepts signed minima. Negating a previously
materialized minimum executes NumericUnary and produces GuestPanic instead.
Unsigned suffixed negation follows the operator path and remains unsupported.

Shift RHS expressions receive u32 context, while bare unresolved count records
are range/sign checked before materialization. Negative and oversized counts
therefore produce InvalidShift GuestPanic instead of a conversion fault. Compound
count arithmetic still executes with u32 context. The first failed source shift
run is retained: this was an implementation ordering bug repaired under the
existing Section11.4 contract, not a backend contradiction/design erratum.

Five further source tests exercise return/local/other-operand contexts, actual
integer and cast results, no default/future-use inference, signed minimum pairs,
invalid shifts and source-origin arithmetic panic cleanup. A CLI subprocess test
now exercises the actual panic branch: source overflow emits developmental panic
JSON with cleanup and exit1; unresolved arithmetic context emits a fault and exit2.
The normative T024 main still performs its original eight completed operations.
These source tests replace the earlier boundary checkpoint's unexercised-dispatch
limitation for the named integer cases; they do not prove user Drop, generated
origins, general symbolic types or complete numeric-language coverage.

Remaining T025 work includes complete float/char source literal decoding and
format summaries, any outstanding numeric context/return fixtures, final
acceptance review and Gate02. Macro-aware ordinary lookup, symbolic type aliases,
generated bodies, trait operators, full cleanup/resource/JIT/performance/security
and later Linux acceptance remain mandatory integrations beyond these initial
original primitive cases.
