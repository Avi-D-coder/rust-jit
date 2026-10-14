Integrated as scoped T044 components from collected V13; fresh MAIN milestone remains pending. Historical draft text follows; it is not full task, rustc fixture or release acceptance. See artifacts/macos-first/t044-call-continuation-integration-v13/integration.json.

# Private T044 caller return admission draft — unvalidated

This mutable V7 successor checks the suspended caller before callee cleanup. It validates the active call, frame anchor, exact result slot and resume PC; validates the returned value's destination and bounded source attachment; and reserves the caller's uninitialized result cell before admitting ReturnContinuation. The returned ticket remains owned until actual cleanup completion and once-only caller resumption.

Two new controls use the unchanged return-nil main/f thunks and normal source contracts/CallAction consumption through a disclosed prepared frontier. They check that an incompatible caller destination refuses without cleanup/fuel/owner consumption, and that successful admission reserves the exact cell before cleanup and fills that same cell only after resumption. The frontier adds no production yield or synthetic call authority.

No tests have run on this source.28 T044 controls/469 expected library bodies;166 official cases/285 unchanged original files. T044 remainsin_progress. Frozen observed V6, integrated V5 and MAIN validation are separate; their evidence is not combined acceptance for V7. Source-selected user Drop, replacement, panic/unwind, general traits, guest JIT, release and Linux acceptance remain incomplete. Full rustc compiletest passes0.


An additional unchanged issue-69841 array prefix tests Section11: the reached suffixed first field fixes the array element descriptor, then the original unresolved second literal binds at transfer. No width default, future operand scan or generic argument inference supplies the type. The original loop/range patterns/unsafe intrinsic and LLVM regression stay cold; this is an unexecuted component control, not full fixture or native acceptance.
