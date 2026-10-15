# Byte test compilation correction

Source-only test correction after V76 terminal `774729/101`: production compilation progressed, but library-test compilation reported seven errors and ran zero bodies. Import the actual NodeRole/ExpressionKind types in two constant-publication test leaves and use stable `Box<[Op]>::as_ref()` in the byte metadata preflight control. Preserve every assertion and expected result.

The byte lexical regression references an original omitted from the prerequisite import list. Add `tests/ui/parser/raw/raw-byte-string-literals.rs` through the normal importer, including its expected stderr. Its unchanged licensed original and sidecar were added canonically in `a009b9/0`; validation must add the case independently and preserve its existing manifest. This diagnostic original provides lexical rejection component evidence, not a full diagnostic-fixture pass. No production semantics or upstream contents are changed.

After exact before/after checks, resume the interrupted strict/scoped queue. Any later overlapping assertion-test leaf must retain these exact API corrections; record its mechanically rebased hashes. All runtime and integration claims remain pending.
