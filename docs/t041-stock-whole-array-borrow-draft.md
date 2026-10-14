# T041 source-bound whole primitive array borrows

Integrated from collected fixed V5 component evidence. Every reached borrow of
a closed primitive array must validate the source element's exact primitive
identity and length against the live descriptor before fuel, allocation or lease
publication. The reference retains that written array view; a dynamic TypeId
cannot invent a missing source binding. The shared borrow does not move/copy the
array owner. This also admits the retained view through the existing source-aware
dereference preflight, without changing the guest lease model.

Only primitive element arrays with checked u32 lengths are added. Generic,
symbolic/nested array elements, unresolved consts and reference annotation syntax
retain their current unsupported results pending the later source/type tasks.
No new opcode, unsafe block, parser/backend change or provider effect is added.

Seven controls reuse unchanged unconditional_panic_98444.rs and its original
build-fail directive/stderr at rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
The existing source runner executes the original explicitly typed array, then
the harness enters its unchanged &xs AST inside the wildcard statement. It stops
after that borrow: wildcard execution, the subsequent OOB, compiler diagnostics
and ordinary aggregate cleanup are not accepted by these controls. The positive
control verifies every original element through the resulting actual lease and
retains array ownership/epoch. Negative supplements erase or corrupt source
facts, forge a destination type, or inject zero fuel outside the licensed source.
The sixth control exercises direct checked frame dereference of that actual
original reference, retains the array source at the destination, checks every
element and requires a stale-lease fault after moving the array owner. The unused
original PathExpr place inside &xs supplies its destination in the same already
active scope. No rewritten dereference expression or whole fixture is counted.
Review found that the earlier frame estimate reserved one boxed source node,
while the admitted array reference owns two. This successor reserves both for
every slot/binding view and the return view before frame admission. The seventh
control saturates the original body's source inventory and compares the estimate
against the actual retained buffers and boxes; it does not execute the program.
No generic source table or changed resource limit is introduced.

V1's first positive control passed, but its second control failed: it injected
the wrong source into an unformed place, which the original MakePlace correctly
replaced from the retained binding before Borrow. V3 corrupts or erases the actual
binding source instead. The original MakePlace charges once; refused Borrow
charges zero and publishes no reference. Destination mismatch likewise follows
that real MakePlace. This preserves the VM's per-operation fuel contract rather
than pretending the whole two-operation recipe has no completed work.

This is separate from the unaccepted assertion-dereference draft, whose full-body
test failed during the ordinary aggregate cleanup assigned to T044. That failure
is retained; this successor does not bypass cleanup or count a stock fixture pass.
V3 stopped at strict Clippy because the sixth test incorrectly named
ExpressionKind::Index. The pinned body index represents index expressions as
ExpressionKind::Other; V4 corrects that discriminator, still matching the exact
IndexExpr range derived by the pinned parser from the original source. Production
is byte-identical to V3. No V3 acceptance control executed.

V4 passed formatting, strict Clippy and its first five exact controls, but the
sixth attempted to enter a sibling index scope while the original borrow scope
was still active. The checked VM rejected that invalid lexical ancestry. V5 uses
the original borrow's unused PathExpr place instead and asserts that dereference
preserves the active scopes. Production remains byte-identical to V3/V4; no
scope check is weakened. The seventh control and broad suites did not run in V4.

V5 actual runner57422 exited0 and independent collection verified every fixed
source input and all10 scoped checks. Corrected dereference and source-memory
controls passed; full library337 exercises all7 new controls, with CLI27, LRIR14,
VM20 and source_run67 also passing. The unchanged2000-frame/256KiB regression and
fresh architecture audit passed. Exact production/test bytes are integrated;
this scope note was updated after copying to reflect collected evidence. MAIN
now has62 T041 controls and127 official cases/221 originals. A fresh combined
MAIN milestone is required before component closure. Full upstream fixture,
JIT, release and Linux acceptance remain false.
