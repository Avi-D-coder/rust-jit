# M013: ordinary module capture and immutable syntax projection

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and the release remain incomplete. CLI guest execution of ordinary modules
inside adopted sources still requires discovery staging/completion integration.

Invariant: an ordinary module uses its exact captured source declaration and
source-origin capture identity. Its adoption and syntax projection never require
a fabricated generated expansion record or generated header.

Captured declarations now prepare exact module capture requests using the existing
source authorities and candidate routing. OrdinaryModuleSourceView checks the
input fingerprint, producer, request, physical caller path/unit and origin, then
validates the adopted ModuleRecord. It parses the actual captured child with the
parent edition/cfg and keeps the input lease and captured declaration alive.
OrdinaryModuleSyntaxInput hashes exact declaration/capture/environment/projection
facts and retains the child prepared view and bounded input rows.

Generated and ordinary captured views share the existing source-input projection
builder, preserving actual dependency aliases, source/header/body/scope rows and
canonical selection checks. Ordinary views retain their own parent origin lease.
The pure discovery preparation API looks up the exact pending source declaration
and captured parent directory. Its syntax API revalidates the parent ModuleRecord,
SourceSelection and frozen compile environment before consuming an adopted child
record. These APIs perform no file capture, build or macro execution.

Three new controls actually open bounded source authorities, capture child files,
accept/adopt through ExpansionCoordinator and prepare immutable syntax. They prove
ordinary identity/body rows without expansion records; reject changed source,
relocation and a substituted declaration; and observe changed actual child bytes
while retaining the same source member identity. The first fixture used a
noncanonical temporary directory; the corrected fixture honors the existing
canonical authority contract. Failed logs remain historical evidence.

The pending CLI/native witness from the previous checkpoint remains unchanged:
LRR catalog-incomplete versus native 2053. No end-to-end ordinary adoption success
is claimed. Staging, parent macro cutoffs/catalogue completion, atomic publication
and CLI coordination follow next. Complete quotas, broader source combinations,
service/facets/cache/runtime/type integration, full MX027/MX028, JIT/release and
Linux remain mandatory; M014–M016 follow in fixed order.
