//! Ordinary namespace resolution of pattern names, prepared once per PC.
//!
//! rustc resolves an identifier pattern to a constant, unit struct or unit
//! variant when one is in scope and otherwise introduces a fresh binding; a
//! path, tuple-struct or record pattern names a constructor (or a constant).
//! The reached dispatcher answers each written name of a retained tree from
//! the run's completed namespace graph through the same routes ordinary code
//! uses: `CallResolver` Value lookups for unit/tuple constructors, the Type
//! namespace for record constructors and the shared typed constructor binder
//! for the selected declaration. Constants retain their actual
//! declaration identity; the matcher demands an ordinary VM initializer only
//! if it reaches that target. Cold alternatives remain unevaluated.
use super::*;
use crate::{
    frontend::{
        names::NameLookup,
        nominal_constructors::{
            ConstructorPreparation, NominalConstructorTarget, prepare_constructor,
        },
        pattern_names::{PatternSelection},
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
                        operation: PatternOperation::Test { owner, tree, .. },
                    } => Some((*owner, Arc::clone(tree))),
                    _ => None,
                }),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some((owner, tree)) = tree else {
            return QueryPoll::Ready(Ok(()));
        };
        if tree.names().is_empty() {
            return QueryPoll::Ready(Ok(()));
        }
        if let Err(error) =
            self.frame
                .require_pattern_nominals_current(owner, &tree, db, &self.store)
        {
            return QueryPoll::Ready(Err(error));
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
            let root = if let Some(stored) = self.const_cells.get(identity) {
                Some(crate::runtime::cell::pattern_constants::ConstantRoot::Cell(
                    stored.cell,
                ))
            } else if let Some((ticket, _)) = self.initialized_values.get(&(pc.clone(), *identity))
            {
                Some(crate::runtime::cell::pattern_constants::ConstantRoot::Ticket(*ticket))
            } else {
                None
            };
            if let Some(root) = root {
                if let Err(error) = self.store.pattern_constant_value(root) {
                    return QueryPoll::Ready(Err(error));
                }
                ready.push((slot, PatternTarget::Constant(root)));
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
    /// The single matcher supplied this goal after identity/variant testing.
    /// Resolve only its actual application through the shared pure member
    /// service; no pattern traversal or guest operand evaluation happens here.
    pub(super) fn prepare_reached_pattern_nominal(
        &mut self,
        db: &mut Database,
        owner: crate::ir::pc::NodeId,
        tree: &crate::syntax::pattern_tree::PatternTree,
        slot: u32,
        ty: crate::runtime::shape::TypeId,
        source: &crate::traits::term::TyTerm,
    ) -> QueryPoll<()> {
        use crate::frontend::nominal_member_goals::{NominalMemberKey, NominalMemberSelection};
        let template = match self.frame.pattern_constructor_template(tree, slot) {
            Ok(template) => template,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let goal_source = source;
        let source = self.frame.bound_view(goal_source, &self.store);
        let crate::traits::term::TyTerm::Nominal(definition, arguments) = &source else {
            return QueryPoll::Ready(Err(unsupported(
                "reached nominal pattern requires actual source arguments".into(),
            )));
        };
        if *definition != template.identity {
            return QueryPoll::Ready(Err(call_fault(
                "pattern goal source/selected definition differs",
            )));
        }
        let key = NominalMemberKey {
            pc: PcKey {
                phase: 0,
                ..self.frame.pc().clone()
            },
            definition: *definition,
            arguments: arguments.clone(),
            selection: NominalMemberSelection::Variant(template.name.clone()),
        };
        let (member, guard) = match self.nominal_member(db, key.clone(), Some(ty)) {
            QueryPoll::Ready(Ok(member)) => member,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::NominalMember(member) = member.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("pattern nominal member outcome differs")));
        };
        let Some(variant) = member.variant().filter(|_| member.key() == &key) else {
            return QueryPoll::Ready(Err(call_fault(
                "pattern nominal member key/variant differs",
            )));
        };
        QueryPoll::Ready(self.frame.prepare_pattern_nominal(
            owner,
            tree,
            slot,
            ty,
            goal_source,
            &source,
            Arc::clone(variant),
            guard,
            &self.store,
        ))
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
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(unsupported("pattern names require a checked source catalogue".into())));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (name, selection) = match crate::frontend::pattern_names::select_source_pattern_name(
            &resolver, graph, &sources, db, self.frame.pc(), name,
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.pattern_selection_target(db, &name, selection)
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
        let owner = variant.as_ref().map_or(identity, |variant| variant.enumeration);
        let variant_name = variant.as_ref().map(|variant| variant.name.as_str());
        match NominalConstructorTarget::local(db, owner, variant_name) {
            QueryPoll::Ready(Ok(Some(target))) => {
                return match self.bind_selected_constructor(db, &target) {
                    QueryPoll::Ready(Ok(bound)) => self.pattern_constructor_target(db, name, bound, variant_name),
                    QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                };
            }
            QueryPoll::Ready(Ok(None)) => {},
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let sources: Vec<_> = self.sources.iter().collect();
        let definition = variant
            .as_ref()
            .map_or(identity, |variant| variant.enumeration);
        let local = match NominalConstructorTarget::local(
            db,
            definition,
            variant.as_ref().map(|variant| variant.name.as_str()),
        ) {
            QueryPoll::Ready(Ok(local)) => local,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let target = match local {
            Some(target) => target,
            None => match &variant {
                Some(variant) => match NominalConstructorTarget::variant(db, &sources, variant) {
                    Ok(target) => target,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
                None => match NominalConstructorTarget::structure(db, &sources, identity) {
                    Ok(target) => target,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
            },
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("pattern constructor requires names")));
        };
        match prepare_constructor(db, graph, &sources, &target) {
            QueryPoll::Ready(Ok(ConstructorPreparation::Ready(bound))) => self
                .pattern_constructor_target(
                    db,
                    name,
                    bound,
                    variant.as_ref().map(|variant| variant.name.as_str()),
                ),
            QueryPoll::Ready(Ok(ConstructorPreparation::Pending(template))) => {
                let ordinals = match self.pattern_field_ordinals(
                    db,
                    name,
                    template.identity,
                    template.name.as_deref(),
                    template.shape,
                ) {
                    QueryPoll::Ready(Ok(ordinals)) => ordinals,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                QueryPoll::Ready(Ok(PatternTarget::ConstructorTemplate {
                    template: Arc::new(template),
                    ordinals,
                }))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
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
        let ordinals = match self.pattern_field_ordinals(
            db,
            name,
            bound.variant().identity(),
            variant_name,
            bound.shape(),
        ) {
            QueryPoll::Ready(Ok(ordinals)) => ordinals,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(PatternTarget::Constructor {
            variant: Arc::clone(bound.variant()),
            shape: bound.shape(),
            ordinals,
        }))
    }
    fn pattern_field_ordinals(
        &self,
        db: &mut Database,
        name: &PatternName,
        definition: crate::core::digest::Digest,
        variant_name: Option<&str>,
        shape: FieldShape,
    ) -> QueryPoll<Box<[u32]>> {
        crate::frontend::pattern_names::field_ordinals(db, definition, variant_name, &name.form, shape)
    }
}
