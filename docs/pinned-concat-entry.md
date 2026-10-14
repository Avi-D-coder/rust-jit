# M011 pinned literal concat entry

The adapter calls the pinned syntax-only concat calculation with schema-checked
owned tokens and a registered caller span. It returns an owned string or a typed
compile-input error. Input allocation and returned string limits are explicit;
the output limit is checked after the pinned calculation. It does not create a HIR
call, enter a SourceDatabase, execute a macro helper or read the filesystem.

The runtime selects only a concat descriptor and retains its exact token/site
fingerprint and caller origin. Unknown sites, wrong builtin kinds and invalid or
exceeded limits refuse. Strings, a character, a boolean and a negative decimal
sample agree with actual native Rust 1.99.0; empty concat agrees as well.

The native `concat!(0x10)` comparison confirms a defect in the pinned calculation:
its result is `0x10`, while native returns `16`. E-M011-002 records that minimal
witness and the required pinned-source repair. The original witness is preserved. The current pinned numeric repair passes native
radix/separator/sign/suffix/float/u128-boundary comparisons and overflow/suffix
rejections. Complete numeric/literal compatibility is not claimed. Full literal/error
semantics, nested expansion, include paths/source authority and full M011 remain
pending.

E-M011-001's two narrow patches expose/reexport the calculation. Archive and source
inventory are unchanged; hash reconstruction and a source-proof regression verify
the original calculation body except the audited numeric branches. No frontend, query framework, backend or profile
substitution is made. Release and Linux acceptance remain unverified.

ConcatLiteralValue::expansion_record now emits the computed value through pinned
builtin quoting and the existing owned span store. Exact compiler catalog selection,
original token digest, builtin definition and expression grammar must match. The
record retains the chosen invocation span/context, and unknown span keys refuse.
String output preflights escaped text and exact schema2 wire size using a bounded
empty-token skeleton before quoting the value. Actual Unicode/control/quote/empty
native comparisons and exact byte-budget/byte-minus-one checks pass; forged
provenance and changed input keys refuse. The nested resumption regression uses
this production emitter in place of its old test-local token construction.
Environment/option/include output emission and recursive eager integration remain
pending. This does not complete all concat grammar, cancellation or guest acceptance.

ConcatArguments now plans original operands through pinned expression parsing.
Top-level comma boundaries are views over the pinned token tree. Each operand
exports only its used owned spans/contexts; literal operands are validated by the
existing pinned concat calculator and nested macro operands retain original
argument tokens/path/reference facts. The plan has checked operand count and
aggregate owned metadata; full boundary acceptance remains pending.

Source-order next_expansion identifies the first pending nested invocation without
executing it. Child identity binds parent plan and ordinal as well as path/input,
so repeated identical macro calls remain distinct. The regression covers literal
radix semantics, trailing commas, empty concat, invalid syntax/literals, qualified
unknown macros and a planned env child emitted from an exact frozen environment.
Joining adopted child output back into concat, recursive eager evaluation and full
MX021 guest behavior remain unfinished.

ConcatArguments::materialize_adopted joins only exact adopted child expansion
records. Missing selection and missing input remain distinct demands. Child keys
must match the original nested invocation/token digest, expression grammar and
consumer environment; unrelated/literal-operand selections refuse. Every child
record is observed through the normal database dependency collector before joining.
Pinned token storage preserves operand marks/source order and original registered
invocation spans; aggregate input/output metadata and selection counts are checked.

The actual macOS regression compares a frozen env child plus string/radix literals
against native concat!(env!("OUT_DIR"), ...) after coordinator reply/adoption.
Unadopted output suspends, invalid slot/environment refuse, query memos record the
child dependency and source-universe replacement invalidates success. Joining
aggregate boundaries, final eager parent-record emission, recursive scheduling and
full MX021 guest acceptance remain pending.

ConcatArguments::expand_adopted now produces the final parent record after exact
compiler definition/original-input/definition-edition/expression checks and child
adoption. The parent key retains the original unexpanded input digest; temporary
joined tokens are used only by the pinned literal calculator. Final string output
uses the original invocation span and checked wire preflight. Missing child
selection/input remain demands; wrong parent input and reduced budgets refuse.

The native regression compiles the emitted parent literal beside actual nested
concat!(env!("OUT_DIR"), ...) and asserts equality. It also adopts the parent record
through the normal coordinator and requires child input again after universe
replacement. The older joining/query-dependency check remains covered. Automatic
recursive scheduling, actual include/generated guest integration, full aggregate
limits and MX021 acceptance remain pending.
