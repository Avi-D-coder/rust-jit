# M007 parsed source macro path contributions

`SourceMacroRoutes` consumes an original outline with its matching owned hygiene
facts and explicit compilation-unit dependency aliases. The existing textual
index validates source definition identities and captured scopes. Disclosed
`#[macro_export]` definitions create public crate-root routes, including macros
declared inside inline modules. Their original identity, defining unit and
edition survive all imported aliases.

Named imports retain their original leaf facts and visibility. `crate`, `self`,
`super`, absolute dependency paths, renamed dependencies and known local modules
produce scope-qualified graph edges. Single-name imports first consult the
hygienic textual index at the source import frontier and retain the selected
definition identity. Ordinary item imports can have no macro candidate; this
component does not turn that into an import error.

Globs, dependency/local-module collisions,
transforming attributes and pending/ambiguous textual origins remain typed
unresolved facts. Module aliases and unknown module prefixes now resolve through the deterministic
worklist in [module-alias-resolution.md](module-alias-resolution.md). Graph
construction refuses remaining producer-dependent catalogs,
validates metadata limits and returns a fresh graph only after success. Paths
have at most 256 components and 32 MiB of text; each catalog and the aggregate
candidate graph have checked 32 MiB metadata admission.

These contributions do not prove namespace completeness or access permissions.
Cold bodies remain opaque, so an export in an undisclosed entered body is not
claimed discovered. Producers, directory/generated-source seals, glob fixed
points and visibility checks must be integrated before
using candidate lookup as final execution dispatch. Native-world export facets
remain M039; this source component retains the fixed live profile.

Two source tests cover actual parsed root exports, raw names, dependency renames,
facade reexports, local textual aliases and nested `crate`/`super` routes, plus
explicit unresolved cases and invalid paths. The existing three-package macOS
Cargo oracle now obtains its macro definition and facade route from these parsed
contributions with actual adapter-owned hygiene facts. Both helper collisions
and Unicode/raw `$crate::café::r#gen` paths still produce `[17,17,53]`. The helper
value map and selected pinned macro expansion remain component evidence, not a
guest VM execution claim.
