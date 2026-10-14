Current local-borrow component milestone is collected:2180 project host/doc tests,9/9 T041 controls,0 failed/ignored, actual runner22053/0 and collector52643/0. T041 is still in progress and full stock fixture passes remain0. [Evidence](../artifacts/macos-first/t041-local-borrow-components-main-milestone-v1/independent-collection.json). Earlier component scope below remains applicable.

# T041 places and leases: integrated components

T041 is in progress. Local borrowing and primitive-reference annotations were
integrated after a frozen candidate passed20 scoped checks/24 selected test bodies,
strict Clippy, format and a fresh architecture audit. Actual runner7419 and its
independent collector exited0. Full current MAIN milestone validation is pending.
No full stock fixture, complete T041, JIT, release or Linux acceptance follows.

The interpreter creates a nonowning local place and then publishes a checked lease;
it never copies or moves the referent just to borrow it. Mutable guest aliases are
allowed by the specified relaxed single-threaded execution model, while shared
writes and stale location/object ancestry fail. Mutable reference values transfer
one owner through cells/tickets/constructors/sealed arguments; shared references
can be copied. Replacement of an enclosing object invalidates an inline lease.

Local primitive-reference annotations preserve access, actual target identity and
a source-local elision identity instead of inventing a static lifetime. The written
source view survives value publication. Abstract/nested references and named-region
annotation resolution remain later source-table work. Pure preflight publishes no
descriptor or guest lease. Bounds metadata controls produce typed backend outcomes;
full source-index execution and VM panic origins are still pending.

The unchanged regions-simple.rs is exercised only through its annotated mutable
borrow. The unchanged immutable-address case reaches the intended access refusal.
The mutable-alias source stops after two original unit borrows. Supplementary LRR
context/ownership mutations live outside the licensed source directory. Later
assignment/printing, trait methods/loops and rustc compile-fail oracles remain
unexecuted. MAIN now retains110 cases/202 original files with directives/auxiliaries
and both rustc licenses at ae0fe511b15ab760d7e963b13389d1bc2ab42b18.

The earlier annotation/source-view failures remain preserved. A separately owned
unvalidated dereference read/write successor is not part of this integration.


Dereference and scalar assignment are integrated with13 T041 controls after actual scoped passing evidence. The outer report-print failure remains preserved separately; a fresh integrated milestone is pending. Assignment checks the complete lease and result batch before moving the RHS into the target. Zero ordinary-op phases charge their real exit once without dummy work or successor preparation. General field/index and source/trait/call/return integration remain pending.
