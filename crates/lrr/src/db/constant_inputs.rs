//! Pure constant-key freshness checks, including unobserved input membership.
use super::*;
use crate::core::digest::StableEncoder;
impl Database {
    pub(crate) fn constant_input_universe(&self) -> Result<Digest, Diagnostic> {
        let mut bytes = 0usize;
        let mut e = StableEncoder::new("lrr/v1/constant-current-input-universe");
        e.u64(
            u64::try_from(self.inputs.len())
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "constant input count overflow"))?,
        );
        for (key, row) in &self.inputs {
            bytes = account_add(
                account_add(bytes, std::mem::size_of::<(InputKey, Digest)>() + 128)?,
                key.heap_bytes(),
            )?;
            e.record(key);
            e.digest(row.fingerprint);
        }
        Ok(e.finish())
    }
    pub(super) fn verify_constant_key(
        &self,
        key: &crate::db::key::ConstantKey,
    ) -> Result<(), Diagnostic> {
        key.validate()?;
        if let Some(universe) = key.universe()
            && universe != self.constant_input_universe()?
        {
            return Err(fault(
                "LRR-E-CONST-SOURCE",
                "constant admitted input universe changed",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as lrr;
    include!("../../../../xtask/tests/support/constant_key.rs");
    include!("../../../../xtask/tests/support/constant_value.rs");
    struct ReadA;
    impl Provider for ReadA {
        fn execute(
            &self,
            db: &mut Database,
            key: &QueryKey,
        ) -> QueryPoll<crate::db::provider::Provided> {
            let input = db.input(&InputKey::CompileEnv("a".into())).unwrap();
            let InputValue::CompileEnv(value) = input.as_ref() else {
                panic!("wrong fixture input")
            };
            crate::db::provider::Provided::pure(fixture_value(key, value.parse().unwrap()))
        }
    }
    fn inputs(extra: bool) -> BTreeMap<InputKey, InputValue> {
        let mut inputs = BTreeMap::from([(
            InputKey::CompileEnv("a".into()),
            InputValue::CompileEnv("1".into()),
        )]);
        if extra {
            inputs.insert(
                InputKey::CompileEnv("not-read".into()),
                InputValue::CompileEnv("2".into()),
            );
        }
        inputs
    }
    fn membership_case(before: bool, after: bool) {
        let mut db = Database::default();
        db.publish_inputs(inputs(before)).unwrap();
        let key = QueryKey::Constant(
            fixture_key(
                Digest([191; 32]),
                vec![],
                Digest::default(),
                crate::core::profile::ProfileIdentity::current(),
            )
            .bind_universe(db.constant_input_universe().unwrap()),
        );
        let old_a = db
            .published_input(&InputKey::CompileEnv("a".into()))
            .unwrap()
            .1;
        db.fetch(&ReadA, key.clone()).into_ready().unwrap().unwrap();
        assert_eq!(db.memo(&key).unwrap().dependencies.len(), 1);
        db.publish_inputs(inputs(after)).unwrap();
        assert_eq!(
            db.published_input(&InputKey::CompileEnv("a".into()))
                .unwrap()
                .1,
            old_a
        );
        let error = db
            .fetch(&ReadA, key.clone())
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-SOURCE");
        assert!(db.memo(&key).is_none());
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 1);
    }
    #[test]
    fn adding_an_unobserved_input_blocks_constant_memo_reuse() {
        membership_case(false, true);
    }
    #[test]
    fn deleting_an_unobserved_input_blocks_constant_memo_reuse() {
        membership_case(true, false);
    }
}
