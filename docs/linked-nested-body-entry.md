# M007 linked external-module nested entry acceptance

The linked nested-entry wrapper is now exercised with main.rs, child.rs and
child/grand.rs from one immutable source publication. The source acceptance
enters an external module's function, its nested function, and a grandchild.
Textual macro lookup and a frozen inherited alias retain the macro captured at
the external module declaration, rather than later containing-module/body
redefinitions. Every retained ancestor is independently changed and rejected;
a missing ancestor and a lookup through the opaque parent are also rejected.

The macOS Rust1.99 native fixture executes nine calls covering containing-module
functions, early/late grand-module functions, their nested textual calls,
inherited aliases and grandchildren. Each selected original definition is
expanded by the pinned matcher/transcriber using the full original source token
forest and preserved content/path/offsets. All nine observed expansions match the
actual native vector. This establishes the tested source-chain/body capture
integration; it does not claim guest VM execution.

Current Gate01, strict Clippy and formatting pass. Container entry and local
module/item publication, local binding ribs, generated publication, namespace
seals, guest VM integration and M012 request accounting remain pending. M007 and
the release remain incomplete.
