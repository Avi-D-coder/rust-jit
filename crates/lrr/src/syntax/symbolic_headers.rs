//! Pinned parser adapter: capture owned symbolic header terms only.
use super::{
    ast_adapter::semantic_name,
    signatures::{DefinitionParameter, GenericKind, canonical_lifetime, lower_type},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    traits::term::{Binder, BoundKind, ConstTerm, FunctionSignature, ParamKey, RegionTerm, TyTerm},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasTypeBounds},
};
use std::collections::BTreeMap;
/// Pure syntactic candidates from one parameter. Repeated uses of the same
/// region in that parameter count once; different parameter positions do not.
enum InputRegions {
    None,
    One(RegionTerm),
    Ambiguous,
}
/// This state belongs to a single function signature. Nested function pointers
/// create their own state and never contribute candidates to their parent.
enum ReferenceElision {
    Forbidden,
    Input {
        variables: u32,
        regions: InputRegions,
    },
    Output(Option<RegionTerm>),
}
fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "symbolic function header needs further ordered source-type integration".into(),
    )
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "symbolic header exceeds binder/source limits".into(),
    )
}
fn names(binder: Option<ast::ForBinder>) -> Result<BTreeMap<String, u32>, Diagnostic> {
    let Some(binder) = binder else {
        return Ok(BTreeMap::new());
    };
    let parameters = binder.generic_param_list().ok_or_else(unsupported)?;
    let mut names = BTreeMap::new();
    for parameter in parameters.generic_params() {
        let ast::GenericParam::LifetimeParam(parameter) = parameter else {
            return Err(unsupported());
        };
        if parameter.attrs().next().is_some() || parameter.type_bound_list().is_some() {
            return Err(unsupported());
        }
        let written = parameter
            .lifetime()
            .ok_or_else(unsupported)?
            .syntax()
            .text()
            .to_string();
        let name = canonical_lifetime(&written).ok_or_else(unsupported)?;
        if name == "'static" || name == "'_" || names.contains_key(&name) {
            return Err(unsupported());
        }
        let ordinal = u32::try_from(names.len()).map_err(|_| limit())?;
        names.insert(name, ordinal);
    }
    Ok(names)
}
/// Captures the predicate binder separately from any function-type binder.
/// It does not select the trait, certify its bounds, or publish a runtime view.
pub fn function_predicate(predicate: &ast::WherePred) -> Result<Binder<TyTerm>, Diagnostic> {
    let outer = names(predicate.for_binder())?;
    let variables = vec![BoundKind::Region; outer.len()];
    let mut scopes = vec![outer];
    let ty = predicate.ty().ok_or_else(unsupported)?;
    let value = parse(&ty, &mut scopes, 0, None, &mut ReferenceElision::Forbidden)?;
    if !matches!(value, TyTerm::Function(_)) {
        return Err(unsupported());
    }
    value.heap_bytes().ok_or_else(limit)?;
    Ok(Binder { variables, value })
}
/// Shared by source-checked signature and alias recipes. Definition parameters
/// stay rigid; explicit `for` and elided pointer-input regions belong to the
/// function pointer's nested de Bruijn scope.
/// Trait bounds/defaults remain in the owning header metadata for later goals.
pub(crate) fn capture_function_type(
    ty: &ast::Type,
    definition: Digest,
    parameters: &[DefinitionParameter],
    depth: usize,
) -> Result<TyTerm, Diagnostic> {
    let depth = u32::try_from(depth).map_err(|_| limit())?;
    let result = parse(
        ty,
        &mut Vec::new(),
        depth,
        Some((definition, parameters)),
        &mut ReferenceElision::Forbidden,
    )?;
    crate::traits::subst::validate_closed(&result)?;
    Ok(result)
}
fn parse(
    ty: &ast::Type,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    // Return each branch result directly. Wrapping a match in Ok with a ? in
    // every arm retains many large result temporaries in recursive debug frames.
    match ty {
        ast::Type::ParenType(parenthesized) => {
            parenthesized_type(parenthesized, scopes, depth, context, elision)
        }
        ast::Type::TupleType(tuple) => tuple_type(tuple, scopes, depth, context, elision),
        ast::Type::RefType(reference) => reference_type(reference, scopes, depth, context, elision),
        ast::Type::ForType(for_type) => forall_type(for_type, scopes, depth, context),
        ast::Type::FnPtrType(function) => {
            function_type(function, BTreeMap::new(), scopes, depth + 1, context)
        }
        ast::Type::SliceType(slice) => slice_type(slice, scopes, depth, context, elision),
        // `*const T` / `*mut T` (E-R5-008): the reserved-identity nominal.
        ast::Type::PtrType(pointer) => Ok(crate::traits::pointer_terms::raw_pointer(
            pointer.mut_token().is_some(),
            parse(
                &pointer.ty().ok_or_else(unsupported)?,
                scopes,
                depth + 1,
                context,
                elision,
            )?,
        )),
        ast::Type::ArrayType(array) => array_type(array, scopes, depth, context, elision),
        ast::Type::PathType(_) | ast::Type::NeverType(_) => named_type(ty, depth, context),
        _ => Err(unsupported()),
    }
}
fn parenthesized_type(
    parenthesized: &ast::ParenType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    parse(
        &parenthesized.ty().ok_or_else(unsupported)?,
        scopes,
        depth + 1,
        context,
        elision,
    )
}
fn forall_type(
    for_type: &ast::ForType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
) -> Result<TyTerm, Diagnostic> {
    let variables = names(for_type.for_binder())?;
    let ast::Type::FnPtrType(function) = for_type.ty().ok_or_else(unsupported)? else {
        return Err(unsupported());
    };
    function_type(&function, variables, scopes, depth + 1, context)
}
fn slice_type(
    slice: &ast::SliceType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    Ok(TyTerm::Slice(Box::new(parse(
        &slice.ty().ok_or_else(unsupported)?,
        scopes,
        depth + 1,
        context,
        elision,
    )?)))
}
fn named_type(
    ty: &ast::Type,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
) -> Result<TyTerm, Diagnostic> {
    // Preserve declaration context: no primitive or nominal identity from text.
    let Some((definition, parameters)) = context else {
        return Err(unsupported());
    };
    lower_type(
        ty,
        definition,
        parameters,
        usize::try_from(depth).map_err(|_| limit())?,
    )?
    .ok_or_else(unsupported)
}
// Keep recursive dispatch frames small on the normal host thread stack.
// Separate AST branches retain exactly the same depth, scope and elision state.
fn tuple_type(
    tuple: &ast::TupleType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    let mut fields = Vec::new();
    for ty in tuple.fields() {
        fields.try_reserve(1).map_err(|_| limit())?;
        fields.push(parse(&ty, scopes, depth + 1, context, elision)?);
    }
    Ok(if fields.is_empty() {
        TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)
    } else {
        TyTerm::Tuple(fields)
    })
}

fn reference_type(
    reference: &ast::RefType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    let written = reference
        .lifetime()
        .map(|lifetime| {
            canonical_lifetime(&lifetime.syntax().text().to_string()).ok_or_else(unsupported)
        })
        .transpose()?;
    let region = if written.as_deref().is_none_or(|name| name == "'_") {
        match elision {
            ReferenceElision::Input { variables, .. } => {
                let index = *variables;
                *variables = variables.checked_add(1).ok_or_else(limit)?;
                RegionTerm::Bound { depth: 0, index }
            }
            ReferenceElision::Output(Some(region)) => region.clone(),
            ReferenceElision::Forbidden | ReferenceElision::Output(None) => {
                return Err(unsupported());
            }
        }
    } else if written.as_deref() == Some("'static") {
        RegionTerm::Static
    } else {
        let written = written.ok_or_else(unsupported)?;
        let bound = scopes
            .iter()
            .rev()
            .enumerate()
            .find_map(|(d, names)| names.get(&written).map(|i| (d, *i)));
        if let Some((depth, index)) = bound {
            RegionTerm::Bound {
                depth: u32::try_from(depth).map_err(|_| limit())?,
                index,
            }
        } else if let Some((definition, parameters)) = context {
            let parameter = parameters
                .iter()
                .find(|parameter| {
                    parameter.kind == GenericKind::Lifetime && parameter.name == written
                })
                .ok_or_else(unsupported)?;
            RegionTerm::Param(ParamKey {
                definition,
                ordinal: parameter.ordinal,
            })
        } else {
            return Err(unsupported());
        }
    };
    if let ReferenceElision::Input { regions, .. } = elision {
        match regions {
            InputRegions::None => *regions = InputRegions::One(region.clone()),
            InputRegions::One(existing) if *existing == region => {}
            InputRegions::One(_) | InputRegions::Ambiguous => *regions = InputRegions::Ambiguous,
        }
    }
    Ok(TyTerm::Ref(
        region,
        reference.mut_token().is_some(),
        Box::new(parse(
            &reference.ty().ok_or_else(unsupported)?,
            scopes,
            depth + 1,
            context,
            elision,
        )?),
    ))
}

fn array_type(
    array: &ast::ArrayType,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
    elision: &mut ReferenceElision,
) -> Result<TyTerm, Diagnostic> {
    // The element must stay in this binder stack. Re-entering ordinary
    // header lowering would lose outer `for` regions inside the array.
    let element = parse(
        &array.ty().ok_or_else(unsupported)?,
        scopes,
        depth + 1,
        context,
        elision,
    )?;
    let argument = array.const_arg().ok_or_else(unsupported)?;
    let mut tokens = argument
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia());
    let token = tokens.next().ok_or_else(unsupported)?;
    if tokens.next().is_some() {
        return Err(unsupported());
    }
    // Like the owning signature capture, accept only a rigid const
    // parameter or a finite unsuffixed decimal literal. No const
    // expression evaluation or spelling-based name resolution occurs.
    let parameter = context.and_then(|(definition, parameters)| {
        let name = semantic_name(token.text())?;
        parameters
            .iter()
            .find(|parameter| parameter.kind == GenericKind::Const && parameter.name == name)
            .map(|parameter| ParamKey {
                definition,
                ordinal: parameter.ordinal,
            })
    });
    let length = if let Some(parameter) = parameter {
        ConstTerm::Param(parameter)
    } else {
        if token.kind() != ra_ap_syntax::SyntaxKind::INT_NUMBER {
            return Err(unsupported());
        }
        let value = token
            .text()
            .bytes()
            .try_fold(0u64, |value, digit| match digit {
                b'_' => Some(value),
                b'0'..=b'9' => value.checked_mul(10)?.checked_add(u64::from(digit - b'0')),
                _ => None,
            });
        ConstTerm::Value(value.ok_or_else(unsupported)?)
    };
    Ok(TyTerm::Array(Box::new(element), length))
}

/// Capture qualifiers from the existing actual function-pointer AST. Binder
/// identity is supplied separately by the consumer's genuine Binder producer.
pub(crate) fn source_function_header(
    function: &ast::FnPtrType,
) -> Option<crate::traits::term::SourceFunctionHeader> {
    use ra_ap_syntax::AstToken;
    if function.async_token().is_some() || function.const_token().is_some() {
        return None;
    }
    let abi = match function.abi() {
        None => "Rust".to_owned(),
        Some(abi) => {
            abi.extern_token()?;
            match abi.string_token() {
                Some(token) => ast::String::cast(token)?.value().ok()?.into_owned(),
                None => "C".to_owned(),
            }
        }
    };
    Some(crate::traits::term::SourceFunctionHeader {
        abi,
        is_unsafe: function.unsafe_token().is_some(),
        c_variadic: function.param_list()?.params().any(|parameter| parameter.dotdotdot_token().is_some()),
    })
}

fn function_type(
    function: &ast::FnPtrType,
    variables: BTreeMap<String, u32>,
    scopes: &mut Vec<BTreeMap<String, u32>>,
    depth: u32,
    context: Option<(Digest, &[DefinitionParameter])>,
) -> Result<TyTerm, Diagnostic> {
    // Qualifiers are retained for source consumers; runtime fn-pointer
    // operations remain permissive under E-R5-008.
    if function.async_token().is_some() || function.const_token().is_some() {
        return Err(unsupported());
    }
    let mut variable_count = u32::try_from(variables.len()).map_err(|_| limit())?;
    scopes.push(variables);
    let parameters = function.param_list().ok_or_else(unsupported)?;
    if parameters.self_param().is_some() {
        return Err(unsupported());
    }
    let mut terms = Vec::new();
    let mut output_regions = InputRegions::None;
    for parameter in parameters.params() {
        if parameter.dotdotdot_token().is_some() || parameter.attrs().next().is_some() {
            return Err(unsupported());
        }
        let mut input = ReferenceElision::Input {
            variables: variable_count,
            regions: InputRegions::None,
        };
        terms.try_reserve(1).map_err(|_| limit())?;
        terms.push(parse(
            &parameter.ty().ok_or_else(unsupported)?,
            scopes,
            depth + 1,
            context,
            &mut input,
        )?);
        let ReferenceElision::Input { variables, regions } = input else {
            return Err(unsupported());
        };
        variable_count = variables;
        // rustc's resolve_fn_params requires one qualifying parameter, even
        // when two different parameter positions repeat the same named region.
        output_regions = match (output_regions, regions) {
            (previous, InputRegions::None) => previous,
            (InputRegions::None, current) => current,
            _ => InputRegions::Ambiguous,
        };
    }
    let mut output = ReferenceElision::Output(match output_regions {
        InputRegions::One(region) => Some(region),
        InputRegions::None | InputRegions::Ambiguous => None,
    });
    let result = if let Some(ret) = function.ret_type() {
        parse(
            &ret.ty().ok_or_else(unsupported)?,
            scopes,
            depth + 1,
            context,
            &mut output,
        )?
    } else {
        TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)
    };
    scopes.pop();
    let count = usize::try_from(variable_count).map_err(|_| limit())?;
    let mut kinds = Vec::new();
    kinds.try_reserve_exact(count).map_err(|_| limit())?;
    kinds.resize(count, BoundKind::Region);
    Ok(TyTerm::Function(Box::new(Binder {
        variables: kinds,
        value: FunctionSignature {
            parameters: terms,
            result: Box::new(result),
            source_header: source_function_header(function),
        },
    })))
}
