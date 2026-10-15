# Source-selected shared string arguments

T041/T062 follow-on to the held string literal, macro and equality packets. Closed source parameters can now select intrinsic `str` through the existing declaration-context lookup and current owning Type namespace. The parameter retains the unsized `TyTerm::Str` pointee; the physical descriptor is used only for the shared reference. Type aliases or shadows do not become primitive string authority from their spelling.

Actual arguments must retain a frame source view and a genuine committed immutable UTF-8 reference. Call-packet validation checks the exact source pointee, access, cell, store session and descriptor. Existing named/elided region binding and current source/epoch checks remain in force. Mutable string views, declared-static parameters, aliases, arbitrary borrowed string storage and reference return contracts remain outstanding.

Four pending component controls use the unchanged upstream `match-borrowed-str.rs` headers and actual first initializer execution from `nul-char-equivalence.rs`, plus LRR authority/tamper controls. The harness joins these two witnesses to exercise call-contract construction; this is not either original's full main or an end-to-end source call. String pattern execution in the match witness remains outstanding.

Three leaves, formatted and exactly replayed; uncompiled and unexecuted. Hold behind the byte milestone and string chain. The copied upstream witness retains the original MIT OR Apache-2.0 licenses and pinned manifest provenance. No runtime/full-fixture count or broad task completion is added.
