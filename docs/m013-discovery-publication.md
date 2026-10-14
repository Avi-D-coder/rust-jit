# M013 discovery record and certificate query publication

SourceDiscovery::publish accepts only an actually completed, matching discovery
certificate. It appends the complete expansion-record set and certificate as one
derived Database revision while preserving the frozen base source revision.
Incomplete discovery publishes neither records nor a seal through this entry point.
Publication is prohibited while any pure query dependency stack is active.

PreparedSourceInputs now publishes stable captured-file location inputs. Their values
bind path, unit, module, content/profile and cfg; a deleted file cannot reuse another
file's identical blob. Publication checks these exact current selections and current
content/cfg, descriptor/record agreement, certificate dependency membership, current
procedural epochs, immutable conflicts and the query metadata budget before mutation.
Record copies use fallible wire reservations after the complete budget/conflict pass.
Repeated identical publication is idempotent. Ordinary base snapshot publication
cannot inject discovery certificates or derived expansion/include records.

QueryKey::DiscoverySeal uses a typed domain/unit-graph/epoch key with an explicit
stable profile encoding. Its immutable input contains the private completion
certificate, scanned source selections and sorted expansion descriptors. SyntaxProvider
reads current captured locations, actual outline queries and every record input,
recording ordinary query/input dependency stamps. Missing certificates produce typed
InputRequest::Discover; missing record payloads produce exact expansion-input demands.
These suspended results remain outside semantic memo values. Providers execute no
matcher or native helper. Discovery demands cannot obtain expansion-coordinator
execution tickets or become include-capture dependencies.

Seven controls cover typed suspension without a semantic memo; atomic publication
with parent/child record dependency tracking and idempotence; incomplete refusal;
changed-source invalidation; metadata-budget refusal without partial inputs; publication
refusal inside a provider; a later record conflict leaving an earlier absent record
unpublished; and deleted-file refusal even when identical content remains elsewhere.
The conflict control uses schema-checked tokens from actual matcher output and the
existing explicit coordinator transport; it does not substitute guest fixture output.

M013 remains in progress. Sealed namespace buckets, generated declaration/body query
contexts and full runtime source integration still require work. Macro namespace
import/export and qualified/contextual/definition-unit routing, external-module source
chains, all overlapping expansion-origin cases, complete quota/publication-failure
boundaries and full MX027/MX028 remain required. M014–M016 follow in fixed order.
Full E24, release and Linux acceptance remain pending. Validation uses macOS ARM64
with Rust 1.99.0.
