# M007 entered-body macro glob contributions

Original body glob imports now contribute macro candidates from disclosed source
modules. The source module path is resolved independently, with caller privacy,
before looking up a member. Body-local named module aliases, forward aliases and
self-as-module routes are preserved. The existing module/macro graph supplies
source-module reexports, glob fixed points and original definition identities;
no synthetic module or namespace binding is published for the body.

A named import in the same block suppresses that block's glob candidates. Inner
block glob candidates are retained when lookup proceeds to a containing block,
so a distinct outer named import produces ambiguity. Repeated paths to the same
original definition deduplicate. Private members do not contribute through a
glob; an inaccessible target module fails separately. Transforming import facts
and pending source-member facts retain NeedInput. Source/body and opaque-entry
guards are unchanged. Shared checked route-work and metadata admission covers
body alias/glob walks, and candidate admission precedes set insertion.

Two source acceptance checks cover named module alias targets, identity diamonds,
inner-glob/outer-import ambiguity, private-member filtering and transforming
import demand. A macOS Rust1.99 fixture uses actual pinned hygiene and the parsed
source graph, compares original definition identities, executes scope-wide globs,
a reexport diamond and nested named-over-glob precedence, and records actual
native values. A native E0659 witness checks outer-scope ambiguity; a separate
native execution checks that private glob members do not shadow the accessible
outer import. This evidence does not claim guest VM execution.

Module names supplied by other entered-body globs now resolve through the
synchronous route table described in `docs/body-glob-module-membership.md`.
Qualified calls retain canonical identity separately from original access paths.
Body item/module catalogues, additional ambiguity/cycle/visibility integration,
generated publication, source local-binding ribs, membership seals, guest VM and
request-wide M012 accounting remain pending. This component does not establish
complete body namespace membership. M007 and release remain incomplete.
