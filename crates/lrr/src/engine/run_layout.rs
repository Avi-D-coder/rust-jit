//! Layout declaration demands for the `core::mem` layout intrinsics
//! (E-R5-012).
//!
//! `size_of::<T>()`, `align_of_val(&v)`, `needs_drop::<T>()` and friends are
//! answered by `GuestStore` from type descriptors (`runtime/cell_layout.rs`,
//! one algorithm in `runtime/layout.rs`). What a descriptor cannot say - a
//! nominal type's declared field types, `repr` and discriminants - is demanded
//! here, once per declaration, through the same schema binder constructors use
//! (`SchemaNames::bind_variant_current`), and installed in the store before the
//! intrinsic runs. Nothing here executes guest code or parses source during
//! the intrinsic itself.
use super::*;
use crate::{
    facade::functions::LibraryFunction,
    frontend::types::{NominalBinding, NominalBindingDemand, NominalSchema, SchemaNames},
    runtime::{
        cell::layouts::{DeclarationFacts, LayoutDemand, NominalKind},
        layout::Repr,
        shape::TypeId,
    },
    source::manifest::ItemKind,
};

fn layout_unsupported(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// Physical layout uses the same source representation plan as the solver.
fn declared_repr(schema: &NominalSchema, name: &str) -> Result<Repr, Diagnostic> {
    Ok(crate::frontend::declaration_repr::declared_repr(schema, name)?.layout)
}

impl RunMachine {
    /// Install, from the source database, every declaration the layout of
    /// `ty` needs, then instantiate their field types.
    pub(super) fn ensure_layout_facts(&mut self, db: &mut Database, ty: TypeId) -> QueryPoll<()> {
        loop {
            let demand = match self.store.layout_demand(ty) {
                Ok(None) => return QueryPoll::Ready(Ok(())),
                Ok(Some(demand)) => demand,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let facts = ready!(self.declaration_layout_facts(db, &demand));
            self.store.install_layout_declaration(demand.identity, facts);
        }
    }

    fn declaration_layout_facts(
        &mut self,
        db: &mut Database,
        demand: &LayoutDemand,
    ) -> QueryPoll<DeclarationFacts> {
        let name = demand.definition.declared_name.clone();
        let kind = match demand.definition.item_kind {
            ItemKind::Struct => NominalKind::Struct,
            ItemKind::Enum => NominalKind::Enum,
            ItemKind::Union => NominalKind::Union,
            _ => {
                return QueryPoll::Ready(Err(layout_unsupported(format!(
                    "`{name}` is not a struct, enum or union"
                ))));
            }
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "type layout requires a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let captured = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(demand.identity)));
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("layout schema outcome differs")));
        };
        let names = ready!(SchemaNames::from_current_database(
            db, graph, &sources, schema
        ));
        let repr = match declared_repr(schema, &name) {
            Ok(repr) => repr,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut variants = Vec::with_capacity(schema.variants.len());
        for variant in &schema.variants {
            match ready!(names.bind_variant_current(db, &sources, variant.name.as_deref())) {
                NominalBinding::Ready(bound) => variants.push(bound.fields().to_vec()),
                NominalBinding::Deferred { demand, .. } => {
                    return QueryPoll::Ready(Err(layout_unsupported(format!(
                        "the layout of `{name}` requires {}",
                        match demand {
                            NominalBindingDemand::GenericContext => "its generic type context",
                            NominalBindingDemand::TypeRecipe =>
                                "a field type the source type binder cannot state yet",
                            NominalBindingDemand::Name(_) => "further source type binding",
                        }
                    ))));
                }
            }
        }
        let discriminants = if kind == NominalKind::Enum {
            let table = match schema.literal_discriminants() {
                Ok(table) => table,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let mut values = Vec::with_capacity(table.len());
            for (index, value) in table.iter().enumerate() {
                match value {
                    Some(value) => values.push(*value),
                    None => {
                        return QueryPoll::Ready(Err(layout_unsupported(format!(
                            "the layout of enum `{name}` needs discriminant {index}, \
                             an expression only the pure constant evaluator can compute"
                        ))));
                    }
                }
            }
            values
        } else {
            Vec::new()
        };
        let library =
            crate::facade::library_layouts::library_layout(self.frame.body.profile(), &demand.definition);
        QueryPoll::Ready(Ok(DeclarationFacts {
            kind,
            repr,
            variants,
            discriminants,
            library,
        }))
    }

    /// Before a layout intrinsic is sealed: install the declarations its
    /// type needs. A type-argument intrinsic reads the enclosing frame's type
    /// parameters; a value intrinsic reads the type its reference designates.
    pub(super) fn prepare_facade_layout(
        &mut self,
        db: &mut Database,
        site: u32,
        arguments: &[SlotId],
    ) -> QueryPoll<()> {
        let identity = self.frame.call_identity(site);
        let Some((function, _)) = self.frame.facade_calls.get(&identity).copied() else {
            return QueryPoll::Ready(Ok(()));
        };
        let types: Vec<TypeId> = match function {
            LibraryFunction::SizeOf
            | LibraryFunction::AlignOf
            | LibraryFunction::NeedsDrop
            | LibraryFunction::VariantCount
            | LibraryFunction::Zeroed
            | LibraryFunction::Transmute => {
                match self.frame.enclosing_type_arguments(function.type_arguments()) {
                    Ok(types) => types,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            LibraryFunction::SizeOfVal | LibraryFunction::AlignOfVal => {
                let Some(slot) = arguments.first() else {
                    return QueryPoll::Ready(Ok(()));
                };
                let value = match self
                    .frame
                    .cell(*slot)
                    .and_then(|cell| self.store.live_value(cell))
                {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                match self.store.referent_sized_type(value) {
                    Ok(ty) => ty.into_iter().collect(),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            _ => Vec::new(),
        };
        for ty in types {
            ready!(self.ensure_layout_facts(db, ty));
        }
        QueryPoll::Ready(Ok(()))
    }
}
