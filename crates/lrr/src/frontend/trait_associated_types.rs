//! Demanded trait-owned type declarations, joined to actual published sources.
//! Legacy projection name keys are lookup keys, never declaration identities.
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::Database,
        poll::QueryPoll, provider::SyntaxProvider, seals::SealKey, value::QueryValue},
    source::manifest::Namespace,
    syntax::{ast_adapter::range, outline::ScopeKey},
};
use ra_ap_syntax::{AstNode, ast::{self, HasName}};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct TraitAssociatedTypesKey {
    pub inventory: SealKey,
    pub trait_definition: Digest,
}
impl StableEncode for TraitAssociatedTypesKey {
    const DOMAIN: &'static str = "lrr/v1/trait-associated-types-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory); e.digest(self.trait_definition);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub enum TraitAssociatedTypePending {
    TraitSource = 0, NotTrait = 1, MemberScope = 2, Producer = 3,
    MemberSource = 4, ParentJoin = 5, MemberKind = 6, Ambiguous = 7,
    ItemUnavailable = 8,
}
#[derive(Debug)]
pub struct PreparedTraitAssociatedTypes {
    key: TraitAssociatedTypesKey,
    declaration: Arc<QueryValue>,
    inventory: Arc<QueryValue>,
    scope: Option<ScopeKey>,
    members: Option<Arc<InputValue>>,
    blockers: Option<Arc<InputValue>>,
    generated: Option<Arc<InputValue>>,
    declarations: Vec<Arc<QueryValue>>,
    items: BTreeMap<Digest, Digest>,
    pending: Option<TraitAssociatedTypePending>,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedTraitAssociatedTypes {
    pub fn key(&self) -> &TraitAssociatedTypesKey { &self.key }
    pub fn pending(&self) -> Option<TraitAssociatedTypePending> { self.pending }
    /// Complete authenticated type-member identities only. A pending producer
    /// or source/parent join never certifies an empty member list.
    pub(crate) fn definitions(&self) -> Result<impl Iterator<Item = Digest> + '_, TraitAssociatedTypePending> {
        if let Some(why) = self.pending { return Err(why); }
        Ok(self.items.values().copied())
    }
    /// A missing key is an unmet source demand, never a negative solver result.
    pub fn lookup(&self, item: Digest) -> Result<Digest, TraitAssociatedTypePending> {
        if let Some(why) = self.pending { return Err(why); }
        self.items.get(&item).copied().ok_or(TraitAssociatedTypePending::ItemUnavailable)
    }
    pub(crate) fn prepare(db: &mut Database, key: &TraitAssociatedTypesKey) -> QueryPoll<Self> {
        let declaration = ready!(db.fetch(&SyntaxProvider,
            QueryKey::DeclarationGenerics(key.trait_definition))).value;
        let inventory = ready!(db.fetch(&SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let mut result = Self { key: key.clone(), declaration, inventory, scope: None,
            members: None, blockers: None, generated: None, declarations: Vec::new(),
            items: BTreeMap::new(), pending: None };
        macro_rules! pending { ($why:ident) => {{
            result.pending = Some(TraitAssociatedTypePending::$why);
            return QueryPoll::Ready(Ok(result));
        }}; }
        let QueryValue::DeclarationGenerics(declaration) = result.declaration.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait generic query differs")));
        };
        let Some((parse, header, context, parsed)) = declaration.source_parts() else { pending!(TraitSource); };
        let trait_node = match parsed.syntax_at(context.range, |node| ast::Trait::can_cast(node.kind())) {
            Ok(Some(node)) => node,
            Ok(None) => pending!(NotTrait),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let actual_trait = ready!(db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(key.trait_definition))).value;
        let QueryValue::TraitDeclaration(trait_) = actual_trait.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait declaration query differs")));
        };
        if !trait_.matches_owner(key.trait_definition, parse, header, &context.scope) {
            pending!(TraitSource);
        }
        let scope = match trait_.member_scope() {
            Ok(scope) => scope.clone(),
            Err(_) => pending!(ParentJoin),
        };
        let generated = db.input(&InputKey::GeneratedSyntax(parse.content));
        let QueryValue::PreparedScopeImplHeaders(inventory) = result.inventory.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait source inventory query differs")));
        };
        if inventory.key() != &key.inventory { return QueryPoll::Ready(Err(fault("trait source inventory differs"))); }
        let parent = match generated.as_deref() {
            Some(InputValue::GeneratedSyntax(unit)) => unit.declaration_for_member_scope(&scope),
            None => inventory.declaration_for_member_scope(&scope),
            _ => unreachable!("checked generated source"),
        };
        if parent != Some(key.trait_definition) { pending!(ParentJoin); }
        let members = db.input(&InputKey::ScopeMembers(scope.clone()));
        let blockers = db.input(&InputKey::ScopeBlockers(scope.clone()));
        let (Some(member_input), Some(blocker_input)) = (&members, &blockers) else { pending!(MemberScope); };
        let (InputValue::ScopeMembers(rows), InputValue::ScopeBlockers(blocker_rows)) =
            (member_input.as_ref(), blocker_input.as_ref()) else {
                return QueryPoll::Ready(Err(fault("trait member scope inputs differ")));
            };
        if !rows.blockers.is_empty() || !blocker_rows.is_empty() { pending!(Producer); }
        let mut items = BTreeMap::new();
        let mut declarations = vec![actual_trait];
        for ((namespace, name), candidates) in &rows.candidates {
            if *namespace != Namespace::Type { continue; }
            // Use the same actual name bucket consumed by ordinary projection
            // selection; do not turn an Outline name alone into a DefId.
            let bucket = ready!(db.fetch(&SyntaxProvider, QueryKey::ScopeBucket(crate::db::key::BucketKey {
                scope: scope.clone(), namespace: Namespace::Type, name: name.clone(),
            }))).value;
            let QueryValue::ScopeBucket(bucket) = bucket.as_ref() else {
                return QueryPoll::Ready(Err(fault("trait type bucket query differs")));
            };
            if !bucket.blockers.is_empty() { pending!(Producer); }
            if bucket.candidates != *candidates || candidates.len() != 1 { pending!(Ambiguous); }
            let identity = candidates[0];
            let member = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(identity))).value;
            let QueryValue::DeclarationGenerics(actual) = member.as_ref() else {
                return QueryPoll::Ready(Err(fault("associated generic query differs")));
            };
            let Some((member_parse, _, member_context, member_parsed)) = actual.source_parts() else { pending!(MemberSource); };
            if member_context.scope != scope { pending!(ParentJoin); }
            let alias = match member_parsed.syntax_at(member_context.range, |node| ast::TypeAlias::can_cast(node.kind())) {
                Ok(Some(node)) => ast::TypeAlias::cast(node).expect("selected type alias"),
                Ok(None) => pending!(MemberKind),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let Some(actual_name) = alias.name() else { pending!(MemberSource); };
            let edition = match member_parsed.token_edition(range(actual_name.syntax().text_range())) {
                Ok(edition) => edition, Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let actual_name = match super::names::owned_name(actual_name.text(), edition) {
                Ok(name) => name, Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if actual_name.as_str() != name { return QueryPoll::Ready(Err(fault("associated declaration name differs"))); }
            let syntax_parent = alias.syntax().ancestors().skip(1).find_map(ast::Item::cast);
            if member_parse == parse {
                if !matches!(syntax_parent, Some(ast::Item::Trait(ref parent)) if parent.syntax() == &trait_node) { pending!(ParentJoin); }
            } else {
                // Admitted fragments must carry their actual parent source
                // join. An item-fragment grammar alone supplies no authority.
                let member_generated = db.input(&InputKey::GeneratedSyntax(member_parse.content));
                let Some(InputValue::GeneratedSyntax(unit)) = member_generated.as_deref() else { pending!(ParentJoin); };
                if unit.key != *member_parse
                    || unit.declaration_for_member_scope(&member_context.scope) != Some(key.trait_definition) {
                    pending!(ParentJoin);
                }
            }
            let item = super::member_index::associated_name_key(name);
            if items.insert(item, identity).is_some() { pending!(Ambiguous); }
            declarations.push(member);
        }
        result.scope = Some(scope); result.members = members; result.blockers = blockers;
        result.generated = generated; result.declarations = declarations; result.items = items;
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-TRAIT-ASSOCIATED-TYPES", message.into()) }
impl StableEncode for PreparedTraitAssociatedTypes {
    const DOMAIN: &'static str = "lrr/v1/actual-trait-associated-types";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        let QueryValue::DeclarationGenerics(declaration) = self.declaration.as_ref() else { unreachable!("captured declaration") };
        e.record(declaration.as_ref());
        let QueryValue::PreparedScopeImplHeaders(inventory) = self.inventory.as_ref() else { unreachable!("captured inventory") };
        e.digest(inventory.fingerprint());
        e.option(&self.scope, |e, scope| e.record(scope));
        for input in [&self.members, &self.blockers, &self.generated] {
            e.option(input, |e, input| e.digest(input.fingerprint()));
        }
        e.sequence(&self.declarations, |e, declaration| match declaration.as_ref() {
            QueryValue::DeclarationGenerics(declaration) => { e.u32(0); e.record(declaration.as_ref()); }
            QueryValue::TraitDeclaration(declaration) => { e.u32(1); e.record(declaration.as_ref()); }
            _ => unreachable!("captured member source queries"),
        });
        e.map(&self.items, |e, name, definition| { e.digest(*name); e.digest(*definition); });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}
