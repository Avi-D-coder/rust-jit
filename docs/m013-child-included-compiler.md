# M013: included-source compiler context acceptance

Revision 4 / Section 57 milestone C, macOS ARM64 Rust 1.99.0 first.
This increment verifies existing retained-view behavior; it changes acceptance
coverage and evidence, not production compiler routing. M013 remains incomplete.

Three new controls exercise compiler item calls inside source included by an
adopted physical child. A direct include/concat chain resolves second.rs relative
to pieces/first.rs. An include emitted by a macro defined in that included file
keeps the same directory. An inherited original concat shadow uses the child's
earlier parent cutoff despite a later conflicting parent declaration. Competing
root files have different values, so choosing a root directory cannot pass.

Every control runs the actual CLI interpreter and compares its result with an
explicitly compiled isolated native Rust copy. No selected live runtime unit is
compiled. Both direct and generated nested calls use the existing included-view
provenance and explicit eager compiler polling/capture/admission path.

These tests establish the named path/cutoff combinations only. Nested physical
module adoption, arbitrary included/generated scope combinations, complete
quota boundaries and service/facets/cache/runtime/type integration remain open.
Sections 50.3/50.4 require the M014 nested producer scan and M015–M016 catalog
integration in fixed order; those are not proved by compiler item tests.
Full MX027/MX028, JIT mode matrix, release artifacts and Linux acceptance remain
unclaimed. The checkpoint records milestone and audit results only after actual
completion and preserves historical failures and native oracle hashes.
