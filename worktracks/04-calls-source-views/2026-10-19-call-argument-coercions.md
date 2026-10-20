# T053 / M018 / M025 — reached bound-parameter coercion

Baseline c48e7a2717; LOCAL049, shared WT04 call-contract reservation. The
unchanged upstream witness is tests/ui/functions-closures/fn-item-type-coerce.rs
(imported original, all four editions). V420 executes its alias annotation then
fails at eq::<IntMap>(foo, bar), 12 dispatches, before source call entry.
Invariant: an argument coercion uses the selected parameter's actual source
binding and descriptor before source contracts compare it. A function item is
not source-equal to a function pointer; coercion changes the boundary view,
while the original item/cell/identity remains available for exact-once transfer.

Ordinary route: retained indexed call / PreparedExplicitTypeArguments(Q47) ->
PreparedBodyTypeApplication(Q73) for IntMap -> run_argument_hints::apply_context
-> reached argument source views -> call_contracts::prepare_contracts and its
SignatureSubstitution -> ArgumentPacket::seal -> GuestStore::seal_arguments ->
unsizing_coercion / pointer_coercion -> ordinary guest call frame and cleanup.
Generated source joins at the same admitted BodyIndex and call route, preserving
scope/hygiene/edition. No parser, macro identity or second call dispatcher.

Extend the existing pure runtime pointer-coercion admission into a descriptor
predicate shared by pointer_coercion and call contracts. When a callee parameter
is already bound, call contracts may present the already-selected target source
view/descriptor only if that shared physical admission allows conversion (or
existing same-pointer-family admission applies). Keep symbolic constraint and
runtime/source bind checks against that coerced view. The packet still owns the
physical conversion; no source identity equality is relaxed in Unifier and no
arbitrary nominal is admitted as a function item. No new solver/unifier,
canonicalization, static checking, source facts or native pointers.

Remove the unconditional uncoerced comparison for this bound-parameter case;
retain ordinary unbound inference, concrete parameter contracts and existing
slice/other coercions. This is a common pointer coercion join, not a new
function-name, eq, alias or fixture special case. Full trait solver integration,
legacy inference migration and other call/coercion gaps remain held.

Cold dependencies stay with Q47/Q73/current selected callee and source guards.
At the reached boundary, descriptor lookup/admission is O(1) plus the existing
symbolic checks, with no source parsing, query/namespace census, branch/callee
body preparation or immutable body copy. Physical transfer/cleanup stays in the
existing packet/store transaction. No resource accounting/caps or unsafe added.

Evidence required: exact frozen-source asynchronous macOS Rust1.99.0 builds;
unchanged original complete capture in all editions, retained pointer/value-call
and source-guard controls; arbitrary user macro and wrapper call contexts using
ordinary indexed dispatch. Review all eight recurring modes and MA003/MA006
separately against an exact fingerprint. Failed/incomplete cases remain visible;
a full original pass is one runtime-contract case, not four fixtures or a full
suite/card/milestone/interpreter/JIT closure.
