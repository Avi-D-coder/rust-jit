//! Definition-context type lookup; unresolved producers never become primitives.
use super::names::{NameDefinition, NameLookup, NameNamespaces, NamePath};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    ir::op::PrimitiveType,
    source::{changes::PreparedSourceView, manifest::Namespace},
    syntax::{
        outline::ScopeKey,
        signatures::{FunctionSignature, GenericKind},
    },
    traits::term::ParamKey,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SignatureName {
    Parameter(ParamKey),
    Primitive(PrimitiveType),
    Str,
    Source(NameDefinition),
    Unresolved(NameLookup),
    NeedOwnerContext(ScopeKey),
}
pub struct SignatureNames<'a> {
    graph: &'a NameNamespaces,
    signature: &'a FunctionSignature,
}
impl<'a> SignatureNames<'a> {
    pub fn new(
        graph: &'a NameNamespaces,
        source: &PreparedSourceView,
        signature: &'a FunctionSignature,
    ) -> Result<Self, Diagnostic> {
        graph.require_source(source.selection(), source.outline())?;
        let item = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == signature.definition)
            .ok_or_else(|| fault("signature definition is absent from its source context"))?;
        if item.scope != signature.scope
            || item.header.fingerprint() != signature.header.fingerprint()
        {
            return Err(fault("signature/source declaration context mismatch"));
        }
        Ok(Self { graph, signature })
    }
    /// Declaration-context lookup from immutable query inputs, including generated units.
    pub fn from_database(
        db: &mut crate::db::memo::Database,
        graph: &'a NameNamespaces,
        signature: &'a FunctionSignature,
    ) -> Result<Self, Diagnostic> {
        use crate::db::dependency::{InputKey, InputValue};
        if !matches!(db.input(&InputKey::DefinitionHeader(signature.definition)).as_deref(),Some(InputValue::DefinitionHeader(header)) if header==&signature.header)
            || !matches!(db.input(&InputKey::DeclarationContext(signature.definition)).as_deref(),Some(InputValue::DeclarationContext(context)) if context.scope==signature.scope)
            || !matches!(db.input(&InputKey::DefinitionSource(signature.definition)).as_deref(),Some(InputValue::DefinitionSource(source)) if source.profile==signature.header.profile)
        {
            return Err(fault("signature/database declaration context mismatch"));
        }
        Ok(Self { graph, signature })
    }
    pub fn unqualified(
        &self,
        name: &str,
        prelude: Option<&ScopeKey>,
    ) -> Result<SignatureName, Diagnostic> {
        declaration_type_name(
            self.graph,
            self.signature.definition,
            &self.signature.scope,
            self.signature.header.profile.edition,
            &self.signature.generics,
            name,
            prelude,
        )
    }

    pub fn qualified(&self, path: &NamePath) -> Result<SignatureName, Diagnostic> {
        if self.signature.scope.owner.is_some() {
            return Ok(SignatureName::NeedOwnerContext(
                self.signature.scope.clone(),
            ));
        }
        Ok(
            match self
                .graph
                .lookup(Namespace::Type, path, &self.signature.scope)?
            {
                NameLookup::Defined(definition) => SignatureName::Source(definition),
                other => SignatureName::Unresolved(other),
            },
        )
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn builtin(name: &str) -> Option<SignatureName> {
    use PrimitiveType as P;
    let primitive = match name {
        "str" => return Some(SignatureName::Str),
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
        _ => return None,
    };
    Some(SignatureName::Primitive(primitive))
}

pub(crate) fn declaration_type_name(
    graph: &NameNamespaces,
    definition: crate::core::digest::Digest,
    scope: &ScopeKey,
    edition: u16,
    generics: &[crate::syntax::signatures::DefinitionParameter],
    name: &str,
    prelude: Option<&ScopeKey>,
) -> Result<SignatureName, Diagnostic> {
    if name == "Self" {
        return Ok(SignatureName::NeedOwnerContext(scope.clone()));
    }
    let name = crate::expand::scopes::MacroName::parse(name, edition)?;
    if let Some(parameter) = generics
        .iter()
        .find(|p| p.kind == GenericKind::Type && p.name == name.as_str())
    {
        return Ok(SignatureName::Parameter(ParamKey {
            definition,
            ordinal: parameter.ordinal,
        }));
    }
    if scope.owner.is_some() {
        return Ok(SignatureName::NeedOwnerContext(scope.clone()));
    }
    let lookup =
        graph.lookup_unqualified(Namespace::Type, name.as_str(), scope, edition, prelude)?;
    Ok(match lookup {
        NameLookup::Defined(definition) => SignatureName::Source(definition),
        NameLookup::Absent => {
            builtin(name.as_str()).unwrap_or(SignatureName::Unresolved(NameLookup::Absent))
        }
        other => SignatureName::Unresolved(other),
    })
}
