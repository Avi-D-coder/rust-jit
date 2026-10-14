# T041 place and lease preparation

Review only. T041 has not started; T040 component acceptance and its collected
integration milestone remain prerequisites. No new language fixture pass, import,
production implementation or test execution follows from this document.

The original fixture selection, commit, directives and hashes are retained in
[the upstream review](../artifacts/macos-first/t041-upstream-lease-selection-review-v1.json).
Start with the unchanged Point assignments in borrowck-assign-comp.rs, shared
assignment in assignment-to-immutable-ref.rs, immutable local borrowing in
borrowck-mut-addr-of-imm-var.rs, two unit aliases in mut-borrow-outside-loop.rs, and
empty-array bounds in array_const_index-1.rs. These native compile-fail cases must
retain their original expected output. Their borrow-checker rejection is not LRR's
oracle for relaxed aliasing; adaptations outside the licensed directory must say
which original operation is exercised and which later source operations are pending.

Section 13.2 requires a location lease to retain the root cell ID and epoch, every
enclosing object identity, and the final cell ID and epoch. Validation must also
prove each object still belongs to the same traversed parent cell and that each
child cell remains installed at its recorded projection. GuestStore::replace keeps
the root cell epoch stable and transfers the old aggregate to an old-value ticket.
The old field cells can remain live with unchanged epochs until cleanup. Thus a
lease checked only against root and final cell epochs would wrongly permit access
to the old aggregate. The unchanged enclosing-record assignment in the Point stock
source is the primary language witness for this requirement.

Same-type scalar replacement must preserve a valid lease so reads see the newly
assigned value. Whole inline aggregate moves must invalidate the source-root lease
even if the destination reuses the same field object. Nested projection must retain
every enclosing identity, not only the outermost one. Moving/reinitializing a field
must invalidate references to that field without invalidating independent siblings.
Epoch exhaustion creates a replacement logical cell; field projection catalogs must
install that replacement identity rather than retaining a retired field cell.

Field storage is currently created writable by construction. Reference permission
must still inherit root mutability and shared/mutable access through all dereference
steps. A writable child must never override an immutable root or a shared reference.
Two independently created mutable guest aliases are allowed and serialize through
the VM; host RefCell alias tracking is excluded. Reference resolution obtains IDs
and immutable metadata, then releases arena inspection before a checked mutation.

Dynamic indexes require the actual usize descriptor (ID 13), checked conversion
and bounds before access. Equal-width u64 or isize is insufficient. Tuple and nominal
field projection must inspect their matching shape, source-selected variant and
exact installed field identities. Inspecting a cell-owned aggregate must not move
it into a ticket merely to use today's ticket-only aggregate_field API.

Lease records belong to a run-owned typed arena and carry no host pointers.
Projection metadata uses bounded, fallible reservations and checked byte charges
before publication. RefId and any new reference ValueRepr must be validated against
the target descriptor and current run epoch. Reset invalidates all handles and
physically reclaims metadata without invoking guest Drop. Buffer leases for later
Vec/String facades require their own storage identity/version; do not treat them as
inline cell leases or claim those later integrations passed.

Once T040 is accepted, copy these originals and expected-output files with both
rustc licenses and pin/hash records, then implement runtime/place.rs and
runtime/reference.rs through GuestStore's checked APIs. Add stock-driven assertions
before marking T041 complete and keep complete CLI/traits/buffer/JIT assertions
pending until their actual later dependencies and final fixture oracle run.
