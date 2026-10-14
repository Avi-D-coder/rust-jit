# M007 linked external-module invocation selection

Linked module item-position and entered-body invocation entry points now validate
the entire retained source selection chain before reading the selected call.
The source/body range, cfg eligibility, attribute demand and namespace routing
guards are shared with original source and entered-frame selection. Their
textual resolver uses the existing captured outer module chain, preserving parent
visibility at module declarations rather than using later parent definitions.

Item calls derive their enclosing module from the retained file root and owned
AST module suffix. Entered calls retain the exact BodyKey and reject calls outside
that frame. Unqualified selection joins body-local, containing-module and captured
ancestor textual scopes. Namespace fallback still uses the supplied graph and
canonical containing module. Shared helpers do not cache results or run macros.

One source acceptance checks item/body selection, local shadowing, captured
parent order, missing/changed ancestors and outside-frame calls. The existing
actual three-file macOS native fixture now selects all five invocation paths
through these APIs before pinned expansion of the selected original rules from
full original token forests. Original source unit/content/offsets remain intact.

Body-local import namespaces, source local-binding ribs, generated source and
attribute publication, module path discovery/seals, guest VM and M012 aggregate
accounting remain pending. Supplied file selections and native oracle execution
do not establish full runtime or release acceptance.
