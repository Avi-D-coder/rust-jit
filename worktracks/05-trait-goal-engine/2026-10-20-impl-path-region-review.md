# M020 impl path lifetime-arity review

Read-only review of frontend/source_types.rs::impl_path_regions_pending and its
DeclarationGenericTypes call, before parent freeze. No production source, Cargo,
Git or tests changed by this reviewer. Unchanged repr128/full actual candidate
catalog remains the acceptance witness; this source review is not acceptance.

## Concrete findings reported immediately

1. Initial helper used actual_associated_owner to walk an enclosing impl, but
   DeclarationGenericTypes supplied the selected child's definition/header/context
   frame. For an associated function in `impl Tr for S { fn f() {} }`, the impl's
   Tr/S occurrences lie outside f's declaration context. The common actual
   occurrence service correctly rejects them (declaration_type_names.rs::
   ParsedFile::declaration_type_name checks context.range and nearest item owner).
   Generated `$crate` likewise cannot borrow the child's receipt for parent tokens.
   Parent corrected this: helper now accepts only directly selected Impl syntax;
   every associated GenericTypes request fetches its genuine parent GenericTypes,
   propagates pending, then uses the real parent's declaration root for existing
   mapping/const rebasing. No child frame scans parent tokens.
2. const_parameter initially rejected qualifier paths but not leading absolute
   syntax. In Rust 2015, `::N` is a root Type path even with a local const parameter
   N. The common member_index::path_segments reads leading absolute syntax from
   the first PathSegment::coloncolon_token, not merely qualifier presence. Such a
   path must not become that local const parameter in either preflight or generic
   argument transport. Reported exact marker and a 2015 source witness to parent.
3. Bare TypeArg N must obey common Type-namespace priority before becoming a local
   const parameter. Pinned rustc_resolve/src/late.rs::visit_generic_arg (1331-1360)
   explicitly resolves TypeNS first and tries ValueNS only when TypeNS is absent;
   its const generic declaration goes into ValueNS (around 3289). Minimal valid
   witness: `struct N; struct S; trait Tr<T> {} impl<const N: usize> Tr<N> for
   [S; N] {}`. The target trait argument denotes the visible Type N, but the new
   direct const_parameter shortcut initially emits Const first, causing exact
   argument-kind transport to hold a valid head. Preflight also skips the path,
   potentially missing actual Type N's lifetime arity. The remedy is existing
   ordinary actual Type lookup/absence before ambiguous TypeArg-to-const transport;
   explicit ConstArg uses its actual const parameter. No new resolver or static
   checker is warranted. Reported with exact pinned source to parent.

## Shared route and dependency inspection

The helper is cold memo preprocessing over actual impl trait/self Type syntax;
it skips function binders and expression subtrees, avoiding executable const/body
inspection and avoiding adding function-bound lifetimes as impl parameters.
Unqualified names use WrittenTypeDeclaration::resolve_name_at and its common
actual source/namespace service; qualified names use projection_trait_key, which
is a name for the existing general source_type_paths::lookup and does not impose
a trait-only test. `$crate` goes through actual occurrence/token provenance.
The owning PreparedScopeImplHeaders environment supplies the retained names/source
handles and memo dependencies. Namespace answers and target DeclarationGenerics
queries stay ordinary typed query reads. A target with no genuine layout stays
pending; no omitted count is assumed zero.

After the associated-owner correction, child GenericTypes -> genuine parent
GenericTypes is structurally outward, then parent path-target reads fetch raw
DeclarationGenerics (ParseFile/actual metadata), not GenericTypes recursively.
No reverse GenericTypes edge was found in the inspected name/path services or
PreparedScopeImplHeaders producer. Associated scope resolution may fetch already
completed raw declaration metadata but does not reenter the active GenericTypes
query. Parent identity must remain joined through real generated/inventory
member-scope mapping; no lexical owner digest alone is a declaration certificate.

Lifetime counting uses the actual target's own declaration parameters under a
real layout, and compares written lifetime arguments of the relevant segment.
Missing/unknown targets, parenthesized path arguments or absent layouts stay
pending. The helper does not implement omitted path lifetime production. Existing
qualified type/associated path gaps remain typed preparation holds. It does not
infer lifetimes, erase them to Static or default missing metadata.

CompleteTraitArgs' new full-arity path zips actual declaration kinds against
actual Type/Region/Const argument variants, so genuine const transport is accepted
without inventing a const default. The partial/default path requires already
supplied Const values and only prepares existing actual Type defaults. Its exact
kind check usefully exposes the ambiguity bug above; it must not be weakened.

## Architecture and currentness review scope

All eight modes inspected: no alternate evaluator or literal-only generated route;
no trait/member/numeric special cases; actual generated/source lookup identity is
preserved; no pattern/cold executable-arm checking; preprocessing is in the memo
producer rather than dispatch; no new hot world scan/invalidation; no BodyIndex
clone or repeated whole-file executable-node search. Work is one cold walk over
actual trait/self syntax plus demanded actual target lookups/layout queries,
retained by existing memo dependencies. There are no new resource caps and no
measured performance claim. Namespace/service code predates this helper; existing
broader costs are not represented as measured release behavior.

Initial corrected-associated snapshots inspected:

- source_types.rs: 24218de4f3facd0d9eaec1f8780e63fc608f2cfa144c97cd6602d8d5328fd756
- declaration_generic_types.rs: efd4194fe32fec668413aa6009b939717aba33a067fbc5559d0ef378c32be347
- source_type_paths.rs: ff3f537164b3ea6f84bb5d3705d4a8a7903d0d16843ebde93e315b2fb7b07e4b
- type_name_namespace.rs: 2b2555a86440996cfd53c35a14a4ac719806a5cc255d4dda12231e1211a489b5

Findings 2/3 require parent correction/review against new fingerprints before
freeze; the preceding hashes are not a clean final endorsement. No compile or
runtime observation is asserted. Dormant catalog files stayed frozen.

## Corrected source re-review

Parent corrected all reported new-path findings. Re-read exact source:

- source_types.rs: 48e4798549a5174027ca304c0adf35021ca7fb2581f3cd2d3ef5fe749c6f5a7f
- declaration_generic_types.rs: efd4194fe32fec668413aa6009b939717aba33a067fbc5559d0ef378c32be347

No actionable source finding remains in the bounded new producer at these
fingerprints. const_parameter now recognizes only a simple argless nonabsolute
path, rejecting the actual first-segment leading marker and qualification.
ambiguous_const_parameter requires an actual completed TypeNS Absent answer from
the existing source/namespace occurrence service before using a const parameter.
Unknown, ambiguous, pending or visible Type/primitive/parameter answers never
become absence. written_generic_argument uses this same helper for TypeArg;
explicit ConstArg keeps its actual const parameter route. The preflight skips a
const parameter only at an actual TypeArg location and through that same absence
proof. It no longer applies a value-namespace shortcut to arbitrary Type paths.

Absolute paths now use the whole common source_type_paths service in the new
arity helper. Direct named paths use the actual retained occurrence/header owner;
associated children fetch their actual parent GenericTypes rather than walking
parent syntax under child identity. Child memo fetch dependencies retain the
parent pending/layout result; extracting its declaration_root does not discard
the recorded query dependency. Target arity reads raw DeclarationGenerics and
absent layout stays pending, so no fake zero or new direct GenericTypes cycle was
found. Genuine Type/Const full-arity transport stays exact and does not default.

The existing separate resolve_actual_type walk's single-segment absolute path
handling is prior common source debt; this review does not claim complete path
semantics from the new arity guard. Missing omitted named-path lifetime production
and unsupported qualified projection shapes remain explicit preparation holds.
The source-aware all-eight/cold metadata review above applies to the corrected
producer; no new evaluator, parser, static check, hot scan or body preparation was
introduced. No compile/runtime/edition acceptance is inferred from source review.
Dormant catalog source remained unchanged and no Cargo/Git/tests ran.
