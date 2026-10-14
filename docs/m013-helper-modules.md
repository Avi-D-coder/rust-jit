# M013 helper environments in generated modules and attribute chains

The generated-module control forwards the original tuple struct through an
attribute expansion into a module, preserving its token spans and outer/field
helper attributes. Five native invocations produce two derive outputs; discovery
resolves the constructor reexport and derived constant before sealing and
publication. The chained-attribute control performs another identity attribute
expansion, preserving the same helpers and member cfg payload through six native
invocations. These exercise the attribute-ancestor helper rule already grounded
in the pinned rustc resolver and unchanged copied shadowing fixture.

Before cloning inherited environments, production now checks their aggregate
retained byte estimate, introduced-occurrence storage and environment count.
The same checked calculation feeds discovery metadata accounting. The regression
refuses arithmetic overflow, excessive aggregate retention and more than 4096
environments, with `LRR-E-DISCOVERY-LIMIT`.

These are LRR-specific native integration and resource controls. They do not
constitute full acceptance of any imported rustc test. The licensed copies and
their original directives, auxiliary crates and expected outputs remain unchanged.
Original-source derives, builtin integration, legacy helper ordering, complete
transformation/import/resource matrices and full fixture acceptance remain
pending. M013, release and Linux acceptance remain incomplete.

Validation evidence is recorded in
[the macOS checkpoint](../artifacts/macos-first/m013-helper-modules-checkpoint.json).
