# M010 original-source derive catalog

The pinned AST catalog visits enabled root items in source order, materializes cfg
and cfg_attr, and leaves function and macro-definition bodies opaque. It collects
static proc_macro_derive declarations without choosing functions by name. Enabled
item macro calls, unknown transforming attributes and derives that may generate
items produce pending source ranges. Unknown crate attributes suspend the entire
catalog. Disabled producers are absent. Duplicate enabled export names and bounded
count/metadata failures refuse.

`register_source_derive_catalog` requires the frozen preparation/context, a macro
host producer actually loaded for that consuming unit, Cargo's exact recorded
source path, captured bytes and producer cfg. An arbitrary captured file in the
same package is insufficient. Pending producers return NeedExpansion before any
registry mutation. Missing loaded exports return NeedSource, never an empty helper
set. A complete catalog is admitted transactionally; a failed declaration restores
the previous registry. Ready requires catalog coverage of the loaded derive names,
and every admitted declaration must match that exact artifact's inventory.

The prepared-service acceptance discovers every original derive in the actual
compiled fixture and compares the count with the matched-server inventory. A new
real preparation fixture has a static export plus a macro_rules-generated export.
It observes both names from the supplied server, returns one pending producer,
and verifies that catalog admission does not grant even the static helper before
completion. Explicit fragment admission remains a separately scoped operation;
it grants the static helper while the generated export's helpers remain unavailable.
A captured but unused source file falsely declaring the loaded static export is
rejected by both admission APIs because it is not Cargo's source root. Macro
session invocation count remains zero throughout these metadata operations.

Unit acceptance covers conditional declarations, disabled producers, cold bodies,
unknown item/crate attributes, duplicate exports and deterministic source order.
Automatic execution/adoption of producer output, includes/module source membership,
generated catalog reconstruction, full mixed-edition/generated facets and full MX018–MX020 remain
pending. This is component evidence, not release completion or Linux acceptance.
