//! Direct module-function identity before argument evaluation; no body entry.
use super::names::{NameLookup, NameNamespaces, NamePath};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, RetainedOriginalDeclaration, RetainedSourceLease},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind, Namespace},
    },
    syntax::{
        outline::{BodyHandle, DeclarationContext, ScopeKey},
        parse::ParseKey,
        signatures::FunctionSignature,
    },
};
#[path = "lexical_callables.rs"]
pub(crate) mod lexical;
pub(crate) use lexical::admit_lexical_function;
#[path = "ordinary_local_callables.rs"]
pub(crate) mod ordinary_local;
pub(crate) use ordinary_local::{EnteredLocalCall, prepare_entered_local_function};
#[path = "entered_module_callables.rs"]
pub(crate) mod entered_module;

#[derive(Debug, Clone)]
pub struct SourceCallable {
    definition: DefKey,
    identity: Digest,
    signature: Box<FunctionSignature>,
    body: Option<BodyHandle>,
    source: ParseKey,
    revision: u64,
    context: DeclarationContext,
    lexical: Option<Box<lexical::LexicalAdmission>>,
    ordinary_local: Option<std::sync::Arc<ordinary_local::OrdinaryLocalAdmission>>,
    ordinary_module: Option<std::sync::Arc<entered_module::EnteredModuleAdmission>>,
    // This actual additional handle makes an externally selected callable
    // visible to parent-plan retirement accounting when its proof Arc is also
    // retained by an entered-call memo. It never clones the immutable graph.
    ordinary_owner: Option<std::sync::Arc<super::source_locals::PreparedSourceLocalPlan>>,
    retained: Option<RetainedSourceLease>,
}
/// Source/body correspondence shared by held legacy receipts and the ordinary
/// entered Value receipt during migration. It does not expose the legacy
/// primitive profile or grant name/type/call execution authority.
pub(crate) enum LocalCallableSource<'a> {
    Legacy(&'a lexical::LexicalAdmission),
    Entered(&'a ordinary_local::OrdinaryLocalAdmission),
}
impl LocalCallableSource<'_> {
    pub(crate) fn caller(&self) -> &BodyHandle {
        match self {
            Self::Legacy(proof) => proof.plan().caller(),
            Self::Entered(proof) => proof.parent().context().source().body(),
        }
    }
    pub(crate) fn caller_header(&self) -> &crate::syntax::outline::HeaderRecipe {
        match self {
            Self::Legacy(proof) => proof.plan().caller_header(),
            Self::Entered(proof) => proof.parent().context().source().header(),
        }
    }
    pub(crate) fn caller_context(&self) -> &DeclarationContext {
        match self {
            Self::Legacy(proof) => proof.plan().caller_context(),
            Self::Entered(proof) => proof.parent().context().source().context(),
        }
    }
    pub(crate) fn source(&self) -> &ParseKey {
        match self {
            Self::Legacy(proof) => proof.plan().source(),
            Self::Entered(proof) => proof.source(),
        }
    }
    pub(crate) fn item(&self) -> &crate::syntax::outline::ItemOutline {
        match self {
            Self::Legacy(proof) => proof.plan().item(),
            Self::Entered(proof) => proof.item(),
        }
    }
    pub(crate) fn scope(&self) -> u32 {
        match self {
            Self::Legacy(proof) => proof.plan().scope(),
            Self::Entered(proof) => proof.scope(),
        }
    }
    pub(crate) fn site(&self) -> &crate::ir::pc::PcKey {
        match self {
            Self::Legacy(proof) => proof.plan().site(),
            Self::Entered(proof) => proof.site(),
        }
    }
}
impl SourceCallable {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn definition(&self) -> &DefKey {
        &self.definition
    }
    pub fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    pub fn body(&self) -> Option<&BodyHandle> {
        self.body.as_ref()
    }
    pub(crate) fn declaration_context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn lexical_contract(&self) -> Option<&lexical::LexicalAdmission> {
        self.lexical.as_deref()
    }
    pub(crate) fn ordinary_local_contract(
        &self,
    ) -> Option<&ordinary_local::OrdinaryLocalAdmission> {
        self.ordinary_local.as_deref()
    }
    pub(crate) fn ordinary_module_contract(
        &self,
    ) -> Option<&entered_module::EnteredModuleAdmission> {
        self.ordinary_module.as_deref()
    }
    pub(crate) fn local_source_contract(&self) -> Option<LocalCallableSource<'_>> {
        self.ordinary_local
            .as_deref()
            .map(LocalCallableSource::Entered)
            .or_else(|| self.lexical.as_deref().map(LocalCallableSource::Legacy))
    }
    pub(crate) fn retained_lexical_metadata_bytes(&self) -> usize {
        self.lexical
            .as_ref()
            .map_or(0, |proof| proof.metadata_bytes())
            .saturating_add(
                self.ordinary_local
                    .as_ref()
                    .map_or(0, |proof| proof.metadata_bytes()),
            )
            .saturating_add(
                self.ordinary_module
                    .as_ref()
                    .map_or(0, |proof| proof.metadata_bytes()),
            )
            .saturating_add(std::mem::size_of_val(&self.ordinary_module))
            .saturating_add(std::mem::size_of_val(&self.ordinary_owner))
            .saturating_add(std::mem::size_of::<Option<RetainedSourceLease>>())
    }
    pub fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        if let Some(proof) = &self.lexical {
            proof.revalidate(db)?;
        }
        if let Some(proof) = &self.ordinary_local {
            proof.revalidate(db)?;
        }
        if let Some(proof) = &self.ordinary_module {
            proof.revalidate(db)?;
        }
        if db.source_revision() != self.revision {
            return Err(fault("selected callable source revision changed"));
        }
        if let Some(lease) = &self.retained {
            match lease.require_current(db) {
                QueryPoll::Ready(result) => result?,
                QueryPoll::NeedInput(_) => {
                    return Err(fault(
                        "selected callable retained discovery is no longer available",
                    ));
                }
            }
        }
        let definition = self.identity;
        if !matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),Some(InputValue::DefinitionSource(key))if *key==self.source)
        {
            return Err(fault("selected callable source locator changed"));
        }
        if !matches!(db.input(&InputKey::DefinitionHeader(definition)).as_deref(),Some(InputValue::DefinitionHeader(header))if header==&self.signature.header)
        {
            return Err(fault("selected callable header changed"));
        }
        if !matches!(db.input(&InputKey::DeclarationContext(definition)).as_deref(),Some(InputValue::DeclarationContext(context))if context==&self.context)
        {
            return Err(fault("selected callable declaration context changed"));
        }
        let body = db.input(&InputKey::DefinitionBody(definition));
        match (self.body.as_ref(), body.as_deref()) {
            (None, None) => {}
            (Some(expected), Some(InputValue::DefinitionBody(current))) if current == expected => {}
            _ => return Err(fault("selected callable body handle changed")),
        }
        Ok(())
    }
}
/// Fixed-size, owned proof that a reached call selects an ordinary function.
/// Constructed only after exact caller/callee input revalidation.
#[derive(Debug, Clone)]
pub(crate) struct OrdinaryCallWitness {
    caller: crate::source::manifest::BodyKey,
    site: crate::ir::pc::NodeId,
    definition: Digest,
}
impl OrdinaryCallWitness {
    /// A call selecting a closed library facade function by its identity.
    pub(crate) fn facade(
        caller: crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
        definition: Digest,
    ) -> Self {
        Self {
            caller,
            site,
            definition,
        }
    }
    /// A trait-path call (`Trait::name(..)`) before its self argument
    /// selects the impl: the trait's declaration of the function.
    pub(crate) fn trait_declaration(
        caller: crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
        declaration: Digest,
    ) -> Self {
        Self {
            caller,
            site,
            definition: declaration,
        }
    }
    pub(crate) fn require(
        &self,
        caller: &crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
    ) -> Result<Digest, Diagnostic> {
        if &self.caller != caller || self.site != site {
            return Err(fault(
                "ordinary call scope witness belongs to another body/site",
            ));
        }
        Ok(self.definition)
    }
}
impl SourceCallable {
    pub(crate) fn ordinary_call_witness(
        &self,
        db: &mut Database,
        caller: crate::source::manifest::BodyKey,
        site: crate::ir::pc::NodeId,
    ) -> Result<OrdinaryCallWitness, Diagnostic> {
        self.revalidate(db)?;
        if self.definition.item_kind != ItemKind::Function {
            return Err(fault(
                "temporary-scope barrier requires a selected source function",
            ));
        }
        if !matches!(db.input(&InputKey::DefinitionBody(caller.definition)).as_deref(),
            Some(InputValue::DefinitionBody(body)) if body.key() == caller)
        {
            return Err(fault(
                "ordinary call scope witness requires the exact captured caller body",
            ));
        }
        Ok(OrdinaryCallWitness {
            caller,
            site,
            definition: self.identity,
        })
    }
}

/// Lowered call-path root for a generated `$crate` naming another crate's
/// root (the defining crate of the emitting macro).
pub(crate) fn dollar_crate_segment(unit: Digest) -> String {
    format!("$crate@{}", unit.hex())
}
/// The builtin self type a primitive type name denotes.
fn primitive_self(name: &str) -> Option<super::member_index::BuiltinSelf> {
    use crate::ir::op::PrimitiveType as P;
    Some(super::member_index::BuiltinSelf::Primitive(match name {
        "bool" => P::Bool,
        "char" => P::Char,
        "i8" => P::I8,
        "i16" => P::I16,
        "i32" => P::I32,
        "i64" => P::I64,
        "i128" => P::I128,
        "isize" => P::Isize,
        "u8" => P::U8,
        "u16" => P::U16,
        "u32" => P::U32,
        "u64" => P::U64,
        "u128" => P::U128,
        "usize" => P::Usize,
        "f32" => P::F32,
        "f64" => P::F64,
        "str" => return Some(super::member_index::BuiltinSelf::Str),
        _ => return None,
    }))
}
/// A written value path whose first segment is a qualified type anchor,
/// `<Type as Trait>::name` or `<Type>::name` (the value-path lowering's
/// spelling): the anchor type, its trait and the remaining segments.
pub(crate) fn split_qualified_anchor(spelled: &str) -> Option<(&str, Option<&str>, &str)> {
    let inner = spelled.strip_prefix('<')?;
    let close = inner.find('>')?;
    let (anchor, rest) = (&inner[..close], inner[close + 1..].strip_prefix("::")?);
    match anchor.split_once(" as ") {
        Some((ty, tr)) => Some((ty.trim(), Some(tr.trim()), rest)),
        None => Some((anchor.trim(), None, rest)),
    }
}
/// A written path spelling as import segments (`crate`/`self`/`super`
/// keywords, `Self` and checked names), and whether it is absolute.
fn written_segments(
    text: &str,
    edition: u16,
) -> Result<(bool, Vec<crate::syntax::imports::ImportSegment>), Diagnostic> {
    use crate::syntax::imports::ImportSegment;
    let absolute = text.starts_with("::");
    let text = if absolute { &text[2..] } else { text };
    let mut segments = Vec::new();
    for part in text.split("::") {
        segments.push(match part {
            "crate" => ImportSegment::Crate,
            "self" => ImportSegment::SelfModule,
            "super" => ImportSegment::Super,
            "Self" => ImportSegment::Name(part.into()),
            _ => {
                crate::expand::scopes::MacroName::parse(part, edition)?;
                ImportSegment::Name(part.into())
            }
        });
    }
    Ok((absolute, segments))
}
/// The ident spelling of a generated `$crate` root inside written Type
/// syntax (signature parameter and result types), which the source Type
/// parser must accept as a path segment.
pub(crate) fn dollar_crate_type_segment(unit: Digest) -> String {
    format!("{DOLLAR_CRATE_TYPE_PREFIX}{}", unit.hex())
}
const DOLLAR_CRATE_TYPE_PREFIX: &str = "__lrr_dollar_crate_root_";
/// The crate a `$crate` root spelling names (value or Type syntax).
pub(crate) fn dollar_crate_root(segment: &str) -> Option<Digest> {
    let hex = segment
        .strip_prefix("$crate@")
        .or_else(|| segment.strip_prefix(DOLLAR_CRATE_TYPE_PREFIX))?;
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, pair) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(pair).ok()?;
        bytes[i] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(Digest(bytes))
}

#[derive(Debug)]
pub enum CallSelection {
    Source(Box<SourceCallable>),
    /// A closed library facade function selected by its definition identity.
    Facade(crate::facade::functions::LibraryFunction, Digest),
    /// A unit/tuple struct or enum variant constructor, a Value-namespace
    /// member bound by `nominal_constructors` (unit values, tuple calls).
    Constructor(super::nominal_constructors::NominalConstructorTarget),
    NeedProducer(Digest),
    NeedOwnerContext(ScopeKey),
    NeedValue(DefKey),
    Unresolved(NameLookup),
    /// A type-relative path whose function the call's reached self type
    /// selects (`run_trait_calls.rs`): `Trait::name` for a function the trait
    /// declares, or a path through an enclosing generic parameter (`T::name`,
    /// `<T as Trait>::name`, a trait body's `Self::name`).
    TypeRelative,
}
pub struct CallResolver<'a> {
    graph: &'a NameNamespaces,
    sources: &'a [&'a PreparedSourceView],
    members: Option<&'a super::member_index::MemberIndex>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
impl<'a> CallResolver<'a> {
    pub fn new(
        graph: &'a NameNamespaces,
        sources: &'a [&'a PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        for source in sources {
            graph.require_view(source)?;
        }
        Ok(Self {
            graph,
            sources,
            members: None,
        })
    }
    /// Impl members for type-relative associated paths (`Type::function`).
    pub(crate) fn with_members(
        mut self,
        members: Option<&'a super::member_index::MemberIndex>,
    ) -> Self {
        self.members = members;
        self
    }
    /// `Type::name` where `Type` names a nominal declaration (or `Self` inside
    /// one of its impls): an inherent associated function first, then a
    /// unique trait-impl function, as rustc's associated item lookup.
    fn associated_function(
        &self,
        db: &mut Database,
        caller: Digest,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> QueryPoll<Option<CallSelection>> {
        use crate::syntax::imports::ImportSegment;
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some((ImportSegment::Name(name), prefix)) = segments.split_last() else {
            return QueryPoll::Ready(Ok(None));
        };
        if prefix.is_empty() {
            return QueryPoll::Ready(Ok(None));
        }
        // `Self`, a nominal declaration, or an unshadowed primitive type
        // name (`i64::from`, `u8::max_value`).
        let self_type = match self.resolve_written_type(
            members, caller, scope, edition, prefix, absolute, true,
        ) {
            Ok(self_type) => self_type,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(self_type) = self_type else {
            return QueryPoll::Ready(Ok(None));
        };
        self.associated_of(db, members, self_type, name)
    }
    /// `Type::function` where the written type has no inherent `function`
    /// and several impls of one trait provide it, differing only by the
    /// trait's arguments (`u64::from` through `From<u8>`, `From<u32>`, ..):
    /// the trait, its declaration of `function`, and the impls' functions.
    /// The evaluated call arguments select among them. `None` for any other
    /// path (modules, traits, inherent functions, a single candidate).
    pub(crate) fn associated_alternatives(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
    ) -> QueryPoll<Option<(Digest, Digest, Vec<Digest>, Option<crate::traits::term::TyTerm>)>> {
        use crate::syntax::imports::ImportSegment;
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        if spelled.starts_with("::") || spelled.contains('<') {
            return QueryPoll::Ready(Ok(None));
        }
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut segments = Vec::new();
        for part in spelled.split("::") {
            segments.push(match part {
                "crate" => ImportSegment::Crate,
                "self" => ImportSegment::SelfModule,
                "super" => ImportSegment::Super,
                _ => ImportSegment::Name(part.into()),
            });
        }
        let Some((ImportSegment::Name(name), prefix)) = segments.split_last() else {
            return QueryPoll::Ready(Ok(None));
        };
        if prefix.is_empty() {
            return QueryPoll::Ready(Ok(None));
        }
        let self_type =
            match self.resolve_written_type(members, caller, &scope, edition, prefix, false, true) {
                Ok(Some(self_type)) => self_type,
                Ok(None) => return QueryPoll::Ready(Ok(None)),
                Err(_) => return QueryPoll::Ready(Ok(None)),
            };
        if let Ok(nominal) = self_type
            && members.is_trait(nominal)
        {
            return QueryPoll::Ready(Ok(None));
        }
        let inherent = match self_type {
            Ok(nominal) => members.inherent(nominal, name),
            Err(builtin) => members.builtin_inherent(builtin, name),
        };
        if !inherent.is_empty() {
            return QueryPoll::Ready(Ok(None));
        }
        // A primitive Self is a closed term; other Self types stay open.
        let self_term = match self_type {
            Err(super::member_index::BuiltinSelf::Primitive(primitive)) => {
                Some(crate::traits::term::TyTerm::Primitive(primitive))
            }
            _ => None,
        };
        let mut functions = Vec::new();
        let direct = members.trait_candidates(self_type, name);
        // The direct impls, plus the applicable blanket impls when the written
        // Self is a closed term to hold their result to: the arguments may
        // fit only the blanket one (`u64::from(9u64)`).
        let blanket = if self_term.is_some() {
            members.blanket_candidates(self_type, name)
        } else {
            Vec::new()
        };
        for function in direct.iter().copied().chain(blanket) {
            if !functions.contains(&function) {
                functions.push(function);
            }
        }
        if functions.len() < 2 {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(trait_key) = members.same_trait_of(&functions) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(declaration) = members.trait_declaration(trait_key, name) else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some((trait_key, declaration, functions, self_term))))
    }
    /// The associated function `name` of a resolved self type or trait.
    fn associated_of(
        &self,
        db: &mut Database,
        members: &super::member_index::MemberIndex,
        self_type: Result<Digest, super::member_index::BuiltinSelf>,
        name: &str,
    ) -> QueryPoll<Option<CallSelection>> {
        if let Ok(nominal) = self_type
            && members.is_trait(nominal)
        {
            // A trait path (`Trait::name`) selects by its reached self type.
            return QueryPoll::Ready(Ok(members
                .trait_declaration(nominal, name)
                .map(|_| CallSelection::TypeRelative)));
        }
        // Inherent functions first, then the type's trait impls, else
        // applicable blanket impls (`i64::from` through `impl<T> From<T> for T`).
        let inherent = match self_type {
            Ok(nominal) => members.inherent(nominal, name),
            Err(builtin) => members.builtin_inherent(builtin, name),
        };
        let traits = if inherent.is_empty() {
            members.trait_candidates(self_type, name)
        } else {
            Vec::new()
        };
        let function = match inherent.as_slice() {
            [function] => *function,
            [] => match traits.as_slice() {
                [function] => *function,
                [] => return QueryPoll::Ready(Ok(None)),
                // Impls of one trait that differ only by the trait's
                // arguments (`From<u8>`, `From<u32>` for `u64`): the call's
                // evaluated arguments select the impl at run time.
                candidates
                    if members.same_trait_of(candidates).is_some_and(|trait_key| {
                        members.trait_declaration(trait_key, name).is_some()
                    }) =>
                {
                    return QueryPoll::Ready(Ok(Some(CallSelection::TypeRelative)));
                }
                _ => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-NAME-AMBIGUOUS",
                        format!("associated function `{name}` is provided by several trait impls"),
                    )));
                }
            },
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    format!("associated function `{name}` has several inherent definitions"),
                )));
            }
        };
        let Some(definition) = self.member_definition(db, function) else {
            return QueryPoll::Ready(Ok(None));
        };
        match self.select(db, NameLookup::Defined(definition)) {
            QueryPoll::Ready(Ok(selection)) => QueryPoll::Ready(Ok(Some(selection))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// The Value-namespace definition of an associated function the member
    /// index selected: its defining unit and edition from its original
    /// declaration, or from a macro-generated one's published inputs.
    fn member_definition(
        &self,
        db: &mut Database,
        function: Digest,
    ) -> Option<super::names::NameDefinition> {
        let (defining_unit, definition_edition) = match self.item(function) {
            Some((_, item)) => (item.def_key.crate_key, item.header.profile.edition),
            None => {
                let header = db.input(&InputKey::DefinitionHeader(function));
                let context = db.input(&InputKey::DeclarationContext(function));
                match (header.as_deref(), context.as_deref()) {
                    (
                        Some(InputValue::DefinitionHeader(header)),
                        Some(InputValue::DeclarationContext(context)),
                    ) => (context.scope.crate_key, header.profile.edition),
                    _ => return None,
                }
            }
        };
        Some(super::names::NameDefinition {
            namespace: Namespace::Value,
            identity: function,
            defining_unit,
            definition_edition,
        })
    }
    /// The associated constant a written value path names (`Type::NAME`,
    /// `Self::NAME`, `<Type as Trait>::NAME`, `<Type>::NAME`): the type's
    /// inherent constant, else a trait impl's own, else the trait's provided
    /// default (`MemberIndex::associated_const`). `None` when the path names
    /// no associated constant; the type prefix resolves in the caller's
    /// module Type namespace, `Self` as the caller's impl self type.
    pub(crate) fn associated_value(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
    ) -> QueryPoll<Option<Digest>> {
        use crate::syntax::imports::ImportSegment;
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        let segments_of = |text: &str| written_segments(text, edition);
        let resolve_type = |segments: &[ImportSegment],
                            absolute: bool,
                            anchored: bool|
         -> Result<Option<Result<Digest, super::member_index::BuiltinSelf>>, Diagnostic> {
            self.resolve_written_type(members, caller, &scope, edition, segments, absolute, anchored)
        };
        let result = (|| -> Result<Option<Digest>, Diagnostic> {
            let (ty, trait_key, name) = if let Some((ty, tr, rest)) = split_qualified_anchor(spelled)
            {
                if rest.contains("::") {
                    return Ok(None);
                }
                let (absolute, segments) = segments_of(ty)?;
                let Some(ty) = resolve_type(&segments, absolute, true)? else {
                    return Ok(None);
                };
                let trait_key = match tr {
                    Some(tr) => {
                        let (absolute, segments) = segments_of(tr)?;
                        match resolve_type(&segments, absolute, false)? {
                            Some(Ok(definition)) => Some(definition),
                            _ => return Ok(None),
                        }
                    }
                    None => None,
                };
                (ty, trait_key, rest)
            } else {
                let Some((prefix, name)) = spelled.rsplit_once("::") else {
                    return Ok(None);
                };
                if prefix.is_empty() || prefix.starts_with("$crate@") {
                    return Ok(None);
                }
                let (absolute, segments) = segments_of(prefix)?;
                // An unshadowed primitive name is a type prefix too
                // (`u8::MAX`, `f64::EPSILON`), as for associated functions.
                let Some(ty) = resolve_type(&segments, absolute, true)? else {
                    return Ok(None);
                };
                (ty, None, name)
            };
            members.associated_const(ty, trait_key, name)
        })();
        QueryPoll::Ready(result)
    }
    /// The member-index self type a written type path names from the
    /// caller's module scope: `Self` as the caller's impl self type, else a
    /// nominal declaration of the Type namespace, else (for an `anchored`
    /// `<T as Trait>` type) an unshadowed primitive name.
    #[allow(clippy::too_many_arguments)]
    fn resolve_written_type(
        &self,
        members: &super::member_index::MemberIndex,
        caller: Digest,
        scope: &ScopeKey,
        edition: u16,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        anchored: bool,
    ) -> Result<Option<Result<Digest, super::member_index::BuiltinSelf>>, Diagnostic> {
        use crate::syntax::imports::ImportSegment;
        if !absolute && matches!(segments, [ImportSegment::Name(n)] if n == "Self") {
            return Ok(members.owner_of(caller).and_then(|owner| {
                owner
                    .self_nominal
                    .map(Ok)
                    .or_else(|| owner.self_builtin.map(Err))
            }));
        }
        Ok(
            match self
                .graph
                .lookup_source_path(Namespace::Type, segments, absolute, scope, edition)?
            {
                NameLookup::Defined(definition) => Some(Ok(definition.identity)),
                NameLookup::Absent if anchored && !absolute => match segments {
                    [ImportSegment::Name(name)] => primitive_self(name).map(Err),
                    _ => None,
                },
                _ => None,
            },
        )
    }
    /// A written type spelling (`Self`, a nominal path or a primitive name)
    /// as the member-index self type it names in the caller's module scope.
    pub(crate) fn written_self_type(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
    ) -> QueryPoll<Option<Result<Digest, super::member_index::BuiltinSelf>>> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        // A generated `$crate` root names the defining crate directly.
        if spelled.starts_with("$crate@") {
            return QueryPoll::Ready(self.lookup_written_type(&scope, edition, spelled).map(
                |found| match found {
                    NameLookup::Defined(definition) => Some(Ok(definition.identity)),
                    _ => None,
                },
            ));
        }
        QueryPoll::Ready(written_segments(spelled, edition).and_then(|(absolute, segments)| {
            self.resolve_written_type(members, caller, &scope, edition, &segments, absolute, true)
        }))
    }
    /// Type-namespace lookup of a written path spelling from `scope`,
    /// including a generated `$crate@<unit>` root.
    fn lookup_written_type(
        &self,
        scope: &ScopeKey,
        edition: u16,
        spelled: &str,
    ) -> Result<NameLookup, Diagnostic> {
        if let Some((root, tail)) = spelled.split_once("::")
            && let Some(unit) = dollar_crate_root(root)
        {
            let tail = tail.split("::").collect::<Vec<_>>();
            let path = NameNamespaces::path(unit, &tail, edition)?;
            return self.graph.lookup(Namespace::Type, &path, scope);
        }
        let (absolute, segments) = written_segments(spelled, edition)?;
        self.graph
            .lookup_source_path(Namespace::Type, &segments, absolute, scope, edition)
    }
    /// The trait definition a written path names in the caller's module
    /// scope; `None` when it names no trait.
    pub(crate) fn written_trait(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
    ) -> QueryPoll<Option<Digest>> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(None));
        };
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        // `Self` names the enclosing impl or trait's self type, never a trait.
        if scope.owner.is_some() || spelled == "Self" || spelled.starts_with("Self::") {
            return QueryPoll::Ready(Ok(None));
        }
        // A generated `$crate` root names its defining crate, as in module
        // value lookup.
        if let Some((root, tail)) = spelled.split_once("::")
            && let Some(unit) = dollar_crate_root(root)
        {
            let tail = tail.split("::").collect::<Vec<_>>();
            return QueryPoll::Ready(
                NameNamespaces::path(unit, &tail, edition)
                    .and_then(|path| self.graph.lookup(Namespace::Type, &path, &scope))
                    .map(|found| match found {
                        NameLookup::Defined(definition) if members.is_trait(definition.identity) => {
                            Some(definition.identity)
                        }
                        _ => None,
                    }),
            );
        }
        QueryPoll::Ready(self.lookup_written_type(&scope, edition, spelled).map(
            |found| match found {
                NameLookup::Defined(definition) if members.is_trait(definition.identity) => {
                    Some(definition.identity)
                }
                _ => None,
            },
        ))
    }
    /// The source callable of a function definition the member index
    /// selected (an impl's function or a trait's provided body).
    pub(crate) fn select_member_function(
        &self,
        db: &mut Database,
        function: Digest,
    ) -> QueryPoll<Box<SourceCallable>> {
        let Some(definition) = self.member_definition(db, function) else {
            return QueryPoll::Ready(Err(fault(
                "selected member function has no published declaration",
            )));
        };
        match self.select(db, NameLookup::Defined(definition)) {
            QueryPoll::Ready(Ok(CallSelection::Source(callee))) => QueryPoll::Ready(Ok(callee)),
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(fault(
                "selected member function requires its owner context",
            ))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// Module lookup only. Executed local ribs/callee values route before this API.
    pub fn module_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        let found =
            match self
                .graph
                .lookup_unqualified(Namespace::Value, name, &scope, edition, None)
            {
                Ok(found) => found,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
        self.select(db, found)
    }
    /// Owned source spelling from the entered call recipe; no AST or runtime effects.
    pub fn source_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        if !name.contains("::") {
            return self.module_function(db, caller, name);
        }
        // A path through an enclosing generic parameter is selected from the
        // parameter's reached binding at the call, never from a same-named
        // module type.
        match self.generic_owner(db, caller, name) {
            QueryPoll::Ready(Ok(true)) => return QueryPoll::Ready(Ok(CallSelection::TypeRelative)),
            QueryPoll::Ready(Ok(false)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        // A qualified type anchor names an associated item, never a module path.
        if name.starts_with('<') {
            return match self.associated_value(db, caller, name) {
                QueryPoll::Ready(Ok(Some(constant))) => self.associated_value_selection(constant),
                QueryPoll::Ready(Ok(None)) => self.anchored_function(db, caller, name),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        }
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        if let Some((root, tail)) = name.split_once("::")
            && let Some(unit) = dollar_crate_root(root)
        {
            let tail = tail.split("::").collect::<Vec<_>>();
            let path = match NameNamespaces::path(unit, &tail, edition) {
                Ok(path) => path,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            return match self.graph.lookup(Namespace::Value, &path, &scope) {
                // `$crate::path::Type::name`: an associated function of the
                // type or trait its prefix names, as ordinary paths select.
                Ok(NameLookup::Absent) if tail.len() > 1 => {
                    let Some(members) = self.members else {
                        return self.select(db, NameLookup::Absent);
                    };
                    let (owner, function) = tail.split_at(tail.len() - 1);
                    let owner = match NameNamespaces::path(unit, owner, edition)
                        .and_then(|path| self.graph.lookup(Namespace::Type, &path, &scope))
                    {
                        Ok(NameLookup::Defined(definition)) => definition.identity,
                        Ok(_) => return self.select(db, NameLookup::Absent),
                        Err(e) => return QueryPoll::Ready(Err(e)),
                    };
                    match self.associated_of(db, members, Ok(owner), function[0]) {
                        QueryPoll::Ready(Ok(Some(selection))) => QueryPoll::Ready(Ok(selection)),
                        QueryPoll::Ready(Ok(None)) => self.select(db, NameLookup::Absent),
                        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                    }
                }
                Ok(found) => self.select(db, found),
                Err(e) => QueryPoll::Ready(Err(e)),
            };
        }
        let absolute = name.starts_with("::");
        let text = if absolute { &name[2..] } else { name };
        let mut segments = Vec::new();
        for part in text.split("::") {
            use crate::syntax::imports::ImportSegment;
            let segment = match part {
                "crate" => ImportSegment::Crate,
                "self" => ImportSegment::SelfModule,
                "super" => ImportSegment::Super,
                // `Self::item` in an impl; resolved by associated lookup.
                "Self" => ImportSegment::Name(part.into()),
                _ => {
                    if let Err(e) = crate::expand::scopes::MacroName::parse(part, edition) {
                        return QueryPoll::Ready(Err(e));
                    }
                    ImportSegment::Name(part.into())
                }
            };
            segments.push(segment);
        }
        // `Self::item` names an associated item of the caller's impl.
        if !absolute
            && matches!(segments.first(), Some(crate::syntax::imports::ImportSegment::Name(n)) if n == "Self")
        {
            return match self.associated_function(db, caller, &segments, absolute, &scope, edition) {
                QueryPoll::Ready(Ok(Some(selection))) => QueryPoll::Ready(Ok(selection)),
                QueryPoll::Ready(Ok(None)) => self.associated_value_or_absent(db, caller, name),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        }
        match self
            .graph
            .lookup_source_path(Namespace::Value, &segments, absolute, &scope, edition)
        {
            Ok(NameLookup::Absent) => {
                match self.associated_function(db, caller, &segments, absolute, &scope, edition) {
                    QueryPoll::Ready(Ok(Some(selection))) => QueryPoll::Ready(Ok(selection)),
                    QueryPoll::Ready(Ok(None)) => self.associated_value_or_absent(db, caller, name),
                    QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                }
            }
            Ok(found) => self.select(db, found),
            Err(e) => QueryPoll::Ready(Err(e)),
        }
    }
    /// Whether `name`'s owner (`T` of `T::f` or `<T as Tr>::f`) is a Type
    /// parameter the caller declares (a trait body's implicit `Self`
    /// included); generic parameters shadow module types.
    fn generic_owner(&self, db: &mut Database, caller: Digest, name: &str) -> QueryPoll<bool> {
        let owner = match split_qualified_anchor(name) {
            Some((ty, _, _)) => ty,
            None => match name.split_once("::") {
                Some((owner, rest)) if !rest.contains("::") => owner,
                _ => return QueryPoll::Ready(Ok(false)),
            },
        };
        if owner.is_empty() || owner.contains("::") || owner.starts_with('$') {
            return QueryPoll::Ready(Ok(false));
        }
        if self.item(caller).is_none_or(|(_, item)| item.def_key.item_kind != ItemKind::Function) {
            return QueryPoll::Ready(Ok(false));
        }
        match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(caller)) {
            QueryPoll::Ready(Ok(value)) => QueryPoll::Ready(Ok(matches!(
                value.value.as_ref(),
                QueryValue::FunctionSignature(signature)
                    if signature.generics.iter().any(|parameter| {
                        parameter.kind == crate::syntax::signatures::GenericKind::Type
                            && parameter.name == owner
                    })
            ))),
            QueryPoll::Ready(Err(_)) => QueryPoll::Ready(Ok(false)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// `<Type as Trait>::name` / `<Type>::name` naming no associated constant:
    /// the function the member index selects for that type (the named
    /// trait's impl, else inherent then trait impls), as the dispatcher's
    /// trait-path calls select it.
    fn anchored_function(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        let Some(members) = self.members else {
            return self.select(db, NameLookup::Absent);
        };
        let Some((ty, tr, function)) = split_qualified_anchor(name) else {
            return self.select(db, NameLookup::Absent);
        };
        if function.contains("::") {
            return self.select(db, NameLookup::Absent);
        }
        let self_type = match self.written_self_type(db, caller, ty) {
            QueryPoll::Ready(Ok(self_type)) => self_type,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let trait_key = match tr {
            Some(tr) => match self.written_trait(db, caller, tr) {
                QueryPoll::Ready(Ok(Some(trait_key))) => Some(trait_key),
                QueryPoll::Ready(Ok(None)) => return self.select(db, NameLookup::Absent),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
            None => None,
        };
        let Some(self_type) = self_type else {
            return self.select(db, NameLookup::Absent);
        };
        let candidates = match trait_key {
            Some(trait_key) => members
                .trait_method(self_type, trait_key, function)
                .into_iter()
                .map(|(_, function)| function)
                .collect::<Vec<_>>(),
            None => {
                let inherent = match self_type {
                    Ok(nominal) => members.inherent(nominal, function),
                    Err(builtin) => members.builtin_inherent(builtin, function),
                };
                if inherent.is_empty() {
                    members.trait_candidates(self_type, function)
                } else {
                    inherent
                }
            }
        };
        match candidates.as_slice() {
            [selected] => match self.member_definition(db, *selected) {
                Some(definition) => self.select(db, NameLookup::Defined(definition)),
                None => self.select(db, NameLookup::Absent),
            },
            [] => self.select(db, NameLookup::Absent),
            _ => QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                format!("several impls provide `{function}` for `{name}`"),
            ))),
        }
    }
    /// A type-relative path naming no associated function: an associated
    /// constant is a Value item that is not a function (`NeedValue`).
    fn associated_value_or_absent(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
    ) -> QueryPoll<CallSelection> {
        match self.associated_value(db, caller, name) {
            QueryPoll::Ready(Ok(Some(constant))) => self.associated_value_selection(constant),
            QueryPoll::Ready(Ok(None)) => self.select(db, NameLookup::Absent),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn associated_value_selection(&self, constant: Digest) -> QueryPoll<CallSelection> {
        match self.item(constant) {
            Some((_, item)) => QueryPoll::Ready(Ok(CallSelection::NeedValue(item.def_key.clone()))),
            None => QueryPoll::Ready(Err(fault(
                "selected associated constant is outside the source catalogue",
            ))),
        }
    }
    /// Raw ordinary lookup of a written path spelling (the encoding call
    /// recipes, value paths and pattern paths share) in `namespace` from the
    /// caller's module scope. Pattern paths use it to reach Type-namespace
    /// record constructors and Value-namespace constants; it selects nothing.
    pub(crate) fn lookup_written(
        &self,
        db: &mut Database,
        caller: Digest,
        name: &str,
        namespace: Namespace,
    ) -> QueryPoll<NameLookup> {
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let module = ScopeKey {
            crate_key: scope.crate_key,
            module_path: scope.module_path.clone(),
            owner: None,
        };
        if !name.contains("::") {
            return QueryPoll::Ready(self.graph.lookup_unqualified(
                namespace, name, &module, edition, None,
            ));
        }
        // A qualified type anchor or `Self` names no module member.
        if name.starts_with('<') || name.starts_with("Self::") {
            return QueryPoll::Ready(Ok(NameLookup::Absent));
        }
        if let Some((root, tail)) = name.split_once("::")
            && let Some(unit) = dollar_crate_root(root)
        {
            let tail = tail.split("::").collect::<Vec<_>>();
            return QueryPoll::Ready(
                NameNamespaces::path(unit, &tail, edition)
                    .and_then(|path| self.graph.lookup(namespace, &path, &module)),
            );
        }
        let absolute = name.starts_with("::");
        let text = if absolute { &name[2..] } else { name };
        let mut segments = Vec::new();
        for part in text.split("::") {
            use crate::syntax::imports::ImportSegment;
            segments.push(match part {
                "crate" => ImportSegment::Crate,
                "self" => ImportSegment::SelfModule,
                "super" => ImportSegment::Super,
                _ => {
                    if let Err(e) = crate::expand::scopes::MacroName::parse(part, edition) {
                        return QueryPoll::Ready(Err(e));
                    }
                    ImportSegment::Name(part.into())
                }
            });
        }
        QueryPoll::Ready(
            self.graph
                .lookup_source_path(namespace, &segments, absolute, &module, edition),
        )
    }
    pub fn qualified(
        &self,
        db: &mut Database,
        caller: Digest,
        path: &NamePath,
    ) -> QueryPoll<CallSelection> {
        let (scope, _) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if scope.owner.is_some() {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(scope)));
        }
        let found = match self.graph.lookup(Namespace::Value, path, &scope) {
            Ok(found) => found,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        self.select(db, found)
    }
    fn caller_context(&self, db: &mut Database, caller: Digest) -> QueryPoll<(ScopeKey, u16)> {
        if let Some((source, item)) = self.item(caller) {
            // An impl introduces no item namespace: names inside its functions
            // resolve in the enclosing module.
            // Neither impls nor traits introduce an item namespace.
            let impl_owned = item.scope.owner.is_some();
            let module = |mut scope: ScopeKey| {
                if impl_owned {
                    scope.owner = None;
                }
                scope
            };
            match self.current(db, source, caller) {
                QueryPoll::Ready(Ok(Some(current))) => {
                    return QueryPoll::Ready(Ok((
                        module(current.context().scope.clone()),
                        current.header().profile.edition,
                    )));
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            }
            return QueryPoll::Ready(Ok((module(item.scope.clone()), item.header.profile.edition)));
        }
        // A body-local function: a module-graph path from its body resolves in
        // its enclosing module (a lexical item is selected by the entered body
        // context before this route), at the local declaration's own edition.
        if db
            .input(&InputKey::LocalDeclarationOwner(caller))
            .is_some()
        {
            let (Some(InputValue::DeclarationContext(context)), Some(InputValue::DefinitionHeader(header))) = (
                db.input(&InputKey::DeclarationContext(caller)).as_deref().cloned(),
                db.input(&InputKey::DefinitionHeader(caller)).as_deref().cloned(),
            ) else {
                return QueryPoll::Ready(Err(fault("local caller declaration context absent")));
            };
            let mut scope = context.scope;
            scope.owner = None;
            return QueryPoll::Ready(Ok((scope, header.profile.edition)));
        }
        let input = match db.input(&InputKey::DefinitionSource(caller)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated caller source absent"))),
        };
        let InputValue::DefinitionSource(parse) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated caller source variant differs")));
        };
        let Some(owner) = db.generated_declaration_owner(parse, caller) else {
            return QueryPoll::Ready(Err(fault("generated caller syntax absent")));
        };
        let definition = &owner.def_key;
        let context = match db.input(&InputKey::DeclarationContext(caller)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated caller context absent"))),
        };
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated caller context variant differs")));
        };
        if context.scope.crate_key != owner.boundary.crate_key
            || context.scope.crate_key != definition.crate_key
            || context.scope.module_path != definition.module_path
            || !context
                .scope
                .module_path
                .starts_with(&owner.boundary.module_path)
        {
            return QueryPoll::Ready(Err(fault("generated caller owner differs")));
        }
        // As for original callers: neither an impl nor a trait introduces an
        // item namespace, so an owned generated function resolves names in
        // its enclosing module.
        let mut scope = context.scope.clone();
        scope.owner = None;
        match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok((scope, owner.edition))),
            QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
        }
    }
    fn select_generated(&self, db: &mut Database, identity: Digest) -> QueryPoll<CallSelection> {
        match self.generated_callable(db, identity, &[ItemKind::Function]) {
            QueryPoll::Ready(Ok(Ok(callee))) => {
                QueryPoll::Ready(Ok(CallSelection::Source(Box::new(callee))))
            }
            QueryPoll::Ready(Ok(Err(definition))) => {
                QueryPoll::Ready(Ok(CallSelection::NeedValue(definition)))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// A macro-generated declaration of one of `kinds` as an ordinary source
    /// callable from its published inputs; `Err(definition)` for another
    /// kind of declaration.
    fn generated_callable(
        &self,
        db: &mut Database,
        identity: Digest,
        kinds: &[ItemKind],
    ) -> QueryPoll<Result<SourceCallable, DefKey>> {
        let source = match db.input(&InputKey::DefinitionSource(identity)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("selected generated source absent"))),
        };
        let InputValue::DefinitionSource(parse) = source.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected generated source variant differs")));
        };
        let Some(owner) = db.generated_declaration_owner(parse, identity) else {
            return QueryPoll::Ready(Err(fault("selected generated syntax absent")));
        };
        let definition = &owner.def_key;
        if !kinds.contains(&definition.item_kind) {
            return QueryPoll::Ready(Ok(Err(definition.clone())));
        }
        let outcome = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(identity)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated signature variant differs")));
        };
        let context = match db.input(&InputKey::DeclarationContext(identity)) {
            Some(input) => input,
            None => return QueryPoll::Ready(Err(fault("generated declaration context absent"))),
        };
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated declaration context variant differs")));
        };
        // An impl/trait-owned generated function (a method emitted by a macro
        // or derive) keeps its owner; the unit admitted every owner when it
        // was prepared (GeneratedSyntaxInput::prepare).
        if signature.scope != context.scope
            || context.scope.crate_key != owner.boundary.crate_key
            || context.scope.crate_key != definition.crate_key
            || context.scope.module_path != definition.module_path
            || !context
                .scope
                .module_path
                .starts_with(&owner.boundary.module_path)
        {
            return QueryPoll::Ready(Err(fault("generated callable owner differs")));
        }
        let body = match db.input(&InputKey::DefinitionBody(identity)).as_deref() {
            Some(InputValue::DefinitionBody(body)) => Some(body.clone()),
            None => None,
            _ => return QueryPoll::Ready(Err(fault("generated body variant differs"))),
        };
        // A retained admission of this generated function (its body's macro
        // calls attached) is projected exactly like a retained original.
        let retained = match db.retained_generated_declaration(identity) {
            QueryPoll::Ready(Ok(retained)) => retained,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        if retained.as_ref().is_some_and(|current| {
            current.parse() != &*parse
                || Some(current.body()) != body.as_ref()
                || current.context() != &*context
        }) {
            return QueryPoll::Ready(Err(fault("generated retained callable rows differ")));
        }
        let callee = SourceCallable {
            identity,
            definition: definition.clone(),
            signature: signature.clone(),
            body,
            source: parse.clone(),
            revision: db.source_revision(),
            context: context.clone(),
            lexical: None,
            ordinary_local: None,
            ordinary_module: None,
            ordinary_owner: None,
            retained: retained.map(RetainedOriginalDeclaration::into_lease),
        };
        if let Err(e) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(e));
        }
        QueryPoll::Ready(Ok(Ok(callee)))
    }
    fn item(
        &self,
        definition: Digest,
    ) -> Option<(&PreparedSourceView, &crate::syntax::outline::ItemOutline)> {
        self.sources.iter().find_map(|source| {
            source
                .outline_item(definition)
                .map(|item| (*source, item))
        })
    }
    fn current(
        &self,
        db: &mut Database,
        source: &PreparedSourceView,
        definition: Digest,
    ) -> QueryPoll<Option<RetainedOriginalDeclaration>> {
        db.retained_original_declaration(source, definition)
    }
    /// A reached `const` or `static` item's initializer as an ordinary
    /// zero-parameter callable whose result is the declared type (E-R5-007):
    /// the interpreter runs the item's own initializer body through the
    /// common call route. `None` when the item has no source initializer.
    pub(crate) fn initializer(
        &self,
        db: &mut Database,
        identity: Digest,
    ) -> QueryPoll<Option<SourceCallable>> {
        let Some((source, item)) = self.item(identity) else {
            // A body-local item publishes its header only; its initializer
            // needs the local declaration body route (not yet available).
            if db
                .input(&InputKey::LocalDeclarationOwner(identity))
                .is_some()
            {
                return QueryPoll::Ready(Ok(None));
            }
            // A macro-emitted item's initializer joins the same route through
            // its published generated inputs.
            return match self.generated_callable(
                db,
                identity,
                &[ItemKind::Constant, ItemKind::Static],
            ) {
                QueryPoll::Ready(Ok(Ok(callee))) => QueryPoll::Ready(Ok(Some(callee))),
                QueryPoll::Ready(Ok(Err(_))) => QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        };
        if !matches!(item.def_key.item_kind, ItemKind::Constant | ItemKind::Static) {
            return QueryPoll::Ready(Ok(None));
        }
        let retained = match self.current(db, source, identity) {
            QueryPoll::Ready(Ok(current)) => current,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let header = retained
            .as_ref()
            .map_or(&item.header, |current| current.header())
            .clone();
        let context = retained.as_ref().map_or_else(
            || DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            |current| current.context().clone(),
        );
        let Some(body) = retained
            .as_ref()
            .map_or_else(|| item.body.clone(), |current| Some(current.body().clone()))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let parse = retained.as_ref().map_or_else(
            || source.selection().outline.parse.clone(),
            |current| current.parse().clone(),
        );
        // The same initializer signature the signature query serves to
        // every consumer of this definition (`ParsedFile::initializer_signature`).
        let outcome = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(identity)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("initializer signature query result mismatch")));
        };
        if signature.scope != context.scope || signature.header.fingerprint() != header.fingerprint()
        {
            return QueryPoll::Ready(Err(fault("initializer signature source declaration mismatch")));
        }
        let mut signature = signature.clone();
        signature.header = header;
        let callee = SourceCallable {
            definition: item.def_key.clone(),
            identity,
            signature,
            body: Some(body),
            source: parse,
            revision: db.source_revision(),
            context,
            lexical: None,
            ordinary_local: None,
            ordinary_module: None,
            ordinary_owner: None,
            retained: retained.map(RetainedOriginalDeclaration::into_lease),
        };
        if let Err(error) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        QueryPoll::Ready(Ok(Some(callee)))
    }
    pub(crate) fn select(&self, db: &mut Database, found: NameLookup) -> QueryPoll<CallSelection> {
        let selected = match found {
            NameLookup::Defined(selected) => {
                if let Some(function) = crate::facade::functions::select(&selected) {
                    return QueryPoll::Ready(Ok(CallSelection::Facade(
                        function,
                        selected.identity,
                    )));
                }
                selected
            }
            NameLookup::Absent => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    "reached direct function identity is absent before argument evaluation".into(),
                )));
            }
            NameLookup::NeedInput(producer) => {
                return QueryPoll::Ready(Ok(CallSelection::NeedProducer(producer)));
            }
            other => return QueryPoll::Ready(Ok(CallSelection::Unresolved(other))),
        };
        let Some((source, item)) = self.item(selected.identity) else {
            if let Some(variant) = self.graph.enum_variant(selected.identity) {
                return QueryPoll::Ready(
                    super::nominal_constructors::NominalConstructorTarget::variant(
                        self.sources,
                        variant,
                    )
                    .map(CallSelection::Constructor),
                );
            }
            return self.select_generated(db, selected.identity);
        };
        let retained = match self.current(db, source, selected.identity) {
            QueryPoll::Ready(Ok(current)) => current,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        // Impl-owned associated functions are ordinary callables; a trait's
        // provided body is callable with its implicit `Self` parameter bound
        // from the call; a required trait method has no body to call.
        if item.scope.owner.is_some()
            && item.def_key.enclosing_impl != item.scope.owner
            && item.body.is_none()
        {
            return QueryPoll::Ready(Ok(CallSelection::NeedOwnerContext(item.scope.clone())));
        }
        if item.def_key.item_kind == ItemKind::Struct {
            return QueryPoll::Ready(Ok(CallSelection::Constructor(
                super::nominal_constructors::NominalConstructorTarget::structure(item),
            )));
        }
        if item.def_key.item_kind != ItemKind::Function {
            return QueryPoll::Ready(Ok(CallSelection::NeedValue(item.def_key.clone())));
        }
        let outcome = match db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(selected.identity),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "selected function signature query result mismatch",
            )));
        };
        let header = retained
            .as_ref()
            .map_or(&item.header, |current| current.header());
        let context = retained.as_ref().map_or_else(
            || DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            |current| current.context().clone(),
        );
        if signature.scope != context.scope
            || signature.header.fingerprint() != header.fingerprint()
            || retained.is_none() && signature.header != *header
        {
            return QueryPoll::Ready(Err(fault("selected signature source declaration mismatch")));
        }
        // Cached signatures carry semantic header identity. Reattach the actual
        // current header locations without changing any inferred declaration.
        let mut signature = signature.clone();
        signature.header = header.clone();
        let body = retained
            .as_ref()
            .map_or_else(|| item.body.clone(), |current| Some(current.body().clone()));
        let parse = retained.as_ref().map_or_else(
            || source.selection().outline.parse.clone(),
            |current| current.parse().clone(),
        );
        let callee = SourceCallable {
            definition: item.def_key.clone(),
            identity: selected.identity,
            signature,
            body,
            source: parse,
            revision: db.source_revision(),
            context,
            lexical: None,
            ordinary_local: None,
            ordinary_module: None,
            ordinary_owner: None,
            retained: retained.map(RetainedOriginalDeclaration::into_lease),
        };
        if let Err(error) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        QueryPoll::Ready(Ok(CallSelection::Source(Box::new(callee))))
    }
}
