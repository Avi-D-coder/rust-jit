# T021 original called-body structural index

The pure `ParsedFile::index_body` helper receives an explicit retained body handle
and limits. `PreparedSourceView::index_body` also requires the exact source and
body selection from its immutable publication. Indexing never resolves names,
proves traits, evaluates constants, creates guest cells or emits machine code.

Owned nodes use stable preorder ordinals, body-relative source ranges, explicit
role tags, syntactic scope/label identities, direct expression operand ordinals
and logical value/place slots. Pattern locations are syntax locations, not
resolved binding declarations; parameter facts remain in the header/local index.
Scope and label ancestry is structural. Break/continue target selection is later
lowering. No rust-analyzer pointer, interner or syntax-kind number crosses the
owned index boundary.

The explicit PC phase encoding is Entry=0, Commit=1, Exit=2 and Operand(n)=3+n,
with checked u16 representability and node-specific operand bounds. Value/place
slot pairs are symbolic; cells only become guest state when later execution
commits them. Existing BodyKey/PcKey and body-relative span attachments remain
the identities used by this component.

Nested items, closure bodies, async/gen/const blocks and macro payloads are
opaque boundaries. Original macro markers remain structural metadata; they are
not completed expansion results. Final expanded-body/source-view/origin-ancestry
integration belongs to M014/M016, and must suspend on missing effectful inputs.
This original-source index cannot issue a discovery seal.

Section26's fixed 100,000-node and 200,000-slot ceilings are enforced only during
explicit body indexing. Tests may reduce budgets, never increase these ceilings.
The index additionally checks a 32 MiB metadata budget with checked counters and
fallible vector reservations. Accounting conservatively includes traversal
scratch; its reported estimate is not a performance or whole-worker memory
measurement. No node/slot budget is spent merely outlining a cold body.

Executed acceptance covers cold unresolved ordinary names/type references, deterministic
repeat keys, opaque nested/deferred payloads, source relocation, stale handles and
publication selections, scope/label/operand PCs, typed slot bounds, pattern
locations, reduced node/slot/metadata budgets, the actual 100,000-node boundary,
u16 phase boundaries and counter overflow atomicity. See the T021 ledger and
[macOS checkpoint](../artifacts/macos-first/t021-body-index-checkpoint.json).

This is component acceptance under E-57-001. LRIR lowering/verifier/frame/cleanup
integration (T022/T024/T025), guest acceptance (T034), expanded bodies/origins
(M014/M016) and full cache/regression matrices (M043) remain mandatory. MX014 and
MX029 retain pending full-LRR evidence. Diagnostic cleanup flags describe the
future reached-fault contract; these pure tests do not claim cleanup or guest
execution passed. Reproducible release/resource/performance/security acceptance
and later Linux validation remain outstanding.
