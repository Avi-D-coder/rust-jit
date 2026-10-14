# M007 original source import facts

The pinned syntax AST supplies use-tree facts attached to the exact source
outline. Iterative traversal flattens grouped imports in source order while
retaining each leaf range, item identity and leaf ordinal, source scope,
visibility, absolute qualification, raw identifier spelling, rename, anonymous
import and glob. `crate`, `self` and `super` remain typed path segments.
Source spelling is retained for later canonical semantic resolution.

Cfg-disabled items are excluded by the existing outline phase. Cold function
bodies remain opaque. These facts describe original syntax; consumers must
inspect the associated item header and producer demands before treating an
attribute-transformed import as published. This component does not establish
namespace completeness, resolve imports or authorize macro execution.

Admission checks 256 path/group depth, 100,000 visited use-tree nodes per item,
100,000 leaves across the outline and a cumulative 32 MiB owned metadata
allocation charge across the outline. Prefix duplication is charged before
cloning grouped paths. Allocation and size failures return typed diagnostics;
no partial outline is published. The outline cache estimate includes retained
import paths, names, scopes, aliases and restricted visibility tokens.

Acceptance exercises the actual pinned parser on nested groups, absolute paths,
raw and decomposed Unicode names, renames, globs, anonymous imports, visibility,
inline modules, cfg filtering and cold bodies. A deep path and a small input
with excessive duplicated-prefix metadata both refuse. Namespace/export
integration and generated-source publication remain M007 work in progress.
