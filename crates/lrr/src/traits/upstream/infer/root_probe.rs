// Pinned inference variable/resolution client; MIT OR Apache-2.0.
// Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
// Registered mechanical port: tools/sync_upstream_inference_context.py.
impl<'a, 'db> TypeVariableTable<'a, 'db> {
    pub(crate) fn probe_with_root_vid(&mut self, vid: TyVid) -> (TyVid, TypeVariableValue<'db>) {
        self.inlined_probe_with_vid(vid)
    }

    /// An always-inlined variant of `probe_with_root_vid`, for very hot call sites.
    #[inline(always)]
    pub(crate) fn inlined_probe_with_vid(&mut self, vid: TyVid) -> (TyVid, TypeVariableValue<'db>) {
        let (id, value) = self.eq_relations().inlined_probe_key_value(vid);
        (id.vid, value)
    }
}
