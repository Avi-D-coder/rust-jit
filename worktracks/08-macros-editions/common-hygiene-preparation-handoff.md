# Reuse pinned context facts in common source preparation

Coordinator /root, LOCAL-024, T024/T063/M014/M017; baseline fa071ee. This source
successor is separate from frozen transaction/reference packets and the four-leaf
macro occurrence prerequisite. No interpreter, task or fixture acceptance yet.

Invariant: ordinary source preparation retains actual raw token contexts and
reuses pinned immutable normalization/comparison facts. Do not normalize an
already admitted GeneratedItems context forest again for its parser, or again
for the GeneratedDeclarationMacroIndex when its parser already prepared facts.
The existing route remains ParseFile -> ordinary local/parameter lookup ->
binding_name_matches -> same_local_name_context. No name, macro identity, runtime
layout or display-text parse chooses semantics. Unchanged macro-stmt-2.rs,
macro-nested_stmt_macros.rs and existing all-edition retained lexical/origin
controls remain primary witnesses. Original files stay unchanged.

Root owns syntax/parse.rs, parse_retained.rs and lexical_hygiene.rs;
expand/source_hygiene.rs and generated_declarations.rs; the narrow generated
facts reuse call in frontend/impl_index.rs. Factor existing adapter output-fact
conversion without a new hygiene algorithm. GeneratedItems parsing reuses its
already retained Arc<HygieneFacts>. Other generated/retained conversions prepare
these facts once and retain them with the parser. Raw token keys select those
facts; the same pinned local-name classes are compared. Generated declaration
indexes extend their parent facts from this retained parser result, keeping the
existing immutable owner/currentness/edition boundaries.

Account retained fact tables under the existing parse/input limits; any shared
cross-owner charge is conservative, never omitted. Full macro definition-site
lookup still needs the actual occurrence/rib plus entered-body scope/cache join.
This refactor must not silently claim that missing join, complete frontend
preparation caching, per-call reference planning or global namespace buckets.
No installed execution work or new query/evaluator is added. Recheck all eight
architecture modes, freeze exact source and delegate actual validation to the
sole test agent. Structural reuse does not establish measured speed/scaling.

The same retained facts also replace output_facts reconstruction in the ordinary generated attribute path classifier (service/source_attribute_requests.rs). Actual path span/context and producer checks remain unchanged. Legacy runtime-macro generated-break and eager compile-input prefix normalization are named unchanged cost debt; no new semantics or promotion of those held routes.

## Separate eight-mode source review and freeze

1-3: no evaluator, literal/break output executor, macro identity, method whitelist or numeric context logic added. Actual parser/index/name services retain their source/edition semantics.

4: both generated item and retained/declaration parsers retain exact raw token contexts backed by pinned facts. Missing/cross-origin identifier attachment still refuses. Existing single-edition fallback is unchanged; no spelling-only bypass.

5: no pattern or cold executable shape changes. Existing originals and handwritten/generated/wrapper route controls remain required.

6: GeneratedItems parser shares the already admitted Arc; GeneratedDeclarationMacroIndex and generated attribute lookup use the parser retained facts instead of normalization replay. Raw token lookup uses existing sorted spans. This does not remove remaining eager compile-input prefix normalization, legacy generated-break normalization, reference header/whole pattern-plan reconstruction, or actual definition-site entered-body plan prerequisites.

7: no namespace census/dependency change, capability authority or source currentness bypass. Existing namespace cost debt remains.

8: no BodyIndex copy or complete-file lookup. Fact tables are immutable, bounded/accounted; comparisons perform two retained BTreeMap class lookups instead of two preprojected class reads. This structural tradeoff retains raw keys for the existing resolver and makes no measured performance claim.

Only targeted rustfmt and diff checks have actual exit0; no Cargo tests/native/runtime evidence. Exact packet `artifacts/macos-first/common-hygiene-preparation-v1/SOURCE.json`, SHA256 `c78029e0101f7f3c9ac860467e6087e207002b810ba4b167d099ced5d59e794d`. Existing parse/edition/retained lexical/source-attribute/generated macro scopes plus strict checks must run on frozen bytes through sole validator; do not infer new compatibility.

- `crates/lrr/src/syntax/parse.rs`: `214b369295fcffe27d5d3cded75aa48ae89f5d4ecfc3656de6a8026c9938b6f4`
- `crates/lrr/src/syntax/parse_retained.rs`: `5c44f8229aa6a526290d7dc90069c44dd3a5928649a0109d1031c706366cf310`
- `crates/lrr/src/syntax/lexical_hygiene.rs`: `2f560b17c37f1966fc0ef450b6c44eaa1f47bad138500f54d922e13b272000a7`
- `crates/lrr/src/syntax/generated_item_editions.rs`: `bda66590b45eac5221786e19159036c759d7b08e5566cc9da54fdf4b298b54f7`
- `crates/lrr/src/expand/source_hygiene.rs`: `ad504faae9276fc4b45a3bcdae5c3fd6f5532e448869aaf34cb15f63e95b2989`
- `crates/lrr/src/expand/generated_declarations.rs`: `acbb98694b7b40b193e1102a0cdd9015d7c0a676b6f6d9cc270c2ede874d49a9`
- `crates/lrr/src/frontend/impl_index.rs`: `b68da8e748bc676f1dc99af3c9c1c8bca91d241ef388d169362a453a35f2a5ea`
- `crates/lrr/src/service/source_attribute_requests.rs`: `fce85d6c9b9adfa3ef999bfa1b0c015513216112d9986b1dd13f50255aaa80f2`
