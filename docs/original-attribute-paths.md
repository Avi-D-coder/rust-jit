# M010 original attribute candidate paths

`ParsedFile::attribute_path_at` selects one exact effective meta range in the
pinned AST. It emits owned source/parse identity, raw name segments, explicit
crate/self/super qualifiers, absolute-path flag and enclosing module suffix.
Function item attributes do not enter the function body. Attributes on nested
items or associated members carry explicit body/container entry requirements.
Selected parse errors, partial ranges, generic/invalid paths and excessive depth
or metadata refuse. Bare-name attributes correctly select their token's parent
meta node; source path and macro-call extraction share the existing pinned walker.

`SourceMacroIndex::lookup_attribute_candidate` checks original source identity,
crate, module and entry context before using the existing namespace segment router.
Attributes use the path namespace rather than macro_rules textual invocation
lookup. Dependency renames, module aliases and visibility use the same source
route machinery. This returns a candidate result: namespace seals and pending
expansion frontiers remain the query-provider's duty. An absent candidate does not
certify a complete namespace or an inert attribute.

The existing five-case native builtin/procedural alias acceptance now obtains its
positive attribute targets from original AST metas, replacing hand-supplied path
vectors. Exact builtin/procedural classification and resolved derive preparation
continue to agree with native compiler discovery/recursion evidence. Foreign
source and crate/module scopes refuse. A separate acceptance covers cfg_attr
nested metas, absolute/raw names, self/super qualifiers, cold function headers,
associated member requirements, nested body requirements and bare-name selection.
No macro executes during path extraction or candidate lookup.

Automatic item query/worklist orchestration, entered attribute namespace binding,
real compiler/sysroot facets, expansion-frontier sealing, generated-source
reconstruction, test discovery and guest execution remain pending. This component
does not complete M010, full MX018–MX020, release acceptance or Linux support.
