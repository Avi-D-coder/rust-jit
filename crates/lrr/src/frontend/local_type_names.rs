//! Exact declaration-name evidence for the ordinary local Type traversal.
use super::{PreparedBodyContext, error};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        names::{NameDefinition, NameLookup},
    },
    syntax::{
        ast_adapter::{range, semantic_name, tokens},
        spans::SourceRange,
        token_wire::{Key, NameContextKey},
    },
    traits::term::ParamKey,
};
use ra_ap_syntax::{
    AstNode, TextRange,
    ast::{self, HasName},
};
use std::collections::BTreeMap;

struct DeclarationName {
    name: MacroName,
    context: NameContextKey,
    raw: Key,
    edition: u16,
    range: SourceRange,
}
struct GenericName {
    declaration: DeclarationName,
    parameter: ParamKey,
}
pub(in crate::frontend::source_locals) struct LocalTypeNames {
    owner: Digest,
    items: BTreeMap<Digest, DeclarationName>,
    generics: Vec<GenericName>,
    need_owner: bool,
    bytes: usize,
    fingerprint: Digest,
}
pub(in crate::frontend::source_locals) enum ItemNames {
    Selected(Option<NameLookup>),
    NeedDeclaration(Digest),
}
impl LocalTypeNames {
    pub(in crate::frontend::source_locals) fn prepare(
        context: &PreparedBodyContext,
        root: Key,
        hygiene: &HygieneFacts,
    ) -> Result<Self, Diagnostic> {
        let source = context.source();
        let parsed = source.parsed();
        let capture = |name: ast::Name| -> Result<DeclarationName, Diagnostic> {
            let actual = range(name.syntax().text_range());
            let edition = parsed.token_edition(actual)?;
            let raw = source.identifier_context_or_original(actual, root)?;
            Ok(DeclarationName {
                name: MacroName::parse(name.text(), edition)?,
                context: hygiene.name_item(raw)?,
                raw,
                edition,
                range: actual,
            })
        };
        let mut items = BTreeMap::new();
        let mut item_bytes = 0usize;
        for row in &context.facts().items {
            if row.item.def_key.declared_name.is_empty()
                || row
                    .namespaces
                    .iter()
                    .all(|namespace| *namespace == crate::source::manifest::Namespace::Macro)
            {
                continue;
            }
            let syntax = parsed
                .syntax_at(row.item.declaration_span, |node| {
                    ast::Item::can_cast(node.kind())
                })?
                .ok_or_else(|| error("local declaration exact syntax absent"))?;
            let excluded = row
                .item
                .body
                .as_ref()
                .map(|body| body.range)
                .or(row.container_range)
                .map(|span| TextRange::new(span.start.into(), span.end.into()));
            if tokens(&syntax, excluded) != row.item.header.tokens {
                return Err(error("local declaration header/source differs"));
            }
            // `extern crate ... as ...` uses its actual alias declaration token.
            // Unsupported declaration spelling stays missing evidence, not a root.
            let name = if let Some(external) = ast::ExternCrate::cast(syntax.clone()) {
                external.rename().and_then(|rename| rename.name())
            } else {
                syntax.children().find_map(ast::Name::cast)
            };
            let Some(name) = name else {
                continue;
            };
            if semantic_name(name.text()).as_deref()
                != Some(row.item.def_key.declared_name.as_str())
            {
                return Err(error("local declaration name/source differs"));
            }
            let actual = capture(name)?;
            item_bytes = item_bytes
                .checked_add(16 * std::mem::size_of::<(Digest, DeclarationName)>() + 256)
                .and_then(|n| n.checked_add(actual.name.as_str().len().checked_mul(2)?))
                .ok_or_else(crate::runtime::arena::resource)?;
            if items
                .insert(row.item.def_key.fingerprint(), actual)
                .is_some()
            {
                return Err(error("local declaration occurrence identity repeats"));
            }
        }
        let declaration = parsed
            .syntax_at(source.context().range, |node| {
                ast::Item::can_cast(node.kind())
            })?
            .ok_or_else(|| error("owning generic declaration exact syntax absent"))?;
        let body = source.body().range;
        if !declaration
            .children()
            .any(|node| range(node.text_range()) == body)
            || tokens(
                &declaration,
                Some(TextRange::new(body.start.into(), body.end.into())),
            ) != source.header().tokens
        {
            return Err(error("owning generic declaration header/body differs"));
        }
        let mut generics = Vec::new();
        if let Some(list) = declaration.children().find_map(ast::GenericParamList::cast) {
            let declared = crate::syntax::signatures::capture_generics(Some(list.clone()))?;
            for (ordinal, parameter) in list.generic_params().enumerate() {
                if let ast::GenericParam::TypeParam(parameter) = parameter {
                    let name = parameter
                        .name()
                        .ok_or_else(|| error("generic Type declaration name absent"))?;
                    generics
                        .try_reserve(1)
                        .map_err(|_| crate::runtime::arena::resource())?;
                    let actual = capture(name)?;
                    let common = declared
                        .get(ordinal)
                        .filter(|parameter| {
                            parameter.kind == crate::syntax::signatures::GenericKind::Type
                                && parameter.name == actual.name.as_str()
                        })
                        .ok_or_else(|| {
                            error("generic declaration differs from common signature capture")
                        })?;
                    generics.push(GenericName {
                        declaration: actual,
                        parameter: ParamKey {
                            definition: source.body().def_key,
                            ordinal: common.ordinal,
                        },
                    });
                }
            }
        }
        // An impl or trait member also sees its owner's generic parameters,
        // inherited after its own exactly as the common signature capture
        // (`owner_generics`) numbers them. With the owner's actual declaration
        // tokens captured, an owner parameter selects its inherited parameter
        // and every other name continues to the enclosing module, as for a
        // free function. An owner this syntax does not reach stays pending.
        let mut need_owner = !source.header().context.is_empty();
        if need_owner
            && let Some(function) = ast::Fn::cast(declaration.clone())
            && let Some(owner) = function.syntax().parent().and_then(|items| items.parent())
        {
            let owner_list = if let Some(implementation) = ast::Impl::cast(owner.clone()) {
                Some(ast::HasGenericParams::generic_param_list(&implementation))
            } else {
                ast::Trait::cast(owner)
                    .map(|definition| ast::HasGenericParams::generic_param_list(&definition))
            };
            if let Some(owner_list) = owner_list {
                let own = crate::syntax::signatures::capture_generics(
                    ast::HasGenericParams::generic_param_list(&function),
                )?;
                let combined = crate::syntax::signatures::owner_generics(&function, own.clone())?;
                let mut inherited = Vec::new();
                let captured = owner_list.iter().flat_map(|list| list.generic_params()).enumerate().try_for_each(
                    |(index, parameter)| -> Result<(), Diagnostic> {
                        let ast::GenericParam::TypeParam(parameter) = parameter else {
                            return Ok(());
                        };
                        let name = parameter
                            .name()
                            .ok_or_else(|| error("owner generic Type declaration name absent"))?;
                        let at = range(name.syntax().text_range());
                        let edition = parsed.token_edition(at)?;
                        let raw = source
                            .owner_identifier_context(at, root)?
                            .ok_or_else(|| error("owner generic lacks its actual syntax context"))?;
                        let actual = DeclarationName {
                            name: MacroName::parse(name.text(), edition)?,
                            context: hygiene.name_item(raw)?,
                            raw,
                            edition,
                            range: at,
                        };
                        let common = own
                            .len()
                            .checked_add(index)
                            .and_then(|ordinal| combined.get(ordinal))
                            .filter(|parameter| {
                                parameter.kind == crate::syntax::signatures::GenericKind::Type
                                    && parameter.name == actual.name.as_str()
                            })
                            .ok_or_else(|| {
                                error("owner generic differs from common signature capture")
                            })?;
                        inherited
                            .try_reserve(1)
                            .map_err(|_| crate::runtime::arena::resource())?;
                        inherited.push(GenericName {
                            declaration: actual,
                            parameter: ParamKey {
                                definition: source.body().def_key,
                                ordinal: common.ordinal,
                            },
                        });
                        Ok(())
                    },
                );
                if captured.is_ok() {
                    generics
                        .try_reserve(inherited.len())
                        .map_err(|_| crate::runtime::arena::resource())?;
                    generics.extend(inherited);
                    need_owner = false;
                }
            }
        }
        let mut bytes = items
            .len()
            .checked_mul(16 * std::mem::size_of::<(Digest, DeclarationName)>() + 256)
            .and_then(|n| {
                n.checked_add(
                    generics
                        .capacity()
                        .checked_mul(std::mem::size_of::<GenericName>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        for name in items
            .values()
            .chain(generics.iter().map(|generic| &generic.declaration))
        {
            bytes = bytes
                .checked_add(
                    name.name
                        .as_str()
                        .len()
                        .checked_mul(2)
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        let mut fingerprint = StableEncoder::new("lrr/v1/local-type-declaration-occurrences");
        fingerprint.digest(source.identity());
        fingerprint.u64(items.len() as u64);
        let encode = |e: &mut StableEncoder, name: &DeclarationName| {
            e.string(name.name.as_str());
            e.digest(Digest(name.raw.0));
            e.digest(Digest(name.context.0.0));
            e.u32(u32::from(name.edition));
            e.u32(name.range.start);
            e.u32(name.range.end);
        };
        for (identity, name) in &items {
            fingerprint.digest(*identity);
            encode(&mut fingerprint, name);
        }
        fingerprint.u64(generics.len() as u64);
        for generic in &generics {
            fingerprint.digest(generic.parameter.definition);
            fingerprint.u32(generic.parameter.ordinal);
            encode(&mut fingerprint, &generic.declaration);
        }
        fingerprint.bool(need_owner);
        Ok(Self {
            owner: source.identity(),
            items,
            generics,
            need_owner,
            bytes,
            fingerprint: fingerprint.finish(),
        })
    }
    pub(in crate::frontend::source_locals) fn extra_bytes(&self) -> usize {
        self.bytes
    }
    pub(in crate::frontend::source_locals) fn owner(&self) -> Digest {
        self.owner
    }
    pub(in crate::frontend::source_locals) fn item_spelling(
        &self,
        identity: Digest,
    ) -> Option<(&MacroName, u16)> {
        self.items
            .get(&identity)
            .map(|name| (&name.name, name.edition))
    }
    pub(in crate::frontend::source_locals) fn items(
        &self,
        items: &[NameDefinition],
        context: NameContextKey,
    ) -> Result<ItemNames, Diagnostic> {
        items
            .len()
            .checked_mul(std::mem::size_of::<NameDefinition>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut found = Vec::new();
        found
            .try_reserve_exact(items.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for item in items {
            let Some(name) = self.items.get(&item.identity) else {
                return Ok(ItemNames::NeedDeclaration(item.identity));
            };
            if name.context == context {
                let mut item = item.clone();
                item.definition_edition = name.edition;
                found.push(item);
            }
        }
        Ok(ItemNames::Selected(match found.len() {
            0 => None,
            1 => Some(NameLookup::Defined(found.remove(0))),
            _ => Some(NameLookup::Ambiguous(found)),
        }))
    }
    pub(in crate::frontend::source_locals) fn generic(
        &self,
        name: &MacroName,
        context: NameContextKey,
    ) -> Result<Option<ParamKey>, Diagnostic> {
        let mut found = None;
        for generic in &self.generics {
            if &generic.declaration.name == name && generic.declaration.context == context {
                if found.replace(generic.parameter).is_some() {
                    return Err(error("ambiguous generic Type declaration occurrence"));
                }
            }
        }
        Ok(found)
    }
    pub(in crate::frontend::source_locals) fn needs_owner(&self) -> bool {
        self.need_owner
    }
    pub(in crate::frontend::source_locals) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
}
