# Anonymous impl source region validation selection (unexecuted)

All originals retain rustc revision `ae0fe511b15ab760d7e963b13389d1bc2ab42b18` and the existing MIT OR Apache-2.0 license inputs. The three newly required originals were imported with the maintained command:

```sh
/opt/homebrew/bin/python3.12 tools/import_rustc_tests.py tests/ui/impl-header-lifetime-elision/inherent-impl.rs tests/ui/async-await/async-assoc-fn-anon-lifetimes.rs tests/ui/delegation/generics/infer-defaults.rs
```

The importer returned0, copied3 cases, verified7 files (including required stderr and license/provenance inputs), and updated the maintained manifest. No test or Cargo execution occurred. The first three fixtures below were already imported.

- `tests/ui/impl-header-lifetime-elision/ref-underscore.rs`: //@ run-pass; SHA256 `d1a43b900835da95c938bda546a9cbcdd1fa6c9680ff1235e019df3ac6bae9af`.
- `tests/ui/impl-header-lifetime-elision/trait-underscore.rs`: //@ run-pass; SHA256 `aa60988e2ea1649df770074e7255678735a859cd9b6d996c724f44bc625304e1`.
- `tests/ui/impl-header-lifetime-elision/explicit-and-elided-same-header.rs`: //@ run-pass; SHA256 `7b66390cde6b3efd323b4290d1ec18b09236eb0deea7d65fe1f7ee82617aeadb`.
- `tests/ui/impl-header-lifetime-elision/inherent-impl.rs`: //@ build-pass (FIXME(62277): could be check-pass?); SHA256 `45d5c311788ff37845319700fe16e3e0d3ece3c351534c062b7221ef17b4dac7`.
- `tests/ui/async-await/async-assoc-fn-anon-lifetimes.rs`: //@ check-pass, //@ edition:2018; SHA256 `b94e8dbd5bfbfb6b7f465dd91caf461fd6893c3d255cfaf6236a25c7a9564836`.
- `tests/ui/delegation/generics/infer-defaults.rs`: ; SHA256 `c787e7f477fc1ab7d281cedcd70b2f611a5c08e6434db7bb3ff11b50c5df3519`.

`ref-underscore` supplies one true omitted self-reference lifetime. `trait-underscore` explicitly documents independent trait `'_` and self-reference sites; those two sites must remain distinct. `explicit-and-elided-same-header` combines a written lifetime with an omitted trait-argument reference. `inherent-impl` supplies a true associated `fn x` under anonymous parent source generics. `async-assoc-fn-anon-lifetimes` supplies repeated `'_` parent sites and associated/nested methods; its original edition is2018. `infer-defaults` is a diagnostic fixture with `fn_delegation`, not a passing executable fixture: only its genuine first `impl<XX,Y,T,const N> Trait<'_,XX,Y,T,N> for X {}` is a source declaration-order witness. Its expected errors, delegation semantics and other declarations remain outside the proposed source component control. No convenient replacement Rust fixture is introduced.

The parent source API is pending. Before adding a selected cfg control, identify exact actual impl/method identities through the source inventory, then assert original-site identity and distinctness, written source ordinals versus genuine upstream lifetime-first IR slots, parent-prefix inheritance for associated methods, exact encoded source regions in one context, retained dependency currentness and a cold query trace. Do not substitute runtime-erased Static or anonymous display names for source sites; do not claim sameCx encoding from metadata counts. Preserve complete source candidate buckets and existing factory coverage=1.

A selected source-only control must preserve the original source text and actual owning edition; any origin wrappers or LRR assertions stay external to the original. The test should not execute original mains, native compiler diagnostics, delegation bodies or solver roots. Component success would not close runtime, full-fixture, macro-origin, interpreter or JIT acceptance. Runtime replay stays held until a real reached-call consumer exists. All source and HEAD inputs must be captured unchanged before the next explicitly frozen asynchronous validation.

The source API is now available. A cfg-only helper was appended to source_terms
without modifying the encoder: check_anonymous_impl_regions obtains the actual
local module's sole captured impl, memo-shares ImplSourceTypes, compares written
source parameters and exact header/declaration capture, checks each anonymous
site against its retained RefType/LifetimeArg node, asserts distinct site ranges,
and verifies explicit expected upstream-to-source ordinal vectors. The associated
`x` method query checks the actual inherited parent prefix and rebased owner.
SourceGenericLayouts drives ordinary same-context plan.encode; a TypeVisitor
checks every anonymous parameter survives as its exact genuine ReEarlyParam
and identity argument with no invented name. Context encoding makes no source
queries. No ready check, fake root or solver evaluation is invoked.

One cfg test was appended to existing upstream_projection_types and added to
the maintained selection. It uses exactly three unchanged owning-2015 worlds:
trait-underscore (two sites, upstream source ordinals[0,1]), inherent-impl (one
site and associated fn x), and free_to_trait's actual empty impl in infer-defaults
(written XX/Y/T/N source ordinals0..3, anonymous4, upstream[4,0,1,2,3]).
The repeated sites original and async inheritance input remain imported for
further work but the async original is unexecuted by this three-world control.
All assertions and the existing factory complete-bucket coverage=1 are retained.
The diagnostic original may expose broader source catalog admission gaps; these
must remain actual failures rather than being replaced with a convenient suite.
No Cargo/build/control execution has occurred for these additions.

V530 first execution is partial: the trait-underscore and inherent-impl worlds
complete all actual AST/site/mapping/IR/memo/cold assertions, including associated
method inheritance. The mixed infer-defaults world fails before the helper:
Rust2015 declaration parse error at353..353, expected an item (the original
fn_delegation reuse syntax). The original remains unchanged, selected and failed.
Mixed Type/Const/anonymous slot-order assertions are unexecuted. Exact terminal
evidence is [V530](../evidence/2026-10-20-v530-source-factory-controls.json).
Two source worlds passing is not a pass of the third, full fixtures, diagnostics
or runtime. No alternate mini-language witness or fixture filtering was applied.

Read-only V530 parser triage (no Cargo/source changes): the exact failed source
SHA256 remains `c787e7f477fc1ab7d281cedcd70b2f611a5c08e6434db7bb3ff11b50c5df3519`.
Byte353 is whitespace immediately after the first `reuse` token on line12:
`reuse Trait::<'_, _, _, _, _>::foo as foo;`. The failed impl itself is earlier
and valid; failure is broader ordinary declaration grammar admission, before
the source component helper executes.

Actual producer: ParsedFile::parse_structural (syntax/parse.rs180..196) maps the
owning edition and calls ra_ap_syntax::ast::SourceFile::parse on original UTF8.
Cargo.toml pins ra_ap_syntax=0.0.356 (Cargo.lock SHA checksum
`91a2308fdfc18a9ebb897e82c0fa283ef2684160b6a9d4f388dd4ce3aada51b6`).
The ra_ap_parser dependency is patched to vendor/ra-ap-parser, also0.0.356.
vendor/UPSTREAM.toml126..137 identifies rust-analyzer commit
`bcf0514497034e1156f3cc2bb72b99cc9634a949`, archive SHA256
`665a0ff94e8be7170aeb3ecf23e1a9ffb5b418daf2c6999646efd8bc9ac13271`,
and the maintained const-closure item patch. Current grammar/items.rs SHA256
`57145aeb0e97b9d00b4755c394dccc0d75cf47ca0610602d10a4d3c34d3d292b`
matches that recorded postimage. No delegation/reuse token or item production
appears in that parser's grammar/generated SyntaxKind. item_or_macro at48..113
tries ordinary items then parses a path-looking token as a possible macro; a path
without `!` becomes ERROR and reports `expected an item` (85..89). This explains
the terminal diagnostic after `reuse`; it is not an anonymous-region mismatch.
The existing parse diagnostic adjustments cover exact CRLF/final modifier
shapes, not delegation; PreparedSourceView declaration admission propagates
the genuine error as LRR-E-PARSE-BLOCKS-ENTRY (source/changes.rs620..637).

The exact pinned official rustc source supports this syntax: rustc_parse
parser/item.rs333 dispatches is_reuse_item to parse_item_delegation;480..495
recognizes actual `reuse`;851..866 parses delegation and records the fn_delegation
feature gate;916..956 parses the path/rename/body into actual Delegation or
DelegationMac AST items. The unchanged fixture explicitly enables fn_delegation.
This is a concrete mismatch between the current pinned rA item grammar and the
pinned rustc syntax, not evidence that a different edition or weakened expected
slot vector would fix it. No native rustc compile or diagnostic-compatibility
pass was run. The common parser/AST/source declaration prerequisite remains
open; ignoring this error, stripping reuse items or replacing the original with
a synthetic miniature would not establish the missing metadata assertion.

V532 retains the exact known unsupported original/control. The first two
anonymous worlds still pass; the third fails unchanged reuse grammar at353..353
before mixed-order helper execution. The 12-world projected-predicate source
control and new2world const-clause control pass independently; neither result
is a pass of this third anonymous world. [V532 evidence](../evidence/2026-10-20-v532-source-factory-controls.json).
