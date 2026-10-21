# M019/M020 / LOCAL-049: explicit source regions in the common Type walk

Before code: V527's unchanged repr128 source factory reaches real publication,
but its complete PartialEq bucket remains Unprepared (coverage0 vs required1).
The actual facade bucket contains explicit `impl<'a,'b,A,B>` reference heads.
`frontend::source_types::symbolic_type_pending` rejects every RefType before
`impl_head_types::PreparedImplHeadTypes::prepare` can call the ordinary Type walk.
This is a source-confirmed blocker for those actual heads, not yet proof that
they are the only missing heads. Keep the complete coverage assertion and print
actual encoding diagnostics before it; never delete candidates to pass it.

Shared route: retained impl syntax/canonical declaration context ->
WrittenTypeDeclaration::implementation -> symbolic_type_pending ->
resolve_actual_type::walk -> WrittenTypeDeclaration::region -> actual ParamKey
-> SourceBindings/SourceGenericLayouts -> source_terms::Encoder::region.
The latter encodes real early-region parameters through prepared source layouts.
The same common preflight also serves declaration/impl predicates, const generic
Types and source generic defaults; no call-site-specific region parser is added.
Original/generated declarations use the same owning source metadata/hygiene.

Allow a reference with an explicit lifetime to reach that existing resolver.
Unknown names still fail the actual declaration lookup; '_' remains a source
Local region and is held by the existing IR encoder. Omitted reference lifetimes
stay pending: the runtime-oriented legacy walk otherwise defaults them to static,
which is not a valid compiler declaration-region fact. Lifetime binder/declaration
syntax outside RefType/LifetimeArg stays pending. Unsafe/ABI function-pointer,
dyn and opaque identity exclusions remain until their real producers exist.

This changes one retained metadata preflight, not a solver, inference algorithm,
runtime expression/pattern engine, macro policy, static checker or borrow check.
No parsing or namespace census is added in execution. Type-plan memoization and
dependency guards remain unchanged. Cost is the existing cold syntax traversal;
body storage remains Arc shared. No obsolete alternate executor is added or
expanded. Full clause generation/root/caller/VM migration remains held.

Acceptance requires direct macOS builds and the unchanged full-bucket source
factory control with actual diagnostics. Runtime repr128 and all edition/origin,
elision/binder/epoch/cleanup controls remain unexecuted until measured. This is
not a complete reference feature or root/interpreter/JIT gate claim.

Before V528 capture, independent review found the now-superseded unsafe/ABI
function-pointer preflight still rejects genuine source qualifiers before the
new shared SourceFunctionHeader producer and exact FnPtr encoder can use them.
Remove only that old preflight exclusion: dyn/opaque identity, omitted regions
and unrepresented binders remain held. Exact Encoder::ty requires actual Some
metadata and rejects unknown/invalid ABI information as FunctionMetadata; the
runtime-erased representation is never guessed to be safe Rust. Strengthen the
existing unchanged unsafe-C alias control to require its positive exact encoding,
then separately strip its source header and require the ordinary encoder's hold.
This replaces an obsolete refusal, not a new runtime policy or ABI checker.
