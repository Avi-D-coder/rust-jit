# E-M015-015: builtin-inert test flags require Attribute-facet resolution

M015/revision4 sections49/50.4 require resolved macro kinds and final test
metadata before TestCatalogSeal. An unqualified builtin-inert spelling cannot be
certified by spelling alone when a captured procedural Attribute has the same
name. A same-named FunctionLike export remains a valid builtin-inert contrast.

The unchanged rustc ambiguous-builtin-attrs fixture retains its original2018
compile-fail source, auxiliary and expected stderr. It diagnoses competing builtin
and attribute names; pinned rustc_resolve/src/ident.rs classifies this as BuiltinAttr
ambiguity. Those original full fixture assertions were not executed here.
An LRR supplement uses its exact fn test tokens, fixed2024/testcfg, and the
unchanged test-macros.rs identity_attr auxiliary, legitimately imported as ignore
or should_panic. Both actual original controls captured the Attribute export,
then incorrectly published a sealed test catalog with ignored=true or
should_panic=true, zero callbacks and no signature/body/thunk queries. Strict
required-ambiguity assertions failed. Actual FunctionLike identity aliases retained
exact original identities and correct flags and passed. No selected live consumer
was compiled. Worker V2/V3 immutable archive/member hashes and scope are retained
in artifacts/macos-first/m015-proc-inert-attribute-ambiguity-v2-witness-worker-retention.json
and the corresponding V3 record. Generated V3 controls reached an unhandled owned
attribute continuation; no generated sealed catalog is inferred. The corrected V6
observer proves actual generated Attribute requests instead of ambiguity, including
one earlier legitimate decorator callback and accepted record before the missed
later conflict. V7 independently proves both cold Statements requests instead of
ambiguity. Neither generated nor cold controls sealed a catalog; all strict
expectations failed and their verified archives are retained.

The parent pure metadata witness independently failed the original compiler Test
proof assertion on unchanged production. Its FunctionLike contrast passed:
artifacts/macos-first/m015-required-admission-inert-attribute-binding-v1-witness.json.
This pure metadata witness is separate from actual matched-server evidence.

The bounded production correction resolves the competing Attribute facet without
executing it. A captured Attribute yields LRR-E-MACRO-AMBIGUOUS, pending membership
cannot certify a negative binding, and a non-Attribute binding remains allowed.
Checked stable proof encoding retains exact negative candidate identities for
original and generated compiler annotation replay. A generated earlier replacing
attribute must finish before a later inert facet is resolved; it may remove the
later annotation. Qualified paths do not receive builtin spelling authority.

Private compatibility passed 200 library, 61 source-discovery, 67 declaration-scan
and eight catalog tests on current production (session 65109). The independent
parent actual-server run passed one source and all nine native controls (session
61658), agreeing with worker V8. Seven Attribute conflicts refuse without a seal;
two FunctionLike contrasts retain final metadata. Only the legitimate prefix
callback executes, once; no body queries or live consumer compilation occur.
Receipts: artifacts/macos-first/m015-required-admission-inert-attribute-binding-repair-v4-acceptance.json
and artifacts/macos-first/m015-required-admission-inert-attribute-native-joint-v1-acceptance.json.
The repair is integrated; the fresh main milestone remains pending. The first private repair phase passed four
focused and200library controls but its driver named source_discovery in lrr rather
than its actual xtask package; the driver failure and unexecuted later checks are
preserved. It is not a production failure or a successful complete run. FullM015,
MX031/MX032, stock fixtures, guest JIT, release and actual Linux acceptance remain
incomplete. No native runtime authority or query-provider execution is added.

The source/declaration selector repair also validates and retains negative inert
facets in original cold and generated declaration retirement, suspending on
unresolved membership and replaying the exact fingerprint at seal. The first
version incorrectly demanded a namespace during compiler-only scheduling:
66 declaration-scan controls passed and the unchanged module-frontier control
failed. The correction preserves that scheduling result and its still-pending
annotation frontier when no procedural catalog is configured. Such a scheduling
proof cannot be reused as a procedural namespace certificate. The failed phase
and unexecuted later checks remain preserved in the V3 partial-failure receipt.
