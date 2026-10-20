# Runtime discriminant source representation (bounded continuation)

Baseline: 7233790da9cf1023beb302177f9afb6d5f0a14e8; solo, macOS first.
Tasks: M018/M020 source/runtime joins and T040 nominal values; E-R5-012/013.
This is a held component, not completion of those cards or the solver gate.

Invariant/witness: unchanged `tests/ui/enum-discriminant/discriminant_size.rs`
assigns `discriminant_value(&E2::A)` into an i8 array. V477 reaches that
assignment but the closed helper returns isize. `repr128.rs` and the same
fixture's E4 expose full-width values. Runtime representation must come from
the actual declaration, never the surrounding assertion, destination or name.

Common route: admitted source/schema and hygiene -> SchemaNames variant binding
-> BoundNominalVariant -> GuestStore::intern_nominal_with -> TypeDescriptor.
Both handwritten and generated declarations already join this route. Extend
the bound variant with the existing declaration_repr integer and retain it
beside the descriptor's existing discriminant table. Do not demand layouts or
all fields merely to read a tag. Shared enum value decoding serves ordinary
PrimitiveCast and the source-selected LibraryFunction::DiscriminantValue;
ValueRepr and the existing wide integer arena own scalar results.

Remove the fixed-isize intrinsic producer and i64 truncation; do not add a
second table, parser, evaluator, unifier or source query at execution. Keep
ordinary E013 initializer evaluation and cached table refinement. Read through
the common cell/view and reference validation mechanisms. Pending expression
values must not silently fall back to ordinal values. The legacy mem::Discriminant
facade's fixed-isize field and incomplete associated projection integration
remain named obligations; inspect affected runtime regressions before integration.

Cold cost: existing admitted schema binding plus the shared repr header walk,
one small immutable integer identity in variant and descriptor metadata. Hot:
reference validation, selected variant/table lookup and existing scalar materialization;
no parsing, namespace census or preparation. Wide results use the existing arena.
No new resource limits or static checks. Cache identity must encode source
integer identity with explicit stable tags; repeated attachments must agree.

Evidence required: asynchronous macOS 1.99.0 builds, unchanged primary and
repr128 runtime captures, existing enum cast/initializer and mem discriminant
controls, source metadata identity/agreement checks and handwritten/user macro/
wrapper/standard macro routes across all editions where exercised. Record all
failures and unexecuted contexts. Review all eight recurring architecture modes
against the final exact source fingerprint (MA audit IDs remain separate).
Full upstream source-facts/root/projection solving and JIT remain held.
