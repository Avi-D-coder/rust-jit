# Explicit arguments retain the ordinary Value name goal

LOCAL-024, WT04, T032/T033/M017/M018; E-R5-005 MA003/MA004 and
recurring failure modes 3/4/6/7/8. Future source-only slice, separate from
frozen aacb call acceptance and the zero-argument stock successor.

Unchanged official witnesses: the already imported complete
`tests/ui/macros/macro-nested_stmt_macros.rs` has three calls to the same generic
module function with actual generated local type arguments. Pinned
`tests/ui/const-generics/issues/issue-75299.rs` has an actual local const-generic
callee `foo::<1>()` returning `[u8; N]`. Its MIR optimization directive remains
compiler evidence; the full runtime array/const contract remains a blocker.
Neither case is a passing runtime fixture for this source-only change.

Violated source invariant: `source_value_goals::prepare_query` rejects the
actual unqualified PathExpr before obtaining its NameRef merely because its
segment has explicit generic arguments. An entered Value fact selects the base
declaration; it does not substitute arguments or grant callable entry. The
ordinary source route can resolve that same actual token through its retained
source origin, owning edition, hygiene context, lexical prefix and sealed
positive/negative dependencies regardless of explicit argument syntax.

Root owns only `frontend/source_value_goals.rs`: remove that early generic-list
refusal, preserving absolute/qualified/associated path refusals and every
actual NameRef/source/edition/hygiene/currentness/publication check. Do not
resolve the name from written argument text, parse a synthetic body, infer a
runtime type, execute const arguments or compile a selected source unit.
`prepare_entered_local_function` and `EnteredCallGoal::written_target` retain the
actual selected declaration and actual written explicit syntax through the
existing common APIs. No new parser, namespace/index or evaluator is needed.
The former context-only refusal is superseded only for this metadata fact.
Argument/type/const/substitution/contract checking remains at its ordinary
reached boundary; qualified and associated member joins stay pending.

Worker owns only its new controls and the existing entered-call control whose
old oracle expected every generic path to be Pending: actual Module selection
must now be distinguished from nonpath/function-value Pending. Root will
register a new control module separately; no imported original is modified.

Cold cost: the same ordinary source plan and bounded NameRef/prefix goal; no
argument/body executable preparation. Memo39 retains facts and dependencies;
normal execution cannot re-enter this source provider. No added global census,
BodyIndex clone or whole-file phase search. Full typed argument resolution,
source-aware generic result hygiene, const/array runtime execution and complete
runtime matrix remain explicit prerequisites. Closure requires actual source
controls, relevant invalidation, normal-call contracts and unchanged whole
fixture execution, followed by removal review and the current milestone.

Runtime safety hold before combining these future source facts: source-name
admission must not bypass the old local-Type safety refusal and then resolve
explicit arguments through module `SignatureNames` text. Minimal counterexample
is a local generic function called with `::<u8>` where a body-local alias named
`u8` has another meaning. Until the common indexed Type/const-argument service
joins, normal Local source call lowering and reached selection explicitly refuse
nonempty explicit arguments before argument execution/body publication. This
preserves the previously unsupported local-generic runtime boundary across all
origins. It does not narrow a formerly accepted shared runtime feature or confer
permission to keep the text resolver as the new mechanism. Module legacy routes
remain held under their existing checks; the new name facts are not Type authority.
