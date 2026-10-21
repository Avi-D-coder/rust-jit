//! Concrete immutable fact reader for the shared solver. This is not the
//! source producer or an inference client: publication requires a real complete
//! impl catalog, and all nondata client functions are mandatory inputs.
use super::model::{Clause, Predicate, Span, Symbol, Ty};
use super::{
    context::Context,
    facts::SourceFacts,
    identity::DefinitionId,
    model::*,
    parameters::SourceParameter,
    storage::{Interned, ScopedCache},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::{DiscoveryInput, SealDomain},
        value::QueryValue,
    },
    traits::term::{BoundKind, ParamKey},
};
use ra_ap_syntax::{AstNode, ast};
use rustc_index::bit_set::DenseBitSet;
use rustc_type_ir::fast_reject::{SimplifiedType, TreatParams, simplify_type};
use rustc_type_ir::lang_items::{SolverAdtLangItem, SolverProjectionLangItem, SolverTraitLangItem};
use rustc_type_ir::search_graph::RequiredDepth;
use rustc_type_ir::solve::{QueryResult, inspect};
use rustc_type_ir::{self as ty, inherent::*, *};
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use super::query_facts::{QueryAvailability, QueryFact};

pub(super) type Cx<'cx> = Context<'cx, PreparedProvider<'cx>>;
type T<'cx> = Ty<'cx, PreparedProvider<'cx>>;
type A<'cx> = Args<'cx, PreparedProvider<'cx>>;
type C<'cx> = Clause<'cx, PreparedProvider<'cx>>;
type Early<'cx, V> = ty::EarlyBinder<Cx<'cx>, V>;
type ClauseList<'cx> = Early<'cx, List<'cx, C<'cx>>>;
type Spanned<'cx> = Early<'cx, &'cx [(C<'cx>, Span)]>;
type Conditions<'cx> = Early<'cx, &'cx [Binder<Cx<'cx>, TraitRef<Cx<'cx>>>]>;
type ProofResult<'cx> = (
    QueryResult<Cx<'cx>>,
    Box<inspect::Probe<Cx<'cx>>>,
    RequiredDepth,
);

/// Captured only by reading the actual definition-keyed preparation inputs.
/// This is a source attachment, not name selection or catalog completeness.
/// Cloning shares immutable source handles; it adds no currentness or entry authority.
#[derive(Debug, Clone)]
pub(super) struct DefinitionReceipt {
    definition: Digest,
    source: Arc<InputValue>,
    header: Arc<InputValue>,
    context: Arc<InputValue>,
    parsed: Arc<QueryValue>,
    generic_declaration: Arc<QueryValue>,
    implementation: Option<bool>,
    trait_declaration: bool,
}
impl DefinitionReceipt {
    pub(super) fn definition_id(&self, world: Digest, local_unit: Digest) -> DefinitionId {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("captured source context")
        };
        DefinitionId::new(
            world,
            self.definition,
            context.scope.crate_key == local_unit,
        )
    }
    pub(super) fn is_nominal(&self) -> Result<bool, Diagnostic> {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("captured source context")
        };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else {
            unreachable!("captured source parse")
        };
        parsed
            .syntax_at(context.range, |node| {
                ast::Struct::can_cast(node.kind())
                    || ast::Enum::can_cast(node.kind())
                    || ast::Union::can_cast(node.kind())
            })
            .map(|node| node.is_some())
    }
    pub(super) fn opaque_returns(
        &self,
    ) -> Option<&crate::frontend::declaration_generics::OpaqueReturnHeaders> {
        let QueryValue::DeclarationGenerics(generics) = self.generic_declaration.as_ref() else {
            unreachable!("captured generic declaration query")
        };
        generics.opaque_returns()
    }
    pub(super) fn is_enum(&self) -> Result<bool, Diagnostic> {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("captured source context")
        };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else {
            unreachable!("captured source parse")
        };
        parsed
            .syntax_at(context.range, |node| ast::Enum::can_cast(node.kind()))
            .map(|node| node.is_some())
    }
    fn is_type_alias(&self) -> Result<bool, Diagnostic> {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("captured source context")
        };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else {
            unreachable!("captured source parse")
        };
        parsed
            .syntax_at(context.range, |node| ast::TypeAlias::can_cast(node.kind()))
            .map(|node| node.is_some())
    }
    pub(super) fn nominal_kind_and_name(&self) -> Result<(AdtKind, String), Diagnostic> {
        use ra_ap_syntax::ast::HasName;
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("captured source context")
        };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else {
            unreachable!("captured source parse")
        };
        let node = parsed
            .syntax_at(context.range, |node| ast::Item::can_cast(node.kind()))?
            .and_then(ast::Item::cast)
            .ok_or_else(|| fault("ADT actual source item absent"))?;
        let (kind, name) = match node {
            ast::Item::Struct(item) => (AdtKind::Struct, item.name()),
            ast::Item::Enum(item) => (AdtKind::Enum, item.name()),
            ast::Item::Union(item) => (AdtKind::Union, item.name()),
            _ => return Err(fault("ADT source item is not nominal")),
        };
        Ok((
            kind,
            name.ok_or_else(|| fault("ADT source name absent"))?
                .text()
                .to_string(),
        ))
    }
    pub(super) fn same_source_metadata(&self, other: &Self) -> bool {
        self.definition == other.definition
            && matches!(
            (self.source.as_ref(),other.source.as_ref(),self.header.as_ref(),other.header.as_ref(),self.context.as_ref(),other.context.as_ref()),
            (InputValue::DefinitionSource(a),InputValue::DefinitionSource(b),InputValue::DefinitionHeader(ha),InputValue::DefinitionHeader(hb),InputValue::DeclarationContext(ca),InputValue::DeclarationContext(cb))
            if a == b && ha == hb && ca == cb)
    }
    pub(super) fn matches_enum_schema(
        &self,
        schema: &crate::frontend::types::NominalSchema,
    ) -> bool {
        matches!((self.header.as_ref(),self.context.as_ref()),
            (InputValue::DefinitionHeader(header),InputValue::DeclarationContext(context))
            if schema.definition == self.definition && schema.header == *header && schema.scope == context.scope)
    }
    pub(super) fn is_trait(&self) -> bool {
        self.trait_declaration
    }
    pub(super) fn is_impl(&self) -> bool {
        self.implementation.is_some()
    }
    pub(super) fn matches_generics(
        &self,
        generics: &crate::frontend::declaration_generics::DeclarationGenerics,
    ) -> bool {
        let (
            InputValue::DefinitionSource(parse),
            InputValue::DefinitionHeader(header),
            InputValue::DeclarationContext(context),
        ) = (
            self.source.as_ref(),
            self.header.as_ref(),
            self.context.as_ref(),
        )
        else {
            return false;
        };
        generics.definition() == self.definition
            && generics.matches_metadata(parse, header, context)
    }
    pub(super) fn observe(db: &mut Database, definition: Digest) -> QueryPoll<Option<Self>> {
        let source = db.input(&InputKey::DefinitionSource(definition));
        let header = db.input(&InputKey::DefinitionHeader(definition));
        let context = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (
            InputValue::DefinitionSource(parse),
            InputValue::DefinitionHeader(recipe),
            InputValue::DeclarationContext(declaration),
        ) = (source.as_ref(), header.as_ref(), context.as_ref())
        else {
            return QueryPoll::Ready(Err(fault("definition receipt input variants differ")));
        };
        if parse.profile != recipe.profile {
            return QueryPoll::Ready(Err(fault("definition receipt profile differs")));
        }
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(tree) = parsed.as_ref() else {
            return QueryPoll::Ready(Err(fault("definition receipt parser differs")));
        };
        let generic_declaration =
            match db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(definition)) {
                QueryPoll::Ready(Ok(value)) => value.value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        let QueryValue::DeclarationGenerics(generics) = generic_declaration.as_ref() else {
            return QueryPoll::Ready(Err(fault("definition generic declaration query differs")));
        };
        if !generics.matches_metadata(parse, recipe, declaration) {
            return QueryPoll::Ready(Err(fault("definition generic declaration source differs")));
        }
        let implementation =
            match tree.syntax_at(declaration.range, |node| ast::Impl::can_cast(node.kind())) {
                Ok(Some(node)) => ast::Impl::cast(node).map(|item| item.trait_().is_some()),
                Ok(None) => None,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let trait_declaration =
            match tree.syntax_at(declaration.range, |node| ast::Trait::can_cast(node.kind())) {
                Ok(node) => node.is_some(),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        QueryPoll::Ready(Ok(Some(Self {
            definition,
            source,
            header,
            context,
            parsed,
            generic_declaration,
            implementation,
            trait_declaration,
        })))
    }
}

/// Configuration must be translated from the real owning compiler/profile
/// inputs. No default limit or invented feature answers are provided here.
/// Recursion-limit source: pinned compiler/rustc_interface/src/limits.rs:33–41
/// (default128 and crate override); the producer must honor that configuration.
pub(super) struct Settings<'cx> {
    pub(super) catalog: &'cx DiscoveryInput,
    pub(super) construction_generics: Option<&'cx super::source_generics::SourceGenericLayouts>,
    pub(super) recursion_limit: usize,
    pub(super) features: FeatureSet<'cx>,
    pub(super) assumptions_on_binders: bool,
    pub(super) renormalize_rigid_aliases: bool,
    pub(super) enable_next_solver_overflow_fcw: bool,
    pub(super) disable_trait_solver_fast_paths: bool,
}

pub(super) struct PredicateFacts<'cx> {
    pub(super) explicit_item: QueryFact<Spanned<'cx>>,
    pub(super) item: QueryFact<ClauseList<'cx>>,
    pub(super) item_self: QueryFact<ClauseList<'cx>>,
    pub(super) item_non_self: QueryFact<ClauseList<'cx>>,
    pub(super) all: QueryFact<ClauseList<'cx>>,
    pub(super) own: QueryFact<ClauseList<'cx>>,
    pub(super) explicit_super: QueryFact<Spanned<'cx>>,
    pub(super) explicit_implied: QueryFact<Spanned<'cx>>,
    pub(super) const_conditions: QueryFact<Conditions<'cx>>,
    pub(super) implied_const_bounds: QueryFact<Conditions<'cx>>,
}
pub(super) struct ConstFacts<'cx> {
    pub(super) direct: QueryFact<bool>,
    pub(super) value: QueryFact<Option<Early<'cx, Const<Cx<'cx>>>>>,
    pub(super) abstract_value: QueryFact<Result<Option<Early<'cx, Const<Cx<'cx>>>>, ErrorToken>>,
    pub(super) anon_kind: QueryFact<ty::AnonConstKind>,
}
pub(super) struct CoroutineFacts<'cx> {
    pub(super) hidden: QueryFact<Early<'cx, Binder<Cx<'cx>, CoroutineWitnessTypes<Cx<'cx>>>>>,
    pub(super) movability: QueryFact<Movability>,
    pub(super) general: QueryFact<bool>,
    pub(super) asynchronous: QueryFact<bool>,
    pub(super) generator: QueryFact<bool>,
    pub(super) async_generator: QueryFact<bool>,
}
/// No Default: each value/absence must follow from real completed preparation.
pub(super) struct DefinitionFacts<'cx> {
    pub(super) receipt: DefinitionReceipt,
    pub(super) name: Symbol<'cx>,
    pub(super) span: Span,
    pub(super) generics: Generics<'cx, PreparedProvider<'cx>>,
    pub(super) variance_queries:
        QueryFact<super::source_variance_queries::DefinitionVarianceQueries<'cx>>,
    pub(super) ty: QueryFact<Early<'cx, T<'cx>>>,
    pub(super) opaque_hidden: QueryFact<Early<'cx, T<'cx>>>,
    pub(super) constant: ConstFacts<'cx>,
    pub(super) signature: QueryFact<Early<'cx, Binder<Cx<'cx>, FnSig<Cx<'cx>>>>>,
    pub(super) coroutine: CoroutineFacts<'cx>,
    pub(super) closure_coroutine: QueryFact<DefinitionId>,
    pub(super) parent: QueryFact<Option<DefinitionId>>,
    pub(super) predicates: PredicateFacts<'cx>,
    pub(super) requires_sized_self: QueryFact<bool>,
    pub(super) constant_function: QueryFact<bool>,
    pub(super) constant_closure: QueryFact<bool>,
    pub(super) has_const_conditions: QueryFact<bool>,
    pub(super) conditionally_const: QueryFact<bool>,
    pub(super) target_features: QueryFact<bool>,
    pub(super) impl_trait_in_trait: QueryFact<bool>,
    pub(super) has_definition: QueryFact<bool>,
    pub(super) opaques: QueryFact<List<'cx, DefinitionId>>,
    pub(super) opaques_and_coroutines: QueryFact<List<'cx, DefinitionId>>,
    /// Actual aliases may have distinct Self/Impl argument layouts.
    pub(super) alias_layouts: QueryFact<Vec<(AliasTermKind<Cx<'cx>>, Vec<BoundKind>)>>,
}
macro_rules! prepared_fields {
    ($value:expr; $($field:ident),+ $(,)?) => {
        true $(&& $value.$field.availability() != QueryAvailability::Unprepared)+
    };
}
impl PredicateFacts<'_> {
    pub(super) fn unprepared() -> Self {
        Self {
            explicit_item: QueryFact::Unprepared,
            item: QueryFact::Unprepared,
            item_self: QueryFact::Unprepared,
            item_non_self: QueryFact::Unprepared,
            all: QueryFact::Unprepared,
            own: QueryFact::Unprepared,
            explicit_super: QueryFact::Unprepared,
            explicit_implied: QueryFact::Unprepared,
            const_conditions: QueryFact::Unprepared,
            implied_const_bounds: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        prepared_fields!(self; explicit_item, item, item_self, item_non_self,
            all, own, explicit_super, explicit_implied, const_conditions, implied_const_bounds)
    }
}
impl ConstFacts<'_> {
    pub(super) fn unprepared() -> Self {
        Self {
            direct: QueryFact::Unprepared,
            value: QueryFact::Unprepared,
            abstract_value: QueryFact::Unprepared,
            anon_kind: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        prepared_fields!(self; direct, value, abstract_value, anon_kind)
    }
}
impl CoroutineFacts<'_> {
    pub(super) fn unprepared() -> Self {
        Self {
            hidden: QueryFact::Unprepared,
            movability: QueryFact::Unprepared,
            general: QueryFact::Unprepared,
            asynchronous: QueryFact::Unprepared,
            generator: QueryFact::Unprepared,
            async_generator: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        prepared_fields!(self; hidden, movability, general, asynchronous, generator, async_generator)
    }
}
impl<'cx> DefinitionFacts<'cx> {
    pub(super) fn metadata(
        receipt: DefinitionReceipt,
        name: Symbol<'cx>,
        span: Span,
        generics: Generics<'cx, PreparedProvider<'cx>>,
    ) -> Self {
        Self {
            receipt,
            name,
            span,
            generics,
            constant: ConstFacts::unprepared(),
            coroutine: CoroutineFacts::unprepared(),
            predicates: PredicateFacts::unprepared(),
            variance_queries: QueryFact::Unprepared,
            ty: QueryFact::Unprepared,
            opaque_hidden: QueryFact::Unprepared,
            signature: QueryFact::Unprepared,
            closure_coroutine: QueryFact::Unprepared,
            parent: QueryFact::Unprepared,
            requires_sized_self: QueryFact::Unprepared,
            constant_function: QueryFact::Unprepared,
            constant_closure: QueryFact::Unprepared,
            has_const_conditions: QueryFact::Unprepared,
            conditionally_const: QueryFact::Unprepared,
            target_features: QueryFact::Unprepared,
            impl_trait_in_trait: QueryFact::Unprepared,
            has_definition: QueryFact::Unprepared,
            opaques: QueryFact::Unprepared,
            opaques_and_coroutines: QueryFact::Unprepared,
            alias_layouts: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        self.predicates.prepared()
            && self.constant.prepared()
            && self.coroutine.prepared()
            && prepared_fields!(self; variance_queries, ty, opaque_hidden, signature,
                closure_coroutine, parent, requires_sized_self, constant_function,
                constant_closure, has_const_conditions, conditionally_const, target_features,
                impl_trait_in_trait, has_definition, opaques, opaques_and_coroutines, alias_layouts)
    }
}
pub(super) struct TraitFacts<'cx> {
    pub(super) automatic: QueryFact<bool>,
    pub(super) marker: QueryFact<bool>,
    pub(super) coinductive: QueryFact<bool>,
    pub(super) alias: QueryFact<bool>,
    pub(super) dyn_compatible: QueryFact<bool>,
    pub(super) fundamental: QueryFact<bool>,
    pub(super) unsafe_trait: QueryFact<bool>,
    pub(super) default_trait: QueryFact<bool>,
    pub(super) sizedness_trait: QueryFact<bool>,
    pub(super) associated_types: QueryFact<&'cx [DefinitionId]>,
    /// Complete actual associated members, including constants and functions.
    pub(super) associated_items: QueryFact<&'cx [DefinitionId]>,
    pub(super) supertraits: QueryFact<&'cx [DefinitionId]>,
    /// Complete relevant trait bucket; no caller-selected scope subset.
    pub(super) implementations: QueryFact<&'cx [DefinitionId]>,
}
pub(super) struct ImplFacts<'cx> {
    pub(super) trait_ref: QueryFact<Option<Early<'cx, TraitRef<Cx<'cx>>>>>,
    pub(super) polarity: QueryFact<ImplPolarity>,
    pub(super) constant: QueryFact<bool>,
    pub(super) default_impl: QueryFact<bool>,
    pub(super) reflection_generic: QueryFact<bool>,
    pub(super) guaranteed_unsized: QueryFact<bool>,
    pub(super) super_outlives: QueryFact<ClauseList<'cx>>,
    /// Complete specialization ancestry prepared by the actual client.
    pub(super) specializes: QueryFact<BTreeSet<DefinitionId>>,
}
impl TraitFacts<'_> {
    pub(super) fn unprepared() -> Self {
        Self {
            automatic: QueryFact::Unprepared,
            marker: QueryFact::Unprepared,
            coinductive: QueryFact::Unprepared,
            alias: QueryFact::Unprepared,
            dyn_compatible: QueryFact::Unprepared,
            fundamental: QueryFact::Unprepared,
            unsafe_trait: QueryFact::Unprepared,
            default_trait: QueryFact::Unprepared,
            sizedness_trait: QueryFact::Unprepared,
            associated_types: QueryFact::Unprepared,
            associated_items: QueryFact::Unprepared,
            supertraits: QueryFact::Unprepared,
            implementations: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        prepared_fields!(self; automatic, marker, coinductive, alias, dyn_compatible,
            fundamental, unsafe_trait, default_trait, sizedness_trait, associated_types,
            associated_items, supertraits, implementations)
    }
}
impl ImplFacts<'_> {
    pub(super) fn unprepared() -> Self {
        Self {
            trait_ref: QueryFact::Unprepared,
            polarity: QueryFact::Unprepared,
            constant: QueryFact::Unprepared,
            default_impl: QueryFact::Unprepared,
            reflection_generic: QueryFact::Unprepared,
            guaranteed_unsized: QueryFact::Unprepared,
            super_outlives: QueryFact::Unprepared,
            specializes: QueryFact::Unprepared,
        }
    }

    fn prepared(&self) -> bool {
        prepared_fields!(self; trait_ref, polarity, constant, default_impl, reflection_generic,
            guaranteed_unsized, super_outlives, specializes)
    }
}
/// Actual graph leaf result, supplied by source specialization preparation.
/// No Default: missing data must suspend preparation before shared solver entry.
#[derive(Debug, Clone, Copy)]
pub(super) struct AssociatedLeaf {
    pub(super) item: DefinitionId,
    pub(super) defining_node: DefinitionId,
    pub(super) finalizing_node: Option<DefinitionId>,
}
impl AssociatedLeaf {
    /// Pinned rustc_middle::traits::specialization_graph::LeafDef::is_final.
    pub(super) fn is_final(self) -> bool {
        self.finalizing_node.is_some()
    }
}
pub(super) struct AdtFacts<'cx> {
    pub(super) info: AdtInfo<'cx, PreparedProvider<'cx>>,
    pub(super) source_variants: Arc<PreparedAdtVariants>,
    pub(super) unsizing: QueryFact<DenseBitSet<u32>>,
}
/// Captured actual nominal shape, not a proof of caller-supplied field Types.
/// The complete source producer must translate each actual field occurrence
/// before publishing typed facts; that producer remains an integration hold.
pub(super) struct PreparedAdtVariants {
    receipt: DefinitionReceipt,
    schema: Arc<QueryValue>,
    kind: AdtKind,
    name: String,
    guard: crate::db::memo::PreparedInputGuard,
}
impl PreparedAdtVariants {
    pub(super) fn prepare(db: &mut Database, definition: Digest) -> QueryPoll<Self> {
        let receipt = match DefinitionReceipt::observe(db, definition) {
            QueryPoll::Ready(Ok(Some(receipt))) => receipt,
            QueryPoll::Ready(Ok(None)) => {
                return QueryPoll::Ready(Err(fault("ADT source preparation is incomplete")));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let (kind, name) = match receipt.nominal_kind_and_name() {
            Ok(actual) => actual,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let key = QueryKey::TypeSchema(definition);
        let captured = match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT source schema variant differs")));
        };
        if !captured.cacheable || !receipt.matches_enum_schema(schema) {
            return QueryPoll::Ready(Err(fault("ADT actual source schema differs")));
        }
        let guard = match db.prepare_input_guard(&SyntaxProvider, &key, captured.fingerprint) {
            QueryPoll::Ready(Ok(guard)) => guard,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(Self {
            receipt,
            schema: captured.value,
            kind,
            name,
            guard,
        }))
    }
    pub(super) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
    fn require_shape<'cx>(
        &self,
        definition: &DefinitionFacts<'cx>,
        info: &AdtInfo<'cx, PreparedProvider<'cx>>,
    ) -> Result<(), Diagnostic> {
        let QueryValue::TypeSchema(schema) = self.schema.as_ref() else {
            unreachable!("captured ADT source schema")
        };
        if !self.receipt.same_source_metadata(&definition.receipt)
            || !definition.receipt.matches_enum_schema(schema)
            || info.kind != self.kind
            || definition.name.0 != self.name
        {
            return Err(fault(
                "ADT variant metadata differs from its actual source schema",
            ));
        }
        let Some(variants) = info.variants.known().copied() else {
            if info.fields.known().is_some() {
                return Err(fault("ADT flattened fields lack their actual variant view"));
            }
            return Ok(());
        };
        if variants.len() != schema.variants.len() {
            return Err(fault("ADT variant count differs from actual source"));
        }
        let mut flat = info.fields.known().map(|fields| fields.iter());
        for (actual, typed) in schema.variants.iter().zip(variants) {
            if typed.name.0 != actual.name.as_deref().unwrap_or(&self.name)
                || actual.fields.len() != typed.fields.len()
            {
                return Err(fault("ADT variant identity or field count differs"));
            }
            for (ordinal, (actual, typed)) in actual.fields.iter().zip(typed.fields).enumerate() {
                let name_matches = match &actual.name {
                    Some(name) => typed.name.0 == name,
                    None => typed.name.0 == ordinal.to_string(),
                };
                if !name_matches
                    || flat
                        .as_mut()
                        .is_some_and(|flat| flat.next() != Some(typed.ty.skip_binder()))
                {
                    return Err(fault(
                        "ADT field name, order or flattened Type view differs",
                    ));
                }
            }
        }
        if flat.as_mut().is_some_and(|flat| flat.next().is_some()) {
            return Err(fault("ADT flattened Type view contains extra fields"));
        }
        Ok(())
    }
}
pub(super) struct LangItems {
    pub(super) const_param_ty: QueryFact<Option<DefinitionId>>,
    pub(super) field_representing_type: QueryFact<Option<DefinitionId>>,
    // Missing key is unprepared; a present None is proven role absence.
    pub(super) traits: ty::data_structures::HashMap<SolverTraitLangItem, Option<DefinitionId>>,
    pub(super) projections:
        ty::data_structures::HashMap<SolverProjectionLangItem, Option<DefinitionId>>,
    pub(super) adts: ty::data_structures::HashMap<SolverAdtLangItem, Option<DefinitionId>>,
    // A requested predicate may be prepared without classifying every other
    // role. Each present bool is an actual source-backed query answer.
    pub(super) trait_tests: ty::data_structures::HashMap<(DefinitionId, SolverTraitLangItem), bool>,
    // Per-definition classification is independent of role-registry lookup.
    projection_by_definition: BTreeMap<DefinitionId, Option<SolverProjectionLangItem>>,
    trait_by_definition: BTreeMap<DefinitionId, Option<SolverTraitLangItem>>,
    adt_by_definition: BTreeMap<DefinitionId, Option<SolverAdtLangItem>>,
}
impl LangItems {
    pub(super) fn new(
        const_param_ty: QueryFact<Option<DefinitionId>>,
        field_representing_type: QueryFact<Option<DefinitionId>>,
        traits: ty::data_structures::HashMap<SolverTraitLangItem, Option<DefinitionId>>,
        projections: ty::data_structures::HashMap<SolverProjectionLangItem, Option<DefinitionId>>,
        adts: ty::data_structures::HashMap<SolverAdtLangItem, Option<DefinitionId>>,
        trait_by_definition: BTreeMap<DefinitionId, Option<SolverTraitLangItem>>,
        projection_by_definition: BTreeMap<DefinitionId, Option<SolverProjectionLangItem>>,
        adt_by_definition: BTreeMap<DefinitionId, Option<SolverAdtLangItem>>,
        trait_tests: ty::data_structures::HashMap<(DefinitionId, SolverTraitLangItem), bool>,
    ) -> Result<Self, Diagnostic> {
        fn check<K: Copy + Eq + Hash>(
            items: &ty::data_structures::HashMap<K, Option<DefinitionId>>,
            classified: &BTreeMap<DefinitionId, Option<K>>,
        ) -> Result<(), Diagnostic> {
            let mut seen = BTreeSet::new();
            for (role, id) in items {
                if let Some(id) = id
                    && (!seen.insert(*id) || classified.get(id) != Some(&Some(*role)))
                {
                    return Err(fault(
                        "language role and actual declaration classification differ",
                    ));
                }
            }
            for (id, role) in classified {
                if let Some(role) = role
                    && items
                        .get(role)
                        .is_some_and(|selected| *selected != Some(*id))
                {
                    return Err(fault(
                        "prepared language role contradicts actual classification",
                    ));
                }
            }
            Ok(())
        }
        check(&traits, &trait_by_definition)?;
        check(&projections, &projection_by_definition)?;
        check(&adts, &adt_by_definition)?;
        for ((id, role), answer) in &trait_tests {
            if trait_by_definition
                .get(id)
                .is_some_and(|known| (*known == Some(*role)) != *answer)
            {
                return Err(fault(
                    "trait language predicate contradicts actual classification",
                ));
            }
        }
        Ok(Self {
            const_param_ty,
            field_representing_type,
            traits,
            projections,
            adts,
            trait_by_definition,
            projection_by_definition,
            adt_by_definition,
            trait_tests,
        })
    }
}
pub(super) struct PreparedFacts<'cx> {
    context: Cx<'cx>,
    source_membership: Arc<QueryValue>,
    pub(super) definitions: BTreeMap<DefinitionId, DefinitionFacts<'cx>>,
    pub(super) traits: BTreeMap<DefinitionId, TraitFacts<'cx>>,
    pub(super) implementations: BTreeMap<DefinitionId, ImplFacts<'cx>>,
    pub(super) associated_leaves:
        BTreeMap<(DefinitionId, DefinitionId), Result<AssociatedLeaf, ErrorToken>>,
    /// Demand-prepared impls, each joined to its actual raw/canonical header.
    pub(super) impl_declarations: BTreeMap<Digest, DefinitionId>,
    pub(super) adts: BTreeMap<DefinitionId, AdtFacts<'cx>>,
    pub(super) parameters: BTreeMap<(ParamKey, BoundKind), DefinitionId>,
    pub(super) lang: LangItems,
    pub(super) const_evaluation: QueryFact<super::infer::const_eval::PreparedConstEvalInputs<'cx>>,
    pub(super) transmutability: QueryFact<super::infer::transmute::PreparedTransmuteInputs<'cx>>,
    /// Optional diagnostic-print attachments, not semantic region inference.
    pub(super) anonymous_bounds: BTreeMap<(usize, usize), Region<Cx<'cx>>>,
    pub(super) anonymous_canonical: BTreeMap<usize, Region<Cx<'cx>>>,
    indexes: OnceCell<CandidateIndex>,
}
struct CandidateIndex {
    traits: BTreeMap<DefinitionId, ImplBucket>,
}
impl CandidateIndex {
    /// Derive only genuinely complete demanded buckets. The caller has already
    /// validated actual source membership and every raw/canonical typed head.
    fn prepare<'cx>(cx: Cx<'cx>, facts: &PreparedFacts<'cx>) -> Result<Self, Diagnostic> {
        if cx != facts.context {
            return Err(fault("candidate classification belongs to another context"));
        }
        let mut index = Self {
            traits: facts.traits.iter().filter_map(|(id, row)| {
                row.implementations.known().map(|_| (*id, ImplBucket::default()))
            }).collect(),
        };
        let mut heads = Vec::new();
        let mut automatic = BTreeMap::new();
        for (id, implementation) in &facts.implementations {
            let Some(Some(reference)) = implementation.trait_ref.known() else { continue; };
            let reference = reference.skip_binder();
            if !index.traits.contains_key(&reference.def_id) {
                continue; // No complete coverage was published for this query.
            }
            // Pinned fast_reject::simplify_type recursively unwraps Pat and
            // reads trait_is_auto only for an actual dyn principal. Certify
            // exactly those callbacks before entering that upstream code.
            let mut self_ty = reference.self_ty();
            while let ty::Pat(inner, ..) = self_ty.kind() { self_ty = inner; }
            if let ty::Dynamic(predicates, ..) = self_ty.kind()
                && let Some(principal) = predicates.principal_def_id() {
                let flag = *facts.traits.get(&principal)
                    .ok_or_else(|| fault("candidate dyn principal trait metadata is unprepared"))?
                    .automatic.checked("candidate dyn principal automatic flag")?;
                automatic.insert(principal, flag);
            }
            heads.push((*id, reference));
        }
        let provider = cx.services();
        let _scope = provider.candidate_classification.enter(CandidateClassificationInputs {
            context: cx, automatic,
        });
        for (id, reference) in heads {
            let bucket = index.traits.get_mut(&reference.def_id)
                .expect("validated complete candidate trait");
            match simplify_type(cx, reference.self_ty(), TreatParams::InstantiateWithInfer) {
                Some(key) => bucket.constructors.entry(key).or_default().push(id),
                None => bucket.blankets.push(id),
            }
        }
        // Check every derived entry against its complete source-validated
        // bucket once during publication, never during installed lookup.
        for (trait_id, bucket) in &index.traits {
            let mut observed = BTreeSet::new();
            for id in bucket.constructors.values().flatten().chain(&bucket.blankets) {
                if !observed.insert(*id) {
                    return Err(fault("duplicate classified candidate"));
                }
            }
            let expected = *facts.traits[trait_id].implementations
                .checked("complete candidate bucket")?;
            if !observed.iter().copied().eq(<[DefinitionId]>::iter(expected).copied()) {
                return Err(fault("pinned candidate index differs from the complete catalog"));
            }
        }
        Ok(index)
    }
}
#[derive(Default)]
pub(super) struct ImplBucket {
    constructors: ty::data_structures::HashMap<SimplifiedType<DefinitionId>, Vec<DefinitionId>>,
    blankets: Vec<DefinitionId>,
}
impl ImplBucket {
    pub(super) fn non_blanket_impls(
        &self,
    ) -> &ty::data_structures::HashMap<SimplifiedType<DefinitionId>, Vec<DefinitionId>> {
        &self.constructors
    }
    pub(super) fn blanket_impls(&self) -> &[DefinitionId] {
        &self.blankets
    }
}
impl<'cx> PreparedFacts<'cx> {
    /// All source facts and typed compiler-query inputs are explicit; the candidate index is
    /// derived later with the actual pinned classifier, never caller-supplied.
    pub(super) fn new(
        context: Cx<'cx>,
        source_membership: Arc<QueryValue>,
        definitions: BTreeMap<DefinitionId, DefinitionFacts<'cx>>,
        traits: BTreeMap<DefinitionId, TraitFacts<'cx>>,
        implementations: BTreeMap<DefinitionId, ImplFacts<'cx>>,
        associated_leaves: BTreeMap<
            (DefinitionId, DefinitionId),
            Result<AssociatedLeaf, ErrorToken>,
        >,
        impl_declarations: BTreeMap<Digest, DefinitionId>,
        adts: BTreeMap<DefinitionId, AdtFacts<'cx>>,
        parameters: BTreeMap<(ParamKey, BoundKind), DefinitionId>,
        lang: LangItems,
        const_evaluation: QueryFact<super::infer::const_eval::PreparedConstEvalInputs<'cx>>,
        transmutability: QueryFact<super::infer::transmute::PreparedTransmuteInputs<'cx>>,
        anonymous_bounds: BTreeMap<(usize, usize), Region<Cx<'cx>>>,
        anonymous_canonical: BTreeMap<usize, Region<Cx<'cx>>>,
    ) -> Self {
        Self {
            context,
            source_membership,
            definitions,
            traits,
            implementations,
            associated_leaves,
            impl_declarations,
            adts,
            parameters,
            lang,
            const_evaluation,
            transmutability,
            anonymous_bounds,
            anonymous_canonical,
            indexes: OnceCell::new(),
        }
    }
}

/// Immutable metadata/index snapshot. This is not permission to evaluate a
/// root, and cannot be paired with an arbitrary goal or inference owner.
#[derive(Clone, Copy)]
pub(super) struct PreparedIndexSnapshot<'cx> {
    provider: PreparedProvider<'cx>,
    context: Cx<'cx>,
}

/// Only the exact callbacks certified by Q89 are available during this pure
/// compiler elaboration. This scope does not publish final facts, enter the
/// inference/search graph, or authorize any arbitrary solver goal.
struct ImplElaborationInputs<'cx> {
    context: Cx<'cx>,
    implied: BTreeMap<DefinitionId, Spanned<'cx>>,
    sized: BTreeMap<DefinitionId, bool>,
}

/// Only simplify_type's actual dyn-principal flag queries may read this cold
/// preparation scope. It is neither published source facts nor root admission.
struct CandidateClassificationInputs<'cx> {
    context: Cx<'cx>,
    automatic: BTreeMap<DefinitionId, bool>,
}

/// Genuine compiler-derived query value, still owned by the same IR context.
/// A source fingerprint is provenance, not permission to run another root.
pub(super) struct PreparedImplSuperOutlives<'cx> {
    pub(super) implementation: DefinitionId,
    pub(super) source: Digest,
    pub(super) clauses: ClauseList<'cx>,
}

#[derive(Clone, Copy)]
pub(crate) struct PreparedProvider<'cx> {
    world: Digest,
    configuration: Digest,
    elaboration: &'cx ScopedCache<ImplElaborationInputs<'cx>>,
    candidate_classification: &'cx ScopedCache<CandidateClassificationInputs<'cx>>,
    settings: &'cx Settings<'cx>,
    facts: &'cx OnceCell<PreparedFacts<'cx>>,
    generic_facts: &'cx ScopedCache<BTreeMap<DefinitionId, Generics<'cx, PreparedProvider<'cx>>>>,
    diagnostics: &'cx RefCell<Vec<(Span, String)>>,
    root_active: &'cx Cell<Option<Digest>>,
}
pub(super) struct SourceRootGuard<'cx> { provider: PreparedProvider<'cx> }
impl Drop for SourceRootGuard<'_> {
    fn drop(&mut self) { self.provider.root_active.set(None); }
}
impl fmt::Debug for PreparedProvider<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedLrrSolverFacts")
    }
}
impl PartialEq for PreparedProvider<'_> {
    fn eq(&self, b: &Self) -> bool {
        std::ptr::eq(self.settings, b.settings)
            && std::ptr::eq(self.facts, b.facts)
            && std::ptr::eq(self.generic_facts, b.generic_facts)
            && std::ptr::eq(self.elaboration, b.elaboration)
            && std::ptr::eq(self.candidate_classification, b.candidate_classification)
            && self.configuration == b.configuration
            && std::ptr::eq(self.diagnostics, b.diagnostics)
            && std::ptr::eq(self.root_active, b.root_active)
            && self.world == b.world
    }
}
impl Eq for PreparedProvider<'_> {}
impl Hash for PreparedProvider<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        std::ptr::from_ref(self.settings).hash(h);
        std::ptr::from_ref(self.facts).hash(h);
        std::ptr::from_ref(self.generic_facts).hash(h);
        std::ptr::from_ref(self.elaboration).hash(h);
        std::ptr::from_ref(self.candidate_classification).hash(h);
        self.configuration.hash(h);
        std::ptr::from_ref(self.diagnostics).hash(h);
        std::ptr::from_ref(self.root_active).hash(h);
        self.world.hash(h);
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
pub(super) fn catalog_world(catalog: &DiscoveryInput) -> Result<Digest, Diagnostic> {
    if !matches!(catalog.seal.domain(), SealDomain::ImplCatalog { .. }) {
        return Err(fault(
            "upstream solving requires actual ImplCatalog completion, not a scope/test seal",
        ));
    }
    let completion = catalog.impl_catalog.as_ref().ok_or_else(|| {
        fault("upstream solving requires actual configured source-catalog completion")
    })?;
    completion.require_publication(catalog)?;
    Ok(catalog.seal.fingerprint())
}

/// Own the nonescaping context lifetime. This type-checks the concrete reader
/// and scoped arenas without inventing a source provider or inference client.
/// The callback may translate/freeze source facts and build an index snapshot.
/// Actual root admission remains separate. No borrowed IR handles can escape.
pub(super) fn with_provider<R>(
    settings: &Settings<'_>,
    configuration: &super::configuration::PreparedSolverConfiguration,
    run: impl for<'cx> FnOnce(Cx<'cx>, PreparedProvider<'cx>) -> R,
) -> Result<R, Diagnostic> {
    let facts = OnceCell::new();
    let diagnostics = RefCell::new(Vec::new());
    let root_active = Cell::new(None);
    let generic_facts = ScopedCache::empty();
    let elaboration = ScopedCache::empty();
    let candidate_classification = ScopedCache::empty();
    let provider = PreparedProvider::new(
        settings,
        configuration.fingerprint(),
        &facts,
        &generic_facts,
        &elaboration,
        &candidate_classification,
        &diagnostics,
        &root_active,
    )?;
    let state = super::context::State::new(provider);
    let view = state.view();
    let _cache = state.enter_cache();
    let prepared = settings
        .construction_generics
        .map(|source| source.materialize(view.context(), configuration))
        .transpose()?;
    // This owner drops the typed table before the context/interner. The
    // separate initially empty slot never retains arena handles at destruction.
    let _generic_guard = prepared.map(|prepared| generic_facts.enter(prepared));
    Ok(run(view.context(), provider))
}
impl<'cx> PreparedProvider<'cx> {
    /// Actual immutable source layout, available before const-Type generic
    /// materialization enters its IR table. Missing layout is never zero args.
    pub(super) fn source_argument_kinds(self, definition: DefinitionId) -> Option<&'cx [crate::traits::term::BoundKind]> {
        if definition.world() != self.world { return None; }
        self.settings.construction_generics?.argument_kinds(definition)
    }
    pub(super) fn const_eval_inputs(
        self,
    ) -> &'cx super::infer::const_eval::PreparedConstEvalInputs<'cx> {
        self.data()
            .const_evaluation
            .require("const evaluation inputs")
    }
    pub(super) fn transmute_inputs(
        self,
    ) -> &'cx super::infer::transmute::PreparedTransmuteInputs<'cx> {
        self.data()
            .transmutability
            .require("transmutability inputs")
    }
    pub(super) fn const_param_ty_trait(self) -> Option<DefinitionId> {
        self.data()
            .lang
            .const_param_ty
            .copied("const_param_ty language role")
    }
    pub(super) fn associated_leaf(
        self,
        implementation: DefinitionId,
        item: DefinitionId,
    ) -> Result<AssociatedLeaf, ErrorToken> {
        *self
            .data()
            .associated_leaves
            .get(&(implementation, item))
            .expect("actual specialization leaf was not prepared before solver entry")
    }
    pub(super) fn record_solver_overflow(self, span: Span, message: String) {
        // Diagnostic evidence only; this is not a lint/static rejection.
        self.diagnostics.borrow_mut().push((span, message));
    }
    pub(super) fn enable_next_solver_overflow_fcw(self) -> bool {
        self.settings.enable_next_solver_overflow_fcw
    }
    pub(super) fn disable_trait_solver_fast_paths(self) -> bool {
        self.settings.disable_trait_solver_fast_paths
    }
    pub(super) fn active_source_root(self) -> Option<Digest> { self.root_active.get() }
    pub(super) fn enter_source_root(self, admission: &super::reached_call::SourceRootAdmission) -> Result<SourceRootGuard<'cx>, Diagnostic> {
        if self.root_active.get().is_some() || self.facts.get().is_none()
            || admission.world() != self.world || admission.configuration() != self.configuration {
            return Err(fault("reached source root admission differs from prepared context"));
        }
        self.root_active.set(Some(admission.identity()));
        Ok(SourceRootGuard { provider: self })
    }
    pub(super) fn source_world(self) -> Digest {
        self.world
    }
    pub(super) fn source_configuration(self) -> Digest {
        self.configuration
    }
    pub(super) fn source_parameter(
        self,
        key: ParamKey,
        kind: BoundKind,
    ) -> Option<SourceParameter> {
        self.settings.construction_generics?.parameter(key, kind)
    }
    pub(super) fn source_early_region(self, key: ParamKey) -> Option<EarlyRegion<'cx>> {
        self.settings.construction_generics?.early_region(key)
    }
    /// Actual construction metadata only; absence is an unmet preparation
    /// demand, never permission to construct an unchecked projection alias.
    pub(super) fn source_projection_parent(self, id: DefinitionId) -> Option<DefinitionId> {
        self.settings.construction_generics?.projection_parent(id)
    }
    pub(super) fn require_generic_value(
        self,
        id: DefinitionId,
        value: Generics<'cx, Self>,
    ) -> Result<(), Diagnostic> {
        if id.world() != self.world
            || self
                .generic_facts
                .with(|generics| generics.get(&id).copied())
                != Some(value)
        {
            return Err(fault(
                "typed generics differ from actual source identity/order/parent count/const Type",
            ));
        }
        Ok(())
    }
    /// A typed reader handle is not a ready solver context. Its actual complete
    /// source producer must publish every demanded record before entry.
    fn new(
        settings: &'cx Settings<'cx>,
        configuration: Digest,
        facts: &'cx OnceCell<PreparedFacts<'cx>>,
        generic_facts: &'cx ScopedCache<
            BTreeMap<DefinitionId, Generics<'cx, PreparedProvider<'cx>>>,
        >,
        elaboration: &'cx ScopedCache<ImplElaborationInputs<'cx>>,
        candidate_classification: &'cx ScopedCache<CandidateClassificationInputs<'cx>>,
        diagnostics: &'cx RefCell<Vec<(Span, String)>>,
        root_active: &'cx Cell<Option<Digest>>,
    ) -> Result<Self, Diagnostic> {
        let world = catalog_world(&settings.catalog)?;
        Ok(Self {
            world,
            configuration,
            elaboration,
            candidate_classification,
            settings,
            facts,
            generic_facts,
            diagnostics,
            root_active,
        })
    }
    /// Execute the pinned compiler query on actual, retained source inputs.
    /// Every reachable callback domain must already be known; callbacks never
    /// discover source or unwind to request more preparation.
    pub(super) fn prepare_impl_super_outlives(
        self,
        cx: Cx<'cx>,
        plan: &super::source_impl_outlives::PreparedSourceImplOutlives,
    ) -> Result<PreparedImplSuperOutlives<'cx>, Diagnostic> {
        if cx.services() != self || self.facts.get().is_some() || self.root_active.get().is_some() {
            return Err(fault(
                "impl elaboration requires its unpublished preparation context",
            ));
        }
        if plan
            .configuration()
            .map_err(|why| fault(&format!("impl elaboration configuration pending: {why:?}")))?
            .fingerprint()
            != self.configuration
        {
            return Err(fault(
                "impl elaboration configuration differs from its context",
            ));
        }
        let encoded = plan
            .encode_inputs(cx)
            .map_err(|why| fault(&format!("impl elaboration inputs pending: {why:?}")))?;
        if !encoded.requirements().is_empty() {
            return Err(fault(&format!(
                "impl elaboration callback inputs pending: {:?}",
                encoded.requirements()
            )));
        }
        let implied = encoded
            .implied()
            .iter()
            .map(|(id, clauses)| {
                let retained: List<'cx, (C<'cx>, Span)> = cx.list(clauses.clone());
                (
                    *id,
                    Early::bind(cx, retained).map_bound(|clauses| clauses.items()),
                )
            })
            .collect();
        let inputs = ImplElaborationInputs {
            context: cx,
            implied,
            sized: encoded.sized().clone(),
        };
        let _scope = self.elaboration.enter(inputs);
        // Pinned rustc_hir_analysis/collect/item_bounds.rs:530-542. The
        // upstream iterator owns substitution, deduplication and outlives
        // decomposition; this adapter only supplies its genuine query inputs.
        let clause: C<'cx> = encoded.head().upcast(cx);
        let clauses = rustc_type_ir::elaborate::elaborate(cx, [clause])
            .filter(|clause| {
                matches!(
                    clause.kind().skip_binder(),
                    ty::ClauseKind::TypeOutlives(_) | ty::ClauseKind::RegionOutlives(_)
                )
            })
            .collect();
        Ok(PreparedImplSuperOutlives {
            implementation: encoded.implementation(),
            source: encoded.source_fingerprint(),
            clauses: Early::bind(cx, cx.list(clauses)),
        })
    }
    pub(super) fn publish(self, facts: PreparedFacts<'cx>) -> Result<(), Diagnostic> {
        if self.facts.get().is_some() || self.root_active.get().is_some() {
            return Err(fault("source facts require their unpublished preparation context"));
        }
        if facts.context.services() != self {
            return Err(fault("typed facts belong to another provider"));
        }
        if facts.indexes.get().is_some() {
            return Err(fault(
                "caller-supplied candidate indexes are not preparation evidence",
            ));
        }
        let world = self.world;
        for (id, row) in &facts.definitions {
            if let Some(variance) = row.variance_queries.known() {
                variance.require_source(
                    &row.receipt,
                    row.generics,
                    *row.impl_trait_in_trait.checked("is_impl_trait_in_trait")?,
                    &self.settings.catalog.seal.key(),
                )?;
            }
            let source = self
                .settings
                .construction_generics
                .and_then(|source| source.get(*id))
                .ok_or_else(|| fault("typed definition lacks actual generic source preparation"))?;
            if !row.receipt.matches_generics(source)
                || self.require_generic_value(*id, row.generics).is_err()
            {
                return Err(fault(
                    "typed generics differ from actual source parameter identity, order or const Type",
                ));
            }
            if id.world() != world || id.source() != row.receipt.definition {
                return Err(fault("prepared source declaration identity differs"));
            }
        }
        let QueryValue::ImplTraitCatalog(source_membership) = facts.source_membership.as_ref()
        else {
            return Err(fault(
                "typed facts lack the actual source membership catalog",
            ));
        };
        if !source_membership.matches_catalog(&self.settings.catalog) {
            return Err(fault(
                "source membership catalog is incomplete or belongs to another world",
            ));
        }
        let inventory = &self.settings.catalog.declaration_inventory;
        if facts.implementations.len() != facts.impl_declarations.len() {
            return Err(fault("demanded impl source/typed metadata sets differ"));
        }
        let mut declarations = BTreeSet::new();
        for (raw, id) in &facts.impl_declarations {
            let Ok(ordinal) = inventory
                .headers()
                .binary_search_by_key(raw, |header| header.identity)
            else {
                return Err(fault(
                    "translated impl is absent from the actual complete catalog",
                ));
            };
            let header = &inventory.headers()[ordinal];
            if !declarations.insert(*id) {
                return Err(fault(
                    "distinct impl declarations share a translated identity",
                ));
            }
            let Some(definition) = facts.definitions.get(id) else {
                return Err(fault("demanded impl lacks its prepared definition"));
            };
            let Some(implementation) = facts.implementations.get(id) else {
                return Err(fault("demanded impl lacks its prepared metadata"));
            };
            let (InputValue::DefinitionHeader(recipe), InputValue::DeclarationContext(context)) = (
                definition.receipt.header.as_ref(),
                definition.receipt.context.as_ref(),
            ) else {
                return Err(fault("actual impl source receipt differs"));
            };
            let InputValue::DefinitionSource(parse) = definition.receipt.source.as_ref() else {
                return Err(fault("demanded impl source variant differs"));
            };
            let Some(classified) = source_membership.classification(*raw) else {
                return Err(fault("demanded impl actual classification absent"));
            };
            if classified
                .syntax()
                .canonical_owner()
                .map(|owner| owner.identity())
                != Some(id.source())
                || !classified.matches_metadata(parse, recipe, context)
                || recipe.tokens != header.header
                || context.range != header.range
                || context.scope != header.scope
                || implementation
                    .trait_ref
                    .known()
                    .is_some_and(|head| definition.receipt.implementation != Some(head.is_some()))
            {
                return Err(fault(
                    "translated impl does not match its actual retained source",
                ));
            }
        }
        for (id, row) in &facts.adts {
            let definition = facts
                .definitions
                .get(id)
                .ok_or_else(|| fault("ADT definition is absent"))?;
            row.source_variants.require_shape(definition, &row.info)?;
            if let Some(discriminant) = row.info.discriminant.known() {
                match (discriminant, definition.receipt.is_enum()?) {
                    (Some(plan), true)
                        if row.info.kind == AdtKind::Enum && plan.matches(&definition.receipt) => {}
                    (None, false) if row.info.kind != AdtKind::Enum => {}
                    _ => {
                        return Err(fault(
                            "enum discriminant metadata lacks its actual source schema",
                        ));
                    }
                }
            }
        }
        for id in facts
            .traits
            .keys()
            .chain(facts.implementations.keys())
            .chain(facts.adts.keys())
            .chain(facts.parameters.values())
            .chain(facts.lang.traits.values().filter_map(Option::as_ref))
            .chain(facts.lang.projections.values().filter_map(Option::as_ref))
            .chain(facts.lang.adts.values().filter_map(Option::as_ref))
            .chain(facts.lang.trait_tests.keys().map(|(id, _)| id))
            .chain(facts.lang.trait_by_definition.keys())
            .chain(facts.lang.projection_by_definition.keys())
            .chain(facts.lang.adt_by_definition.keys())
        {
            if id.world() != world || !facts.definitions.contains_key(id) {
                return Err(fault(
                    "prepared fact refers to a missing/foreign declaration",
                ));
            }
        }
        let translated_sources = facts
            .impl_declarations
            .iter()
            .map(|(raw, id)| (*raw, id.source()))
            .collect();
        for (id, row) in &facts.traits {
            let source = source_membership;
            if !facts.definitions[id].receipt.trait_declaration {
                return Err(fault(
                    "trait membership is incomplete, foreign, or not a trait",
                ));
            }
            let Some(implementations) = row.implementations.known().copied() else {
                continue; // Trait flags do not establish candidate coverage.
            };
            let candidates = source.require_translation(
                &self.settings.catalog,
                id.source(),
                &translated_sources,
            )?;
            let mut actual = Vec::new();
            for (raw, canonical) in candidates {
                let candidate = *facts.impl_declarations.get(raw).ok_or_else(|| {
                    fault("demanded typed trait bucket omits an actual source impl")
                })?;
                if candidate.source() != *canonical {
                    return Err(fault("trait candidate canonical identity differs"));
                }
                let receipt = &facts.definitions[&candidate].receipt;
                let (
                    InputValue::DefinitionSource(parse),
                    InputValue::DefinitionHeader(header),
                    InputValue::DeclarationContext(context),
                ) = (
                    receipt.source.as_ref(),
                    receipt.header.as_ref(),
                    receipt.context.as_ref(),
                )
                else {
                    return Err(fault("trait candidate receipt variants differ"));
                };
                if !source
                    .classification(*raw)
                    .is_some_and(|identity| identity.matches_metadata(parse, header, context))
                {
                    return Err(fault(
                        "typed candidate differs from its actual classified source",
                    ));
                }
                actual.push(candidate);
            }
            actual.sort_unstable();
            if actual != implementations {
                return Err(fault(
                    "typed trait bucket differs from its complete source candidates",
                ));
            }
            if implementations.windows(2).any(|p| p[0] >= p[1]) {
                return Err(fault(
                    "prepared candidate identities are not unique and ordered",
                ));
            }
            for implementation in implementations {
                let Some(implementation) = facts.implementations.get(implementation) else {
                    return Err(fault("candidate bucket refers to an unprepared impl"));
                };
                if implementation
                    .trait_ref
                    .checked("impl trait head")?
                    .map(|r| r.def_id())
                    != Some(*id)
                {
                    return Err(fault("candidate belongs to a different actual trait"));
                }
            }
        }
        if let Some(Some(id)) = facts.lang.field_representing_type.known().copied()
            && (id.world() != world || !facts.adts.contains_key(&id))
        {
            return Err(fault(
                "reflection lang item lacks actual nominal source metadata",
            ));
        }
        if let Some(Some(id)) = facts.lang.const_param_ty.known().copied()
            && (id.world() != world
                || !facts.traits.contains_key(&id)
                || !facts
                    .definitions
                    .get(&id)
                    .is_some_and(|row| row.receipt.is_trait()))
        {
            return Err(fault(
                "WF const-parameter lang item lacks its actual trait source",
            ));
        }
        for (id, implementation) in &facts.implementations {
            if let Some(Some(reference)) = implementation.trait_ref.known() {
                let definition = facts
                    .definitions
                    .get(&reference.def_id())
                    .ok_or_else(|| fault("actual impl trait declaration is missing"))?;
                if !definition.receipt.trait_declaration {
                    return Err(fault("actual impl head does not denote a trait"));
                }
                if let Some(candidates) = facts
                    .traits
                    .get(&reference.def_id())
                    .and_then(|data| data.implementations.known())
                    && candidates.binary_search(id).is_err()
                {
                    return Err(fault("prepared trait bucket omits an actual trait impl"));
                }
            }
        }
        for (id, row) in &facts.traits {
            // This query is independent of complete function/constant member
            // enumeration. Validate its published identities even when the
            // neighboring associated_items domain is not prepared.
            if let Some(types) = row.associated_types.known() {
                let mut unique = BTreeSet::new();
                for member in *types {
                    let declaration = facts
                        .definitions
                        .get(member)
                        .ok_or_else(|| fault("associated type declaration is missing"))?;
                    if member.world() != world
                        || !unique.insert(*member)
                        || declaration.parent.known() != Some(&Some(*id))
                        || !declaration.receipt.is_type_alias()?
                    {
                        return Err(fault(
                            "associated type identity/role/parent differs from source",
                        ));
                    }
                }
            }
            if let Some(items) = row.associated_items.known() {
                let unique: BTreeSet<_> = <[DefinitionId]>::iter(items).copied().collect();
                if unique.len() != items.len()
                    || <[DefinitionId]>::iter(items).any(|item| {
                        facts
                            .definitions
                            .get(item)
                            .is_none_or(|item| item.parent.known() != Some(&Some(*id)))
                    })
                    || row.associated_types.known().is_some_and(|types| {
                        <[DefinitionId]>::iter(types).any(|item| !unique.contains(item))
                    })
                {
                    return Err(fault(
                        "associated member coverage differs from its prepared trait",
                    ));
                }
            }
        }
        for ((implementation, member), outcome) in &facts.associated_leaves {
            let implementation_data = facts
                .implementations
                .get(implementation)
                .ok_or_else(|| fault("associated leaf impl metadata is absent"))?;
            let trait_id = implementation_data
                .trait_ref
                .checked("associated leaf impl trait")?
                .ok_or_else(|| fault("associated leaf requires an actual trait impl"))?
                .def_id();
            let trait_data = facts
                .traits
                .get(&trait_id)
                .ok_or_else(|| fault("associated leaf trait metadata is absent"))?;
            if !trait_data
                .associated_items
                .checked("associated members")?
                .contains(member)
            {
                return Err(fault("associated leaf key belongs to another trait"));
            }
            if let Ok(leaf) = outcome {
                let ancestor = |node| {
                    node == *implementation
                        || node == trait_id
                        || implementation_data
                            .specializes
                            .known()
                            .is_some_and(|ancestors| ancestors.contains(&node))
                };
                if !ancestor(leaf.defining_node)
                    || leaf.finalizing_node.is_some_and(|node| !ancestor(node))
                    || [
                        Some(leaf.item),
                        Some(leaf.defining_node),
                        leaf.finalizing_node,
                    ]
                    .into_iter()
                    .flatten()
                    .any(|node| node.world() != world || !facts.definitions.contains_key(&node))
                {
                    return Err(fault(
                        "associated leaf nodes differ from their prepared source ancestry",
                    ));
                }
                if [Some(leaf.defining_node), leaf.finalizing_node]
                    .into_iter()
                    .flatten()
                    .any(|node| {
                        node != trait_id
                            && facts.implementations.get(&node).is_none_or(|row| {
                                row.trait_ref
                                    .known()
                                    .and_then(|head| head.map(|reference| reference.def_id()))
                                    != Some(trait_id)
                            })
                    })
                {
                    return Err(fault(
                        "associated leaf container is not an impl of its actual trait",
                    ));
                }
                let item = &facts.definitions[&leaf.item];
                let declared = &facts.definitions[member];
                if item.parent.known() != Some(&Some(leaf.defining_node))
                    || item.name != declared.name
                    || (leaf.defining_node == trait_id && leaf.item != *member)
                {
                    return Err(fault(
                        "associated leaf item differs from its prepared container/member",
                    ));
                }
                if let Some(finalizing) = leaf.finalizing_node
                    && finalizing != leaf.defining_node
                    && leaf.defining_node != trait_id
                    && !facts.implementations.get(&finalizing).is_some_and(|row| {
                        row.specializes
                            .known()
                            .is_some_and(|ancestors| ancestors.contains(&leaf.defining_node))
                    })
                {
                    return Err(fault(
                        "associated leaf finalizer is outside its source ancestry segment",
                    ));
                }
            }
        }
        let index = CandidateIndex::prepare(facts.context, &facts)?;
        facts.indexes.set(index)
            .map_err(|_| fault("candidate index was already prepared"))?;
        self.facts
            .set(facts)
            .map_err(|_| fault("prepared source facts were already frozen"))
    }
    pub(super) fn ready(self, cx: Cx<'cx>) -> Result<PreparedIndexSnapshot<'cx>, Diagnostic> {
        if self.facts.get().is_none() {
            return Err(fault("complete actual source facts are not prepared"));
        }
        if cx.services() != self || cx != self.data().context {
            return Err(fault("source-fact reader belongs to another context"));
        }
        // The historical coarse entry must not authorize arbitrary goals from
        // partially published query slots. Exact root admission is separate.
        if self.data().definitions.values().any(|row| !row.prepared())
            || self.data().traits.values().any(|row| !row.prepared())
            || self
                .data()
                .implementations
                .values()
                .any(|row| !row.prepared())
        {
            return Err(fault("root query closure has unprepared compiler domains"));
        }
        for (id, implementation) in &self.data().implementations {
            if let Some(reference) = implementation.trait_ref.copied("impl trait head") {
                let members = self
                    .trait_data(reference.def_id())
                    .associated_items
                    .copied("complete associated members");
                if members
                    .iter()
                    .any(|member| !self.data().associated_leaves.contains_key(&(*id, *member)))
                {
                    return Err(fault("coarse root entry lacks complete associated leaves"));
                }
            }
        }
        if self.data().indexes.get().is_none() {
            return Err(fault("published candidate indexes are not prepared"));
        }
        Ok(PreparedIndexSnapshot {
            provider: self,
            context: cx,
        })
    }
    pub(super) fn prepared_impls(self, id: DefinitionId) -> &'cx ImplBucket {
        self.data()
            .indexes
            .get()
            .expect("pinned candidate indexes must be ready before selection")
            .traits
            .get(&id)
            .expect("actual source trait index is missing")
    }
    fn data(self) -> &'cx PreparedFacts<'cx> {
        self.facts
            .get()
            .expect("source preparation must freeze facts before upstream callbacks")
    }
    fn definition(self, id: DefinitionId) -> &'cx DefinitionFacts<'cx> {
        assert_eq!(id.world(), self.world, "foreign prepared definition world");
        self.data()
            .definitions
            .get(&id)
            .expect("demanded source declaration was not prepared")
    }
    fn trait_data(self, id: DefinitionId) -> &'cx TraitFacts<'cx> {
        self.data()
            .traits
            .get(&id)
            .expect("demanded source trait was not prepared")
    }
    fn implementation(self, id: DefinitionId) -> &'cx ImplFacts<'cx> {
        self.data()
            .implementations
            .get(&id)
            .expect("demanded source impl was not prepared")
    }
    fn const_data(self, id: DefinitionId) -> &'cx ConstFacts<'cx> {
        &self.definition(id).constant
    }
    fn parent(self, id: DefinitionId) -> DefinitionId {
        if let Some(parent) = self
            .settings
            .construction_generics
            .and_then(|source| source.projection_parent(id))
        {
            return parent;
        }
        self.definition(id)
            .parent
            .copied("associated parent")
            .expect("source item has no associated parent")
    }
    fn alias_layout(self, kind: AliasTermKind<Cx<'cx>>) -> &'cx [BoundKind] {
        if let AliasTermKind::ProjectionTy { def_id } = kind
            && let Some(layout) = self
                .settings
                .construction_generics
                .and_then(|source| source.projection_argument_kinds(def_id))
        {
            return layout;
        }
        self.definition(alias_definition(kind))
            .alias_layouts
            .require("alias argument layouts")
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| v.as_slice())
            .expect("actual alias argument layout was not prepared")
    }
    fn compatible(self, layout: impl IntoIterator<Item = BoundKind>, args: A<'cx>) -> bool {
        let mut layout = layout.into_iter();
        for arg in args {
            let kind = match arg {
                Arg::Type(_) => BoundKind::Type,
                Arg::Region(_) => BoundKind::Region,
                Arg::Const(_) => BoundKind::Const,
            };
            if layout.next() != Some(kind) {
                return false;
            }
        }
        layout.next().is_none()
    }
    fn ordinary_layout(self, id: DefinitionId) -> impl Iterator<Item = BoundKind> + 'cx {
        assert_eq!(
            id.world(),
            self.world,
            "foreign constructor definition world"
        );
        let source = self
            .settings
            .construction_generics
            .and_then(|layouts| layouts.argument_kinds(id));
        let typed = source
            .is_none()
            .then(|| self.definition(id).generics.parameters);
        source
            .into_iter()
            .flat_map(|kinds| kinds.iter().copied())
            .chain(
                typed
                    .into_iter()
                    .flat_map(|parameters| parameters.iter().map(parameter_kind)),
            )
    }
    fn report(self, span: Span, message: String) -> ErrorToken {
        let mut e = StableEncoder::new("lrr/v1/upstream-client-error");
        e.digest(span.origin);
        e.u32(span.start);
        e.u32(span.end);
        e.string(&message);
        let token = ErrorToken(e.finish());
        self.diagnostics.borrow_mut().push((span, message));
        token
    }
}
fn parameter_kind<'cx, F: SourceFacts<'cx>>(parameter: GenericParameter<'cx, F>) -> BoundKind {
    match parameter {
        GenericParameter::Type(_) => BoundKind::Type,
        GenericParameter::Region(_) => BoundKind::Region,
        GenericParameter::Const(_, _) => BoundKind::Const,
    }
}
fn alias_definition(kind: AliasTermKind<Cx<'_>>) -> DefinitionId {
    match kind {
        AliasTermKind::ProjectionTy { def_id }
        | AliasTermKind::ProjectionConst { def_id }
        | AliasTermKind::InherentTy { def_id }
        | AliasTermKind::OpaqueTy { def_id }
        | AliasTermKind::FreeTy { def_id }
        | AliasTermKind::AnonConst { def_id }
        | AliasTermKind::FreeConst { def_id }
        | AliasTermKind::InherentConstSelf { def_id }
        | AliasTermKind::InherentConstImpl { def_id } => def_id,
    }
}

impl<'cx> SourceFacts<'cx> for PreparedProvider<'cx> {
    fn assert_normal_root_scope(self) { assert!(self.root_active.get().is_some(), "global cache requires admitted reached-call scope"); }
    fn abstract_const(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Result<Option<Early<'cx, Const<Cx<'cx>>>>, ErrorToken> {
        self.const_data(id).abstract_value.copied("abstract_value")
    }
    fn generics_of(self, _: Cx<'cx>, id: DefinitionId) -> Generics<'cx, Self> {
        assert_eq!(id.world(), self.world, "foreign generic fact world");
        self.generic_facts
            .with(|generics| generics.get(&id).copied())
            .unwrap_or_else(|| self.definition(id).generics)
    }
    fn variances_of(self, _: Cx<'cx>, id: DefinitionId) -> &'cx [Variance] {
        self.definition(id)
            .variance_queries
            .require("variances_of")
            .variances_of()
    }
    fn opt_alias_variances(
        self,
        _: Cx<'cx>,
        kind: impl Into<AliasTermKind<Cx<'cx>>>,
    ) -> Option<&'cx [Variance]> {
        super::source_variance_queries::opt_alias_variances(
            kind.into(),
            |id| {
                self.definition(id)
                    .impl_trait_in_trait
                    .copied("impl_trait_in_trait")
            },
            |id| {
                self.definition(id)
                    .variance_queries
                    .require("variances_of")
                    .variances_of()
            },
        )
    }
    fn type_of(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, T<'cx>> {
        self.definition(id).ty.copied("ty")
    }
    fn type_of_opaque_hir_typeck(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, T<'cx>> {
        self.definition(id).opaque_hidden.copied("opaque_hidden")
    }
    fn is_direct_const(self, _: Cx<'cx>, alias: AliasConstKind<Cx<'cx>>) -> bool {
        self.const_data(alias_definition(alias.into()))
            .direct
            .copied("direct")
    }
    fn const_of_item(
        self,
        _: Cx<'cx>,
        alias: AliasConstKind<Cx<'cx>>,
    ) -> Option<Early<'cx, Const<Cx<'cx>>>> {
        self.const_data(alias_definition(alias.into()))
            .value
            .copied("value")
    }
    fn anon_const_kind(self, _: Cx<'cx>, id: DefinitionId) -> AnonConstKind {
        self.const_data(id).anon_kind.copied("anon_kind")
    }
    fn def_span(self, _: Cx<'cx>, id: DefinitionId) -> Span {
        self.definition(id).span
    }
    fn trait_ref_and_own_args_for_alias(
        self,
        cx: Cx<'cx>,
        id: DefinitionId,
        args: A<'cx>,
    ) -> (TraitRef<Cx<'cx>>, &'cx [Arg<'cx, Self>]) {
        let reference = TraitRef::from_assoc(cx, self.parent(id), args);
        (reference, &args.items()[reference.args.len()..])
    }
    fn check_alias_term_args_compatible(
        self,
        _: Cx<'cx>,
        kind: AliasTermKind<Cx<'cx>>,
        args: A<'cx>,
    ) -> bool {
        self.compatible(self.alias_layout(kind).iter().copied(), args)
    }
    fn debug_assert_args_compatible(self, _: Cx<'cx>, id: DefinitionId, args: A<'cx>) {
        debug_assert!(self.compatible(self.ordinary_layout(id), args));
    }
    fn debug_assert_alias_term_args_compatible(
        self,
        cx: Cx<'cx>,
        kind: AliasTermKind<Cx<'cx>>,
        args: A<'cx>,
    ) {
        debug_assert!(self.check_alias_term_args_compatible(cx, kind, args));
    }
    fn debug_assert_existential_args_compatible(self, _: Cx<'cx>, id: DefinitionId, args: A<'cx>) {
        let mut layout = self.ordinary_layout(id);
        debug_assert_eq!(layout.next(), Some(BoundKind::Type));
        debug_assert!(self.compatible(layout, args));
    }
    fn projection_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn impl_or_trait_assoc_term_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn inherent_alias_term_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn recursion_limit(self) -> usize {
        self.settings.recursion_limit
    }
    fn features(self) -> FeatureSet<'cx> {
        self.settings.features
    }
    fn assumptions_on_binders(self) -> bool {
        self.settings.assumptions_on_binders
    }
    fn renormalize_rigid_aliases(self) -> bool {
        self.settings.renormalize_rigid_aliases
    }
    fn coroutine_hidden_types(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, Binder<Cx<'cx>, CoroutineWitnessTypes<Cx<'cx>>>> {
        self.definition(id)
            .coroutine
            .hidden
            .copied("coroutine hidden")
    }
    fn fn_sig(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, Binder<Cx<'cx>, FnSig<Cx<'cx>>>> {
        self.definition(id).signature.copied("signature")
    }
    fn coroutine_movability(self, _: Cx<'cx>, id: DefinitionId) -> Movability {
        self.definition(id)
            .coroutine
            .movability
            .copied("coroutine movability")
    }
    fn coroutine_for_closure(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.definition(id)
            .closure_coroutine
            .copied("closure_coroutine")
    }
    fn generics_require_sized_self(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .requires_sized_self
            .copied("requires_sized_self")
    }
    fn item_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.item.copied("item")
    }
    fn explicit_item_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        self.definition(id)
            .predicates
            .explicit_item
            .copied("explicit_item")
            .map_bound(|values| values.iter().copied())
    }
    fn is_conditionally_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .conditionally_const
            .copied("conditionally_const")
    }
    fn item_self_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.item_self.copied("item_self")
    }
    fn item_non_self_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id)
            .predicates
            .item_non_self
            .copied("item_non_self")
    }
    fn clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.all.copied("all")
    }
    fn own_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.own.copied("own")
    }
    fn explicit_super_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        self.definition(id)
            .predicates
            .explicit_super
            .copied("explicit_super")
            .map_bound(|v| v.iter().copied())
    }
    fn explicit_implied_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        let value = self
            .elaboration
            .with_if_entered(|inputs| {
                assert_eq!(
                    inputs.context.services(),
                    self,
                    "foreign elaboration provider"
                );
                *inputs
                    .implied
                    .get(&id)
                    .expect("uncertified impl elaboration implied-clause query")
            })
            .unwrap_or_else(|| {
                self.definition(id)
                    .predicates
                    .explicit_implied
                    .copied("explicit_implied")
            });
        value.map_bound(|v| v.iter().copied())
    }
    fn impl_super_outlives(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.implementation(id)
            .super_outlives
            .copied("super_outlives")
    }
    fn supertrait_def_ids(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> impl Iterator<Item = DefinitionId> {
        self.trait_data(id)
            .supertraits
            .copied("supertraits")
            .iter()
            .copied()
    }
    fn impl_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).constant.copied("constant")
    }
    fn fn_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .constant_function
            .copied("constant_function")
    }
    fn closure_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .constant_closure
            .copied("constant_closure")
    }
    fn alias_has_const_conditions(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .has_const_conditions
            .copied("has_const_conditions")
    }
    fn const_conditions(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = Binder<Cx<'cx>, TraitRef<Cx<'cx>>>>> {
        self.definition(id)
            .predicates
            .const_conditions
            .copied("const_conditions")
            .map_bound(|v| v.iter().copied())
    }
    fn explicit_implied_const_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = Binder<Cx<'cx>, TraitRef<Cx<'cx>>>>> {
        self.definition(id)
            .predicates
            .implied_const_bounds
            .copied("implied_const_bounds")
            .map_bound(|v| v.iter().copied())
    }
    fn impl_self_is_guaranteed_unsized(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id)
            .guaranteed_unsized
            .copied("guaranteed_unsized")
    }
    fn has_target_features(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .target_features
            .copied("target_features")
    }
    fn require_projection_lang_item(
        self,
        _: Cx<'cx>,
        item: SolverProjectionLangItem,
    ) -> DefinitionId {
        self.data()
            .lang
            .projections
            .get(&item)
            .copied()
            .expect("required projection language role lookup is unprepared")
            .expect("required projection language role is absent")
    }
    fn require_trait_lang_item(self, _: Cx<'cx>, item: SolverTraitLangItem) -> DefinitionId {
        self.data()
            .lang
            .traits
            .get(&item)
            .copied()
            .expect("required trait language role lookup is unprepared")
            .expect("required trait language role is absent")
    }
    fn require_adt_lang_item(self, _: Cx<'cx>, item: SolverAdtLangItem) -> DefinitionId {
        self.data()
            .lang
            .adts
            .get(&item)
            .copied()
            .expect("required ADT language role lookup is unprepared")
            .expect("required ADT language role is absent")
    }
    fn is_projection_lang_item(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
        item: SolverProjectionLangItem,
    ) -> bool {
        self.data()
            .lang
            .projection_by_definition
            .get(&id)
            .expect("actual projection language classification is unprepared")
            .as_ref()
            == Some(&item)
    }
    fn is_trait_lang_item(self, _: Cx<'cx>, id: DefinitionId, item: SolverTraitLangItem) -> bool {
        if let Some(answer) = self.elaboration.with_if_entered(|inputs| {
            assert_eq!(
                item,
                SolverTraitLangItem::Sized,
                "uncertified elaboration language-role query"
            );
            *inputs
                .sized
                .get(&id)
                .expect("uncertified impl elaboration Sized query")
        }) {
            return answer;
        }
        if let Some(answer) = self.data().lang.trait_tests.get(&(id, item)) {
            return *answer;
        }
        self.data()
            .lang
            .trait_by_definition
            .get(&id)
            .expect("actual trait language classification is unprepared")
            .as_ref()
            == Some(&item)
    }
    fn is_adt_lang_item(self, _: Cx<'cx>, id: DefinitionId, item: SolverAdtLangItem) -> bool {
        self.data()
            .lang
            .adt_by_definition
            .get(&id)
            .expect("actual ADT language classification is unprepared")
            .as_ref()
            == Some(&item)
    }
    fn is_default_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).default_trait.copied("default_trait")
    }
    fn is_sizedness_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id)
            .sizedness_trait
            .copied("sizedness_trait")
    }
    fn as_projection_lang_item(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Option<SolverProjectionLangItem> {
        *self
            .data()
            .lang
            .projection_by_definition
            .get(&id)
            .expect("actual projection language classification is unprepared")
    }
    fn as_trait_lang_item(self, _: Cx<'cx>, id: DefinitionId) -> Option<SolverTraitLangItem> {
        *self
            .data()
            .lang
            .trait_by_definition
            .get(&id)
            .expect("actual trait language classification is unprepared")
    }
    fn as_adt_lang_item(self, _: Cx<'cx>, id: DefinitionId) -> Option<SolverAdtLangItem> {
        *self
            .data()
            .lang
            .adt_by_definition
            .get(&id)
            .expect("actual ADT language classification is unprepared")
    }
    fn associated_type_def_ids(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> impl IntoIterator<Item = DefinitionId> {
        self.trait_data(id)
            .associated_types
            .copied("associated_types")
            .iter()
            .copied()
    }
    fn for_each_relevant_impl<R: VisitorResult>(
        self,
        cx: Cx<'cx>,
        reference: TraitRef<Cx<'cx>>,
        f: impl FnMut(DefinitionId) -> R,
    ) -> R {
        self.for_each_relevant_source_impl(cx, reference.def_id, reference.self_ty(), f)
    }
    fn for_each_blanket_impl<R: VisitorResult>(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
        mut f: impl FnMut(DefinitionId) -> R,
    ) -> R {
        for id in self.prepared_impls(id).blanket_impls() {
            ty::try_visit!(f(*id));
        }
        R::output()
    }
    fn has_item_definition(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).has_definition.copied("has_definition")
    }
    fn impl_specializes(self, _: Cx<'cx>, id: DefinitionId, victim: DefinitionId) -> bool {
        self.implementation(id)
            .specializes
            .require("specialization ancestry")
            .contains(&victim)
    }
    fn impl_is_default(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).default_impl.copied("default_impl")
    }
    fn impl_trait_ref(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, TraitRef<Cx<'cx>>> {
        self.implementation(id)
            .trait_ref
            .copied("impl trait reference")
            .expect("trait reference requested for an inherent impl")
    }
    fn impl_polarity(self, _: Cx<'cx>, id: DefinitionId) -> ImplPolarity {
        self.implementation(id).polarity.copied("polarity")
    }
    fn is_fully_generic_for_reflection(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id)
            .reflection_generic
            .copied("reflection_generic")
    }
    fn trait_is_auto(self, cx: Cx<'cx>, id: DefinitionId) -> bool {
        if let Some(answer) = self.candidate_classification.with_if_entered(|inputs| {
            assert_eq!(inputs.context, cx, "foreign candidate classification context");
            *inputs.automatic.get(&id)
                .expect("uncertified candidate classification automatic query")
        }) {
            return answer;
        }
        self.trait_data(id).automatic.copied("automatic")
    }
    fn trait_is_marker(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).marker.copied("marker")
    }
    fn trait_is_coinductive(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).coinductive.copied("coinductive")
    }
    fn trait_is_alias(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).alias.copied("alias")
    }
    fn trait_is_dyn_compatible(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).dyn_compatible.copied("dyn_compatible")
    }
    fn trait_is_fundamental(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).fundamental.copied("fundamental")
    }
    fn trait_is_unsafe(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).unsafe_trait.copied("unsafe_trait")
    }
    fn is_impl_trait_in_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .impl_trait_in_trait
            .copied("impl_trait_in_trait")
    }
    fn delay_bug(self, _: Cx<'cx>, message: impl ToString) -> ErrorToken {
        self.report(
            <Span as ty::inherent::Span<Cx<'cx>>>::dummy(),
            message.to_string(),
        )
    }
    fn span_delayed_bug(self, _: Cx<'cx>, span: Span, message: impl ToString) -> ErrorToken {
        self.report(span, message.to_string())
    }
    fn is_general_coroutine(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .general
            .copied("coroutine general")
    }
    fn coroutine_is_async(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .asynchronous
            .copied("coroutine asynchronous")
    }
    fn coroutine_is_gen(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .generator
            .copied("coroutine generator")
    }
    fn coroutine_is_async_gen(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .async_generator
            .copied("coroutine async_generator")
    }
    fn unsizing_params_for_adt(self, _: Cx<'cx>, id: DefinitionId) -> &'cx DenseBitSet<u32> {
        self.data()
            .adts
            .get(&id)
            .expect("ADT metadata was not prepared")
            .unsizing
            .require("ADT unsizing parameters")
    }
    fn opaque_types_defined_by(self, _: Cx<'cx>, id: DefinitionId) -> List<'cx, DefinitionId> {
        self.definition(id).opaques.copied("opaques")
    }
    fn opaque_types_and_coroutines_defined_by(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> List<'cx, DefinitionId> {
        self.definition(id)
            .opaques_and_coroutines
            .copied("opaques_and_coroutines")
    }
    fn evaluate_root_goal_for_proof_tree_raw(
        self,
        cx: Cx<'cx>,
        goal: Interned<'cx, ty::solve::CanonicalInputData<Cx<'cx>>>,
        depth: usize,
    ) -> ProofResult<'cx> {
        assert!(
            false,
            "raw proof entry is not authorized by an ordinary reached-call session"
        );
        assert_eq!(
            self.data().context,
            cx,
            "proof-tree context differs from prepared source"
        );
        assert!(
            self.data().indexes.get().is_some(),
            "proof-tree candidate index is not prepared"
        );
        rustc_next_trait_solver::solve::evaluate_root_goal_for_proof_tree_raw_provider::<
            super::infer::delegate::SolverContext<'cx>,
            _,
        >(cx, goal, depth)
    }
    fn item_name(self, _: Cx<'cx>, id: DefinitionId) -> Symbol<'cx> {
        self.definition(id).name
    }
    fn get_anon_re_bounds_lifetime(
        self,
        _: Cx<'cx>,
        index: usize,
        var: usize,
    ) -> Option<Region<Cx<'cx>>> {
        self.data().anonymous_bounds.get(&(index, var)).copied()
    }
    fn get_anon_re_canonical_bounds_lifetime(
        self,
        _: Cx<'cx>,
        index: usize,
    ) -> Option<Region<Cx<'cx>>> {
        self.data().anonymous_canonical.get(&index).copied()
    }
    fn parameter_definition(self, _: Cx<'cx>, parameter: SourceParameter) -> DefinitionId {
        *self
            .data()
            .parameters
            .get(&(parameter.source(), parameter.kind()))
            .expect("source parameter declaration was not prepared")
    }
    fn adt_info(self, _: Cx<'cx>, id: DefinitionId) -> &'cx AdtInfo<'cx, Self> {
        &self
            .data()
            .adts
            .get(&id)
            .expect("ADT metadata was not prepared")
            .info
    }
    fn field_representing_type_definition(self, _: Cx<'cx>) -> Option<DefinitionId> {
        self.data()
            .lang
            .field_representing_type
            .copied("field_representing_type language role")
    }
    fn field_representing_type_info(
        self,
        cx: Cx<'cx>,
        id: DefinitionId,
        args: A<'cx>,
    ) -> Option<FieldInfo<Cx<'cx>>> {
        AdtDefinition {
            context: cx,
            definition: id,
        }
        .pinned_field_representing_type_info(cx, args)
    }
    fn discriminant_type(self, cx: Cx<'cx>, ty: T<'cx>) -> T<'cx> {
        ty.pinned_discriminant_ty(cx)
    }
}

/// Host-only source-attachment control, called with the actual discovery
/// prepared by the ordinary CLI harness. It never supplies invented solver
/// facts or promotes that scope certificate into catalog completeness.
#[cfg(test)]
pub(crate) fn check_scope_receipts(db: &mut Database, seal: crate::db::seals::SealKey) -> usize {
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("{error}"),
            QueryPoll::NeedInput(input) => panic!("unprepared source receipt: {input:?}"),
        }
    }
    let input = db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        panic!("actual discovery input absent")
    };
    assert!(matches!(catalog.seal.domain(), SealDomain::Scope { .. }));
    assert_eq!(
        catalog_world(catalog).unwrap_err().code,
        "LRR-E-UPSTREAM-SOLVER-INPUT"
    );
    let prepared = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal)));
    let QueryValue::PreparedScopeImplHeaders(inventory) = prepared.value.as_ref() else {
        panic!("actual source inventory absent")
    };
    db.trace.detailed = true;
    let start = db.trace.events.len();
    let mut observed = 0;
    for header in inventory.inventory_root().headers() {
        let owner = inventory
            .canonical_owner(header.identity)
            .expect("actual impl owner")
            .identity();
        let receipt =
            ready(DefinitionReceipt::observe(db, owner)).expect("actual declaration receipt");
        let repeated = ready(DefinitionReceipt::observe(db, owner)).unwrap();
        assert_eq!(receipt.definition, owner);
        assert!(Arc::ptr_eq(&receipt.source, &repeated.source));
        assert!(Arc::ptr_eq(&receipt.header, &repeated.header));
        assert!(Arc::ptr_eq(&receipt.context, &repeated.context));
        assert!(Arc::ptr_eq(&receipt.parsed, &repeated.parsed));
        let InputValue::DefinitionSource(parse) = receipt.source.as_ref() else {
            panic!()
        };
        let InputValue::DefinitionHeader(recipe) = receipt.header.as_ref() else {
            panic!()
        };
        let InputValue::DeclarationContext(context) = receipt.context.as_ref() else {
            panic!()
        };
        assert_eq!(recipe.tokens, header.header);
        assert_eq!(context.range, header.range);
        assert_eq!(context.scope, header.scope);
        assert_eq!(parse.profile, recipe.profile);
        assert!(receipt.implementation.is_some());
        observed += 1;
    }
    let absent = Digest::of_bytes(b"unpublished declaration receipt control");
    assert!(db.input(&InputKey::DefinitionSource(absent)).is_none());
    assert!(ready(DefinitionReceipt::observe(db, absent)).is_none());
    assert!(db.trace.events[start..].iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_)
            | QueryKey::Thunk(_)
            | QueryKey::PatternPlan(_)
            | QueryKey::FunctionSignature(_)
    )));
    observed
}

#[cfg(test)]
pub(crate) fn check_completed_source_catalog(input: &DiscoveryInput) -> Result<Digest, Diagnostic> {
    catalog_world(input)
}

/// Supplemental source receipt control; never constructs an opaque Type ID,
/// full facts, client table or ReadyProvider from syntax-only information.
#[cfg(test)]
pub(crate) fn check_opaque_return_receipt(
    db: &mut Database,
    definition: Digest,
    expected: &Arc<QueryValue>,
    previous: Option<&Arc<QueryValue>>,
) {
    let receipt = match DefinitionReceipt::observe(db, definition) {
        QueryPoll::Ready(Ok(Some(receipt))) => receipt,
        other => panic!("actual opaque function receipt: {other:?}"),
    };
    let QueryValue::DeclarationGenerics(current) = expected.as_ref() else {
        panic!("actual generic declaration result")
    };
    assert!(receipt.matches_generics(current));
    let plan = current
        .opaque_returns()
        .expect("actual declared return Type");
    assert!(
        std::ptr::eq(receipt.opaque_returns().unwrap(), plan),
        "receipt must share the actual retained declaration plan"
    );
    if let Some(previous) = previous {
        let QueryValue::DeclarationGenerics(old) = previous.as_ref() else {
            panic!()
        };
        assert!(
            !receipt.matches_generics(old),
            "replaced source metadata must not acquire current authority"
        );
    }
}
