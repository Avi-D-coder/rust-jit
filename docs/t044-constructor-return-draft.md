Integrated as scoped T044 components from collected V13; fresh MAIN milestone remains pending. Historical draft text follows; it is not full task, rustc fixture or release acceptance. See artifacts/macos-first/t044-call-continuation-integration-v13/integration.json.

# Unvalidated T044 constructor and return successor

This private successor has 21 registered T044 controls and retains 164 official cases / 283 unchanged upstream files at ae0fe511b15ab760d7e963b13389d1bc2ab42b18. The registered controls have not run on this source. T044 remains in progress, release_complete=false. MAIN and frozen ReturnV1 are separate snapshots.

The constructor guard records exact field ordinal, transport ticket and value until real cleanup completes. A completed field now also gets a frame-anchored Dropping location. Admission checks the guard/owner, reserves both arena handles and cleanup metadata, polls cancellation and charges one owner before cursor/ownership mutation. The same ticket enters the ordinary iterative cleanup machine; no second move/ticket can retire the source identity early. A guard cannot advance or retire while a child is pending, and acknowledgement rejects ticket forwarding into another live owner.

ScopeLeave and Return scan one slot per bounded transition to select the newest active construction in the current lexical scope. Each guard drains completed fields in reverse completion order before ordinary storage. A partial enclosing value and its never-completed fields have no destructor. Saved guard/field/ticket/machine state survives suspension and fatal interruption. The legacy synchronous leave path continues to refuse surviving guards. Nominal NeedWitness still refuses; source-selected guest Drop/self activation and M026/M029 integration are not implemented by this change.

Acceptance sources and limits:

- nested-return-drop-order.rs supplies original Foo field ordinals, completed Bar.x and original nested return recipe. Prepared intrinsic String/tuple/257-element array adaptations check location lifetime, exact acknowledgement, reservation refusal, bounded return cleanup and fatal cancellation. Actual Foo/Bar construction, Result and user Drop assertions remain unexecuted.
- RFC1857 drop-order.rs supplies both interrupted TestStruct written field orders, scalar tags and following original expected vectors. Intrinsic String cleanup is checked against those vectors. Rc, RefCell, catch_unwind, PushOnDrop callbacks and the full program remain unexecuted.
- return-nil.rs and early-return-with-unreachable-code-24353.rs add acceptance checks for unchanged original main bodies: callee-to-caller unit return once after cleanup and explicit unit return with the later binding cold. They remain unexecuted. Rust2024 execution adaptation is distinct from full compiletest acceptance.
- return-in-block-tuple.rs is imported unchanged and pending. Source inspection shows that prepare_constructor demands the tuple result descriptor before evaluating its diverging field. This is an implementation gap for lazy unknown-field construction, not an executed failure, a spec contradiction or an accepted fixture. No invented field type or synthetic success workaround is used.

Full rustc compiletest passes remain zero; recorded stock program observations remain 13 / 54 original assertions across earlier tested snapshots. User Drop, caller return authority/reservation admission, replacement, panic/unwind, general traits, guest JIT, release/security/performance and Linux acceptance remain required.

The successor also charges the inline Option<cleanup Activation> in Frame::metadata_bytes before calculating the remaining machine budget. Constructor mappings, projection/source-field metadata and iterative task capacity keep their separate checked charges. Earlier frozen v2 omits this fixed charge and is retained unvalidated; it must not be integrated. This mechanical accounting repair has not been validated.
