# M013 exact module capture adoption and pure input lookup

M013 remains in progress. CaptureModule is a typed input demand with an explicit
stable tag, frozen module request and compile-environment identity. The demand
coordinator checks every candidate's consuming unit and source generation plus the
compile environment. Captured-module validation checks layout, candidate request/
kind/unit bindings and the exact stable observation identity. Admission and immutable
input publication are separate steps; successful reply acceptance cannot resume a
saved phase until adoption succeeds.

ModuleRecord is a distinct immutable query input with an explicit stable encoding.
Only coordinator admission can publish it; ordinary source publication rejects record
injection. Source revision mismatch, a foreign/cancelled context, an unaccepted or
substituted reply, and conflicting content for an existing input all refuse. Publication
preflights checked metadata/source quotas and leaves the universe and phase unchanged
on refusal. Module inputs join the compiler-input stamp classes used by seal validation.

SyntaxProvider's module lookup only reads adopted input evidence, checks it against
the exact demand, and returns typed suspension if missing. It neither captures files
nor certifies a completed namespace. Missing or ambiguous source selection remains a
diagnostic on the adopted evidence. A consumer-held input lease keeps every candidate
blob in cache accounting after input replacement; dropping the lease releases that
accounting. Existing source/include accounting now enumerates all owned blobs and
continues deduplicating actual retained storage.

Ten controls verify acceptance/adoption separation and one-time phase consumption,
unit/generation/environment/producer identity, stale source and cancellation refusal,
immutable conflicts and ordinary-publication injection denial, atomic source quotas,
multiple candidate bytes and retired input leases, absence evidence, and pure lookup
after the original file disappears. Explicit coordinator revalidation still detects
the filesystem change. Native capture comparisons from the prior checkpoint remain
component evidence; no new full LRR execution claim is made.

The next required integration matches these adopted records to retained generated
module declarations, enters a fresh child scope with its parent macro context,
schedules child declarations/imports/macros through the fixed point, and discharges
the exact frontier only when complete. Actual child-function execution/native comparison
remains mandatory. The historical positive module witness retains pending status and
comparison_passed=false; record adoption alone does not resolve that witness.

Compiler/SDK attributes, procedural strings, complete Cargo/world/facet and origin/
quota/runtime/type integration, M014–M016, full MX027/MX028/JIT and release remain
required. Linux is deferred; current acceptance is macOS ARM64 Rust1.99.0.
