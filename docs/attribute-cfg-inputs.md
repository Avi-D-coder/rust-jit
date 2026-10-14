# M010 cfg and ordered attribute inputs

ParsedFile::attribute_inputs_at selects one exact original-source item and returns owned active attribute facts with original meta ranges, token recipes, ParseKey and explicit CfgSet identity. It performs no macro lookup, native execution, body preparation or filesystem access. Token recipes are structural metadata; procedural token input must still use the lossless token/span bridge.

The shared pinned-AST preprocessing walks attributes in lexical order. Active cfg_attr inserts child metas in that position, including nested cfg_attr. Inactive cfg returns no attributes for the item, so an earlier transformation candidate cannot schedule native work for a removed item. Traversal has checked inventory, nesting and owned metadata limits. Original outline consumers use the same preprocessing.

The native compiler acceptance demonstrates why order matters: cfg(any()) suppresses a later invalid not predicate, unknown macro attribute and broken body; placing that invalid predicate first fails. Nested cfg_attr acceptance verifies exact source ranges and ordering with explicit captured cfg identity, without resolving an unentered invalid body.

This is the first M010 substrate. Transformation dispatch/replacement, derive append, helper visibility and full MX018–MX020 remain pending. No generated catalog, guest execution, release or Linux acceptance is claimed.
