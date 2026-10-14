# M010 original attribute resolution composition

`OriginalAttributeResolver` composes source eligibility, existing namespace routing,
item-local helper precedence, exact procedural export classification and compiler
builtin definition classification. It borrows immutable indexes/catalogs and
returns `Inactive` or a typed candidate decision. No helper, build tool, file read
or query publication occurs.

Original outlines now retain their explicit cfg fingerprint in their stable index
encoding. The resolver requires that cfg and the helper owner/cfg context agree.
It checks source/scope and effective-meta membership before classification, so a
disabled unknown attribute cannot become a helper or a macro demand. Unqualified
name detection uses original path segments and the absolute flag; crate/self/super
or absolute paths do not borrow item-local helper visibility. Pending declarations,
namespace and opaque definition inputs remain typed outcomes.

The existing native builtin/procedural alias fixture now calls this resolver
instead of separately invoking candidate lookup and classification. Each source
adds a cfg-disabled unknown attribute with an invalid body: it is inactive in LRR
and ignored by native compilation. Cfg mismatch refuses. Existing exact identity,
source scope, alias/qualified preparation and native discovery/recursion checks
continue to pass without an LRR macro invocation during resolution.

Candidate classification is not a namespace seal or execution grant. The provider
must complete relevant expansion frontiers before using a candidate. Automatic
query/worklist orchestration, entered attribute binding, real compiler/sysroot
facets, generated reconstruction, complete discovery and guest execution remain
pending. This component does not complete M010, full MX018–MX020, release acceptance
or Linux support.
