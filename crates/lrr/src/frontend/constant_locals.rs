//! Private evaluator activations; no guest handles or persistent type authority.
use super::*;

struct LocalCell {
    value: ConstValue,
    mutable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        source::snapshot::{CfgSet, SourceBlob},
    };
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/locals-in-const-fn.rs"
    );
    fn source(text: &str, edition: u16) -> ConstantSource {
        ConstantSource::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            Digest([217; 32]),
            CfgSet::guest_default(),
        )
        .unwrap()
    }
    fn call(source: &ConstantSource, name: &str) -> ConstantPlan {
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == name)
            .unwrap()
            .range();
        source
            .const_call(
                at,
                vec![
                    ConstValue::from_scalar(ValueRepr::unsigned(TypeId(13), 42).unwrap()).unwrap(),
                ],
            )
            .unwrap()
    }
    #[test]
    fn original_mutable_parameter_and_nested_local_writes_keep_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let source = source(ORIGINAL, edition);
            for (name, expected) in [("foo", 42), ("boo", 52)] {
                let plan = call(&source, name);
                let value = evaluate(&plan, ConstLimits::default()).unwrap();
                assert_eq!(value.source(), Digest::of_bytes(ORIGINAL.as_bytes()));
                assert_eq!(
                    value.value().scalar().unwrap(),
                    ValueRepr::unsigned(TypeId(13), expected).unwrap()
                );
                assert_eq!(plan.profile().edition, edition);
            }
        }
    }
    #[test]
    fn original_shared_reference_const_function_keeps_its_reached_frontier() {
        let source = source(ORIGINAL, 2024);
        let error = evaluate(&call(&source, "bar"), ConstLimits::default()).unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-UNSUPPORTED");
        assert_eq!(error.source(), Digest::of_bytes(ORIGINAL.as_bytes()));
    }
    fn number(text: &str) -> ValueRepr {
        let source = source(text, 2024);
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == "A")
            .unwrap()
            .range();
        evaluate(&source.constant(at).unwrap(), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap()
    }
    #[test]
    fn loop_scope_retains_outer_writes_and_pops_shadow_on_break_and_continue() {
        assert_eq!(
            number(
                "const A:i32={let mut x:i32=0; while x<4 {x+=1; let x:i32=99; if x==99 {continue;} missing();} x};"
            ),
            ValueRepr::signed(TypeId(6), 4).unwrap()
        );
        assert_eq!(
            number(
                "const A:i32={let mut x:i32=0; loop {x+=1; let x:i32=99; if x==99 {break;} missing();} x};"
            ),
            ValueRepr::signed(TypeId(6), 1).unwrap()
        );
    }
    #[test]
    fn primitive_compound_assignment_reads_lhs_after_rhs_scope_writes() {
        assert_eq!(
            number("const A:i32={let mut x:i32=1; x += {x=10; 2}; x};"),
            ValueRepr::signed(TypeId(6), 12).unwrap()
        );
    }
    #[test]
    fn local_replace_refuses_immutable_and_wrong_type_without_commit() {
        let mut locals = LocalScopes::default();
        locals.push().unwrap();
        let old = ConstValue::from_scalar(ValueRepr::signed(TypeId(6), 7).unwrap()).unwrap();
        locals.insert("x".into(), false, old.clone()).unwrap();
        let next = ConstValue::from_scalar(ValueRepr::signed(TypeId(6), 8).unwrap()).unwrap();
        assert_eq!(
            locals.replace("x", next).unwrap_err().code,
            "LRR-E-CONST-TYPE"
        );
        assert_eq!(locals.get("x"), Some(&old));
        locals.insert("x".into(), true, old.clone()).unwrap();
        assert!(
            locals
                .replace(
                    "x",
                    ConstValue::from_scalar(ValueRepr::boolean(true)).unwrap()
                )
                .is_err()
        );
        assert_eq!(locals.get("x"), Some(&old));
        locals.pop().unwrap();
        assert!(locals.get("x").is_none());
    }
}
#[derive(Default)]
pub(super) struct LocalScopes {
    scopes: Vec<BTreeMap<String, LocalCell>>,
    bindings: usize,
}
impl LocalScopes {
    pub(super) fn push(&mut self) -> Result<(), Diagnostic> {
        self.scopes
            .try_reserve(1)
            .map_err(|_| fault("LRR-E-CONST-LIMIT", "constant local scope allocation"))?;
        self.scopes.push(BTreeMap::new());
        Ok(())
    }
    pub(super) fn pop(&mut self) -> Result<(), Diagnostic> {
        let scope = self
            .scopes
            .pop()
            .ok_or_else(|| fault("LRR-E-CONST-SOURCE", "constant local scope is absent"))?;
        self.bindings = self.bindings.checked_sub(scope.len()).ok_or_else(|| {
            fault(
                "LRR-E-CONST-SOURCE",
                "constant local scope accounting differs",
            )
        })?;
        Ok(())
    }
    pub(super) fn get(&self, name: &str) -> Option<&ConstValue> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .map(|cell| &cell.value)
    }
    pub(super) fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    pub(super) fn insert(
        &mut self,
        name: String,
        mutable: bool,
        value: ConstValue,
    ) -> Result<(), Diagnostic> {
        let scope = self.scopes.last_mut().ok_or_else(|| {
            fault(
                "LRR-E-CONST-SOURCE",
                "constant binding activation is absent",
            )
        })?;
        if !scope.contains_key(&name) {
            self.bindings = self
                .bindings
                .checked_add(1)                .ok_or_else(|| fault("LRR-E-CONST-LIMIT", "constant local binding limit"))?;
        }
        scope.insert(name, LocalCell { value, mutable });
        Ok(())
    }
    pub(super) fn replace(&mut self, name: &str, value: ConstValue) -> Result<(), Diagnostic> {
        let cell = self
            .scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
            .ok_or_else(|| {
                fault(
                    "LRR-E-CONST-SOURCE",
                    "assignment has no local source binding",
                )
            })?;
        if !cell.mutable {
            return Err(fault(
                "LRR-E-CONST-TYPE",
                "assignment requires a mutable constant local",
            ));
        }
        if cell.value.scalar().map(|v| v.type_id) != value.scalar().map(|v| v.type_id)
            || value.scalar().is_none()
        {
            return Err(fault(
                "LRR-E-CONST-TYPE",
                "constant local assignment type differs",
            ));
        }
        cell.value = value;
        Ok(())
    }
}
