# M011 compile-input builtin identities

The adapter discovers env, option_env, concat, include, include_str and include_bytes
through the pinned upstream builtin inventory. Private descriptors carry stable
identities tied to the upstream commit, Rust 1.99.0 profile, name and explicit kind
tag. Discovery supplies compiler facet metadata; it never establishes the identity
of a source invocation by spelling.

The runtime copies only owned descriptors across the adapter boundary.
`CompileInputCatalog` binds each descriptor to its original compiler unit and
edition, exposes root exports through the existing source macro graph, and selects
only exact retained definitions. Aliases preserve defining identity; forged
unit/edition provenance and duplicate/excessive facets refuse. The frontend imports
no upstream semantic or adapter type.

Frozen environment operand dispatch accepts only a selected env or option_env
descriptor. Other compile-input kinds refuse this operand path. A native macOS
fixture resolves original core aliases through the existing graph and compares
captured required/optional operands with Rust 1.99.0. An exported user macro named
env remains a source definition outside the compiler catalog and executes its own
native recipe. Genuine alias and qualified paths retain the same definition.

The compiler facet is controlled input in these checks. Automatic real compiler
facet capture, completed namespace seals, token operand parsing, nested compile-time
evaluation, source capture and guest integration remain pending. This does not
complete M011, MX021–MX024, release acceptance or Linux support.
