# M007 macro binding visibility

The source route builder retains visibility on both definitions and reexport
edges. `lookup_from` takes an explicit source caller scope, canonicalizes its
module path, and returns a typed inaccessible result instead of a callable
candidate when a binding is private. The raw `lookup` API remains candidate-only.

The pinned syntax parser resolves restricted visibility recipes into owned,
stably encoded ranges. Public, private, self, parent, crate and explicit ancestor
scopes are supported. Invalid/nonancestor ranges fail before namespace mutation;
raw and decomposed Unicode names use canonical source identity. Pending producer
outcomes retain precedence and are never converted into a visibility absence.

Each reexport target is read from its importing declaration scope. Its target
binding must also permit the visibility exposed by that reexport. E-M007-011
records the native E0364 witness that corrected the initial widening assumption.
The source and native regression cover the correction.

Native macOS Rust 1.99.0 executes the permitted call matrix, rejects five external
private calls with E0603, and rejects widening with E0364. Unit tests check
canonical ancestor restrictions, stable encodings, producer transitions and
atomic refusal of invalid ranges. These checks cover macro bindings and reexport
width. Module-prefix and module-alias visibility now integrate through
[module-path-resolution.md](module-path-resolution.md). Glob propagation, membership
seals, generated epochs and guest execution remain pending. No complete Rust
privacy or runtime security claim is made by this component.
