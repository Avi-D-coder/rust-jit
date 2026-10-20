# M018/M025 — actual declaration owner Self in the common Type route

Solo root LOCAL-049; baseline8f38932a99. No agent or overlapping owner.
Primary unchanged witnesses: tests/ui/self/self-impl-2.rs (Self cannot be
shadowed by an inner Foo alias; ordinary impl/trait signature/body uses) and
tests/ui/consts/issue-58435-ice-with-assoc-const.rs (generic S<T>::id reads
Self::ID). Existing associated_constants_select_through_the_member_index fails
inside get at Self::BAR on both V409 controlled HEAD and V414. These are source
Type context failures before a valid member application, not macro-specific
semantics or an upstream solver speed issue.

Current ordinary route: actual indexed SelfType segment -> SourceLocalPlan
LocalTypePrefix -> named Type finalizer -> Q51 AssociatedItemQualifier -> shared
member/VM route. The prefix currently captures a trait's implicit Self parameter
but leaves an impl's actual applied self Type pending. Q53's existing initializer
owner template already proves actual source/header/signature/parent AST and
resolves an impl self Type over the selected declaration's inherited ParamKeys
using WrittenTypeDeclaration::function_syntax + resolve_actual_type. Extend that
single source mechanism to ordinary methods and constants. Rename its owning
query/value/module to DeclarationOwnerTemplate, retaining wire tag53 and
advancing schema domains, rather than copying an initializer-only parent walk.
The query is a declaration template, never selected impl applicability or entry.

Its source site is an actual Entry node in the retained SourceLocalPlan, rather
than requiring an EnteredValue on Type syntax. Signature/source/header/context/
cfg/parse/current environment still prove the selected declaration. Real Fn
header capture uses parsed.function_header; Const/Static retain existing actual
header exclusion. Parent AssocItemList -> Impl/Trait and actual parent header
remain required. Runtime descriptors, a guessed module or a reconstructed header
cannot provide Self. The common prefix retains applied Self terms; its named
Type finalizer returns the existing actual AliasTarget::Type representation.
Trait implicit Self remains its genuine parameter. No fabricated alias, type
parameter or declaration identity. Keep missing envelopes/argument packets
explicitly pending and preserve full generic Type/Region/Const source terms.

Cold dependency cost: one retained source query per actual site/declaration;
shared parser/AST/signature handles and actual header/cfg/epoch guards. No callee
body lowering/execution to determine Self; installed VM operations unchanged.
No new inference/canonical/cycle algorithm, resource accounting/caps, sandbox,
static checks, namespace census or immutable body clone. Legacy source_types::
impl_self_type/generated_impl_self_type/impl_owned_name synthetic-header routes
remain explicitly held for unmigrated signature clients and are not reused or
expanded here. Removing those remaining consumers needs its own complete join.
MA002/MA003/MA006 are distinct from recurring modes1-8. Required source review
must inspect both original/generated admission and ordinary Type caller sites.

Acceptance: unchanged source-driven original/emit/wrapper control over all four
editions, actual generic key ownership and target application, immutable query
reuse, source/epoch withdrawal and no executable callee preparation; existing
associated constants runtime control; unchanged whole-main runtime captures with
actual directives. Unsupported whole programs/matrix entries remain gaps; no
M018/M025/solver/interpreter/JIT or full-fixture completion follows from a source
component. Do not weaken a guard or promote legacy runtime member selection.

V415 actual evidence: both builds pass;3/4 selected controls pass. The new source
matrix observes36Self sites across24fixture/edition/origin worlds, actual method
parameter keys and source guards. Existing closed alias source matrix and the
16world trait-Self interpreter regression pass. The old associated constant
control now passes its first inherent Self::BAR and explicit trait-impl cases
in2015, then fails its provided trait default at the missing generic owner
application packet. It is still a failed full control, not a four-edition runtime
pass. Both selected whole originals remain failing all4editions: self-impl-2 at
81dispatches (closed parameter source mismatch), issue58435 at12dispatches
(callee classification). All source/HEAD/binary guards close true. V415's
encoding tag conflict is held and corrected to fresh13 in V416.

Generic applied Self terms are now retained by the named-Type finalizer; Q51's
closed_written_type representation still cannot materialize open member-owned
parameters in S<T>. The common ExplicitType descriptor/argument application
mechanism must transport those exact source parameter bindings; do not add a
Self-only descriptor builder or erase Region/Const inputs. Provided trait
initializers likewise need the actual complete source argument packet and
selected provider/currentness evidence. These remaining consumers/gates are
explicitly unimplemented, not implied by the successful source template.
