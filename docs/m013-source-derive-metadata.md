# M013 original-source derive metadata provenance

Helper-declaration requests now distinguish a captured-source parent from an
adopted generated-output parent. The source variant binds the exact selection,
item header and producer identity. The generated variant retains its descriptor,
database adoption, owning-run output ledger and epoch checks. Both variants use
explicit stable encodings and checked metadata sizes; receipt fingerprints cannot
cross the variant, source context or occurrence boundary.

The source worklist resolves the first compiler-owned derive annotation, then
selects each procedural derive export through the captured namespace and prepared
catalog. Its helper metadata is an explicit coordinator demand. Preparing it
requires separate macro authority, the matched session, current captured source,
catalog and preparation/graph/environment/epoch. It reads the exact captured
macro-host declaration outside query providers. Generated syntax cannot be
presented as an original-source parent.

The existing licensed `derive-helper-configured.rs` input supplies the guest
shape. Adaptations outside the licensed directory alias the controlled fixture
`CheckDeriveInput` export as `Empty`, replace `cfg(true)` with `cfg(all())` and use
the fixture helper name. A second control repeats the derive path. This is
metadata/provenance testing, not execution of the original rustc auxiliary macro.
Both controls observe zero native invocations and zero accounted expansions.
Repeated occurrences have distinct demands; a receipt for the first is refused
for the second. Permission denial occurs before source-facet preparation. Metadata
adoption leaves the original producer incomplete and publication is refused.

The first control failed because original-source discovery did not emit a metadata
demand. This witness is retained. A later harness helper expectation was corrected
to select the fixture export that actually declares the helper; it was not a
compiler/backend contradiction.
The first integrated gate also caught a stale raw-parent fingerprint assertion.
Its replacement checks the exact generated descriptor and the distinct tagged
origin fingerprint; failure and repair evidence are retained.
A subsequent gate found the same stale comparison in production generated receipt
admission, rejecting twelve existing controls. Admission now requires the Generated
variant and its exact descriptor; source variants cannot enter that catalog. The
failure and targeted generated-path regression remain recorded.

Nonempty original-source execution, exact helper classification, ordered source
append integration, later transformations, full source/generated/world/resource
matrices and original rustc fixture acceptance remain pending. M013 is in progress.
This does not implement guest JIT compilation or the demand-driven trait engine.

See [the checkpoint](../artifacts/macos-first/m013-source-derive-metadata-checkpoint.json),
[architecture progress](architecture-progress.md) and
[prior direct-source empty derive evidence](m013-source-empty-derives.md).
