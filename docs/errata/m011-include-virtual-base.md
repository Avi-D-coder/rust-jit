# E-M011-003: physical capture location used as lexical include base

Rust 1.99.0 on macOS ARM64 evaluates include!("link.rs") with link.rs pointing to
real/one.rs, whose content is include!("inner.rs"), from the original link.rs
containing directory. With root inner.rs containing 17 and real/inner.rs containing
29, native prints 17. Using the canonical captured file location as the next
include base selects 29. The minimal native witness is preserved in
artifacts/macos-first/m011-symlink-base-witness-1791283377619364000/evidence.json.

This is an implementation correction to revision 4 section 49.5's existing
original/virtual containing-file requirement, not an architecture or pin change.
CapturedInclude.location remains the physical canonical address used for
capability checks and immutable output routing. virtual_location retains the
requested file address for lexical syntax origins and nested includes. Syntax
emission and IncludedSourceView use that virtual address. Both addresses are
accounted in metadata; captured identity explicitly binds the virtual root/path/
origin as well as request, physical address and content under the addresses
encoding domain. Physical canonicalization, root checks, regular-file limits,
immutable output-only rules and explicit source authority remain unchanged.

Two regressions compare real native behavior with virtual routing, preserve the
physical-only mismatch observation, and drive nested include source views through
adoption with a decoy physical-directory file. Existing escape and raw symlink
parent capture checks pass. Directory-symlink/dot-parent combinations, broader
alias/security matrices and full MX022 guest execution remain mandatory pending
acceptance. This erratum authorizes no alternate sandbox or native loading.

Directory-symlink refinement: the native witness at
artifacts/macos-first/m011-directory-base-witness-1791283923252230000/evidence.json
prints 29 for include!("link/../one.rs") where link points to real/deep and
real/one.rs includes inner.rs. Lexically normalizing the parent selects root
inner.rs (17), so a normalized virtual address alone is insufficient. Source
views now retain the captured raw absolute filename as a separate base. Only an
authority-matched validated view can supply this base to pure request routing;
relative children join its raw parent before the existing lexical authority and
capability canonicalization checks. Request encoding explicitly includes the raw
target under the raw-base domain. Logical source address and physical authority
remain distinct. Source-view certification can use the parent view and carries
its dependency proof forward; nested views cannot validate from child records
alone when the parent is absent. The actual driver/native regression uses a
normalized-directory decoy and removes the captured physical files after adoption.
Broader alias/root/security matrices and full guest acceptance remain pending.
