# E-M007-007: include a transforming scope's opening frontier

This is an implementation correction, preserving revision 4's generated-source
completeness requirement. An attribute on a module begins at the same source
position as that module's scope opening. Capturing only positions strictly
before the opening excluded its transformation demand, allowing lookup in its
unexpanded original body to return a definition.

The minimal witness is an original `choose` macro inside `#[transform] mod
altered`. `source_index_waits_for_transforming_scope_attributes_and_derive_output`
failed before the fix. Its output is preserved in
`artifacts/macos-first/m007-source-index-before-transform-fix.log`.

Source-index child visibility now includes the opening item's frontier through
checked position addition. Later declarations remain excluded. Attribute and
derive generation stays typed input demand and executes no macro here. Outlined
function bodies remain opaque. The source index additionally marks an external
module as ModuleSource demand until its selected file is loaded, rather than
publishing an empty textual scope.
