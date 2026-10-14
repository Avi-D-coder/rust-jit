# Procedural span protocol checkpoint

M009 uses the pinned proc-macro client's existing bidirectional postcard codec through its audited explicit selector. The matched Rust1.99 API6 server must enable rust-analyzer spans before any procedural library loads. No ambient protocol switch or retry chooses a different codec.

Captured source bytes have a checked content identity and an optional explicit absolute local-file capability. The consuming request binds these inputs into both the session invocation fingerprint and the service demand fingerprint. Callbacks consult only these immutable bytes and registered private anchors. Plain file requests and editioned anchored requests have different pinned wire encodings; malformed IDs, unknown anchors, invalid UTF-8 boundaries and unavailable source snapshots produce typed failures.

Actual macOS native compiler comparisons exercise source_text, file/local_file, line/column, start/end, resolved_at and located_at. The relocation fixture uses two source literals at separate Unicode source positions, so copying location is observable independently of preserving the original location. These tests compare returned observations with native execution rather than supplying an expected macro result. Caller sites and context rows remain explicit request data; full generated-source discovery/hygiene integration is still required.

The procedural engine fingerprint generation is 5 and explicitly identifies API6/bidirectional-postcard/captured-spans-v1/context-labels-v1/sites-v1. An earlier protocol engine key cannot fulfil a current demand. Source capabilities changed under an existing invocation identity cannot reuse or reexecute the cached macro.

Pinned codec acceptance checks distinguish a complete frame at its exact budget from a truncated budget, bound unterminated frames before decoder growth, and reject malformed COBS. Conversation acceptance verifies that callback denial is encoded as Cancel and callback output cannot exceed the independent writer budget. These checks do not claim general protocol fuzzing, full span ancestry/join acceptance, or stable native byte_range support. Rust1.99 rejected byte_range with E0658 in the preparation experiment; it was removed from the stable fixture.

Normal helper exit permits one fixed 100ms pipe drain window to observe actual queued bytes/EOF during concurrent process startup. The window cannot reset; a held descriptor remains an error. Cancellation and deadline termination do not receive that grace. Failed gates and the regression are preserved in the M009 span checkpoint artifacts. M009 and the project remain in progress; Linux acceptance is deferred to the later port.

## Measured stable capability admission

The matched Rust1.99 capability fixture compiles synthetic proc-macro metadata for each operation with the actual host compiler. call_site, mixed_site, source_text, resolved_at, located_at, start/end, line/column and file/local_file compile. def_site, source, parent, join and byte_range fail specifically with E0658. Compiler statuses and JSON diagnostics are retained; an unrelated compilation error cannot count as the expected instability result. No probe library is loaded or executed.

SessionConfig carries a deterministic set of required ProcSpanOperation values. After checking macro authority, the factory rejects any operation unavailable in the measured Rust1.99/API6 contract before tool-path verification, server startup or procedural library loading. The service requires the complete stable set. The compiler fixture compares the measured accepted operations with that production set. Missing-path denial fixtures prove capability admission and authority ordering independently of valid tool files.

These checks establish this exact stable tuple. They do not replace the separate actual native semantic fixtures, admit unstable operations with bootstrap/nightly, or claim general span ancestry/join support. The required project macros, traits and generated-source work remain unchanged.
