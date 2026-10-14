# M011 explicit coordinator include capture

`SourceCapture` opens preparation-canonical explicit source roots as cap-std
directories. It accepts only a request bound to the current unit/generation/root
authority fingerprint. It performs no build, macro or native-symbol operation, and
query providers cannot call it as a side effect.

Source requests retain raw spelling for capability-relative physical
canonicalization, preserving symlink/parent traversal semantics while refusing
escapes. A source capability anchor must cover that spelling. Physical targets are
routed to the most-specific authority; build-output targets use captured immutable
trees, including targets discovered after source canonicalization. Direct output
requests never open the live OUT_DIR.

Source root opening compares directory device/inode identity before opening, on
the capability and at the named path afterward, with canonical-path revalidation.
Source reads use cap-std nonblocking opens with the pinned safe rustix flag constant
to prevent a replacement FIFO from blocking the coordinator.

Source reads require regular-file metadata, an 8 MiB bound, matching pre-open file
identity and checked buffer growth. Post-read named/open-file stamps, size and
canonical address must match. A changed file yields a race diagnostic rather than
adopted bytes. Returned source/UTF-8 includes validate encoding; byte includes retain
arbitrary bytes. Records are immutable and bind request identity, physical address,
content and original spelling. Source/encoding failures retain caller/include origins.

Actual checks cover binary capture with native include_bytes comparison, UTF-8
failure, stale-generation denial, symlink escape refusal and raw symlink/parent
semantics versus native Rust 1.99.0. A captured build tree remains readable after
its original directory is removed; an unknown output file remains absent. A
deterministic replacement after reading changes the inode and proves race refusal.
Additional deterministic macOS tests replace a root directory before opening and
replace a checked regular file with a real FIFO; both return a race diagnostic.

Production source snapshot membership/revalidation,
generated parsing/discovery, complete eager operand integration and final
MX021–MX024 guest assertions remain pending. Cross-root symlink routing and broader
root/open-race scenarios still need broader security acceptance. M011, release and
Linux acceptance remain incomplete.

The existing QueryPoll input protocol now carries the private authority-bound
include request and consumer compile-environment identity. Demand issuance checks
unit, source generation and environment. Coordinator reply admission checks the
captured record against the exact request and started token. Shared waiters retain
one admitted output; source adoption is required before phase resumption.
The regression captures two real files and rejects unstarted, mismatched, repeated
and stale-context replies. This admission API is separate from the publication API.

Coordinator adopt_include now publishes the exact admitted capture as an immutable
derived database input, keyed by the complete input-request fingerprint. It checks
the frozen source revision, request and admitted output before publication. The
database rejects provider-time and conflicting publication, keeps the base source
revision unchanged, and advances the query revision only for a new record. A
source/metadata budget refusal leaves the database and resumption state unchanged.
Only successful publication permits one phase resumption per waiter.

Queries reading IncludeRecord register ordinary dependency stamps. A new complete
input universe drops derived includes; dependent queries then suspend instead of
using the old success. Disk edits do not mutate the current captured bytes. Cache
accounting includes current and retired captured blobs retained by readers. The
actual regression covers these properties and a one-byte source-budget refusal.
Generated parsing/discovery and the production prepare/membership revalidation
workflow remain pending; this is not full MX024 or guest-runtime acceptance.

SyntaxProvider::included_items is the pure item-grammar entry over an adopted
IncludeRecord. A missing record propagates NeedInput, and unit, syntax-kind,
profile and immutable-record identities are checked. Pinned structural parsing
and outlining receive the caller crate/module/cfg/profile, preserving the lexical
module context and cfg suppression. Nested include invocations remain demands;
unresolved cold bodies are not type-checked. Parse failure reports a bounded first
error with included path/range and both caller/include origins. The returned value
retains the input lease and physical base path. This helper does not merge generated
definitions or seal namespaces: M013-M015 must assign producer-aware identities
and perform lexical integration. Expression includes, eager paths, memoized include
parse sharing and production membership revalidation remain pending.

IncludeArguments decodes one original string operand through the shared pinned
syntax bridge, enforcing arity, trailing comma, literal suffix, escape and decoded
size rules. A nested macro expression stays a span-bearing NeedExpansion operand.
Arguments retain descriptor/token fingerprints; routing refuses another builtin
identity and uses the explicit source authority mapper without reads. The actual
macOS native comparison covers include/include_str/include_bytes with raw and
Unicode-escaped paths and exact binary bytes. This does not fulfill nested eager
expansions or evaluate paths in the guest VM. Shared environment parsing continues
to use the same bounded token/syntax/literal implementation.

SourceCapture::capture_set stages requested immutable records, rechecks each
through the same capabilities, and returns only after root/record revalidation.
Sorted request identities provide deterministic set encoding regardless of input
order. Duplicate requests refuse; implemented bounds are 100,000 requests, 128 MiB
source bytes and 32 MiB record metadata. Aggregate boundary acceptance is pending.
SourceCapture::revalidate checks a prior record against its exact request and
recaptured physical address/content. Roots are compared with retained directory
identities before and after capture, so a named root replaced after open refuses.

Actual macOS checks change file bytes, remove a file, replace an opened root and
move to a new source generation; the old immutable records remain unchanged and
old-generation requests cannot enter the new capture service. A deterministic
change between staging and revalidation refuses the set; stable retry succeeds.
These explicit coordinator APIs do not automatically run tools, advance a source
snapshot or revalidate undeclared procedural-macro reads. Automatic prepare
transaction integration and full MX024 remain unfinished.

Nested include string operands now retain pinned-AST path facts (qualified/raw/
absolute paths) and original argument token forests with owned spans/contexts.
Invocation identity binds the original include operand, macro path and argument
wire; argument identity is the exact wire digest. Extraction does not resolve names
or select a builtin from spelling. Operand metadata is limited before cloning.

IncludeArguments::resume_adopted validates the expected expression expansion key
against that invocation and input digest, then reads only its adopted database
record. Absent or merely admitted output stays NeedInput; matching adopted output
is parsed again through the pinned string bridge and retains producer dependency
identity. A complete source-universe replacement removes that output and demands
it again. The regression uses an exact compiler-facet concat definition, original
raw qualified path, pinned concat calculation, coordinator reply/adoption and
pure include routing; wrong input keys refuse. Automatic nested namespace resolution,
recursive eager evaluation of env/concat/user macros, builtin output emission and
full MX021 guest acceptance remain pending.

NestedCompileInputResolver joins the existing hygiene-aware textual resolver,
source-segment path mapper and namespace lookup. Its caller supplies the retained
source/entered-frame lexical context and explicit expansion origin. The original
reference span must match the consuming unit, source content and edition. Simple
relative names honor textual selection first; qualified paths use existing
crate/self/super/dependency routing, while unknown declaration frontiers suspend.
Compiler classification uses exact CompileInputCatalog identity/provenance; a user
macro named concat stays Other. No builtin dispatch is based on spelling.

The regression uses actual pinned normalization/name-comparison facts, checks
reexport and absolute raw-path compiler selection, user path/textual shadowing,
source-content mismatch and pending frontiers. Generated lexical contexts,
automatic caller-publication/frame binding, recursive eager evaluation and full
MX021 acceptance remain pending. Existing ambiguous/private/absent lookup outcomes
are preserved; their nested-specific matrix is not yet complete.

IncludeStringExpansion is the pure include_str output path. It validates exact
compiler-facet definition, original token digest, definition edition, expression
grammar, known source site and resource limits, then routes path operands through
the existing authority mapper. Missing adopted input returns its exact capture
demand; nested operands remain explicit expansion demands. Emission reads only
IncludeRecord, checks request identity and UTF8 kind, records the ordinary input
dependency and quotes through the pinned string emitter. No file operation occurs.

The native macOS regression covers quote/Unicode/newline text, suspension before
source adoption, original output span, exact record preservation after file removal,
wire-budget refusal and capture demand after complete-universe replacement. This
does not complete nested eager path fulfilment, include_bytes output, automatic
prepare integration or guest acceptance.

IncludeDataExpansion generalizes the adopted input path to include_str and
include_bytes while retaining the previous string API names as aliases. Exact
descriptor kind determines the required capture kind and emitter; bytes never
pass through UTF8 decoding. The byte emitter uses pinned TT byte-string literal
construction and owned span export, with ASCII byte escaping and checked exact
wire/escaped-text preflight before the full output string is built.

The actual native regression compiles the emitted token literal beside native
include_bytes and compares all 256 byte values and an empty input. It verifies
source spans, pending adoption, immutable output after file removal, wire refusal
and renewed demand after source-universe replacement. The string regression also
compiles its emitted literal against native include_str. Maximum-size byte-output
limits, recursive eager paths, automatic preparation and guest acceptance remain
pending.

Resumed IncludeArguments retain the original invocation token digest and adopted
producer key/output pairs (at most 256, checked metadata). validate_resolution
re-observes every producer through normal database inputs, checks the consumer
environment and refuses unrelated original tokens. Missing/changed producer
records remain expansion demands. IncludeDataExpansion::poll_resolved performs
this validation before routing or consuming a source capture. Returned values
therefore cannot silently reuse an old producer after source-universe replacement.

The actual native component regression runs concat calculation, production output,
coordinator adoption, path resumption, source capture/adoption and include_str
emission, then compares the emitted literal to native include_str!(core::concat!).
It rejects unrelated parsed output and a foreign consumer environment and demands
expansion again after removing producer inputs. Automatic recursive env/concat
argument evaluation, caller frame binding, producer-chain limit boundaries,
prepare orchestration and full MX021 guest acceptance remain pending.

Nested compiler expansion key binding uses ordinary source/hygiene resolution
before classifying an exact compiler facet. Only compiler selections receive
expression keys; user macros and pending/absent/ambiguous/inaccessible lookup
outcomes retain their typed resolution result. Keys preserve selected definition
edition, original argument digest, invocation identity, reference-span context,
caller edition, engine identity and the validated frozen consumer environment.
A different environment changes the key; a different consumer unit is refused.
The focused regression covers compiler aliases, user selection and these
identity/environment checks. Automatic recursive scheduling and retained caller
publication integration remain pending; this does not complete M011.

EnvironmentArgumentPlan now retains owned original name/message operand slices
through pinned string parsing and extraction. Nested invocation identities bind
the environment definition, original arguments and operand ordinal. Pure
materialization visits operands in source order, distinguishes missing selection
from missing adopted output, validates invocation/input/environment/expression
keys, and rejects child outputs that are not one string literal. A child cannot
inject another environment argument. Aggregate metadata is checked before pinned
token joining. The resulting operands feed frozen environment evaluation while
parent validation/emission continues to use the original unexpanded token key
and source site. No macro, process or environment effect occurs in this path.

The native regression compares emitted escaped/Unicode environment strings and
custom missing diagnostics against Rust 1.99.0. After only name adoption, the
message is still demanded even when the variable is present. Wrong child
environments, wrong parent input and insufficient output wire limits refuse.
Queries record both child dependencies; complete input-universe replacement
suspends prior success. Optional operand arity is tested, but exact Option facade
output and automatic recursive scheduling remain pending. Full M011 is incomplete.

The service eager compiler worklist now drives nested env/concat calls iteratively.
Each poll rebuilds bounded frames against the frozen source revision, caller
unit/source/edition, original reference context and consumer environment. Exact
ordinary namespace/hygiene resolution supplies child definitions and keys. Depth,
work, frame/token/plan/selection metadata and output wire limits are explicit;
forced overflow is a diagnostic rather than absence. Operand plans expose checked
owned allocation accounting. No recursive host expansion calls are introduced.

A produced record is returned for ordinary coordinator admission/adoption.
Computed or merely accepted child output cannot advance a parent; only an exact
adopted DB record can. The root returns Complete only when it is itself adopted.
Compiler includes hand off a validated owned external payload (definition, key,
original arguments and site), retaining the existing Expand input contract. User
macros and unresolved namespaces retain owned invocation facts and typed lookup
outcomes. Environment capture and exact Option facade needs are distinct outcomes.
The driver does not execute those effects or user/procedural macros.

The regression drives five real compiler outputs through source/path/hygiene
resolution and actual coordinator adoption, comparing the final emitted escaped
Unicode/numeric value with native Rust 1.99.0. Repeated pre-adoption polls,
accepted-but-unadopted polls, adopted-child non-replay, root adoption, reduced
depth/work/metadata limits and stale/new source revisions are exercised. Separate
cases preserve a user definition, an absent Option facade demand and the exact
external include payload. Source capture/driver integration, user-macro
continuations, generated contexts, full optional output, complete error/limit
matrices and full MX021-MX024 guest acceptance remain pending. M011 is incomplete.

The eager compiler worklist accepts an optional explicit source context for
include_str/include_bytes. It checks caller unit/path and source authority/root
before using even an adopted root, drives original nested path calls through
ordinary resolution and adoption, and re-observes the resolved path producer
through the existing IncludeDataExpansion contract. Missing source bytes yield
CaptureInclude; capture and adoption are still coordinator effects. An absent
source context retains the validated external handoff. Boxed include plans and
resolved producer payloads participate in checked worklist metadata accounting.

The regression compares escaped Unicode text used by an outer concat and all
256 byte values from a nested include_bytes path with native Rust 1.99.0. Capture
starts only after env/concat path outputs are adopted. Accepted-but-unadopted
source continues to demand the same capture, and emission succeeds after actual
file removal using immutable adopted bytes. Original root keys/sites are retained;
wrong caller paths refuse before and after root adoption, and input replacement
invalidates the frozen revision. This is data-include/worklist integration. Syntax
includes, generated declarations/source membership, nested virtual source bases,
user continuations, exact optional output and full MX021-MX024 guest acceptance
remain pending. No full M011 or release acceptance is claimed.

Syntax include transport now shares capture/adoption and producer revalidation
with data includes. The eager worklist allows the demanded root include grammar
while env/concat path children remain expression calls. Captured UTF8 source is
lexed through the pinned frontend with its actual content/unit/path, source
ranges and capture origin. Caller context and ancestry records are retained
verbatim; include creates no fresh hygiene mark. The shared pure adapter grammar
validator is the existing procedural-output validator extracted without changing
its slot tags, parsing entry points or associated-item scaffolding. Syntax output
checks token metadata and final wire limits, and diagnostics retain bounded path,
error and caller/include origin information.

The regression drives nested path construction, syntax capture and output
adoption for item and expression includes. It validates actual included source
identities/ranges and caller context after the physical file is removed. Native
fixtures reference a caller type/trait from included items and a caller local
from an included expression; malformed syntax fails both pinned validation and
native compilation. An additional check imports a nonroot caller mark through
the pinned hygiene store and proves included tokens retain that exact ancestry
without a new mark. These establish syntax/context transport and native lexical
scope observations, not LRR guest execution or generated DefKey integration.
Lexical publication/discovery merging, nested virtual source bases, complete
grammar/resource matrices and full MX021-MX024/M011 acceptance remain pending.

IncludedSourceView binds an adopted syntax expansion to an exact adopted capture
and freshly validated pure emission. Both records are required; a transport-valid
record with different output is refused. The view retains immutable source and
expansion input leases, actual file/root location, blob/content, caller context
and origin, original producer key and owned tokens. Its stable identity binds
producer, output and capture identities. Checked token/dependency/request metadata
is bounded, and root/capture/path-producer fingerprints are re-observed before
using a retained view. Removed inputs suspend with their typed original demand;
changed inputs refuse rather than silently using a stale view.

EagerSourceContext can carry this included view. The worklist checks authority,
location, content, context and origin consistency and validates all producing
inputs before evaluating the nested file. The regression acquires a view for
nested/one.rs, resolves its qualified include invocation using the retained
caller scope and pinned hygiene facts, and captures nested/deeper/two.rs from
that virtual file base. Both files are actually removed after capture. Native
Rust confirms the nested included expression sees a caller local. Pre-adoption
view acquisition suspends, mismatched output refuses, and complete input removal
invalidates view acquisition/use while its leased snapshot stays immutable.
This proves regular-file nested base/provenance integration. Full symlink base
compatibility, lexical insertion/discovery merging, generated DefKeys, complete
resource/grammar/error matrices and full MX021-MX024 guest acceptance remain
pending in the required ledger order. M011 and release acceptance are incomplete.

E-M011-003 corrects file-symlink nested bases. Captures keep a physical canonical
location for authority and immutable output selection and a distinct requested
virtual location for lexical source identity. Syntax output and source views use
the virtual address. The addresses fingerprint explicitly includes virtual
root/path/origin; metadata accounting includes both paths. The native witness
selects root inner.rs rather than the canonical target directory's decoy. A
full source-view/adoption fixture now selects deeper/two.rs beside alias.rs while
its physical target nested/one.rs has a different nested/deeper/two.rs. Physical
capability escape and raw symlink-parent checks remain exercised. This accepts
file-symlink base behavior only; directory symlinks with dot-parent spellings,
broader cross-root aliases/security and full MX022 guest execution remain pending.

Directory-symlink/dot-parent bases now preserve the captured raw filename in a
validated included source view. Pure authority routing joins a relative path to
that raw parent and records the raw target in the request identity. Capability
canonicalization and immutable-output selection still decide physical access.
The normalized logical source path remains available for source identity and
diagnostics; it is not used to reconstruct the raw containing directory. The
native witness distinguishes raw-parent 29 from normalized-parent 17.

The worklist and pure include emitter use this view-aware routing. View
certification reproduces output using the parent view, and nested views retain
the parent dependency proof. The dependency list is explicitly bounded at
100,000 entries and its allocated capacity, requests, raw base and tokens remain
subject to the 32 MiB view metadata limit. No recursive host view chain is used.
The regression captures link/../deeper/two.rs as physical nested/deeper/two.rs,
ignores the root-directory decoy, emits after file removal and confirms native
caller-local scope. A database retaining only child records still suspends the
child view on its absent parent. Generalized cross-root aliases, cycle/resource
matrices, declaration/discovery integration and full MX022 guest execution remain
pending. M011/release remain incomplete.

Explicit multi-root capture resolves symbolic-link contents through retained
cap-std source directories and checks each traversed target against the approved
root set. Absolute and relative aliases may enter another approved source root;
unapproved destinations and cycles refuse. No ancestor-directory reads or
ambient canonicalization are used by this resolver. Work, link and pending
component counts are bounded. Physical selection retains the destination root,
while the original virtual source address remains available for nested bases.

Actual Rust 1.99.0 macOS oracles agree for both approved alias spellings. A
deterministic after-read retarget to an unapproved root returns the snapshot-race
diagnostic with both origins and publishes no capture. Existing file replacement,
FIFO swap, immutable build-output and raw-parent acceptance remain exercised.
Broader resource matrices and full M011/generated-source guest acceptance remain
pending; these checks do not claim release completion.

The build-output regression additionally follows a source-root alias into a
prepared output tree after its directory is removed. Recreated live files cannot
change captured bytes or make uncaptured membership appear. The selected physical
root is the output authority, while virtual syntax identity retains the alias.
A real 8 MiB binary capture preserves all bytes; an 8 MiB-plus-one file refuses
with the source-limit diagnostic, a directory refuses as a non-regular file, and
a source set containing the oversized file returns no partial set. These checks
cover capture boundaries, not the separate byte-token expansion output limit.

An actual 8 MiB emitted byte literal matches native include_bytes byte-for-byte; exact wire budget succeeds and byte-minus-one refuses. Largest four-byte-escaped input under combined text/wire limits succeeds; next byte refuses. String and byte emitters include retained source-path text and the existing 32 MiB wire cap in preallocation checks; aggregate text overflows refuse before constructing output. Historical preflight failures are preserved. This is byte/string emission component acceptance; full M011 guest integration and release remain incomplete.
