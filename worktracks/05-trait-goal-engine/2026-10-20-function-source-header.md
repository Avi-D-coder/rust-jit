# M020 common function source header retention

## Bounded architecture record before edits

The unchanged repr128 source-factory coverage witness needs the genuine complete
PartialEq catalog, including facade/core/ptr.rs::PartialEq for fn(). Its head is
currently held by SourceTerms::Encoder::ty because owned FunctionSignature erases
ABI/safety. That refusal is correct for unknown runtime terms, but common source
preparation must retain actual qualifiers rather than guess safe Rust. This is
source metadata retention under Section53/E-R5-014, not trait solving, static ABI
checking, root activation or runtime acceptance.

Extend traits/term.rs::FunctionSignature with optional actual SourceFunctionHeader
(ABI string, unsafe bit and C-variadic bit). None means unknown legacy metadata,
including older serialized values. Preserve it through traits/subst.rs; keep
pointer_terms::fn_pointer the legacy runtime constructor with None. Extend only
the actual frontend/source_types.rs FnPtr walker arm and its common syntax capture
helper. The existing Binder remains authoritative; unknown higher-ranked binders
keep source metadata unknown. The separate symbolic_headers constructor requires
only a mechanical None field, pending parent ownership confirmation.

Both original and admitted generated syntax already enter resolve_actual_type;
this change captures their actual AST header once alongside existing recursive
parameter/result preparation. Alias paths that retain only legacy erased terms
must remain unknown; neither function names nor fixture identity select behavior.
No runtime unsafe/ABI restriction is added: E-R5-008 pointer family relaxation and
guest descriptors still use their existing runtime semantics. Exact upstream
encoding remains the other worker's SourceTerms function arm responsibility.

Dependencies/currentness are unchanged owning source/type query receipts; the
header becomes part of the immutable term's stable encoding (domain bump) and
serde data. Cold work is linear in the actual function signature syntax already
walked; hot substitution copies only the small header and ABI string. No parser,
whole-world scan, per-dispatch source planning, executable cold-body preparation,
pattern machinery or alternate evaluator is introduced. No obsolete evaluator is
removed by this bounded metadata prerequisite.

Acceptance requires source review of every FunctionSignature constructor and
substitution, true direct Rust/extern/unsafe/variadic values, truthful unknown
binder/legacy/alias cases, then parent-owned direct compilation and actual full
bucket control. All eight recurring architecture failures are reviewed against
these shared symbols; no context-only inference/member path, generated exclusion,
body copy, global invalidation or new solver is authorized. No tests/Cargo/Git by
this worker; source-only retention cannot close M020 or repr128 runtime gates.

## Implemented source-only handoff

Approved API is `term::SourceFunctionHeader { abi: String, is_unsafe: bool,
c_variadic: bool }` and `FunctionSignature::source_header: Option<_>`. Missing
serde fields default to None, never safe Rust. Stable encoding now uses
`lrr/v3/symbolic-function-signature` and encodes the optional actual header;
existing checked storage measurement includes its ABI allocation. No new resource
limit was added. Ordinary substitution preserves the header verbatim.

`frontend::source_types::source_function_header` is the one pure capture helper:
no extern means actual Rust ABI; extern without a string means actual C ABI;
explicit strings use pinned AST String::value decoding, retaining any decoded ABI
name for upstream mapping. Unsafe and variadic bits come from actual tokens.
Unsupported async/const shape or undecodable syntax gives None. There is no
reparse of display text or ABI vocabulary whitelist.

The ordinary FnPtr walker retains Some only when its empty binder is known:
ForType parent, any nested ForType, or any reference with an omitted lifetime
anywhere in parameter/result syntax keeps None. This guard is in the actual
producer, independent of outer symbolic preflights and alias lookup. Its legacy
runtime static erasure must never certify a source FnSig. The existing
symbolic_headers::function_type already supplies genuine bound/elided-region
Binder.variables, so it shares the capture helper and retains Some there too.
The legacy pointer_terms::fn_pointer constructor remains None. Runtime pointer
family matching, descriptors and unsafe/ABI permissiveness are unchanged.

Exhaustive LRR FunctionSignature struct-literal search found four symbolic
constructors (legacy pointer constructor, symbolic_headers genuine producer,
substitution reconstruction and new ordinary FnPtr construction), all updated.
Other same-named structs are syntax::signatures::FunctionSignature, a distinct
item. Existing frontend/signature_subst mutates a cloned symbolic binder, so its
header remains retained. Source review inspected the actual source walker,
alias-normalizer join, source header capture, closed term substitution and runtime
unification: no qualifier-based runtime rejection was introduced. The parent-owned
explicit-reference preflight was preserved exactly. All eight architecture modes
remain as bounded above; this change neither adds nor removes a solver or runtime
route. Additional source adapter/query activation and tests remain parent-owned.

Source fingerprints at this handoff:

- traits/term.rs: 024795b67d82684e2c893bd3ae78e742e99d596aa0a3e32908fc3c8d4aeac850
- traits/pointer_terms.rs: c99f57fc2881c34f699d07e25827b9ec34cd0a1ed552f0744e2575eb106a2be9
- traits/subst.rs: 16970444de5bfc4a05dad31b7a75937369be90093332afba057f19b3b95f32f7
- frontend/source_types.rs: e9c2b5f30dc12b26ced8d0f2d4edc83b33f700ae22690a15685d403ca2e15fdc
- syntax/symbolic_headers.rs: cc7849fe64569529b674f01ea9311d02984da2420a6bbd5268e1420e0fb82f76

No tests, Cargo or Git commands ran. Existing check_erased_function_pending was
written for the previous loss of ABI/safety through aliases; genuine metadata
from the binder-aware alias producer may make that assertion obsolete. Test owner
must update it to verify exact qualifiers rather than preserve erasure. Direct
unsafe/extern syntax still encounters the unchanged parent-owned symbolic
preflight; lifting that old qualifier-only hold requires the actual header-aware
adapter and remains separate from this retention handoff. No bucket/root/runtime
acceptance is asserted.

### Parent pre-freeze review corrections

The preceding handoff fingerprints are superseded. Parent review required the
pure AST helper to live in the lower syntax layer, so the exact same
`source_function_header` now lives in syntax/symbolic_headers.rs and the ordinary
frontend FnPtr arm calls it. There is no syntax-to-frontend dependency. The
ordinary producer's unknown-binder guard now conservatively checks every enclosing
ForType ancestor as well as descendant ForType/omitted reference syntax, including
ForType -> ParenType -> FnPtr shapes. It still introduces no runtime rejection.

The newly added ABI-allocation accounting line was removed at parent direction
under E-R5-006; the retained string participates naturally in stable encoding,
without adding memory-accounting policy. The preceding paragraph claiming such
new accounting is historical and does not describe the corrected source. No new
cap or other resource policy exists in this change. No Cargo/tests/Git ran.

Corrected source fingerprints (remaining two files unchanged above):

- traits/term.rs: 13d65e963592e71964894f60bd7e4d0308dde34e0d74dea3f3d52b4845ec9f9a
- frontend/source_types.rs: 4fffb1dbaf67a55a7fffb7309a2d8b000baaa279c0d03fa2feb705331f821f7f
- syntax/symbolic_headers.rs: a1970832966235c2756bf863cfeb3dfee77372663261ad88e3c68d186f903afd

These corrections were re-read against the same constructor/substitution/runtime
entry points; no other metadata, solver or static/runtime checking behavior was
added. Independent reviewer was notified to review the corrected fingerprints.
