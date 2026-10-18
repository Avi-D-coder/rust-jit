//! Ordinary namespace resolution of pattern names, prepared once per PC.
//!
//! rustc resolves an identifier pattern to a constant, unit struct or unit
//! variant when one is in scope and otherwise introduces a fresh binding; a
//! path, tuple-struct or record pattern names a constructor (or a constant).
//! The reached dispatcher answers each written name of a retained tree from
//! the run's completed namespace graph through the same routes ordinary code
//! uses: `CallResolver` Value lookups and `select_nominal_constructor` (shared
//! with value paths and constructor calls) for unit/tuple constructors, the
//! Type namespace for record constructors. Constants retain their actual
//! declaration identity; the matcher demands an ordinary VM initializer only
//! if it reaches that target. Cold alternatives remain unevaluated.
use super::*;
use crate::{
    frontend::{
        names::NameLookup,
        nominal_constructors::{NominalConstructorTarget, bind_constructor},
        pattern_names::{PatternSelection, pattern_declaration, select_pattern_name},
        types::FieldShape,
    },
    ir::op::PatternOperation,
    source::manifest::{ItemKind, Namespace},
    syntax::pattern_tree::{NameForm, PatternName},
    vm::pattern::matcher::PatternTarget,
};

fn unsupported(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}

impl RunMachine {
    pub(super) fn prepare_pattern_names(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        let tree = match self.installed.get_current(db, id) {
            Ok(recipe) => recipe
                .recipe()
                .operations
                .iter()
                .skip(self.frame.resume_operation_index())
                .find_map(|op| match op {
                    Op::Pattern {
                        operation: PatternOperation::Test { tree, .. },
                    } => Some(Arc::clone(tree)),
                    _ => None,
                }),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(tree) = tree else {
            return QueryPoll::Ready(Ok(()));
        };
        if tree.names().is_empty() {
            return QueryPoll::Ready(Ok(()));
        }
        let pc = self.frame.pc().clone();
        let mut targets = match self.pattern_names.get(&pc) {
            Some((identity, targets)) if *identity == tree.identity() => Arc::clone(targets),
            _ => {
                let mut targets = Vec::with_capacity(tree.names().len());
                for name in tree.names() {
                    match self.resolve_pattern_name(db, name) {
                        QueryPoll::Ready(Ok(target)) => targets.push(target),
                        // A refused name faults only when the matcher reaches
                        // it, so an unselected alternative stays cold.
                        QueryPoll::Ready(Err(error)) => targets.push(PatternTarget::Refused(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                }
                let targets: Arc<[PatternTarget]> = targets.into();
                self.pattern_names
                    .insert(pc.clone(), (tree.identity(), Arc::clone(&targets)));
                targets
            }
        };
        // Only hydrate values already produced by the ordinary initializer
        // continuation. Looking at unresolved names never executes a cold
        // initializer; Search is the sole owner of that demand.
        let mut ready = Vec::new();
        for (slot, target) in targets.iter().enumerate() {
            let PatternTarget::Initializer(identity) = target else {
                continue;
            };
            let value = if let Some((cell, _)) = self.const_cells.get(identity) {
                match self.store.admit_move(*cell) {
                    Ok(value) => Some(value),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            } else if let Some((ticket, _)) = self.initialized_values.get(&(pc.clone(), *identity))
            {
                match self.store.peek_ticket(*ticket) {
                    Ok(value) => Some(value),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            } else {
                None
            };
            if let Some(value) = value {
                // Scalar patterns already use the shared numeric comparator.
                // Keep an unsupported structural value's ticket owned by the
                // run instead of treating a raw aggregate handle as a scalar.
                let target = if matches!(value.kind, 1..=7 | 11 | 12 | 17) {
                    PatternTarget::Constant(value)
                } else {
                    PatternTarget::Refused(unsupported(
                        "non-scalar constant pattern requires structural constant integration"
                            .into(),
                    ))
                };
                ready.push((slot, target));
            }
        }
        if !ready.is_empty() {
            let writable = Arc::make_mut(&mut targets);
            for (slot, target) in ready {
                writable[slot] = target;
            }
            self.pattern_names
                .insert(pc.clone(), (tree.identity(), Arc::clone(&targets)));
        }
        self.frame.set_pattern_names(pc, targets);
        QueryPoll::Ready(Ok(()))
    }
    fn resolve_pattern_name(
        &mut self,
        db: &mut Database,
        name: &PatternName,
    ) -> QueryPoll<PatternTarget> {
        self.resolve_written_pattern_name(db, name)
    }
    fn resolve_written_pattern_name(
        &mut self,
        db: &mut Database,
        name: &PatternName,
    ) -> QueryPoll<PatternTarget> {
        if let Some(node) = name.lexical {
            // A block-local item is selected by the entered lexical Value
            // goal of the pattern itself, as an ordinary value path is.
            let key = crate::ir::pc::PcKey {
                body: self.frame.pc().body.clone(),
                node: node.0,
                phase: 0,
            };
            return match self.entered_value_declaration(db, key) {
                QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(PatternTarget::Binding)),
                QueryPoll::Ready(Ok(Some(selected))) => {
                    // A constructor (a local or imported struct, an enum
                    // variant) binds through the service constructor
                    // expressions use.
                    match self.lexical_constructor(db, &selected) {
                        QueryPoll::Ready(Ok(Some(bound))) => {
                            return self.pattern_constructor_target(
                                db,
                                name,
                                bound,
                                selected
                                    .variant
                                    .as_ref()
                                    .map(|v| v.name.as_str())
                                    .or(selected.rooted.as_deref()),
                            );
                        }
                        QueryPoll::Ready(Ok(None)) => {}
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                    // `Root::member` that names no variant of the selected
                    // enum: an associated item of a block-scoped or imported
                    // type is not selected here.
                    if selected.rooted.is_some() {
                        return QueryPoll::Ready(Err(unsupported(format!(
                            "pattern path `{}` names an associated item of a block-scoped or imported type; its member selection awaits integration",
                            name.spelling
                        ))));
                    }
                    self.pattern_declaration_target(db, name, selected.identity, selected.kind)
                }
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        }
        // A module-graph name: the selection the static source-constraint
        // service shares (`frontend::pattern_names`), including the `for`
        // desugaring's lang `Option::Some`.
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-PATTERN-BINDING",
                "pattern names require a checked source catalogue".into(),
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (name, selection) = match select_pattern_name(
            &resolver,
            graph,
            &sources,
            db,
            self.frame.pc().body.definition,
            name,
        ) {
            QueryPoll::Ready(Ok(found)) => found,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.pattern_selection_target(db, &name, selection)
    }
    /// The pattern meaning of a selected Value declaration: a constant, a
    /// unit/tuple/record constructor, or (a function) a fresh binding.
    fn pattern_declaration_target(
        &mut self,
        db: &mut Database,
        name: &PatternName,
        identity: crate::core::digest::Digest,
        item_kind: Option<ItemKind>,
    ) -> QueryPoll<PatternTarget> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-PATTERN-BINDING",
                "pattern names require a checked source catalogue".into(),
            )));
        };
        match pattern_declaration(graph, name, identity, item_kind) {
            Ok(selection) => self.pattern_selection_target(db, name, selection),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// The matcher target of a shared pattern-name selection: a constant's
    /// actual initializer identity, or a constructor bound as
    /// constructor expressions bind it.
    fn pattern_selection_target(
        &mut self,
        db: &mut Database,
        name: &PatternName,
        selection: PatternSelection,
    ) -> QueryPoll<PatternTarget> {
        match selection {
            PatternSelection::Binding => QueryPoll::Ready(Ok(PatternTarget::Binding)),
            PatternSelection::Constant(identity) => {
                QueryPoll::Ready(Ok(PatternTarget::Initializer(identity)))
            }
            PatternSelection::Constructor { identity, variant } => {
                self.pattern_constructor(db, name, identity, variant)
            }
        }
    }
    /// Look up a written name from the current body's lexical context in the
    /// run's completed namespace graph (patterns and value paths).
    pub(super) fn lookup_value_name(
        &self,
        db: &mut Database,
        spelling: &str,
        namespace: Namespace,
    ) -> QueryPoll<NameLookup> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-PATTERN-BINDING",
                "pattern names require a checked source catalogue".into(),
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        match CallResolver::new(graph, &sources) {
            Ok(resolver) => {
                resolver.lookup_written(db, self.frame.pc().body.definition, spelling, namespace)
            }
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// Bind the named struct or enum variant with the binder constructor
    /// expressions use, then map written record fields to declaration order.
    fn pattern_constructor(
        &mut self,
        db: &mut Database,
        name: &PatternName,
        identity: crate::core::digest::Digest,
        variant: Option<crate::frontend::names::EnumVariantDefinition>,
    ) -> QueryPoll<PatternTarget> {
        let bound = if matches!(name.form, NameForm::Record(_)) {
            // Record structs/variants live only in the Type namespace.
            let sources: Vec<_> = self.sources.iter().collect();
            let target = match &variant {
                Some(variant) => match NominalConstructorTarget::variant(&sources, variant) {
                    Ok(target) => target,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
                None => match sources
                    .iter()
                    .find_map(|source| source.outline_item(identity))
                {
                    Some(item) => NominalConstructorTarget::structure(item),
                    None => {
                        return QueryPoll::Ready(Err(unsupported(
                            "record pattern type is outside the source catalogue".into(),
                        )));
                    }
                },
            };
            let Some(graph) = &self.graph else {
                return QueryPoll::Ready(Err(call_fault("record pattern requires names")));
            };
            match bind_constructor(db, graph, &sources, &target) {
                QueryPoll::Ready(Ok(bound)) => bound,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        } else {
            match self.select_nominal_constructor(db, &name.spelling) {
                QueryPoll::Ready(Ok(Some(bound))) => bound,
                QueryPoll::Ready(Ok(None)) => {
                    return QueryPoll::Ready(Err(unsupported(format!(
                        "pattern path `{}` does not select a constructor",
                        name.spelling
                    ))));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
        self.pattern_constructor_target(db, name, bound, variant.as_ref().map(|v| v.name.as_str()))
    }
    /// The matcher target of a bound constructor, mapping written record
    /// fields to declaration ordinals through its declaration's schema.
    pub(super) fn pattern_constructor_target(
        &mut self,
        db: &mut Database,
        name: &PatternName,
        bound: crate::frontend::nominal_constructors::BoundConstructor,
        variant_name: Option<&str>,
    ) -> QueryPoll<PatternTarget> {
        let ordinals: Box<[u32]> = match &name.form {
            NameForm::Record(written) => {
                let schema = match db.fetch(
                    &SyntaxProvider,
                    QueryKey::TypeSchema(bound.variant().identity()),
                ) {
                    QueryPoll::Ready(Ok(schema)) => schema,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault("record pattern schema differs")));
                };
                let recipe = match schema.variant(variant_name) {
                    Ok(recipe) => recipe,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                let mut ordinals = Vec::with_capacity(written.len());
                for field in written.iter() {
                    // Named fields by name; tuple fields by their written index.
                    let position = recipe
                        .fields
                        .iter()
                        .position(|declared| declared.name.as_deref() == Some(field.as_str()));
                    let position = position.or_else(|| {
                        (bound.shape() == FieldShape::Tuple)
                            .then(|| field.parse::<usize>().ok())
                            .flatten()
                            .filter(|index| *index < recipe.fields.len())
                    });
                    let Some(position) = position else {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-PATTERN-TYPE",
                            format!("record pattern field `{field}` is not declared"),
                        )));
                    };
                    ordinals.push(u32::try_from(position).unwrap_or(u32::MAX));
                }
                ordinals.into()
            }
            _ => Box::new([]),
        };
        QueryPoll::Ready(Ok(PatternTarget::Constructor {
            variant: Arc::clone(bound.variant()),
            shape: bound.shape(),
            ordinals,
        }))
    }
}
