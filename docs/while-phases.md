# T031 while iteration phases — work in progress

Original unlabeled scalar while now has Entry, condition Operand(0), body
Operand(1) and false Commit phases. Entry activates the loop scope and gives
bool context to the condition. The condition selects body Entry or false Commit
without fetching the unselected body. Selected body completion must produce unit,
then retires iteration storage and emits an explicit Backedge to the while Entry.
False Commit retires the condition/loop scope and creates unit in the enclosing
expression's result slot. A block expression condition follows the same owned
operand identities; pinned HasLoopBody's generic first-block helper is not used
to confuse a block condition with the actual body.

The loop result's physical slot belongs to the enclosing scope. Condition values,
iteration temporaries and the body result belong to the iteration scope; inner
body bindings belong to its nested block. Each backedge therefore starts with
fresh condition/temporary storage and cleared body binding identities, while
outer locals survive. Installed recipes remain immutable and are reused without
querying their PCs again. Source controls cover direct/block conditions, changed
outer values, iteration lets and an if within the body.

Backedge is an owned explicit recipe exit with a separate stable tag. Admission
requires an indexed while/loop Entry PC; an ordinary node or loop Commit is not a
valid target. VM execution records a completed backedge and updates the PC.
The dispatcher handles its distinct outcome and records a checked backedge poll
count before its next iteration. The earlier developmental recipe-step quota was checked before resolving the next
thunk. It is now replaced by [opcode/exit fuel and interruption](run-fuel.md).
Uniform runtime preflight/cleanup element accounting and full service/watchdog
acceptance remain mandatory.

Four added acceptance controls exercise actual repeated source execution,
installed-cache reuse, false-body coldness, bounded unending while, terminal
non-replay, exhausted quota's zero extra thunk and backedge admission/identity.
The existing cold-branch trace control now includes the complete false while-body
range. All prior branch/numeric/local/storage regressions remain required.

T031 remains in progress. Loop and named/unlabeled break/continue target selection
and scalar scope cleanup are now integrated in [the control transfer checkpoint](control-transfers.md).
Labeled while has the same selected target support. Precise fuel/cancellation and
final acceptance audit remain unfinished. General references, user Drop, patterns/if-let, macro/generated source,
source views, epochs and JIT integration remain later cross-ledger requirements.
Gate03 and full release/security/performance evidence remain pending; Linux
acceptance is deferred.
