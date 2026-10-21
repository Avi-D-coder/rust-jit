# M019/M020 / LOCAL-049: source impl anonymous region parameters

Before edits: V528 accounts for65 of66 actual demanded heads; one actual outer
HeadTypes hold remains. Actual alloc/string.rs contains `impl PartialEq<String>
for &str`, which the common preflight rejects. The identity of that single hold
still needs the new retained source diagnostic; this known source example already
establishes the missing general representation. Primary unchanged runtime witness
remains repr128; no change to its originals or candidate membership is permitted.

Pinned compiler source: rustc_resolve/src/late.rs::resolve_anonymous_lifetime and
create_fresh_lifetime create a distinct parameter for every anonymous lifetime
under the impl AnonymousCreateParameter rib. ast_lowering/src/item.rs::lower_generics
appends actual extra lifetimes after written params; collect/generics_of.rs groups
all early lifetimes before type/const parameters in upstream index order. This is
source binder construction, not region solving or a borrow checker. Function
pointer/for binders introduce their own scopes and must not contribute impl params.

Extend syntax/signatures.rs's existing declaration/associated generic capture,
used by ordinary and generated impl header and function metadata, with explicit
source-range provenance for these anonymous parameters. Preserve written own
source ordinals; append anonymous own params before inherited metadata. The
shared associated-owner route must inherit the same parameters. Common ordinary
Type walking uses those retained actual sites to select ParamKey; absent producers
remain held. SourceParameter transport reorders upstream early lifetimes without
changing source identities. No static diagnostic, runtime lifetime restriction,
new parser/evaluator, trait selection rule or numeric default is introduced.

Remove the blanket omitted-reference preflight only when this shared site-aware
resolver can refuse missing source metadata instead of using runtime static
erasure. Keep function elision/higher binders and unrepresented paths explicitly
pending. Source spans/hygiene/edition/epoch continue through their ordinary
retained query keys. Stable encodings must include new provenance and bump domains
for changed prepared structures. Cold syntax work is linear in actual header
Types; runtime execution performs no new traversal, body clone or namespace scan.

Acceptance: direct macOS builds, exact full-bucket factory assertion, unchanged
unsafe-C alias control and genuine upstream source-region/parameter controls.
Verify written/anonymous mixed parameter ordering, distinct repeated sites,
associated inheritance and original/user/wrapper contexts; absent/unsupported
elision metadata must not become static. Existing source, root, response and VM
joins remain held; no feature or runtime pass follows from this record alone.

Independent pre-build review identified implicit PolyTrait binders inside dyn
Types (`H<dyn Fn(&u32)>`): the inner reference belongs to the function-trait
binder, not the impl. Exclude those subtrees from anonymous impl collection and
retain a typed unproduced-impl-region flag for dyn/opaque/type-macro domains until
their binder service supplies actual identities. Propagate that flag through
associated generic metadata. No partial syntactic list can certify a complete
upstream generic layout. Explicit supported references remain on the same path.

A second review boundary is omitted lifetimes in named paths (`impl Tr for W`
where W declares a lifetime). The syntax-only list is insufficient there. Before
publishing typed generic facts, use the same source Type-name/path lookup and the
actual target DeclarationGenerics to compare provided lifetime arguments with
own declaration arity. A missing implied path-region producer stays typed pending;
never certify zero from written syntax. This check belongs to the existing
DeclarationGenericTypes memo, whose dependencies then guard header/typed-layout
publication. The syntax-only DeclarationGenerics list remains raw metadata.

The selected unchanged mixed-order witness also uses an explicit declared const
parameter as a trait argument. The shared written_generic_argument currently
interprets every ambiguous TypeArg as Type and projection_trait's default-binding
loop admits only Type/region args. Extend those existing common branches to
transport an actual declared Const ParamKey (including rebasing/substitution)
and preserve that kind in the target trait argument map. Do not infer a const's
Type, evaluate a guest expression or default an absent const value. Unrepresented
const expressions/defaults remain pending. The path-arity check uses this same
source const-parameter classification to avoid looking up an N argument as a Type.

Pre-build independent review found parent source tokens were being inspected with
an associated child's declaration frame. Corrected: only direct Impl syntax uses
its own arity walk. Every actual associated declaration first fetches its genuine
parent DeclarationGenericTypes through the existing inventory/generated-member
join and preserves its pending outcome; inherited const-Type rebasing then uses
that exact parent's raw metadata. No child identity/header is used for parent
occurrences. The complete trait-argument branch now admits genuine Const args of
matching declared kind; it does not add missing const values or evaluate defaults.

The const-parameter classifier excludes qualified and leading-absolute paths,
using the same actual segment `::` marker as common path extraction, and excludes
argument-bearing segments. A source const N must not capture Rust2015's absolute
root Type `::N`. These paths continue through ordinary source lookup.

Pre-freeze source review also requires the shared TypeIR encoder to validate
nominal/trait argument count and kind against the actual prepared generic list.
An omitted lifetime inside a fn-pointer's nominal input cannot become a valid
empty-binder FnPtr merely because the qualifier header is present. Missing source
arguments remain ParameterLayout; no hidden lifetime or solver result is guessed.

Another independent review caught Rust's TypeNS-first ambiguous generic argument
rule (pinned resolver::visit_generic_arg). Corrected through one shared helper:
a TypeArg identifier can denote a declared const only after the ordinary Type
lookup proves Absent. A present same-spelled Type wins; a blocked/unprepared lookup
is not absence. Only actual TypeArg occurrences use this disambiguation in the
arity walk; trait/self Type paths never skip their real target arity merely because
a const parameter shares their spelling. Explicit ConstArg paths use Value context.

The argument-shape check must read SourceGenericLayouts::argument_kinds through
the retained construction provider, not cx.generics_of: const parameter Type
materialization encodes Types before the generic IR table is entered. The new
provider accessor checks the actual world and returns the immutable source layout
or None; None becomes ParameterLayout. This avoids recursive/early IR table access
and never substitutes zero arguments or a caller-provided kind vector.

V529 actual lib build stopped at E0004: the new ParameterPending variant was
missing from PreparedSourceVarianceQueries' stable encoding match. No controls or
runtime ran. Add explicit nested parameter tag4 for ImplRegionBinders, preserving
existing tags0..3. This mechanical correction needs a fresh frozen build; V529's
failure and original fingerprint remain attached to its durable capsule.
