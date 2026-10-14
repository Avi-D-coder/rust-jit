# T032 qualified original-source calls — work in progress

Entered call recipes now retain bounded owned module paths, including terminal
explicit type arguments. The pinned AST distinguishes crate/self/super roots,
raw identifiers, ordinary names and unsupported generic/associated owners. Thunk
construction performs no namespace or callee-signature lookup. The dispatcher
routes qualified names through the existing module/import index and selects the
exact value-namespace function before evaluating written arguments.

Relative, crate/self/super, nested-module, module-alias and public reexport paths
are exercised. Ordinary local values do not shadow the module/type namespace;
body-local modules/types and function type parameters prevent inappropriate
module fallback. Rooted paths bypass unrelated body-local declarations. Missing
or inaccessible identities fail before argument arithmetic or call publication.
Associated owner calls and unresolved body-local call/import integration remain
explicitly deferred.

A raw-identifier regression found that canonical namespace module names were
being parsed as source spellings after losing r#. Canonical graph paths now use
the existing pinned-parser raw-name revalidation helper. Actual source call
spellings still use the strict source identifier validator. Raw keyword modules,
raw function names and Unicode NFC identity have execution controls.

Ordinary entered item declarations use the Declaration marker (stable opcode tag
24). It charges the ordinary opcode and exit fuel, produces no guest value, and
advances to the next statement without preparing item bodies or evaluating
constant/static initializers. LRIR admission requires an entered opaque item;
expression nodes cannot use this marker. Attribute-bearing declarations and macro
items retain their existing unsupported/expansion boundary. The opcode counter
inventory grows to 25 entries without renumbering existing tags.

Seven controls cover qualified execution and identity, failure ordering, generic
owner shadowing, declaration laziness, thunk purity, marker admission and actual
CLI execution. This component does not establish expanded catalog integration,
complete lexical body-local item resolution, associated/member calls, dictionaries,
full context ID tables, or native/JIT/service/release acceptance. T032 remains in
progress; Linux acceptance remains pending.
