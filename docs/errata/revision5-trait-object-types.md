# E-R5-011: trait-object (`dyn Trait`) types in declared positions

Status: implemented incrementally (October 17, 2026). Architecture record for the
change that lets `Box<dyn T>`, `&dyn T`, `Vec<Box<dyn T>>`, `Box<dyn Fn(..)>`
struct fields and reassignment of such places run.

## Invariant

A value keeps its **concrete runtime Type** (a `Box<A>` stays a `Box<A>`, a
`&A` stays a `&A`); a trait-object method call dispatches on that concrete
Type through the ordinary member service. A **declared** position that names
`dyn Trait` (a `let` annotation, a struct field, a parameter, a generic
argument such as `Vec<Box<dyn T>>`) therefore *admits* every concrete Type at
that position. The declared Type is a `TypeKey::Dyn { schema }` descriptor (an
unsized pseudo-type) and the mirror source term `TyTerm::Dyn`.

## Minimal witnesses (rustc 1.99.0 output, all four editions)

```rust
trait T { fn n(&self) -> i32; } struct A; struct B;
impl T for A { fn n(&self) -> i32 { 4 } } impl T for B { fn n(&self) -> i32 { 5 } }
fn main() {
    let mut x: Box<dyn T> = Box::new(A); x = Box::new(B);          // reassignment
    struct S { f: Box<dyn T> } let mut s = S { f: Box::new(A) }; s.f = Box::new(B);
    struct R<'a> { f: &'a dyn T } let a = A; let _r = R { f: &a };
    struct F { f: Box<dyn Fn(i32) -> i32> } let q = F { f: Box::new(|x| x + 1) };
}
```

## Shared entry points extended (no second evaluator)

- `runtime/shape.rs` `TypeKey::Dyn`; `runtime/cell.rs` `descriptor_source_term`,
  `intern_structural_term`, `find_structural_term` map it to and from `TyTerm::Dyn`.
- `ir/written_type.rs` `WrittenType::Dyn` is now an *exact* annotation: it
  materializes `TypeKey::Dyn` (`find_in`, `materialize_with`). `frontend/types.rs`
  `compound_type` states `dyn Trait` field types (previously "trait objects" were a
  refusal) and `frontend/written_terms.rs` converts closed `TyTerm::Dyn`.
- Admission: `GuestStore::contract_admits` gained `dyn_admits(declared, actual)`
  (`runtime/cell_pointers.rs`), used by storage contracts, construction pushes
  (`construction_push`, `admit_construction_move`), source-bound field projection
  (`project_bound_field`), intrinsic place assignment against the **cell's
  declared contract** (`preview_intrinsic_place_assignment`), and borrow results
  (`Frame::preflight_borrow`). `TyTerm::admits_dyn` is the term-level mirror
  (`Frame::require_borrow_source_target`).
- Unification: `traits/unify.rs` treats `Dyn` as a rigid leaf that admits any
  term; `call_contracts::closed_term` counts it closed;
  `call_contracts::bind_runtime_parameters` keeps the first binding when a
  parameter meets a dyn-admitting descriptor (`Box::new(A)` under a
  `Box<dyn T>` result context binds `T = A`).
- Source views: a place or slot whose declared view names `dyn` is read as the
  value's concrete runtime Type where a concrete view is required:
  `Frame::deref_place` (`descriptor_view`), `Frame::argument_source_view` (call
  argument sources, used by both argument-collection sites) and
  `ClosedReferenceArgument::capture_budgeted`.
- Where a declared `dyn` view meets a call boundary the value's own closed
  runtime descriptor replaces it: `Frame::argument_source_view` (both argument
  collectors), `ClosedReferenceArgument::admit` (every reference capture),
  the operator-call arguments (`engine/run_operator_calls.rs`) and
  `Frame::deref_place`. A bare `dyn` is never a generic argument:
  `bind_runtime_parameters`, the call-contract hint binding, the unifier
  (`Infer` against `Dyn`) and the borrow-target hint all leave `T` to be bound
  from its value (`Box::new(8)` under `Box<dyn Foo>`, `&5` under `&dyn Foo`).
- Callee autoderef: a `Box<F>` callee (`b(2)`, `foo()()`) is dereferenced like a
  reference (`engine/run_value_calls.rs`).

## Removed or avoided alternate routes

None added. `Dyn` filtering (`PatternNode`, hint paths) was extended, not
duplicated; there is no trait-object-only dispatcher.

## Cold and hot costs

`dyn_admits` runs only when two descriptors differ (the equality fast path is
unchanged). Views are refined per place read, constant work.

## Known gaps

- Array and `vec![]` literals of trait objects (`[&dyn T; 2] = [&A, &B]`,
  `vec![Box::new(A), Box::new(B)]`): element contracts are fixed by the first
  element.
- No `impl Trait for dyn Trait`, `dyn Any` downcasting through `Box<dyn Any>`
  type-identity checks are untested, supertrait upcasting and vtable-sensitive
  behaviour (`size_of_val(&*b)` of the unsized pointee) are not modeled.
- Trait method selection for a `dyn T` receiver follows the concrete Type's
  member probe, so an *inherent* method of the concrete type shadows the trait
  method (rustc picks the trait's).
- Not an acceptance claim: targeted tests only; see the handoff for the last
  full-suite results.
