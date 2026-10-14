# M013: compiler item macros in adopted physical children

Revision 4, Section 57 milestone C. This is component evidence for M013;
M013 and the full release remain incomplete. macOS ARM64 / Rust 1.99.0 is
the acceptance platform. Linux is deferred.

The invariant is that a staged physical child selects compiler macros through
its actual namespace, retains its source and inherited lexical cutoffs, and
requests include capture only through the explicit coordinator. Pure providers
cannot read files or execute macros as a side effect.

A selected compiler definition now produces an exact Builtin expansion key
rather than entering the declarative-owner lookup. Requests retain the actual
physical source, argument snapshot, and bounded owned inherited scopes,
declarations, hygiene facts and ancestry. Nested eager operands use those
scopes before namespace/prelude lookup, so an inherited `concat!` shadow cannot
silently select the compiler definition. Selected declarative operands execute
through the existing explicit source/generated expansion path.

Module compiler polling and included-view admission use the exact pending
module request. Admission still reproduces compiler output from adopted inputs
before permitting an expansion record into discovery. Included source bases and
compiler input stamps participate in the existing retained publication.

Eight acceptance controls cover physical `child/mod.rs` include resolution
with a competing root file, actual compiler `concat!`, missing paths and source
root escape, a child-local `include!` shadow, local/original/generated `concat!`
shadows, and the original parent's source-order cutoff. Six successful controls
compare CLI guest execution to explicitly compiled isolated native oracle
copies on Rust 1.99.0 macOS; selected live runtime units are never compiled.

The initial literal-include witness failed with `selected captured macro owner
absent`. A missing include uses the existing typed LRR-E-SOURCE-PATH diagnostic.
An initial generated-shadow fixture used an invalid nested repetition; its
corrected single-literal matcher is exercised by both CLI and native Rust.
Failed logs remain historical evidence; they do not count as passing checks.

The focused suite passes. Full milestone gate, strict Clippy, formatting,
architecture and final manifest evidence are recorded in the checkpoint only
once observed. Nested generated/included compiler contexts, complete quotas,
service/facets/cache/runtime/type integration, MX027/MX028, JIT, release and
Linux validation remain required. This increment does not close M013.
