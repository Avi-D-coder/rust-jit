# M013 original inline caller layout

M013 remains in progress. An original inline-module macro caller now has a physical
layout derived from its actual captured syntax and cfg, including nested `path`
and `cfg_attr` attributes. The lookup starts at the selected physical source root,
walks only the requested structurally present inline chain, and refuses absent,
external or ambiguous contexts. Depth, traversal, component/name metadata and
allocation bounds are checked. It reads no files and executes no macro or tool.

The lower-level preparation API still accepts the directory supplied by an explicit
coordinator, including generated sibling contexts. Original-syntax derivation is
a separate CLI preparation path; it does not constrain that existing API. The
sibling-context regression is retained and the initial failing gate is saved.

The existing explicit coordinator retains the original file-module directory. At
request preparation the inline caller's physical components are combined with the
actual generated declaration's relative components. File-relative path attributes
stay relative to the physical source file; ordinary components stay relative to
the original file-module directory. This combination also handles an expansion
which itself contains an inline path module and an external child declaration.
The original generated declaration remains in the capture request for origin and
binding checks. Actual candidate requests and the resulting child directory enter
the existing stable request identity. No input schema or execution authority changes.

Nine new CLI controls exercise a root inline caller, a generated child path
override within that inline caller, nested inline path attributes, nonroot physical
callers with ordinary and path inline scopes, a generated inline declaration
combined with an original inline caller, cfg-selected parent path attributes,
canonical Unicode and raw names, and an escape outside the source authority. Eight successful cases compare the actual
CLI values with explicitly compiled isolated Rust1.99 macOS native oracle copies.
Competing files outside the correct directory expose incorrect fallback. The
escape control requires the source-path diagnostic and empty entry stdout.
All earlier CLI module controls remain registered. The pre-change failure of the
root inline control is preserved in `m013-module-inline-before.log`; the Unicode
control's pre-change failure is in `m013-module-inline-unicode-before.log`.

This increment does not complete M013. A later invocation inside a generated inline
scope still requires its retained generated caller layout. Nested adopted physical
file-module context, included-source layout, child compiler/body macro routing,
service coordination, namespace facets, cache/quota boundaries, full MX027/MX028
and JIT/release acceptance remain pending. Linux validation is deferred.
