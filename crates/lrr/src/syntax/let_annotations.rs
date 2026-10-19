//! Written local annotations that name composite or non-primitive Types
//! (`let p: P`, `let t: (i32, P)`, `let a: [E; 2]`, `let s: &str`,
//! `let b: &'static [u8]`, `let v: &[_]`, `let o: &dyn T`). Pure syntax: each
//! named leaf is the canonical indexed Type occurrence the entered Type
//! service selects; this module never resolves a name itself.
use super::{ParsedFile, annotation_hint, error};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{NodeId, PcKey},
    syntax::{
        ast_adapter::range,
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        spans::SourceRange,
    },
    traits::term::RegionTerm,
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind,
    ast::{self},
};

/// The written structure around the annotation's named leaves.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum AnnotationShape {
    /// The canonical indexed Syntax node of one written `PathType`.
    Leaf(NodeId),
    Tuple(Vec<AnnotationShape>),
    Array(Box<AnnotationShape>, u32),
    /// A written generic application (`Opt<i32>`): the base `PathType`'s
    /// canonical Syntax node and its type arguments' shapes.
    Generic(NodeId, Vec<AnnotationShape>),
    Slice(Box<AnnotationShape>),
    /// A written reference. An elided or `'_` region is this reference
    /// annotation's own local region identity.
    Ref {
        region: RegionTerm,
        mutable: bool,
        target: Box<AnnotationShape>,
    },
    /// `_` inside a written structure.
    Infer,
    /// `dyn Trait`: the principal trait `PathType`'s canonical Syntax node.
    Dyn(NodeId),
    /// `*const T` / `*mut T` (E-R5-008).
    Ptr {
        mutable: bool,
        target: Box<AnnotationShape>,
    },
    /// `fn(A) -> R`, `unsafe fn(..)`, `extern "C" fn(..)`: the ABI and
    /// `unsafe` qualifiers are not part of the runtime fn pointer.
    FnPtr {
        params: Vec<AnnotationShape>,
        result: Box<AnnotationShape>,
    },
}
pub(crate) struct LetAnnotationSite {
    pub(crate) range: SourceRange,
    pub(crate) shape: AnnotationShape,
}
/// A selected annotation for one exact `let` statement.
pub(crate) struct LetAnnotation {
    pub(crate) range: SourceRange,
    pub(crate) written: std::sync::Arc<crate::ir::written_type::WrittenType>,
}

impl ParsedFile {
    /// The annotation of the `let` statement at this PC when it needs the
    /// entered Type service: one the primitive/reference/array annotation
    /// lowering does not already admit.
    pub(crate) fn let_annotation_site(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<Option<LetAnnotationSite>, Diagnostic> {
        let node = NodeId(pc.node);
        if index
            .nodes()
            .get(pc.node as usize)
            .is_none_or(|node| node.role != NodeRole::Statement)
        {
            return Ok(None);
        }
        let Some(statement) = self
            .entered_node_syntax(body, index, node)?
            .and_then(ast::LetStmt::cast)
        else {
            return Ok(None);
        };
        if statement.initializer().is_none()
            && matches!(statement.pat(), Some(ast::Pat::WildcardPat(_)))
        {
            return Ok(None);
        }
        let Some(ty) = statement.ty() else {
            return Ok(None);
        };
        if annotation_hint(self, Some(ty.clone()), pc.body.definition)?.is_some() {
            return Ok(None);
        }
        let Some(shape) = self.annotation_shape(body, index, pc.body.definition, &ty)? else {
            return Ok(None);
        };
        Ok(Some(LetAnnotationSite {
            range: range(ty.syntax().text_range()),
            shape,
        }))
    }
    /// The canonical indexed Syntax node of one written `PathType`.
    fn path_type_node(
        body: &BodyHandle,
        index: &BodyIndex,
        path: &ast::PathType,
    ) -> Result<Option<NodeId>, Diagnostic> {
        let absolute = range(path.syntax().text_range());
        let relative = SourceRange {
            start: absolute
                .start
                .checked_sub(body.range.start)
                .ok_or_else(|| error("annotation before its body"))?,
            end: absolute
                .end
                .checked_sub(body.range.start)
                .ok_or_else(|| error("annotation before its body"))?,
        };
        Ok(index
            .nodes_at_range(relative)
            .find(|node| node.role == NodeRole::Syntax)
            .map(|node| node.id))
    }
    /// The written structure around a written Type's named leaves; shared
    /// by `let` annotations, patterns and explicit call Type arguments.
    pub(crate) fn annotation_shape(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        definition: crate::core::digest::Digest,
        ty: &ast::Type,
    ) -> Result<Option<AnnotationShape>, Diagnostic> {
        Ok(Some(match ty {
            ast::Type::PathType(path) => {
                let Some(written) = path.path() else {
                    return Ok(None);
                };
                // Type arguments only on the final segment (`W<u8>`,
                // `m::W<u8>`); the owner path itself is selected by the same
                // declaration kernel as an argument-free qualified Type.
                let arguments = written
                    .segment()
                    .and_then(|segment| crate::syntax::ast_adapter::nonempty_generic_arguments(&segment));
                if std::iter::successors(written.qualifier(), |path| path.qualifier()).any(|path| {
                    path.segment()
                        .is_none_or(|segment| crate::syntax::ast_adapter::nonempty_generic_arguments(&segment).is_some())
                }) {
                    return Ok(None);
                }
                let Some(leaf) = Self::path_type_node(body, index, path)? else {
                    return Ok(None);
                };
                let Some(arguments) = arguments else {
                    return Ok(Some(AnnotationShape::Leaf(leaf)));
                };
                let mut shapes = Vec::new();
                for argument in arguments.generic_args() {
                    match argument {
                        // Regions are not runtime data.
                        ast::GenericArg::LifetimeArg(_) => {}
                        ast::GenericArg::TypeArg(argument) => {
                            let Some(ty) = argument.ty() else {
                                return Ok(None);
                            };
                            let Some(shape) =
                                self.annotation_shape(body, index, definition, &ty)?
                            else {
                                return Ok(None);
                            };
                            shapes.push(shape);
                        }
                        _ => return Ok(None),
                    }
                }
                AnnotationShape::Generic(leaf, shapes)
            }
            ast::Type::ParenType(paren) => {
                let Some(inner) = paren.ty() else {
                    return Ok(None);
                };
                return self.annotation_shape(body, index, definition, &inner);
            }
            ast::Type::TupleType(tuple) => {
                let mut members = Vec::new();
                for field in tuple.fields() {
                    let Some(member) = self.annotation_shape(body, index, definition, &field)?
                    else {
                        return Ok(None);
                    };
                    members.push(member);
                }
                AnnotationShape::Tuple(members)
            }
            ast::Type::ArrayType(array) => {
                let Some(element) = array.ty() else {
                    return Ok(None);
                };
                let Some(element) = self.annotation_shape(body, index, definition, &element)?
                else {
                    return Ok(None);
                };
                // A literal length only; other const expressions need the
                // pure constant evaluator's length demand.
                let Some(ast::Expr::Literal(length)) = array.const_arg().and_then(|arg| arg.expr())
                else {
                    return Ok(None);
                };
                let Some(token) = length
                    .syntax()
                    .children_with_tokens()
                    .filter_map(|item| item.into_token())
                    .find(|token| token.kind() == SyntaxKind::INT_NUMBER)
                else {
                    return Ok(None);
                };
                let Some(number) = ast::IntNumber::cast(token) else {
                    return Ok(None);
                };
                if !matches!(number.suffix(), None | Some("usize")) {
                    return Ok(None);
                }
                let Some(len) = number.value().ok().and_then(|len| u32::try_from(len).ok()) else {
                    return Ok(None);
                };
                AnnotationShape::Array(Box::new(element), len)
            }
            ast::Type::SliceType(slice) => {
                let Some(element) = slice.ty() else {
                    return Ok(None);
                };
                let Some(element) = self.annotation_shape(body, index, definition, &element)?
                else {
                    return Ok(None);
                };
                AnnotationShape::Slice(Box::new(element))
            }
            ast::Type::RefType(reference) => {
                let Some(target) = reference.ty() else {
                    return Ok(None);
                };
                let Some(target) = self.annotation_shape(body, index, definition, &target)? else {
                    return Ok(None);
                };
                let region = match reference.lifetime() {
                    Some(lifetime) if lifetime.syntax().text() == "'static" => RegionTerm::Static,
                    // A named region of the enclosing signature: regions are
                    // not runtime data, so the annotation names it erased, as
                    // every other written-type kernel does.
                    Some(lifetime) if lifetime.syntax().text() != "'_" => RegionTerm::Static,
                    _ => {
                        let range = reference.syntax().text_range();
                        RegionTerm::Local {
                            definition,
                            start: u32::from(range.start()),
                            end: u32::from(range.end()),
                        }
                    }
                };
                AnnotationShape::Ref {
                    region,
                    mutable: reference.mut_token().is_some(),
                    target: Box::new(target),
                }
            }
            ast::Type::PtrType(pointer) => {
                let Some(target) = pointer.ty() else {
                    return Ok(None);
                };
                let Some(target) = self.annotation_shape(body, index, definition, &target)? else {
                    return Ok(None);
                };
                AnnotationShape::Ptr {
                    mutable: pointer.mut_token().is_some(),
                    target: Box::new(target),
                }
            }
            // `for<'a> fn(&'a T)`: the binder names regions only.
            ast::Type::ForType(for_type) => {
                let Some(inner @ ast::Type::FnPtrType(_)) = for_type.ty() else {
                    return Ok(None);
                };
                return self.annotation_shape(body, index, definition, &inner);
            }
            ast::Type::FnPtrType(function) => {
                let Some(list) = function.param_list() else {
                    return Ok(None);
                };
                let mut params = Vec::new();
                for parameter in list.params() {
                    let Some(ty) = parameter.ty() else {
                        return Ok(None);
                    };
                    let Some(shape) = self.annotation_shape(body, index, definition, &ty)? else {
                        return Ok(None);
                    };
                    params.push(shape);
                }
                let result = match function.ret_type() {
                    Some(ret) => {
                        let Some(ty) = ret.ty() else {
                            return Ok(None);
                        };
                        let Some(shape) = self.annotation_shape(body, index, definition, &ty)?
                        else {
                            return Ok(None);
                        };
                        shape
                    }
                    None => AnnotationShape::Tuple(Vec::new()),
                };
                AnnotationShape::FnPtr {
                    params,
                    result: Box::new(result),
                }
            }
            ast::Type::InferType(_) => AnnotationShape::Infer,
            ast::Type::DynTraitType(object) => {
                // The principal trait: the first trait bound; auto-trait and
                // region bounds add no runtime structure.
                let Some(principal) = object.type_bound_list().and_then(|bounds| {
                    bounds.bounds().find_map(|bound| match bound.ty() {
                        Some(ast::Type::PathType(path)) => Some(path),
                        _ => None,
                    })
                }) else {
                    return Ok(None);
                };
                let Some(node) = Self::path_type_node(body, index, &principal)? else {
                    return Ok(None);
                };
                AnnotationShape::Dyn(node)
            }
            _ => return Ok(None),
        }))
    }
}
