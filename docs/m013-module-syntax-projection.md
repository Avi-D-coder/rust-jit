# M013 adopted module syntax projection

M013 remains in progress. Adopted child module views now produce an immutable
syntax projection for later discovery integration. The projection shares the
actual parsed file and outline; it does not reparse generated display text or
lower executable bodies. Its private prepared-source state contains frozen bytes,
cfg, the original dependency graph, exact source selection, declaration headers,
physical parse keys, declaration contexts, body handles and scope members.

The stable projection identity binds the adopted input identity, captured candidate
record, exact generated demand/header and parent descriptor, child and caller
source selections, invocation span and every ordered input row. Identical physical
bytes may reuse a parse identity while different parent provenance or logical
module scope changes the projection identity. Logical scope changes also preserve
distinct definition identities. Checked metadata covers owned rows, selections,
shared parse/outline handles and their ownership records.

The prepared-source state retains the originating module view. Sharing the source
and dropping its projection therefore keeps the adopted input and original caller
lease alive, including source-byte accounting after database input replacement.
Dropping the final shared source releases that accounting.

The existing declaration-syntax check now examines parser errors outside body
ranges even when the outline has no explicit ParseBoundary blocker. This closes a
missing rejection for errors outside indexed items, such as a trailing question
mark. Errors contained in otherwise reliable cold body ranges remain lazy.
A real macOS Rust1.99 library oracle rejects the same root declaration syntax;
the source is an oracle copy, not a selected live runtime unit.

Six controls cover adoption suspension, exact row/outline sharing, untouched
source revision and absent database header publication, lazy body indexing,
provenance/logical-scope identity, malformed child/root declaration refusal,
cold-body parser errors and retained source accounting after input replacement.

These are prepared rows. Child declaration/import/macro scheduling, inherited
macro use, qualified paths, atomic derived publication and cache registration,
recursive file-module discovery, included-file physical layout, frontier closure
and actual LRR child execution remain required. No new successful module LRR/native
execution comparison is claimed. The historical positive module witness remains
pending with comparison_passed=false. Full MX027/MX028, JIT, release and Linux
acceptance remain unfulfilled. macOS ARM64 Rust1.99.0 is the current platform.
