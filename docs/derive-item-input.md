# M010 derive item input

The pinned Rust 1.99.0 macOS probe shows that derive input contains the retained
annotated item, `repr` and registered helper attributes, with all `derive`, `cfg`
and `cfg_attr` wrappers removed. The initial failed assumption that derive input
retained the derive attribute is preserved in
`artifacts/macos-first/m010-derive-input-native-initial-witness.log`.
This is a native convention check, not a specification contradiction.

`expand::attribute_materialize::derive_input` reconstructs effective outer
attributes from original token subtrees, removes every derive attribute, and
retains the item body and original span/context records. It performs no macro
execution or file reads. Inactive and stale source facts refuse. The native
acceptance uses two derives, including one introduced by cfg_attr, and compares
an actual matched-server expansion's observed attribute count with a standalone
native Rust execution. The macro returns a new constant while its input retains
the annotated struct. Unit acceptance checks all derive removal and record
retention.

Sequential transformation scheduling, actual append publication, inner field
cfg processing, automatic helper metadata and full MX018–MX020 remain pending.
These checks establish the outer item-input convention only.
