//! The written Type of a closed source term selected by a call contract
//! (`-> Option<u32>`, `-> (W<i32>, u8)`): each nominal is the declaration
//! the term names, keyed by its current schema, so its run-local descriptor
//! can be interned before any value of it exists.
use super::types::schema_fault;
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    ir::written_type::WrittenType,
    source::{changes::PreparedSourceView, manifest::ItemKind},
    syntax::signatures::GenericKind,
    traits::term::{ConstTerm, GenericArg, RegionTerm, TyTerm},
};

/// Whether a nominal Type occurs in `term`.
pub(crate) fn contains_nominal(term: &TyTerm) -> bool {
    match term {
        TyTerm::Nominal(..) => true,
        TyTerm::Tuple(members) => members.iter().any(contains_nominal),
        TyTerm::Array(element, _) | TyTerm::Slice(element) | TyTerm::Ref(_, _, element) => {
            contains_nominal(element)
        }
        _ => false,
    }
}

/// `None` when some part is not a closed written Type (a parameter,
/// projection, trait object, const-generic argument or function type).
pub(crate) fn closed_written_type(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    term: &TyTerm,
) -> QueryPoll<Option<WrittenType>> {
    written_type(db, sources, term, None)
}

/// The written template of a declared parameter Type (`Option<T>`,
/// `(T, W<u8>)`, `&Self`): the callee's own type parameters become
/// `Parameter(ordinal)`, fixed at the call by explicit or earlier arguments,
/// and a part with no written structure here (another declaration's
/// parameter, a projection, a function Type, a const argument) is left to
/// inference. `None` when the whole Type is such a part.
pub(crate) fn written_template(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    term: &TyTerm,
    definition: crate::core::digest::Digest,
) -> QueryPoll<Option<WrittenType>> {
    match written_type(db, sources, term, Some(definition)) {
        QueryPoll::Ready(Ok(Some(WrittenType::Infer))) => QueryPoll::Ready(Ok(None)),
        other => other,
    }
}

/// Closed (`template` is `None`) or parameter-template conversion.
fn written_type(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    term: &TyTerm,
    template: Option<crate::core::digest::Digest>,
) -> QueryPoll<Option<WrittenType>> {
    // Outside the closed conversion, an unwritten part is an inference hole.
    let open = || match template {
        Some(_) => QueryPoll::Ready(Ok(Some(WrittenType::Infer))),
        None => QueryPoll::Ready(Ok(None)),
    };
    macro_rules! nested {
        ($term:expr) => {
            match written_type(db, sources, $term, template) {
                QueryPoll::Ready(Ok(Some(written))) => written,
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    if let (Some(definition), TyTerm::Param(key)) = (template, term)
        && key.definition == definition
    {
        return QueryPoll::Ready(Ok(Some(WrittenType::Parameter(key.ordinal))));
    }
    if template.is_some() && let TyTerm::Dyn(schema) = term {
        return QueryPoll::Ready(Ok(Some(WrittenType::Dyn(*schema))));
    }
    QueryPoll::Ready(Ok(Some(match term {
        TyTerm::Primitive(primitive) => WrittenType::Primitive(*primitive),
        TyTerm::Str => WrittenType::Str,
        TyTerm::Tuple(members) if members.is_empty() => {
            WrittenType::Primitive(crate::ir::op::PrimitiveType::Unit)
        }
        TyTerm::Tuple(members) => {
            let mut written = Vec::new();
            for member in members {
                written.push(nested!(member));
            }
            WrittenType::Tuple(written)
        }
        TyTerm::Array(element, ConstTerm::Value(len)) => {
            let Ok(len) = u32::try_from(*len) else {
                return open();
            };
            WrittenType::Array(Box::new(nested!(element)), len)
        }
        TyTerm::Slice(element) => WrittenType::Slice(Box::new(nested!(element))),
        // Regions are not runtime data.
        TyTerm::Ref(_, mutable, target) => WrittenType::Ref {
            region: RegionTerm::Static,
            mutable: *mutable,
            target: Box::new(nested!(target)),
        },
        TyTerm::Nominal(identity, arguments) => {
            let definition = match super::source_types::nominal_definition(db, sources, *identity) {
                Ok(definition) => definition,
                // A nominal without a catalogued declaration (a closure or
                // function item's own Type) has no written form here.
                Err(_) => return open(),
            };
            if !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum) {
                return open();
            }
            let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(*identity)) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                return QueryPoll::Ready(Err(schema_fault("written result schema differs")));
            };
            if schema
                .generics
                .iter()
                .any(|parameter| parameter.kind == GenericKind::Const)
            {
                return open();
            }
            let parameters = schema
                .generics
                .iter()
                .filter(|parameter| parameter.kind == GenericKind::Type)
                .count();
            let schema = schema.fingerprint();
            let mut args = Vec::new();
            for argument in arguments {
                match argument {
                    GenericArg::Type(ty) => args.push(nested!(ty)),
                    GenericArg::Region(_) => {}
                    GenericArg::Const(_) => return open(),
                }
            }
            if args.len() != parameters {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "result type argument count differs from the declaration".into(),
                )));
            }
            WrittenType::Generic {
                identity: *identity,
                definition,
                schema,
                args,
            }
        }
        _ => return open(),
    })))
}
