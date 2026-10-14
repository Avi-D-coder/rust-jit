# Integrated projection component evidence

All21 scoped T041 bodies passed across preserved first driver22657/1 and corrected segment30525/0, both independent collectors0. The zero-test row is not counted as a pass. Formatting/strict Clippy/architecture, LRIR14, storage8, two projection-extension controls, ledger and four scalar controls passed; total50 actual bodies. Fresh MAIN milestone pending; full fixture acceptance0. Earlier draft scope follows.

# T041 projection successor — private, unvalidated

Task invariant: intrinsic field/index/deref address formation retains checked
guest identities, validates enclosing objects and leases on use, and freezes an
evaluated usize only after its type and bounds are checked. Simple assignment
still evaluates its RHS before entering the destination. No host pointers,
runtime member-name search, native loading or build actions are added.

This private successor appends ProjectTuple/ProjectIndex tags 35/36, pure stable
encodings, exact operand/place-slot validation, reached source lowering and
per-frame projection allocation accounting. Checked preparation owns the new
projection once; a fuel refusal discards preparation without publishing it.
Read/deref/assignment/borrow and scope retirement release retained projection
charges. Bounds panics retain the original operation PC and source origin.

Primitive array annotations retain a source element and literal length rather
than obtaining source semantics from the runtime descriptor. Descriptor creation
is an executed hint action; pure preflight only looks up existing descriptors.
Source construction and projection compare the retained element/length with the
worker descriptor. General symbolic types and const expressions remain pending.

Eight new controls are registered but have not been built or executed:

- Original primitive array initialization and retained source view.
- Original out-of-bounds index PC/origin and guest panic.
- Wrong signed/u64 index identities before projection effects.
- Huge usize bounds before ordinal conversion.
- Checked element read and source propagation.
- Stale receiver lease after whole movement.
- Fuel refusal without projection publication.
- Original whitespace/comment tuple selector chains.

The added original `tests/ui/lint/unconditional_panic_98444.rs` and stderr are
unchanged at ae0fe511b15ab760d7e963b13389d1bc2ab42b18 with original licenses and
hashes. The private inventory is 111 cases / 204 original files. MAIN remains
110/202. The array harness executes the original first binding, then independently
enters its unchanged index AST; intervening wildcard/array-borrow execution and
the full rustc stderr oracle are not accepted. Successful-index, wrong-kind,
huge-index, stale-lease and fuel injections are supplementary LRR controls.

The unchanged tuple-comment source supplies actual selector ASTs. Its harness
supplies an explicit constructor context, then enters the original selectors;
full inferred type binding and unit-pattern execution remain pending. These
controls do not count as full fixture passes.

Named fields, trait indexing/dereference, computed/temporary receivers,
general source/variant/call/return/Drop integration and later required gates
remain unfinished. T041 is in progress; no JIT/release/Linux acceptance follows.

E-T041-001 is applied in this successor: the verifier permits zero ordinary
operations with one real exit. The original dereference-entry regression
checks one exit fuel unit, no dummy opcode and no preforced receiver recipe.
This successor is unexecuted; earlier failed frozen candidates remain preserved.
