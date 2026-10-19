// Pinned next-solver inference storage client; MIT OR Apache-2.0.
// Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
// Registered mechanical port: tools/sync_upstream_inference.py.
impl<'db> TypeVariableStorage<'db> {
    pub fn sub_unification_table_ref(&self) -> &ut::UnificationTableStorage<TyVidSubKey> {
        &self.sub_unification_table
    }
}
