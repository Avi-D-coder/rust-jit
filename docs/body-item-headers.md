# M007 original item headers in entered bodies

Explicit body discovery now retains owned BodyItem records for cfg-enabled
functions, constants, statics, structs, unions, enums, type aliases, modules,
traits, impls, imports, extern blocks/crates and macro declarations. Each record
has a lexical block ordinal, source DefKey, declaration/header ranges, original
header tokens, visibility, attributes, namespace facets, and any transforming
attribute input. Raw spellings remain in token recipes; declaration names use
the existing semantic name normalization.

Each body block has an owner identity derived from the containing definition and
block ordinal. Local declarations therefore do not collide with module items or
same-name declarations in other blocks/functions. Duplicate ordinals distinguish
same-scope declarations. Impl identities additionally retain their header facts.
The textual macro binding and its item record now share the item's DefKey
fingerprint; the provisional separate body-macro identity encoding was removed.
Existing source/native body-macro selection checks run through this shared key.

Executable contents and module/trait/impl/extern/template containers are pruned
from header-token traversal. Executable bodies retain exact byte hashes and
ranges; container contents retain a separate entry range. Discovery does not
lower those contents or enumerate their nested declarations. A transforming
attribute input blocks an executable handle's entry. Unit and tuple structs
retain both type and value namespace facets; record structs retain the type facet.

Two new source acceptance checks cover all ordinary item classes, constructor
facets, lexical ownership, cfg filtering, derive demand, container opacity,
shared macro identity, stable identity/header fingerprints across cold body
edits, changed exact body hashes and transforming-function entry rejection.
Traversal, token/attribute/record metadata and item counts are bounded with
checked sizes and allocation admission. These are source-fact checks, not guest
execution, local type checking or a native/guest execution comparison.

Explicit nested-body and original container entry now have separate acceptance
(see nested-body-entry.md and container-entry.md). Local module/import catalogue publication,
source local-binding ribs, generated publication, namespace seals, guest VM
integration and request-wide M012 accounting remain pending. M007 and release
remain incomplete.
