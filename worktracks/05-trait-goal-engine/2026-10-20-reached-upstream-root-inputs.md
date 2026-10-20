# M020/M025 reached upstream root inputs and demand boundaries

Before edits: clean baseline `1084e893d05a7020e14aa5583a030984ce365aa9`,
LOCAL-049 / WT05-02 and existing same-root VM/call interfaces. The previous
turn made progress: V508 adds one measured unchanged runtime pass and preserves
the actual repr128 failure. The active goal remains complete runtime interpreter
acceptance before JIT, not completion of this bounded component.

Unchanged witness: tests/ui/enum-discriminant/repr128.rs, first reached
discr(Signed::Zero,0). Ordinary CallSeal currently defaults U's literal to i32
before the declared projected PartialEq<U> requirement can infer it. Neither
numeric comparison, literal retagging, macro identity nor the executed body is
the solution. The actual upstream projection/trait root must constrain the
ordinary call's still-open integer variable before its existing fallback.

Lead owns source_terms.rs common encoder/root variable bridge, new
root_variables.rs and prepared.rs query admission/lifecycle after the policy
decision below. Value worker owns Q83 source_definition_semantics.rs with
independent per-query Known/NotApplicable/Pending. Default worker owns Q84
source_default_bounds, actual crate/default configuration and pinned sizedness
policy/source language roles. Validator owns separate review and asynchronous
Cargo only after global freeze. Registration/shared integration stays with lead.

Common route: admitted source/macro tree -> retained SourceLocalPlan/call
signature and Q80 actual declaration recipes -> actual reached argument source
bindings plus stable owned inference keys/kinds -> common source_terms encoder
with one scoped mapping to genuine pinned InferCtxt variables -> pinned root
evaluation -> resolve the retained original variables from that same delegate
-> owned response/current source guard -> existing VM literal bindings and call.
Do not read GoalEvaluation.goal as the inferred substitution: upstream explicitly
documents it as the pre-response-resolved input. No arena handle may escape.

The current encoder rejects TyTerm::Infer. Extend its existing recursive Type,
argument and predicate walker with an optional root-owned inference map. Keep
ordinary declaration encoding rigid and reject an absent key, mismatched context
or independent inference owner. Allocate integer/float/general variables through
the already mechanically ported InferCtxt APIs, not a new inference store,
canonicalizer or substitution algorithm. Declaration parameters remain genuine
source parameters; numeric inference variables never masquerade as parameters.

Cold source queries/plans remain memo-owned by actual source/scope/edition/epoch
and referenced namespace buckets. One attempt owns inference, arenas, canonical
and search caches, opaque/region/diagnostic state. No guest Frame/GuestStore effect
or literal mutation occurs inside the attempt. Installed execution must not
re-enter source planning. Source admission, real generic identity and complete
relevant trait bucket/negative membership validation remain required. Missing
compiler query domains cannot become false/None/empty/NoSolution. Predicates'
own/all/item/self/nonself/super/implied domains remain independently prepared.

## Actual policy conflict, user decision pending

Cargo.toml and specification Section3 set dev/release panic=abort; Section13.7
requires a release host panic to abort the worker. Upstream SourceFacts callbacks
return bare values, so they cannot directly return QueryPoll::NeedInput.
Catch_unwind does not recover under panic=abort, even when host tests unwind.
The design question is whether to permit host unwinding solely to destroy a pure
solver attempt, prepare the missing fact outside it and restart the same owned
goal, or preserve abort and complete the required query closure before entry.
Guest execution/panics stay explicit VM control in either design. The user has
been asked; do not change Cargo/spec or implement the dependent unwinding route
before an answer. Source producers and the scoped variable bridge are independent.

If disposable unwinding is authorized, catch only a private owned missing-fact
payload outside configuration::with_source_context/prepared::with_provider,
destroying the entire attempt before querying source. Resume every other panic;
never reuse an unwound InferCtxt or roll back only its normal probe snapshot.
Do not serialize borrowed TypeIR/TyVid handles or invent another canonicalizer.
No arbitrary retry cap: require actual monotone new facts, otherwise return
typed pending. Existing candidate completeness and per-published associated-leaf
ancestry validations must remain sound before removing Cartesian preflights.

Acceptance: actual source-derived repr128 predicate with a shared integer
inference variable across original/user/wrapper and all four editions; rigid
encoding refusals, missing/foreign-owner refusals, no source queries during
encoding, exact currentness/cold-source boundaries and actual inference storage
observations. Root execution, response application, source factory and final
runtime case remain separate requirements until genuinely exercised. No dummy
static WF root at main entry; no task/trait/interpreter/JIT completion from a
source/input component. Review all eight recurring modes at exact source hash,
including alternate evaluators, generated exclusions, context rules, cold arms,
replanning, namespace scans and BodyIndex copies. Historical MA001–MA008 are
separate audit identifiers and cannot be closed by this record.

## Q78 implicit-bound integration before edits

Lead also owns `frontend/declaration_source_predicates.rs` and the narrow
`frontend/source_predicates::prepare_selected` own-declaration selection.
Q78 currently calls `prepare_own`, which uses a simple Sized/relaxation filter;
it omits the pinned MetaSized default for trait Self and for `?Sized` type
parameters. Replace that own-declaration path with retained Q84 policy output,
actual own type-parameter spans and actual trait Self. Keep written source rows
and parent requirements separate. Associated output subjects remain Q82-owned.
The general legacy `prepare` path has other consumers and remains named repair
debt; do not claim it removed by this bounded own-source integration.

Q84 reads source headers/configuration, not Q78, so this does not introduce a
query cycle. Q78 retains its exact Q84 value and typed pending reason in its
stable encoding. Only declarations with own type parameters or actual trait
Self demand the default policy. A nongeneric associated function must not force
its parent's predicates or default policy. Application is cold preparation,
never an installed call operation. Existing upstream GAT/higher-ranked/repr128
source controls must retain true pending/currentness outcomes; changed clause
counts must come from actual added clauses, not adjusted fixture expectations.
No static acceptance/rejection rule or runtime side effect is introduced.

## Lead source review at V509 fingerprint

Independent reviewer and lead inspected the integrated source at
`83cf39afff5fb521a9564bafb722aa567f7fd44adfae22bbd216f90d39d9f2be`,
HEAD `1084e893d05a7020e14aa5583a030984ce365aa9`, 8709 validation inputs.
This is a source review, not a passed build or root/runtime acceptance. V509
first compile reports missing local `core` in Q82's two ordinary collector
calls; no test body has run and no repair can edit this frozen source until
the validator closes its guards. Later source fingerprints supersede build
conclusions only for the sources actually rechecked.

1. `RootVariables::{new,variable,resolved}` allocates and reads the actual
   mechanically ported InferCtxt. `source_terms::{encode_predicate,
   encode_root_predicate}` share `encode_predicate_using` and the same private
   recursive `Encoder`. No new expression/assertion evaluator is added. The
   scoped root bridge has no actual runtime caller yet; complete source-fact
   assembly, root evaluation and response export/application remain held.
2. Q78 and Q82 consume authentic source headers, owning generic layouts and
   existing source bound collectors. Q83/Q84 use existing exact AST locations,
   not macro display text, synthetic functions or scalar/break executors.
3. Root key kinds select only the real generic/integer/float InferCtxt variable
   families. Q84 mechanically retains the pinned default policy; no fixture,
   assertion or method name chooses a numeric type. Ordinary omitted trait
   arguments already use `source_types::complete_trait_arguments`; do not add
   another default-argument engine.
4. Original/generated definitions join at existing DefinitionHeaderPlan,
   Q77 member receipts and generic/source bindings with actual parent and Self
   identity. Q84 associated defaults validate that exact child and projection.
   The actual 12-world controls are still unexecuted at this fingerprint.
5. Q78 requests own type defaults/trait Self only. Nongeneric child functions
   do not demand parent predicates/defaults. Q83 distinguishes actual Known,
   invalid query domains and missing producers. No executable body lowering,
   pattern engine, unselected arm preparation or static checker is added.
6. Plans are retained in pure Q78/Q80/Q82/Q83/Q84 memos. Encoder has no Database
   and cannot query source. Q78 policy application is cold O(P*B) for P own
   type parameters and B written bound rows, including exact bound-node checks;
   it does not claim a single linear pass or measured speed. No dispatch or
   installed operation is changed to parse or reconstruct source plans.
7. Owning crate/configuration and language-role lookups happen during cold
   source preparation using existing namespace dependencies. No new full-world
   namespace census, per-operation scan or invalidation mechanism is added.
8. Actual source/header/query roots are retained with Arc. No BodyIndex copy
   or repeated whole-file node search is introduced. No new unsafe code,
   resource counter, fuel or limit is introduced.

Remaining alternate routes: the general (non-own) legacy simple-Sized source
collector and existing runtime associated-type normalizer are unchanged repair
debt. Actual compiler predicate-family completeness, negative/HRTB transport,
const/region root variables, owned inferred results, factory/root integration,
host demand handling and complete runtime/architecture matrices remain open.
The eight recurring modes above are distinct from audit IDs MA001–MA008; none
of those historical findings is closed by this component/source review.
