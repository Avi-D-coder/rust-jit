//! Demand-owned field syntax, using the same original/generated parser boundary.
use super::{
    ast_adapter::{semantic_name, tokens},
    outline::{DeclarationContext, HeaderRecipe},
    parse::ParsedFile,
    signatures::{DefinitionParameter, capture_generics, lower_type},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    frontend::types::{
        FieldRecipe, FieldShape, NominalSchema, VariantRecipe,
        schema_fault, schema_limit,
    },
    source::snapshot::CfgSet,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasGenericParams, HasName},
};

impl ParsedFile {
    /// This is called only by a schema demand, never outline/type identity lookup.
    /// Field type names and discriminants remain unresolved syntax recipes.
    pub fn nominal_schema(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        cfg: &CfgSet,
    ) -> Result<NominalSchema, Diagnostic> {
        self.nominal_schema_with_field_limit(definition, header, context, cfg, usize::MAX)
    }
    fn nominal_schema_with_field_limit(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        cfg: &CfgSet,
        field_limit: usize,
    ) -> Result<NominalSchema, Diagnostic> {
        let mut remaining_fields = field_limit;
        if header.profile != self.profile {
            return Err(schema_fault(
                "schema/source profile or header budget mismatch",
            ));
        }
        let selected = self
            .syntax_at(context.range, |node| {
                ast::Struct::can_cast(node.kind()) || ast::Enum::can_cast(node.kind())
            })?
            .and_then(ast::Item::cast)
            .filter(|item| tokens(item.syntax(), None) == header.tokens)
            .ok_or_else(|| {
                schema_fault("schema declaration absent from immutable source context")
            })?;
        let (generics, variants) = match selected {
            ast::Item::Struct(item) => {
                let generics = capture_generics(item.generic_param_list())?;
                let variant = capture_fields(
                    item.field_list(),
                    None,
                    Vec::new(),
                    definition,
                    &generics,
                    cfg,
                    &mut remaining_fields,
                )?;
                (generics, vec![variant])
            }
            ast::Item::Enum(item) => {
                let generics = capture_generics(item.generic_param_list())?;
                let list = item
                    .variant_list()
                    .ok_or_else(|| schema_fault("enum variant boundaries absent"))?;
                let mut variants = Vec::new();
                for variant in list.variants() {
                    if !enabled(&variant, cfg)? {
                        continue;
                    }
                    let name = variant
                        .name()
                        .and_then(|name| semantic_name(name.text()))
                        .ok_or_else(|| schema_fault("enum variant lacks its declared name"))?;
                    let discriminant = variant
                        .const_arg()
                        .map_or_else(Vec::new, |arg| tokens(arg.syntax(), None));
                    let captured = capture_fields(
                        variant.field_list(),
                        Some(name),
                        discriminant,
                        definition,
                        &generics,
                        cfg,
                        &mut remaining_fields,
                    )?;
                    variants.try_reserve(1).map_err(|_| schema_limit())?;
                    variants.push(captured);
                }
                (generics, variants)
            }
            _ => return Err(schema_fault("non-nominal schema declaration")),
        };
        let schema = NominalSchema {
            definition,
            scope: context.scope.clone(),
            header: header.clone(),
            generics,
            variants,
        };
        Ok(schema)
    }
}
fn enabled(item: &impl HasAttrs, cfg: &CfgSet) -> Result<bool, Diagnostic> {
    super::attributes::effective_metas(item.attrs().filter_map(|attr| attr.meta()).collect(), cfg)
        .map(|(enabled, _)| enabled)
}
fn capture_fields(
    list: Option<ast::FieldList>,
    name: Option<String>,
    discriminant: Vec<super::ast_adapter::TokenRecipe>,
    definition: Digest,
    generics: &[DefinitionParameter],
    cfg: &CfgSet,
    remaining_fields: &mut usize,
) -> Result<VariantRecipe, Diagnostic> {
    let mut fields = Vec::new();
    let shape = match list {
        None => FieldShape::Unit,
        Some(ast::FieldList::RecordFieldList(list)) => {
            for field in list.fields() {
                if !enabled(&field, cfg)? {
                    continue;
                }
                *remaining_fields = remaining_fields.checked_sub(1).ok_or_else(schema_limit)?;
                let name = field
                    .name()
                    .and_then(|name| semantic_name(name.text()))
                    .ok_or_else(|| schema_fault("record field lacks its declared name"))?;
                let ty = field
                    .ty()
                    .ok_or_else(|| schema_fault("record field lacks its type"))?;
                if field.default_val().is_some()
                    || field.mut_restriction().is_some()
                    || field.unsafe_token().is_some()
                {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "reached field schema uses an excluded field feature".into(),
                    ));
                }
                push_field(
                    &mut fields,
                    FieldRecipe {
                        name: Some(name),
                        visibility: super::outline::visibility(field.syntax()),
                        ty: tokens(ty.syntax(), None),
                        type_range: super::ast_adapter::range(ty.syntax().text_range()),
                        symbolic: lower_type(&ty, definition, generics, 0)?,
                    },
                )?;
            }
            FieldShape::Record
        }
        Some(ast::FieldList::TupleFieldList(list)) => {
            for field in list.fields() {
                if !enabled(&field, cfg)? {
                    continue;
                }
                *remaining_fields = remaining_fields.checked_sub(1).ok_or_else(schema_limit)?;
                if field.mut_restriction().is_some() {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "reached tuple field uses an excluded field feature".into(),
                    ));
                }
                let ty = field
                    .ty()
                    .ok_or_else(|| schema_fault("tuple field lacks its type"))?;
                push_field(
                    &mut fields,
                    FieldRecipe {
                        name: None,
                        visibility: super::outline::visibility(field.syntax()),
                        ty: tokens(ty.syntax(), None),
                        type_range: super::ast_adapter::range(ty.syntax().text_range()),
                        symbolic: lower_type(&ty, definition, generics, 0)?,
                    },
                )?;
            }
            FieldShape::Tuple
        }
    };
    Ok(VariantRecipe {
        name,
        shape,
        fields,
        discriminant,
    })
}
fn push_field(fields: &mut Vec<FieldRecipe>, field: FieldRecipe) -> Result<(), Diagnostic> {
    fields.try_reserve(1).map_err(|_| schema_limit())?;
    fields.push(field);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        db::{key::QueryKey, memo::Database, provider::SyntaxProvider, value::QueryValue},
        source::{
            changes::{PreparedSourceInputs, SourceUnitInput},
            snapshot::{SourceBlob, SourcePath},
        },
        syntax::outline::Outline,
    };
    use std::collections::BTreeMap;

    const NEW_FIELD: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-new-as-field-name.rs"
    );
    const DUPLICATE_SCHEMA: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-fields-decl-dupe.rs"
    );
    const DUPLICATE_INITIALIZER: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-fields-dupe.rs"
    );
    const MISSING_INITIALIZER: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-fields-missing.rs"
    );
    const PRIMITIVE_SHADOW: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/primitive-type-shadowing.rs"
    );
    const STRUCT_ORDER: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-2.rs"
    );
    const DROP_ORDER: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/rfcs/rfc-1857-stabilize-drop-order/drop-order.rs"
    );
    const ENUM_COLD_VARIANT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/issue-61696.rs"
    );
    const STRUCT_UPDATE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-3.rs"
    );

    fn publish(source: &str) -> (Database, ParsedFile, Outline) {
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let crate_key = Digest::of_bytes(b"rustc-type-schema-fixture");
        let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
        let outline = parsed.outline(crate_key, vec![], &cfg).unwrap();
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path: SourcePath::new("main.rs").unwrap(),
                crate_key,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            BTreeMap::new(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        (db, parsed, outline)
    }
    fn source_view(source: &str, outline: &Outline) -> crate::source::changes::PreparedSourceView {
        use crate::{
            db::key::OutlineKey,
            source::changes::{SourceSelection, SourceSelectionOutcome},
            syntax::parse::ParseKey,
        };
        let path = SourcePath::new("main.rs").unwrap();
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path: path.clone(),
                crate_key: outline.root_scope.crate_key,
                module_path: vec![],
                profile: outline.profile.clone(),
                cfg: CfgSet::guest_default(),
                blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            }],
            BTreeMap::new(),
        )
        .unwrap();
        let selection = SourceSelection {
            path,
            outline: OutlineKey {
                parse: ParseKey {
                    content: outline.content,
                    profile: outline.profile.clone(),
                },
                crate_key: outline.root_scope.crate_key,
                module_path: vec![],
                cfg: outline.cfg,
            },
        };
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("stock source view absent");
        };
        view
    }
    fn demand(db: &mut Database, outline: &Outline, name: &str) -> NominalSchema {
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap();
        let result = db
            .fetch(
                &SyntaxProvider,
                QueryKey::TypeSchema(item.def_key.fingerprint()),
            )
            .into_ready()
            .expect("captured fixture needs no external input")
            .unwrap();
        let QueryValue::TypeSchema(schema) = result.value.as_ref() else {
            panic!("expected owned nominal schema")
        };
        schema.as_ref().clone()
    }
    fn initializers(parsed: &ParsedFile, name: &str) -> Vec<Vec<String>> {
        parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::RecordExpr::cast)
            .filter(|record| {
                record
                    .path()
                    .is_some_and(|path| path.syntax().text() == name)
            })
            .map(|record| {
                record
                    .record_expr_field_list()
                    .unwrap()
                    .fields()
                    .map(|field| field.name_ref().unwrap().text().to_string())
                    .collect()
            })
            .collect()
    }

    const STRUCT_ORDER_PLAIN: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-order-of-eval-4.rs"
    );
    const SHORTHAND_UNKNOWN: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-fields-shorthand.rs"
    );
    fn record_plan(
        parsed: &ParsedFile,
        outline: &Outline,
        name: &str,
    ) -> (
        crate::syntax::outline::BodyHandle,
        crate::syntax::body_index::BodyIndex,
        crate::syntax::constructors::ConstructorPlan,
    ) {
        let body = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let index = parsed.index_body(&body, Default::default()).unwrap();
        let plan = index
            .nodes()
            .iter()
            .filter_map(|node| {
                parsed
                    .constructor_plan(&body, &index, node.id, &CfgSet::guest_default())
                    .unwrap()
            })
            .find(|plan| {
                matches!(plan.kind(), crate::syntax::constructors::ConstructorKind::Record { path, .. }
            if path.iter().map(|token| token.token.text.as_str()).collect::<String>() == name)
            })
            .unwrap();
        (body, index, plan)
    }
    #[test]
    fn rustc_bound_record_map_retains_written_order_before_unsettled_type_demand() {
        use crate::frontend::{
            names::NameNamespaces,
            types::{NominalConstructorBinding, SchemaNames},
        };
        let (mut db, parsed, outline) = publish(STRUCT_ORDER_PLAIN);
        let schema = demand(&mut db, &outline, "S");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let source = source_view(STRUCT_ORDER_PLAIN, &outline);
        let (_, _, plan) = record_plan(&parsed, &outline, "S");
        let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let NominalConstructorBinding::Deferred { mapping, .. } = names
            .bind_record_constructor(None, &plan, &outline.root_scope)
            .unwrap()
        else {
            panic!("original std import and W field must not be guessed");
        };
        assert_eq!(mapping.written_to_declaration(), [1, 0]);
        assert_eq!(mapping.definition(), schema.definition);
        assert_eq!(mapping.schema(), schema.fingerprint());
        assert_eq!(mapping.constructor(), plan.fingerprint());
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_) | QueryKey::FunctionSignature(_) | QueryKey::Constant(_)
        )));
        // event(), std atomics and whole run-pass execution remain unexercised.
    }
    #[test]
    fn rustc_bound_record_shape_faults_precede_unresolved_field_types() {
        use crate::frontend::{names::NameNamespaces, types::SchemaNames};
        for (source_text, target, expected) in [
            (
                MISSING_INITIALIZER,
                "BuildData",
                "missing field in reached initializer",
            ),
            (
                DUPLICATE_INITIALIZER,
                "BuildData",
                "duplicate field in reached initializer",
            ),
            (
                SHORTHAND_UNKNOWN,
                "Foo",
                "initializer field is absent from reached schema",
            ),
        ] {
            let (mut db, parsed, outline) = publish(source_text);
            let schema = demand(&mut db, &outline, target);
            let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
            let source = source_view(source_text, &outline);
            let (_, _, plan) = record_plan(&parsed, &outline, target);
            let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
                .into_ready()
                .unwrap()
                .unwrap();
            let error = names
                .bind_record_constructor(None, &plan, &outline.root_scope)
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
            assert_eq!(error.message, expected);
            assert!(!db.trace.events.iter().any(|event| matches!(
                event.key,
                QueryKey::Thunk(_) | QueryKey::FunctionSignature(_) | QueryKey::Constant(_)
            )));
        }
    }
    #[test]
    fn rustc_bound_record_source_lowering_installs_isize_hint_before_original_literal() {
        use crate::{
            frontend::{
                names::NameNamespaces,
                types::{NominalConstructorBinding, SchemaNames},
            },
            ir::pc::Phase,
            runtime::{
                cell::{CellState, GuestStore},
                shape::TypeId,
            },
            syntax::constructors::ConstructorKind,
            vm::{
                frame::Frame,
                step::{StepOutcome, step},
            },
        };
        let (mut db, parsed, outline) = publish(NEW_FIELD);
        let schema = demand(&mut db, &outline, "Foo");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let source = source_view(NEW_FIELD, &outline);
        let (body, index, plan) = record_plan(&parsed, &outline, "Foo");
        let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let NominalConstructorBinding::Ready(bound) = names
            .bind_record_constructor(None, &plan, &outline.root_scope)
            .unwrap()
        else {
            panic!("stock Foo primitive field must bind");
        };
        assert_eq!(bound.mapping().written_to_declaration(), [0]);
        let mut foreign_caller = outline.root_scope.clone();
        foreign_caller.crate_key = Digest([222; 32]);
        assert_eq!(
            names
                .bind_record_constructor(None, &plan, &foreign_caller)
                .unwrap_err()
                .message,
            "reached constructor field is inaccessible"
        );
        let ConstructorKind::Record { fields, .. } = plan.kind() else {
            panic!("record absent")
        };
        let child = index.pc(fields[0].operand.node, Phase::Entry).unwrap();
        let parent = &index.nodes()[plan.node().0 as usize];
        let raw = parent
            .operands
            .iter()
            .position(|node| *node == fields[0].operand.node)
            .unwrap();
        let field_pc = index
            .pc(plan.node(), Phase::Operand(u16::try_from(raw).unwrap()))
            .unwrap();
        let entry = index.pc(plan.node(), Phase::Entry).unwrap();
        let commit = index.pc(plan.node(), Phase::Commit).unwrap();
        let start = parsed
            .constructor_thunk(&body, &index, &entry, &plan)
            .unwrap();
        let literal = parsed.initial_thunk(&body, &index, &child).unwrap();
        let field = parsed
            .constructor_thunk(&body, &index, &field_pc, &plan)
            .unwrap();
        let finish = parsed
            .constructor_thunk(&body, &index, &commit, &plan)
            .unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(index.clone(), entry, &mut store).unwrap();
        let mut scopes = Vec::new();
        let mut next = Some(parent.scope);
        while let Some(scope) = next {
            scopes.push(scope);
            next = index.scopes()[scope.0 as usize].parent;
        }
        for scope in scopes.into_iter().rev() {
            frame.enter(scope).unwrap();
        }
        let handles = store.budget().handles();
        frame
            .prepare_nominal_constructor(&plan, bound, &mut store)
            .unwrap();
        assert_eq!(store.budget().handles(), handles);
        assert!(
            matches!(step(&mut frame, &mut store, &start).unwrap(), StepOutcome::Next(pc) if pc == child)
        );
        assert!(
            matches!(step(&mut frame, &mut store, &literal).unwrap(), StepOutcome::Next(pc) if pc == field_pc)
        );
        let source_cell = frame.cell(fields[0].operand.value).unwrap();
        let actual = store.read_scalar(source_cell).unwrap();
        assert_eq!(actual.type_id, TypeId(8).0);
        assert!(
            matches!(step(&mut frame, &mut store, &field).unwrap(), StepOutcome::Next(pc) if pc == commit)
        );
        assert_eq!(store.state(source_cell).unwrap(), CellState::Moved);
        assert!(matches!(
            step(&mut frame, &mut store, &finish).unwrap(),
            StepOutcome::Next(_)
        ));
        for tag in [0, 27, 28, 29] {
            assert_eq!(frame.counters.started_by_opcode[tag], 1);
        }
        let ticket = store
            .move_from(frame.cell(parent.value.unwrap()).unwrap())
            .unwrap();
        let field_cell = store.aggregate_field(ticket, 0).unwrap();
        assert_eq!(store.read_scalar(field_cell).unwrap(), actual);
        let cleanup = store.aggregate_cleanup_next(ticket).unwrap().unwrap();
        store.discard(cleanup).unwrap();
        assert!(store.aggregate_cleanup_next(ticket).unwrap().is_none());
        store.discard(ticket).unwrap();
        // Actual dispatcher path selection, field projection and assert_eq!
        // still need integration before this original program is a fixture pass.
    }

    #[test]
    fn rustc_nominal_new_field_requires_source_guard_and_retains_isize_identity() {
        use crate::{
            frontend::{
                names::NameNamespaces,
                types::{NominalBinding, SchemaNames},
            },
            runtime::{
                cell::{CellState, GuestStore},
                shape::{CopyStatus, TypeId, TypeKey},
                value::ValueRepr,
            },
        };
        let (mut db, parsed, outline) = publish(NEW_FIELD);
        let schema = demand(&mut db, &outline, "Foo");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let source = source_view(NEW_FIELD, &outline);
        let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let NominalBinding::Ready(bound) = names.bind_variant(None).unwrap() else {
            panic!("stock primitive field binding deferred");
        };
        // Read the original initializer, rather than supplying an expected
        // program result. This exercises source binding/storage, not the VM body.
        let record = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::RecordExpr::cast)
            .find(|record| {
                record
                    .path()
                    .is_some_and(|path| path.syntax().text() == "Foo")
            })
            .unwrap();
        let field = record
            .record_expr_field_list()
            .unwrap()
            .fields()
            .next()
            .unwrap();
        let value = tokens(field.expr().unwrap().syntax(), None)[0]
            .text
            .parse::<i64>()
            .unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let (nominal, guard) = store.begin_nominal_construction(&bound).unwrap();
        let descriptor = store.type_descriptor(nominal).unwrap();
        assert!(
            matches!(descriptor.key.as_ref(), TypeKey::Nominal { definition, schema: captured, .. }
            if definition.fingerprint() == schema.definition && *captured == schema.fingerprint())
        );
        assert_eq!(descriptor.copy, CopyStatus::Deferred);
        let wrong = store
            .ticket(ValueRepr::signed(TypeId(7), value).unwrap())
            .unwrap();
        assert_eq!(
            store.construction_push(guard, 0, wrong).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(store.peek_ticket(wrong).unwrap().type_id, 7);
        store.discard(wrong).unwrap();
        let actual = ValueRepr::signed(TypeId(8), value).unwrap();
        let incoming = store.ticket(actual).unwrap();
        store.construction_push(guard, 0, incoming).unwrap();
        let structural = store
            .intern_type(TypeKey::Tuple(vec![TypeId(8)].into_boxed_slice()))
            .unwrap();
        assert_eq!(
            store
                .commit_construction(guard, structural)
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        // Even the identical concrete field contract cannot forge admission.
        let forged = store.begin_construction(&[TypeId(8)]).unwrap();
        let ticket = store.ticket(actual).unwrap();
        store.construction_push(forged, 0, ticket).unwrap();
        assert_eq!(
            store.commit_construction(forged, nominal).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        let cleanup = store.construction_cleanup_next(forged).unwrap().unwrap();
        assert_eq!(store.peek_ticket(cleanup).unwrap(), actual);
        store.discard(cleanup).unwrap();
        assert!(store.construction_cleanup_next(forged).unwrap().is_none());
        let committed = store.commit_construction(guard, nominal).unwrap();
        assert_eq!(store.aggregate_variant(committed).unwrap(), Some(0));
        let cell = store.aggregate_field(committed, 0).unwrap();
        assert_eq!(store.state(cell).unwrap(), CellState::Live(actual));
        let cleanup = store.aggregate_cleanup_next(committed).unwrap().unwrap();
        assert_eq!(store.peek_ticket(cleanup).unwrap(), actual);
        store.discard(cleanup).unwrap();
        assert!(store.aggregate_cleanup_next(committed).unwrap().is_none());
        store.discard(committed).unwrap();
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
    }

    #[test]
    fn rustc_enum_nominal_tags_bind_only_selected_variant_and_leave_source_fields_deferred() {
        use crate::{
            frontend::{
                names::NameNamespaces,
                signature_names::SignatureName,
                types::{NominalBinding, NominalBindingDemand, SchemaNames},
            },
            runtime::cell::GuestStore,
        };
        let (mut db, _, outline) = publish(ENUM_COLD_VARIANT);
        let schema = demand(&mut db, &outline, "E1");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let source = source_view(ENUM_COLD_VARIANT, &outline);
        let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let inaccessible = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "Infallible")
            .unwrap()
            .def_key
            .fingerprint();
        assert!(
            matches!(names.bind_variant(Some("V2")).unwrap(), NominalBinding::Deferred {
            field: Some(0), demand: NominalBindingDemand::Name(SignatureName::Source(found))
        } if found.identity == inaccessible)
        );
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut admitted = Vec::new();
        for name in ["V3", "V4"] {
            let NominalBinding::Ready(bound) = names.bind_variant(Some(name)).unwrap() else {
                panic!("cold unrelated field must not block unit variant");
            };
            let expected = schema
                .variants
                .iter()
                .position(|variant| variant.name.as_deref() == Some(name))
                .unwrap() as u32;
            let (ty, guard) = store.begin_nominal_construction(&bound).unwrap();
            assert_eq!(
                store
                    .commit_construction(guard, crate::runtime::shape::TypeId(1))
                    .unwrap_err()
                    .code,
                "LRR-E-OWNERSHIP"
            );
            let ticket = store.commit_construction(guard, ty).unwrap();
            assert_eq!(store.aggregate_variant(ticket).unwrap(), Some(expected));
            assert!(store.aggregate_cleanup_next(ticket).unwrap().is_none());
            admitted.push(ty);
            store.discard(ticket).unwrap();
        }
        assert_eq!(admitted[0], admitted[1]);
        // No discriminator cast, match evaluation or complete stock assertion
        // is claimed. The original generic E2's unit variant binds with its
        // type parameter left to the construction's context.
        let generic = demand(&mut db, &outline, "E2");
        let names = SchemaNames::from_database(&mut db, &graph, &source, &generic)
            .into_ready()
            .unwrap()
            .unwrap();
        let NominalBinding::Ready(bound) = names.bind_variant(Some("V3")).unwrap() else {
            panic!("generic unit variant must bind without its type context");
        };
        assert!(bound.fields().is_empty());
        assert_eq!(bound.type_parameters(), 1);
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
    }

    #[test]
    fn rustc_nominal_binding_suspends_on_stock_import_before_guest_allocation() {
        use crate::frontend::{
            names::{NameLookup, NameNamespaces},
            signature_names::SignatureName,
            types::{NominalBinding, NominalBindingDemand, SchemaNames},
        };
        let (mut db, _, outline) = publish(STRUCT_UPDATE);
        let schema = demand(&mut db, &outline, "S");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let source = source_view(STRUCT_UPDATE, &outline);
        let names = SchemaNames::from_database(&mut db, &graph, &source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(matches!(
            names.bind_variant(None).unwrap(),
            NominalBinding::Deferred {
                field: None,
                demand: NominalBindingDemand::Name(SignatureName::Unresolved(
                    NameLookup::NeedInput(_)
                )),
            }
        ));
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        // Real std import/native facade disclosure and full record-update
        // evaluation remain pending. No substitute namespace is manufactured.
    }

    #[test]
    fn rustc_named_new_field_captures_schema_without_body_or_type_resolution() {
        let (mut db, parsed, outline) = publish(NEW_FIELD);
        let schema = demand(&mut db, &outline, "Foo");
        let variant = schema.variant(None).unwrap();
        assert_eq!(variant.shape, FieldShape::Record);
        assert_eq!(variant.fields[0].name.as_deref(), Some("new"));
        assert_eq!(
            variant.fields[0]
                .ty
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            ["isize"]
        );
        let written = initializers(&parsed, "Foo");
        assert_eq!(written.len(), 1);
        assert_eq!(
            variant
                .named_initializer_slots(&written[0].iter().map(String::as_str).collect::<Vec<_>>())
                .unwrap(),
            [0]
        );
        let again = demand(&mut db, &outline, "Foo");
        assert_eq!(schema.fingerprint(), again.fingerprint());
        assert_eq!(db.trace.counts["TypeSchema:ProviderRun"], 1);
        assert_eq!(db.trace.counts["TypeSchema:MemoHit"], 1);
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_)
                | QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::Constant(_)
        )));
    }

    #[test]
    fn rustc_record_written_order_maps_to_independent_declaration_slots() {
        let (mut db, parsed, outline) = publish(DROP_ORDER);
        let schema = demand(&mut db, &outline, "TestStruct");
        let variant = schema.variant(None).unwrap();
        assert_eq!(
            variant
                .fields
                .iter()
                .map(|field| field.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["x", "y", "z"]
        );
        let written = initializers(&parsed, "TestStruct");
        assert_eq!(written.len(), 3);
        let plans = written
            .iter()
            .map(|fields| {
                variant
                    .named_initializer_slots(&fields.iter().map(String::as_str).collect::<Vec<_>>())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(plans, [vec![0, 1, 2], vec![0, 1, 2], vec![1, 0, 2]]);
        // This is source-plan acceptance. No Drop/catch_unwind execution is claimed.
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }

    #[test]
    fn rustc_enum_schema_field_cap_applies_across_variants() {
        use crate::db::dependency::{InputKey, InputValue};
        let (mut db, parsed, outline) = publish(DROP_ORDER);
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "TestEnum")
            .unwrap();
        let definition = item.def_key.fingerprint();
        let context = db.input(&InputKey::DeclarationContext(definition)).unwrap();
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            panic!("stock enum context absent");
        };
        let cfg = CfgSet::guest_default();
        let error = parsed
            .nominal_schema_with_field_limit(definition, &item.header, context, &cfg, 5)
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
        let schema = parsed
            .nominal_schema_with_field_limit(definition, &item.header, context, &cfg, 6)
            .unwrap();
        assert_eq!(
            schema
                .variants
                .iter()
                .map(|variant| variant.fields.len())
                .collect::<Vec<_>>(),
            [3, 3]
        );
        assert_eq!(
            schema
                .variants
                .iter()
                .map(|variant| variant.fields.len())
                .sum::<usize>(),
            6
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }

    #[test]
    fn rustc_schema_name_view_respects_source_primitive_shadow_and_refuses_forgery() {
        use crate::frontend::{
            names::NameNamespaces, signature_names::SignatureName, types::SchemaNames,
        };
        let (mut db, _, outline) = publish(PRIMITIVE_SHADOW);
        let schema = demand(&mut db, &outline, "bool");
        let mut graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let current_source = source_view(PRIMITIVE_SHADOW, &outline);
        {
            let view = SchemaNames::from_database(&mut db, &graph, &current_source, &schema)
                .into_ready()
                .unwrap()
                .unwrap();
            let unresolved = view.unqualified("bool", None).unwrap();
            assert!(
                matches!(
                    unresolved,
                    SignatureName::Unresolved(crate::frontend::names::NameLookup::NeedInput(_))
                ),
                "unsettled stock import must suspend source lookup: {unresolved:?}"
            );
        }
        // The original fixture imports bar::QueryId. A disclosed definition
        // does not establish namespace completeness before import settlement.
        let imports = graph.resolve_imports().unwrap();
        assert_eq!(imports.resolved.len(), 1);
        assert!(imports.pending.is_empty(), "{imports:?}");
        assert!(imports.missing.is_empty(), "{imports:?}");
        assert!(imports.inaccessible.is_empty(), "{imports:?}");
        assert!(imports.ambiguous.is_empty(), "{imports:?}");
        let view = SchemaNames::from_database(&mut db, &graph, &current_source, &schema)
            .into_ready()
            .unwrap()
            .unwrap();
        let resolved = view.unqualified("bool", None).unwrap();
        assert!(
            matches!(&resolved, SignatureName::Source(definition) if definition.identity == schema.definition),
            "settled stock shadow must retain captured source identity: {resolved:?}"
        );
        let (mut db, _, outline) = publish(NEW_FIELD);
        let schema = demand(&mut db, &outline, "Foo");
        let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let mut forged = schema.clone();
        forged.variants[0].fields[0].name = Some("forged".into());
        let current_source = source_view(NEW_FIELD, &outline);
        let error = SchemaNames::from_database(&mut db, &graph, &current_source, &forged)
            .into_ready()
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
        assert_eq!(
            error.message,
            "field schema is stale or differs from its captured recipe"
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
        // No field type or trait is inferred from a runtime descriptor/name pair.
        // The original check-pass fixture's full associated-const semantics are pending.
    }

    #[test]
    fn rustc_written_record_plan_commits_owned_fields_in_declaration_order() {
        use crate::runtime::{
            cell::{CellState, GuestStore},
            shape::{FacadeType, TypeKey},
        };
        let (mut db, parsed, outline) = publish(STRUCT_ORDER);
        let schema = demand(&mut db, &outline, "S");
        let variant = schema.variant(None).unwrap();
        let written = initializers(&parsed, "S");
        assert_eq!(written.len(), 1);
        assert_ne!(
            written[0],
            variant
                .fields
                .iter()
                .map(|field| field.name.clone().unwrap())
                .collect::<Vec<_>>()
        );
        assert!(variant.fields.iter().all(|field| {
            field.ty.iter().map(|t| t.text.as_str()).collect::<Vec<_>>() == ["String"]
        }));
        let plan = variant
            .named_initializer_slots(&written[0].iter().map(String::as_str).collect::<Vec<_>>())
            .unwrap();
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let string = store
            .intern_type(TypeKey::Facade {
                family: FacadeType::String,
                args: Box::new([]),
            })
            .unwrap();
        // Structural storage transport only: source nominal binding, actual
        // to_string calls and complete stock assertions remain pending.
        let tuple = store
            .intern_type(TypeKey::Tuple(
                vec![string; variant.fields.len()].into_boxed_slice(),
            ))
            .unwrap();
        let guard = store
            .begin_construction(&vec![string; variant.fields.len()])
            .unwrap();
        let mut values = BTreeMap::new();
        for (name, ordinal) in written[0].iter().zip(plan) {
            let ticket = store.string(name).unwrap();
            values.insert(name.clone(), store.peek_ticket(ticket).unwrap());
            store.construction_push(guard, ordinal, ticket).unwrap();
        }
        let committed = store.commit_construction(guard, tuple).unwrap();
        assert_eq!(
            store.construction_cleanup_next(guard).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(
            store.discard(committed).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        for (ordinal, field) in variant.fields.iter().enumerate() {
            let cell = store.aggregate_field(committed, ordinal as u32).unwrap();
            let name = field.name.as_ref().unwrap();
            assert_eq!(store.state(cell).unwrap(), CellState::Live(values[name]));
        }
        for field in &variant.fields {
            let child = store.aggregate_cleanup_next(committed).unwrap().unwrap();
            let name = field.name.as_ref().unwrap();
            assert_eq!(store.peek_ticket(child).unwrap(), values[name]);
            assert_eq!(store.string_contents(child).unwrap(), name);
            store.discard(child).unwrap();
        }
        assert!(store.aggregate_cleanup_next(committed).unwrap().is_none());
        assert!(store.aggregate_cleanup_next(committed).unwrap().is_none());
        store.discard(committed).unwrap();
        assert_eq!(
            store.peek_ticket(committed).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }

    #[test]
    fn rustc_partial_record_plan_guard_transfers_reverse_written_prefix_once() {
        use crate::runtime::{
            cell::GuestStore,
            shape::{FacadeType, TypeKey},
        };
        let (mut db, parsed, outline) = publish(DROP_ORDER);
        let schema = demand(&mut db, &outline, "TestStruct");
        let variant = schema.variant(None).unwrap();
        let written = initializers(&parsed, "TestStruct");
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let string = store
            .intern_type(TypeKey::Facade {
                family: FacadeType::String,
                args: Box::new([]),
            })
            .unwrap();
        for fields in written.iter().skip(1) {
            let plan = variant
                .named_initializer_slots(&fields.iter().map(String::as_str).collect::<Vec<_>>())
                .unwrap();
            let guard = store
                .begin_construction(&vec![string; variant.fields.len()])
                .unwrap();
            // The two stock initializers before panic supply the completion order.
            // String owners exercise guard transport; user Drop/catch_unwind is
            // still pending. This is not a full execution of the stock fixture.
            for (name, ordinal) in fields.iter().zip(&plan).take(fields.len() - 1) {
                let ticket = store.string(name).unwrap();
                store.construction_push(guard, *ordinal, ticket).unwrap();
                assert_eq!(
                    store.peek_ticket(ticket).unwrap_err().code,
                    "LRR-E-STALE-HANDLE"
                );
            }
            let mut cleaned = Vec::new();
            while let Some(ticket) = store.construction_cleanup_next(guard).unwrap() {
                cleaned.push(store.string_contents(ticket).unwrap().to_owned());
                store.discard(ticket).unwrap();
                assert_eq!(
                    store.discard(ticket).unwrap_err().code,
                    "LRR-E-STALE-HANDLE"
                );
            }
            assert_eq!(
                cleaned,
                fields[..fields.len() - 1]
                    .iter()
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                store.construction_cleanup_next(guard).unwrap_err().code,
                "LRR-E-STALE-HANDLE"
            );
        }
    }

    #[test]
    fn rustc_missing_and_duplicate_initializers_fault_when_plan_is_demanded() {
        for (source, expected) in [
            (MISSING_INITIALIZER, "missing field in reached initializer"),
            (
                DUPLICATE_INITIALIZER,
                "duplicate field in reached initializer",
            ),
        ] {
            let (mut db, parsed, outline) = publish(source);
            let schema = demand(&mut db, &outline, "BuildData");
            let written = initializers(&parsed, "BuildData");
            assert_eq!(written.len(), 1);
            let error = schema
                .variant(None)
                .unwrap()
                .named_initializer_slots(&written[0].iter().map(String::as_str).collect::<Vec<_>>())
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
            assert_eq!(error.message, expected);
            assert!(!db.trace.events.iter().any(|event| matches!(
                event.key,
                QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
            )));
        }
    }

    #[test]
    fn rustc_unused_duplicate_schema_stays_cold_until_reached_field_plan() {
        let (mut db, _, outline) = publish(DUPLICATE_SCHEMA);
        let main = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(main.def_key.fingerprint()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
        db.fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(main.body.as_ref().unwrap().key()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
        assert!(
            !db.trace
                .counts
                .keys()
                .any(|key| key.starts_with("TypeSchema:"))
        );
        let schema = demand(&mut db, &outline, "BuildData");
        let error = schema
            .variant(None)
            .unwrap()
            .named_initializer_slots(&["foo"])
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
        assert_eq!(error.message, "duplicate field in reached schema");
    }
}
