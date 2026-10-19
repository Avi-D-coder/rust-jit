# Source generics / real guarded TraitRef — separate source review

Solo /root. Baseline e5d461c4302a69b92783bb86acc8e8451efdd99c.
Reviewed V327 source fingerprint:
`8c9cb91c847339e32c9050910d6f96e47e865bcb11745d98d6e3960097bad0be`.
Review is separate from terminal evidence. Full facts, inference/delegate and
actual upstream root evaluation remain unfinished. Do not promote this component
as M019/M020 or runtime/JIT acceptance.

1. No alternate expression compiler/evaluator. New DeclarationGenerics::prepare
   observes real definition-keyed source/header/context and the existing ParseFile
   query. It calls ParsedFile::declaration_generics; it does not parse display
   text or execute expressions. TraitHeader now retains that same query Arc,
   removing its independent generic capture/copy. Defaults use the existing
   source_types::resolve_actual_type occurrence walk, not a second Type evaluator,
   synthetic FunctionSignature or separate substitution traversal.
2. No restricted generated-output executor. Original and generated declarations
   use the same DefinitionSource/Header/Context products and retained AST. Real
   TraitRef::new_from_args remains the pinned upstream constructor, including its
   generic-layout guard. The source map only supplies real namespace/arity data
   before complete typed facts can freeze. No ReadyProvider token, source callback
   stub or TraitRef struct-literal escape is created. MA004's broader output
   execution obligations remain distinct and open.
3. No context-only method/numeric semantics. Actual trait/nominal definitions and
   impl source parameters retain their source IDs/world. SourceGenericLayouts
   can only collect genuine ImplSourceTypes plans from the same catalog Arc.
   Implicit trait Self and inherited/own argument ordering derive from actual
   declaration kinds/layouts, not names. Primitive widths remain ordinary source
   Type facts. Both MyTrait<usize> and MyTrait<u8> enter the same constructor with
   distinct real Args. No trait/method/macro/fixture name selects a production rule.
4. No generated-source-only exclusion. The shared default path follows retained
   TypeParam::default_type at actual TraitHeader.generic_ranges, with the trait's
   real module/header/hygiene/edition and actual subject. The same rule handles
   PartialEq's Rhs=Self and arbitrary user trait defaults. Original, macro and
   wrapper sources all use it. Standard-macro-derived headers are also present in
   the existing fixtures; this is not proof of the full standard-macro matrix.
   Missing lifetime/const/function/projection facts stay explicit gaps for every
   origin. Mixed edition, parent, replacing-attribute and $crate default matrices
   remain unexecuted.
5. No duplicate patterns or eager executable arms. Default completion visits only
   omitted parameters of a demanded trait reference. Explicit complete arguments
   do not walk defaults. Existing bound/equation consumers continue to select
   relevant predicates before resolving their arguments. This cold source path
   prepares no method body, thunk, pattern or FunctionSignature. No whole-body
   static inference, borrow checker, lint/privacy/coherence or Sized rejection is
   added. The imported #61631 compile-fail library is a source-type witness only;
   constructing its syntactic Tsized reference does not pass its E0277 test.
6. Pure memoization and retained construction. DeclarationGenerics has appended
   query/value tag69 and an explicit stable domain. TraitHeader/Declaration and
   ImplSourceTypes encodings advance domains for their changed representation.
   Repeated ordinary/header/constructor requests reuse actual query Arcs. During
   scoped IR construction there are no source/database queries. Constructor
   metadata and published typed facts are distinct phases: publish compares
   actual metadata receipts and namespace/arity before admitting typed facts.
   Full typed parameter IDs/binders/const Types and source/currentness closure
   still need factory/integration validation; this check does not replace them.
7. No full-world dispatch census. SourceGenericLayouts merges only demanded
   retained plans, with catalog identity checks. ordinary_layout performs a
   relevant ordered lookup and borrows kind slices; iterator chains allocate no
   new layout vectors during checks. Typed publication compares only supplied
   declarations to their real constructor metadata. No installed operation or
   dispatch guard changes; MA007's wider macro-catalog concern remains separate.
8. No BodyIndex deep copies or complete-file node search. Parsed/header/metadata
   inputs remain Arc handles. Generic/default node access uses existing exact
   syntax_at locations. Default argument bindings are an explicit small source
   instantiation map read at the existing parameter_type leaf; no new tree
   substitution algorithm. No new resource accounting, budgets, sandbox or local
   unsafe. MA008's historical opaque macro index/verifier obligation is not closed.

Changed shared call sites: ImplHeadTypes; source_predicates::prepare's actual
trait-bound path; source_types explicit/shorthand projections and associated
bound equations; associated_constant_applications source bounds; and
SchemaNames::source_parameter_bounds now pass their actual subject to the one
projection_trait argument/default path. WrittenTypeDeclaration::parameter_type
reads only explicit declaration-instantiation bindings; ordinary symbolic source
contexts have no such map and preserve rigid source Params. Defaults may use
actual Self or earlier provided Types through the same ordinary walk. General
const/region default arguments and arbitrary projection/default obligations remain
pending. Broader named/default/alias cases not executed by this matrix remain
unverified even if implemented branches compile. Nominal default completion is
not newly claimed by this trait-reference repair.

The actual V326 positive constructor failed on coherence-impl-in-fn's ordinary
PartialEq header because projection_trait returned zero written Args despite its
real Rhs=Self default. Preserve that failure. The repair completes source defaults;
it does not weaken the guard or add a PartialEq rule. Negative host controls
catch only deliberately malformed Region-in-Self constructor calls, after a
successful actual reference. All positive constructions are outside the catch;
V326's uncaught positive failure demonstrates errors are not swallowed as passes.

Cold cost: one retained generic capture per demanded declaration, one actual
trait query memo lookup per trait-reference preparation, and a walk of only its
omitted default Type syntax. Construction merges demanded metadata maps once,
then relevant layout lookup plus O(argument count) namespace/arity checks. No
measured speed/scaling/memory claim. Context pointers/IR handles do not escape the
existing higher-ranked scoped owner. Full callback/readiness/currentness closure
and runtime migration still need integration before any real goal may be solved.

Terminal outcomes, exact per-attempt sources and accepted execution scopes are
recorded in bounded evidence. V324 mechanical compile errors and V326's failed
positive constructor retain their own fingerprints. Neither imported sources,
ready constructors nor native rustc results count as interpreter runtime passes.

Collected V327: library14882/0,CLI45274/0,controls36380/0,12/12. Source/HEAD/test
binary guards match the reviewed fingerprint; built CLI source/input guard matches
and it was not separately run. Actual constructor matrix:140heads/136TraitRefs/
48worlds, all4editions, including original and actual macro/wrapper defaults and
two explicit argument lists for one trait. Existing source and runtime regressions
pass. Negative host constructor calls alone catch expected namespace assertion
panics. V326's failed positive reference remains7/8on its own source. No new stock
runtime/diagnostic/full-fixture total or integration acceptance.

Currentness hold: these tests exercise actual root/cfg/epoch/catalog guards. A full
per-declaration and transitive dependency/currentness closure for retained facts
and a ready solver entry is not delivered by this construction component. It must
be implemented and exercised before actual root evaluation/dispatch. Source views
and constructor data are not executable dictionaries or runtime authority.

Planning precheck reported overlapping reservations after shared source files
were added to LOCAL-049's exclusive list. The solo owner removed those duplicate
reservations and recorded use through the existing LOCAL-037/046/047 shared
interfaces instead. The actual recheck passes; no additional agent or transfer.
