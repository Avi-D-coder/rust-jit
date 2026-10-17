# E-R5-007 — Constant and static initializers run through the ordinary interpreter

October 17, 2026. Explicit user decision (asked as a design question; the user chose "Normal interpreter (A)"), superseding the conflicting parts of spec Section 18.3.

## Witness

Upstream run-pass fixtures whose `const`/`static` initializers build nominal values, take references or call `const fn`/methods, for example `tests/ui/consts/const-big-enum.rs`, `tests/ui/array-slice-vec/check-static-slice.rs`, and the `consts/const-enum-*` and `const-int-*-rpass` families. About 47 cases stopped with `LRR-E-CONST-UNSUPPORTED: constant initializer outside the pure constant domain`. Byte-string literals (`b"abc"`, `&'static [u8; N]`) and other `'static` borrowed data have no storage that outlives frames.

## What changes

- A `const` or `static` initializer that the runtime reaches is evaluated through the ordinary pipeline: parsing, name resolution, expression lowering, method selection, the VM and drop glue. It runs as an initializer frame, once per item and instantiation, on first use. There is no second evaluator for these values. This follows AGENTS.md failure mode 1 and E-R5-005's one-pipeline rule.
- The result lives in run-long static storage owned by the run, not by any frame. `static` reads borrow or copy from that storage. Each use of a `const` behaves as rustc's: a fresh value per use, or a promoted read-only place when the use borrows it (`&CONST`, `const X: &T`). Byte-string literals and promoted `'static` data use the same storage.
- The pure constant evaluator of Section 18.3 remains only for values needed while preparing code before any frame exists: array lengths in types, enum discriminants, const generic arguments. It is not extended to nominal values, references or calls. Where a preparation-time value needs those, the gap stays an explicit unsupported result until a shared mechanism covers it.
- Programs are assumed to pass `cargo check`. Const-evaluation errors that rustc reports at compile time (a panicking initializer, a non-const operation) are not re-diagnosed. Lazy first-use evaluation is not observable for such programs, because const evaluation has no side effects.

## How to apply

Remove the `LRR-E-CONST-UNSUPPORTED` refusals for nominal/reference/call initializers by routing reached constant and static uses to the initializer-frame mechanism. Do not grow `syntax/pure_constants.rs` to cover them. Keep `recursion_limit`-style rustc limits only (E-R5-006).

## Implementation notes (October 17, 2026)

- **Route.** `RunMachine::demand_initializer` / `enter_initializer` (`engine/run_initializers.rs`) enter a reached `const`/`static` item as a zero-parameter callable built by `CallResolver::initializer` (module items from their outline row; macro-emitted items through `CallResolver::generated_callable`, the same published-inputs route `select_generated` uses). The item signature comes from the ordinary `FunctionSignature` query (`ParsedFile::initializer_signature`). Statics become run-long cells; a `const` of scalar/shared-reference type is cached once and copied per read; any other `const` is re-evaluated per use (a fresh value, as in rustc).
- **Extension.** A non-block initializer is an extending context: `temporary_extensions::annotations` seeds its root expression so `&temp` operands extend to the item's root scope. A block-bodied initializer is rooted at its block (no separate Initializer scope).
- **`'static` storage.** `Op::PromotedPlace` gives rvalue static promotion: a run-long location per `(site, type)`. It is emitted for a shared borrow of (a) a constant expression (literals, tuples, arrays, record literals of them) or (b) a temporary extended to a `const`/`static` initializer's root. `Frame::promoted_place` falls back to an ordinary temporary when `GuestStore::value_promotable` is false (drop glue, `UnsafeCell`, owning heap, `&mut`). Byte-string literals publish their `[u8; N]` once through the same storage.
- **`&CONST`.** A const path that is the operand of an explicit shared borrow takes `ItemValue::Promotable`: plain-data values get the site's promoted location; interior-mutable, dropping or `&mut`-borrowed constants keep a fresh temporary per use (witness `shared_const_borrows_promote_plain_data_only`, outputs identical to rustc 1.99.0).
- **Known gaps.** Body-local `const`/`static` items whose initializer needs the VM (they publish a header only): explicit unsupported result. A tuple/array of non-primitives promoted under a function-return `&'static` context (`fn f() -> &'static (u8, bool) { &(7, true) }`): `borrow target requires its retained symbolic descriptor binding`. `&mut` promotion in `static mut` initializers.
