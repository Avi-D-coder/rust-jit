# H002 conditional storage boundaries — in progress

Reached if and while expressions now enter explicit ConditionalScope metadata.
The condition and consequent inherit this scope; an if's else branch retains its
independent outer ancestry. Indexing is structural, bounded and deterministic;
it does not prepare condition operands or allocate guest cells.

A failed if condition branches to its Exit recipe, which leaves the condition
scope before entering else or producing absent-else unit. A successful consequent
first transfers its result to the outer if destination, then leaves the condition
scope. The consequent's block cleanup precedes that boundary. While conditions
and bodies share a fresh condition scope per iteration; ordinary backedges and
failure leave it before the loop scope. Existing scope-stack control transfer
handles continue, break, return and guest panic.

Six acceptance controls exercise metadata ancestry and repeated identity, bounded
metadata refusal, actual failure/success retirement ordering, distinct condition
cells across while iterations with retired cells remaining dropped, and actual
continue/break/panic handling. Cold branches still use lazy source recipes.

This increment establishes the containing condition storage boundary. Individual
boolean-clause temporary scopes, retained ClauseScope storage, general pattern
binding test/commit and syntactic let-extension remain pending. Actual owning
values, reference lifetimes, destructor/effect transcripts and full E24 tier
acceptance are also pending. H002 is in progress and no full LRR fixture or release
is marked passed.
