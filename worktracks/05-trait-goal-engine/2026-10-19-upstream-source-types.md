# M020 — demanded owned source terms to actual shared IR

Solo /root LOCAL-049; baseline35b509c7e0a732127218a33be8e343a6b23d126a.
Invariant: construct shared rustc_type_ir terms from actual ordinary source query
facts, with source/world/generic/binder identities preserved. Never fabricate a
source provider, InferVid, placeholder universe, trait/ADT ID or unsupported-field
answer to make a solver demo pass. Representation construction is not solving.

Prerequisite exposed by actual Context/PreparedProvider constructors: solver
settings are still caller-written fields. Add source/cfg-bound crate configuration
through the existing ParseFile/query framework. Read actual root attributes using
the shared cfg/cfg_attr preprocessing and pinned AST literal APIs. Honor pinned
rustc_interface/src/limits.rs:33-41 recursion_limit (default128, source override).
Capture all written enabled feature names, including unknown ones without static
feature rejection. For current standalone catalogs, use pinned compiler defaults
for the two unsupported -Z flags (rustc_session/options.rs2401,2832), both false;
future Cargo/flag profile support is not inferred. No new LRR resource limits.

The private actual configured catalog supplies its real root SourceSelection;
never pick the first source/file. Config/currentness must observe CapturedSource,
CrateCfg and ParseFile. A scoped context uses the actual PreparedProvider, with
unpublished callbacks/facts still unavailable until the real factory/inference
closure is complete. No dummy SourceFacts implementation or empty callback bundle.

Source terms then map into existing Context constructors. Bind impl source parameter
keys through actual ImplHeaderSyntax/SourceParameter::from_impl (function parent/late layouts remain separate) and real nominal IDs
through canonical source metadata; immutable lookup maps are prepared once.
Preserve scalar widths, tuple/array/slice/reference/function shape and binder
indices. Inference, placeholders, dyn/opaque/projection and unresolved source
metadata must retain explicit preparation gaps until genuine upstream client
bindings/source recipes exist; never erase them to a convenient concrete type.
Do not close M020 or representation integration while such variants remain gaps.

Extend shared syntax/query/type bridges, not another parser/evaluator. Original,
generated and macro wrapper declarations use ordinary source/currentness proofs.
No method/macro spelling selects representation rules. Repeated execution must
not revisit source preparation, full namespace comparisons or deep BodyIndex
copies. Cold config and binding capture is memoized/source-dependent; actual
shared IR allocation occurs only within the retained scoped context.

Witnesses: unchanged rustc source trait/head and generic function/method fixtures,
all editions; actual emitted shared kinds, canonical/source identities and parent
parameter ordering. Use unchanged upstream root feature/limit attributes as data
witnesses and supplemental cfg/currentness checks outside originals. No native
rustc validation counts as interpreter execution. Review all8recurring failure
modes plus MA004/MA007/MA008 context/pipeline/catalog/index obligations. Keep actual
context term construction, fully ready source facts, true upstream evaluation and
stock runtime/full-fixture outcomes separate. No JIT work or performance claim.

Source review found an insufficient representation: TyTerm::Function intentionally
erases ABI/safety for the interpreter pointer family (E-R5-008), while an alias can
hide unsafe/extern qualifiers from the impl header's direct symbolic_type_pending
scan. The new upstream bridge must not guess safe Rust ABI. Remove its guessed fn
constructor and explicitly hold FunctionMetadata until actual source signature /
alias provenance is carried through the common Type service. The unchanged pinned
lint/fn-ptr-comparisons-some.rs supplies an unsafe extern C alias witness; copied
original and warnings remain unchanged/licensed and pending. An outside-original
control appends only an own trait impl for the actual alias to exercise the shared
source query. No existing runtime function-pointer behavior changes. All function
representation/ABI/binder acceptance remains open; no full-encoder completion.
