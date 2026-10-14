# E-M015-008: original and generated compiler Test target eligibility

Task M015, revision 4 section 50.4: a required transformation or invalid compiler
Test target must fail discovery before catalog sealing or filtering. Sections
25.2/50.4 still defer ordinary signature and body preparation.

The official test-on-not-fn and test-attr-non-associated-functions sources at
rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18 expose three missing target paths:

- Original module/constant Test annotations were delegated to an ordinary
  outline owner that had no annotation producer. A minimal `#[test] mod test {}`
  silently sealed an empty catalog. These annotations must keep cold worklist
  ownership unless an enclosing ordinary transformation owns the complete item.
- Generated module/constant Test annotations returned before compiler facet
  resolution because their header kind was not Function. They then waited on
  their own unresolved header. Resolve the actual facet and complete inert
  prefix first, then reject non-function targets.
- A generated associated-method Test resolved the actual compiler binding and
  matched its exact hygiene/unit context, but the declaration-fragment path
  retired the annotation without checking its attachment. Check the exact
  captured AST target before inert retirement, then retain its primary error
  through both original-item and nested-fragment polling paths.

Minimal generated witness: `macro_rules! emit { () => { struct A;
impl A { #[test] fn recovery_witness() {} } }; } emit!(); #[test] fn known() {}`.
The failing lookup witness proves a Defined compiler binding and a matching unit.
The proposed mixed path-span-context explanation was not reproduced: both the
whole table and actually referenced spans had one identical context class. No
hygiene lookup or namespace-frontier change was made based on that hypothesis.

Each resolved invalid target returns LRR-E-TEST-DISCOVERY with the primary
message `compiler Test attribute requires a free function`. Repeated polling,
publication and catalog access preserve that diagnostic, do not mutate the
database revision, cannot publish a seal, and prepare no signatures, bodies or
thunks. Disabled targets remain excluded. Ordinary replacing attributes retain
ownership of their complete sequence; a procedural attribute named test must
follow its separately authorized selection path.

The copied official source, directives and stderr remain unchanged under the
original rustc licenses. Harness adaptations stay outside that directory.
Original and generated target controls test typed LRR refusals, not rustc stderr
parity or guest execution. The historical harness field `component_sha256` is
an LRR domain-separated Digest of source bytes, not raw SHA-256; authoritative
file/provenance/source-snapshot hashes use raw SHA-256.

Private joint acceptance: artifacts/macos-first/m015-required-admission-repair-generated-associated-joint-v1-acceptance.json.
Integrated macOS milestone passed2014host/doc checks with3497fixedinputs and
unchangedmembership/all9checks0: artifacts/macos-first/m015-generated-target-main-acceptance.json. M015, full MX031/MX032, guest JIT, release and
Linux acceptance remain unfinished. Full stock fixture passes remain zero.
