# M010 builtin derive helper facets

`lrr-expand-adapter::builtin_derives` discovers compiler-owned derive metadata
through the pinned upstream `find_builtin_derive` API. The selected stable profile
admits Copy, Clone, Default, Debug, Hash, Ord, PartialOrd, Eq and PartialEq. Known
unstable upstream derives refuse; unknown names have no builtin descriptor. This
API builds compiler-owned facets and never resolves a user call by spelling.

Descriptors have private fields and expose owned helper names plus stable definition
and engine keys. Encodings include explicit domains, field counts and byte lengths,
the pinned source commit, selected Rust/profile metadata and exact helper recipe.
The pinned Default expander consumes the `default` variant annotation; native
acceptance covers the owned helper recipe. Other admitted stable derives declare
no helper attributes.

`DeriveHelperBinding::builtin` requires the exact builtin definition identity,
engine identity, item grammar and valid non-procedural expansion key. Procedural
identities, another builtin descriptor and modified definition/engine keys refuse.
The existing item-local scope can now contain both builtin and admitted procedural
bindings. Missing declarations remain typed missing inputs, and custom attribute
classification consumes these scopes without macro execution.

The macOS Rust 1.99.0 acceptance binds all nine stable descriptors alongside an
actual source-facet-admitted procedural declaration. Five native compiler/harness
cases cover Default with other stable derives, a genuine `core::default::Default`
alias, absence/disabled Default, and a procedural derive imported as Default.
Native success/rejection agrees with helper scope and custom attribute decisions;
successful harnesses exercise the generated default variant. Metadata inspection
executes no LRR macro. The adapter acceptance checks repeatable distinct identities,
unknown names and unstable-profile refusal.

Compiler-owned builtin namespace/attribute classification, builtin expansion and
generated impl integration remain pending. The fixture does not establish automatic
standard-library alias resolution, trait behavior in the guest, generated-source
discovery, complete MX018–MX020, M010, release acceptance or Linux support.
