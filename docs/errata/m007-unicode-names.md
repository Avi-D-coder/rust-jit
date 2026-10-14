# E-M007-004: owned macro names need NFC identity

The initial owned name boundary accepted Unicode identifiers through the pinned
parser but compared their unnormalized spellings. A declaration named `café`
and a lookup named `cafe` followed by U+0301 were consequently distinct. The
before-fix regression is retained in
`artifacts/macos-first/m007-unicode-before-correction.log`.
The native Rust 1.99.0 macOS oracle accepts that lookup. The Rust Reference
requires NFC identity and normalized identifiers as macro input:
https://doc.rust-lang.org/reference/identifiers.html#normalization.

Retain parser/lexer validation and source spelling/ranges. Normalize the owned
macro name after removing the raw prefix, using exact supplemental dependency
`unicode-normalization = 0.1.25` (Unicode 17.0.0). This does not replace the
frontend, any existing pin or the owned token wire. NFC is not NFKC: full-width
letters remain distinct. Acceptance covers all four origin editions, raw names,
Hangul composition and canonical combining-mark order, plus a real native
compiler comparison with an actual pinned macro expansion.

The helper bounds normalized UTF-8 bytes before every append. The new dependency
and its tinyvec support closure are locked; the existing unicode-ident 1.0.24
repair is preserved. Section 3.2 records the supplemental exact pin explicitly.
Full normalization of identifiers passed to and returned from macro expansion
still needs integration at the token adapter boundary; this checkpoint only
proves macro namespace identity. No full M007 or macro-input acceptance is claimed.

## Token adapter integration

The subsequent adapter regression exposes three failures in
`artifacts/macos-first/m007-unicode-tokens-before-correction.log`: source export
keeps decomposed symbols, external token import keeps them too, and `(café)`
fails to match a decomposed invocation, incorrectly selecting the fallback arm.
The adapter now uses the same exact normalizer on upstream identifier export
and owned identifier import. Original source span bytes, identity, edition,
origin, context and raw-identifier flag remain intact. Literal tokens retain
all their spelling, including decomposed Unicode within strings. Canonical
transport is idempotent; externally supplied noncanonical identifier text is
canonicalized while retaining its registered span.

The marked-transcription regression exercises both shrinking and expanding
NFC spellings. Encoded output size equals admitted bytes; the exact limit
passes and one byte less refuses with zero main output materializations.
A standalone macOS native oracle compares actual pinned literal-rule selection
in editions 2015/2018/2021/2024 for combining accents, Hangul composition and
full-width letters, preserving NFC rather than compatibility equality.
This proves the declarative token adapter component. Authorized procedural
server replies and generated-source scheduling still require their later
integration checks; full M007 acceptance remains pending.

## Original source bucket integration

The source outline previously removed raw prefixes while retaining non-NFC
semantic names. This left `$crate`'s canonical module/name route unable to find
ordinary decomposed source declarations, assigned different duplicate-name
ordinals, and permitted duplicate source contexts whose module paths differed
only by composition. Two actual failing regressions are retained in
`artifacts/macos-first/m007-source-names-before-fix.log`.

The pinned AST boundary now shares bounded semantic-name NFC conversion with
macro-name conversion. Original AST names are already syntax facts; reserved
semantic `gen` remains valid after source `r#gen` validation. Declaration DefKeys,
duplicate ordinals, module scopes and prepared source context keys use canonical
names. Header token spellings and immutable SourcePath identities remain intact.
Equivalent module contexts reject duplicate publication before mutation.

Two source tests cover stable definition identities across composition changes,
canonical duplicate ordinals, raw reserved names, full-width/ASCII distinction,
module-context canonical publication and source bucket candidates. The existing
three-package macOS Cargo fixture now uses `$crate::café::r#gen` against a
decomposed module declaration while preserving dependency renames/reexports and
caller/facade homonyms. Actual pinned marked expansion selects source identities
matching native values. Query consumers must supply canonical semantic bucket
keys; source-wide import/body lowering and generated epochs still need their
later integration.
