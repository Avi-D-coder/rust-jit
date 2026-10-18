# Complete the wide primitive library source entries

Root, LOCAL-043, T023/T025/T032, WT04. Unchanged pinned upstream
`tests/ui/numbers-arithmetic/{i128,u128}.rs` exercise `x.clone()` and
`[1,2,3,4].iter().sum()/product()` after their formatting checks. V10 does not
reach these assertions yet; both stop at an earlier ordinary comparison contract.
Independently, source inspection proves the primitive trait and iterator source
tables omit both 128-bit Types. This is a required execution dependency, not a
claim about V10's first failure.

Pinned rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18 explicitly includes
i128/u128 in core/src/clone.rs primitive_clone (around 708–733), cmp.rs
primitive PartialEq/Eq/PartialOrd/Ord (2185/2209/2410), and
core/src/iter/traits/accum.rs integer_sum_product (47–100,206). Existing LRR
facade/core/primitive_impls.rs spells those ordinary scalar trait bodies for
narrow integers, and core/iter.rs spells value/reference Sum/Product through
the existing ordinary LrrToNum conversion trait. Add the same bodies for the
already admitted 128-bit primitive identities; do not invent a live method
intrinsic, method-name allowlist, native evaluator or assertion handling.

Actual route: captured library declaration -> normal source member/trait query
and generic constraints -> source body lowering -> ordinary VM reference read,
loop, checked addition/multiplication, return and cleanup. Generated callers use
the same service. Defaults select ordinary typed zero; Clone reads Copy bits
through the existing checked wide backing. The width does not change source
discovery/inference rules. Only facade/core/{primitive_impls,iter}.rs change.

Cold cost is additional ordinary indexed declarations/bodies, which remain lazy;
entered folds cost O(items), with the existing iterator dispatch and wide-value
storage. No full namespace scan, parser/planner in installed execution, immutable
body copy or new resource limits. All eight recurring architecture modes reviewed:
none introduced; no duplicate runtime expression/pattern evaluator is added.
Full source/macro origin/edition/currentness matrix remains unexecuted for this
delta. Sole-validator unchanged whole-program captures determine actual acceptance;
no passing Clone/fold feature or interpreter milestone is inferred from source.

## Exact source review before validation

- `crates/lrr/facade/core/primitive_impls.rs`: `75a67cd19cc8866552be0046722aedb5e945b4ca5b9d22896bc96c6795f5079b`
- `crates/lrr/facade/core/iter.rs`: `6fb464c15a64636afba9c5368fc1ee18163bcd242bf56fe4adea81edc7065b6f`

Independent source review receipt SHA-256 `ff6fefb9e9f76c7801f1ccc24e105a28058bff0e2060046a21d31e69287dc319`. No source blocker; runtime validation remains pending.
