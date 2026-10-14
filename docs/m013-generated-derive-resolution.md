# M013 generated derive annotation resolution

Revision 4 Sections 49–51 and Section 57 milestone C; macOS ARM64 first,
Rust 1.99.0. M013 stays in progress. This component connects generated derive
facts to the existing macro namespace and compiler-owned attribute catalog.

The caller supplies its captured logical scope, current hygienic namespace and
compiler facet. The adapter extracts original annotation paths through the pinned
syntax map; owned runtime forests cross into the frontend. Path name contexts are
normalized with retained hygiene facts and lookup uses the actual token edition.
Mixed path contexts stay a typed demand. A namespace-selected definition must
match the compiler catalog's complete identity/provenance and Derive action before
its index can enter the output/header/cfg/epoch-bound annotation proof.
An alias preserves that definition. A user definition with the same spelling
remains an ordinary candidate. The selected proof feeds the existing derive plan.

Candidates retain Defined, Ambiguous, Absent, Inaccessible and NeedInput outcomes.
Absence is unresolved data, never a completeness certificate. Unknown helper
candidates can only be classified after their item-local declarations are known;
an ordinary namespace ambiguity is therefore retained until helper precedence can
be applied. This resolver does not claim to implement that later classification.
Retained namespace outcomes and certificate storage have checked metadata bounds.

Replay validates exact positive compiler definitions and annotation membership.
Previously selected annotations that become ambiguous refuse with the macro
ambiguity diagnostic; missing/pending choices remain catalog-incomplete; changed
definitions, compiler provenance or annotation membership refuse stale input.
Replay validates annotation choices only. It cannot invoke a helper, append a
derive output or certify a source/impl/test seal.

The six unchanged checkout-derived cases retain all original licensing/provenance
in [the import directory](../fixtures/upstream/rustc-checkout/README.md). The new
attribute-after-derive case keeps a known replacing attribute unresolved in both
orders relative to the selected derive, with its disabled field still disabled.
The copied derive-order and configured-helper sources exercise namespace-selected
plans and pending helper candidates. LRR-specific alias/shadow and replay tests
supplement the upstream language cases. Full compiletest/native/helper/runtime
acceptance for these six copied cases remains pending; expand-to-derive is unexercised.

An initial compile failure exposed a private MacroPath field; its public accessor
was used. The architecture audit then rejected direct frontend access to the macro
adapter. Extraction/codec conversion moved into the existing expansion boundary,
which returns owned runtime records. The failed logs remain evidence; the audit
policy was not relaxed. Final results belong to the checkpoint.

Next integration remains the production discovery suspension/coordinator flow,
compiler facet installation in captured source contexts, exact derive export and
helper declaration demands, effective ADT/member input, authorized exact-once
execution, ordered appended output and final replay/publication. Complete
transformation/world/edition/Cargo/CLI/service fixtures, M014–M016, traits/JIT/native/
security/performance, release and Linux acceptance remain mandatory.
