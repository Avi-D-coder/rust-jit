//! Owned symbolic header admission; expression/runtime propagation is M018.
use crate::{
    core::diagnostic::Diagnostic,
    traits::{
        subst::validate_binder_type,
        term::{Binder, TyTerm},
    },
};
/// The syntax adapter supplies an owned term. No parser node enters frontend
/// state, and admission neither selects a trait nor publishes a runtime view.
pub fn function_predicate(predicate: Binder<TyTerm>) -> Result<Binder<TyTerm>, Diagnostic> {
    if !matches!(predicate.value, TyTerm::Function(_)) {
        return Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "symbolic predicate needs further ordered source-type integration".into(),
        ));
    }
    validate_binder_type(&predicate)?;
    Ok(predicate)
}
