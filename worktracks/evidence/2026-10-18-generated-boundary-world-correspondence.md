# Generated source correspondence spans namespace completion facets

Root LOCAL-045/WT04/T032, E-R5-005. V18 failed actual all-origin pattern control
reports owner=true, seal=false, epoch=true[1/1], cfg=true, record=true. The same
generated source record can be published by actual Type and Value discovery under
different seal domains; GeneratedSyntaxInput::same_unit already compares source
world graph/epoch, scope/cfg, actual record and origin, excluding completion domain.
GeneratedBodyBoundary::require_current likewise compares graph/epoch/scope/record
and then checks its own complete discovery seal and exact published source/record
inputs. SourceDiscovery::generated_body_macro_boundary's older full seal equality
contradicts those existing correspondence rules before it can mint the boundary.

Change this cold creator's source-world comparison to the same unit_graph+epoch
contract, keeping actual owner/cfg/items-epoch/record-allocation checks. Its produced
boundary still retains the creator's complete own seal+value; require_current
validates that exact completion, actual generated unit, parents, expansions and
current epoch. SourceBodyContext independently validates the current source unit's
own seal. This joins two already complete namespace facets of one actual source;
it does not invent a world/boundary, compare source by text, ignore stale epochs,
or relax record/source identity. No native/macro execution or provider publication.

Files: expand/source_discovery.rs plus negative authority assertions in existing
service/retained_reference_source_tests.rs. Reuse actual failed ordinary/user/wrapper
initializer matrix as positive runtime evidence and existing ancestry/epoch/range
controls; add a wrong graph unit prepared from the same actual record solely to
prove rejection. No imported Rust originals modified. All8: existing source/body
and macro mechanisms (1-4), no executable/pattern preparation (5), one cold boundary
comparison/no installed-source planning (6), no namespace scan (7), Arc actual record
and parent handles (8). Constant-time extra key comparisons only; no speed claim.
Full generated-source/epoch/suspension matrix remains incomplete until actual
terminals; source review and exact validation required before integration.

Exact source-review snapshot:
- `crates/lrr/src/expand/source_discovery.rs`: `a6347da48d438c5bd0a2292bbd1d64859be65358381be740168ce5929314a726`
- `crates/lrr/src/service/retained_reference_source_tests.rs`: `a04393be972fdf84cb520daab048f3c9b951939efac180c0bdf0e3f6ead00dc0`
- `crates/lrr/src/cli/initializer_tests.rs`: `77c25c3baea2aa4ca51c7a53cc6eb54d34b766060dd79460cac920719297d83a`

The existing origin test now also includes the constant pattern nested in standard matches!/assert! expansions, retaining the same initializer CallSeal count. This is a route control, not macro-specific production behavior. V18 proved only full-seal inequality; next capture verifies that this existing domain-independent source-world correspondence resolves the actual case. Wrong graph, epoch, cfg, scope and record remain refused; no runtime pass is assumed from source review.
