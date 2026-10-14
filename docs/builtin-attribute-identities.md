# M010 builtin attribute identity and resolved derive annotations

The pinned adapter's `find_builtin_attr` supplies private owned descriptors for
the currently implemented `derive` and `test` actions. Descriptor keys encode the
pinned source, selected compiler metadata, name and action with explicit boundaries.
Unknown names do not become builtin descriptors; recognized handlers outside this
implementation return a typed profile diagnostic. No attribute is certified by
its call-site spelling.
The runtime expansion layer converts adapter descriptors to owned runtime names,
actions and digests. Frontend catalogs import only that owned boundary. The first
full gate rejected a direct frontend adapter dependency; the boundary fix preserves
the original layer rules and the failed gate is retained as evidence.

`BuiltinAttributeCatalog` binds descriptors to an explicitly supplied compiler-owned
defining unit and edition. Opaque namespace definitions retain that provenance.
Its module facets feed the existing source route graph; aliases preserve the
original definition. Classification resolves a pending opaque definition to a
builtin action only on an exact catalog match. Existing helper/procedural outcomes
remain intact. Modified provenance refuses. Module-prefix cloning has checked
depth and metadata admission before allocation.

`ResolvedDeriveAnnotations` records the namespace-certified derive annotation
indexes for one owner/parse/cfg/item. The caller supplies the complete resolved
annotation set after namespace classification. Duplicate/out-of-range indexes
and non-derive descriptors refuse. The resolved input assembler removes exactly
those annotations, preserves other attributes even when spelled `derive`, filters
member cfg and parses original derive argument paths through the pinned bridge.
Stable input identity includes the normalized annotation index/descriptor set.
Resolution completion order does not affect identity; stale owner/source/cfg/item
proofs cannot prepare a request.
Proofs also bind a complete preprocessing-fact fingerprint: outer/member flags,
ranges, attribute wrappers, argument ranges, names and token recipes. Matching
parse/cfg labels cannot hide a changed enabled member or reclassified attribute.
Fingerprint work is bounded by member/attribute/token limits and 32 MiB metadata
admission, and uses explicit counts, optional tags and stable token encodings.

Actual macOS Rust 1.99.0 acceptance covers builtin derive/test aliases, a qualified
derive, and procedural attributes imported as derive/test. Source routes and
identity classification agree with native retained/generated test discovery.
The procedural test alias causes native macro recursion and stays procedural in
LRR classification. Alias/qualified derive inputs retain the original item and
original path records after annotation removal. A separate mixed-annotation test
preserves an unselected user `derive` annotation and checks member filtering,
determinism, stale proof refusal and in-place fact mutations. The member-flag
mutation initially reused an old proof; its regression and failing log are retained.
Classification and preparation execute no LRR
macro. The initial native probe is retained independently.

The fixture supplies a controlled compiler-owned core facet; it does not establish
automatic sysroot/source capture or prelude injection. Full inert builtin attribute
handling, builtin derive expansion, generated impl/query integration, test discovery,
guest execution and complete MX018–MX020 remain pending. This is component evidence,
not M010 completion, release acceptance or Linux support.
