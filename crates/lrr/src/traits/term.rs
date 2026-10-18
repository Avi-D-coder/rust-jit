//! Owned symbolic identity, separate from guest layout descriptors.
use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    ir::op::PrimitiveType,
};
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct ParamKey {
    pub definition: Digest,
    pub ordinal: u32,
}
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct InferVarKey {
    pub transaction: Digest,
    pub ordinal: u32,
}
/// Demand-local placeholder identity; namespace remains in its enclosing term.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct PlaceholderKey {
    pub transaction: Digest,
    pub universe: u32,
    pub index: u32,
}
impl StableEncode for PlaceholderKey {
    const DOMAIN: &'static str = "lrr/v2/symbolic-placeholder";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.transaction);
        e.u32(self.universe);
        e.u32(self.index);
    }
}
/// One typed position in a source binder. Spelling is not identity.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum BoundKind {
    Type,
    Region,
    Const,
}
impl BoundKind {
    pub fn tag(self) -> u32 {
        match self {
            Self::Type => 0,
            Self::Region => 1,
            Self::Const => 2,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Binder<T> {
    pub variables: Vec<BoundKind>,
    pub value: T,
}
impl<T: StableEncode> StableEncode for Binder<T> {
    const DOMAIN: &'static str = "lrr/v2/symbolic-binder";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.sequence(&self.variables, |e, kind| e.u32(kind.tag()));
        e.record(&self.value);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct FunctionSignature {
    pub parameters: Vec<TyTerm>,
    pub result: Box<TyTerm>,
}
impl StableEncode for FunctionSignature {
    const DOMAIN: &'static str = "lrr/v2/symbolic-safe-rust-function-signature";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.sequence(&self.parameters, |e, term| e.record(term));
        e.record(self.result.as_ref());
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum ConstTerm {
    Placeholder(PlaceholderKey),
    Bound { depth: u32, index: u32 },
    Value(u64),
    Param(ParamKey),
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum RegionTerm {
    Placeholder(PlaceholderKey),
    Static,
    Param(ParamKey),
    Bound {
        depth: u32,
        index: u32,
    },
    /// Identity of an elided region in a local annotation; not a lifetime proof.
    Local {
        definition: Digest,
        start: u32,
        end: u32,
    },
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum GenericArg {
    Type(TyTerm),
    Const(ConstTerm),
    Region(RegionTerm),
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum TyTerm {
    Dyn(DynSchemaKey),
    Opaque(OpaqueKey, Vec<GenericArg>),
    Projection(Box<ProjectionKey>),
    Placeholder(PlaceholderKey),
    Bound {
        depth: u32,
        index: u32,
    },
    Function(Box<Binder<FunctionSignature>>),
    Primitive(PrimitiveType),
    /// Unsized UTF-8 string pointee. This is not a sized primitive, owned
    /// String, byte slice, runtime value or proof of a written name's identity.
    Str,
    Nominal(Digest, Vec<GenericArg>),
    Tuple(Vec<TyTerm>),
    Array(Box<TyTerm>, ConstTerm),
    Slice(Box<TyTerm>),
    Ref(RegionTerm, bool, Box<TyTerm>),
    Param(ParamKey),
    Infer(InferVarKey),
    Never,
}
/// Source object-schema identity; never a runtime descriptor or dictionary.
/// The schema is admitted by the ordered source/world/trait integration later.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct DynSchemaKey(pub Digest);
impl StableEncode for DynSchemaKey {
    const DOMAIN: &'static str = "lrr/v2/symbolic-dyn-schema-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.0);
    }
}
/// Defining return position and typed capture identities, without a hidden type.
/// Captures are in canonical namespace/key order; opaque arguments use that
/// same order. Source capture-rule and world admission remain separate.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct OpaqueKey {
    definition: Digest,
    return_position: u32,
    captures: Vec<(BoundKind, ParamKey)>,
}
impl OpaqueKey {
    pub fn new(
        definition: Digest,
        return_position: u32,
        mut captures: Vec<(BoundKind, ParamKey)>,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        use crate::core::diagnostic::Diagnostic;
        captures.sort_unstable();
        if captures.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "duplicate typed opaque capture identity".into(),
            ));
        }
        Ok(Self {
            definition,
            return_position,
            captures,
        })
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn return_position(&self) -> u32 {
        self.return_position
    }
    pub fn captures(&self) -> &[(BoundKind, ParamKey)] {
        &self.captures
    }
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        self.captures
            .capacity()
            .checked_mul(std::mem::size_of::<(BoundKind, ParamKey)>())
    }
    pub(crate) fn validate_arguments(
        &self,
        arguments: &[GenericArg],
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        if arguments.len() != self.captures.len()
            || self
                .captures
                .iter()
                .zip(arguments)
                .any(|((expected, _), arg)| {
                    let actual = match arg {
                        GenericArg::Type(_) => BoundKind::Type,
                        GenericArg::Region(_) => BoundKind::Region,
                        GenericArg::Const(_) => BoundKind::Const,
                    };
                    *expected != actual
                })
        {
            return Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "opaque arguments differ from typed capture identities".into(),
            ));
        }
        Ok(())
    }
}
impl StableEncode for OpaqueKey {
    const DOMAIN: &'static str = "lrr/v2/symbolic-opaque-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.u32(self.return_position);
        e.sequence(&self.captures, |e, (kind, key)| {
            e.u32(kind.tag());
            e.record(key);
        });
    }
}
impl StableEncode for ParamKey {
    const DOMAIN: &'static str = "lrr/v2/definition-param";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.u32(self.ordinal);
    }
}
impl StableEncode for ConstTerm {
    const DOMAIN: &'static str = "lrr/v2/const-term";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Placeholder(key) => {
                e.u32(3);
                e.record(key);
            }
            Self::Bound { depth, index } => {
                e.u32(2);
                e.u32(*depth);
                e.u32(*index);
            }
            Self::Value(v) => {
                e.u32(0);
                e.u64(*v)
            }
            Self::Param(p) => {
                e.u32(1);
                e.record(p)
            }
        }
    }
}
impl StableEncode for RegionTerm {
    const DOMAIN: &'static str = "lrr/v2/region-term";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Placeholder(key) => {
                e.u32(4);
                e.record(key);
            }
            Self::Static => e.u32(0),
            Self::Param(p) => {
                e.u32(1);
                e.record(p)
            }
            Self::Bound { depth, index } => {
                e.u32(2);
                e.u32(*depth);
                e.u32(*index)
            }
            Self::Local {
                definition,
                start,
                end,
            } => {
                e.u32(3);
                e.digest(*definition);
                e.u32(*start);
                e.u32(*end);
            }
        }
    }
}
impl StableEncode for GenericArg {
    const DOMAIN: &'static str = "lrr/v2/generic-arg";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Type(t) => {
                e.u32(0);
                e.record(t)
            }
            Self::Const(c) => {
                e.u32(1);
                e.record(c)
            }
            Self::Region(r) => {
                e.u32(2);
                e.record(r)
            }
        }
    }
}
impl StableEncode for TyTerm {
    const DOMAIN: &'static str = "lrr/v2/type-term";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Dyn(key) => {
                e.u32(13);
                e.record(key);
            }
            Self::Opaque(key, args) => {
                e.u32(14);
                e.record(key);
                e.sequence(args, |e, arg| e.record(arg));
            }
            Self::Projection(key) => {
                e.u32(12);
                e.record(key.as_ref());
            }
            Self::Placeholder(key) => {
                e.u32(11);
                e.record(key);
            }
            Self::Bound { depth, index } => {
                e.u32(9);
                e.u32(*depth);
                e.u32(*index);
            }
            Self::Function(f) => {
                e.u32(10);
                e.record(f.as_ref());
            }
            Self::Primitive(p) => {
                e.u32(0);
                e.u32(p.tag())
            }
            Self::Nominal(d, args) => {
                e.u32(1);
                e.digest(*d);
                e.sequence(args, |e, a| e.record(a))
            }
            Self::Tuple(ts) => {
                e.u32(2);
                e.sequence(ts, |e, t| e.record(t))
            }
            Self::Array(t, c) => {
                e.u32(3);
                e.record(t.as_ref());
                e.record(c)
            }
            Self::Slice(t) => {
                e.u32(4);
                e.record(t.as_ref())
            }
            Self::Ref(r, m, t) => {
                e.u32(5);
                e.record(r);
                e.bool(*m);
                e.record(t.as_ref())
            }
            Self::Param(p) => {
                e.u32(6);
                e.record(p)
            }
            Self::Infer(v) => {
                e.u32(7);
                e.digest(v.transaction);
                e.u32(v.ordinal)
            }
            Self::Never => e.u32(8),
            // Append without changing any existing term's encoding. There is
            // no numeric PrimitiveType tag for an unsized string pointee.
            Self::Str => e.u32(15),
        }
    }
}

impl TyTerm {
    /// Whether this declared term admits `actual`: structurally equal up to
    /// regions, where a `dyn Trait` position admits any concrete Type (values
    /// keep their concrete runtime Type; see `TypeKey::Dyn`).
    pub(crate) fn admits_dyn(&self, actual: &TyTerm) -> bool {
        match (self, actual) {
            (Self::Dyn(_), _) => true,
            (Self::Tuple(a), Self::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.admits_dyn(y))
            }
            (Self::Array(a, n), Self::Array(b, m)) => n == m && a.admits_dyn(b),
            (Self::Slice(a), Self::Slice(b)) => a.admits_dyn(b),
            (Self::Ref(_, m, a), Self::Ref(_, n, b)) => m == n && a.admits_dyn(b),
            (Self::Nominal(d, a), Self::Nominal(e, b)) => {
                d == e
                    && a.len() == b.len()
                    && a.iter().zip(b).all(|pair| match pair {
                        (GenericArg::Type(x), GenericArg::Type(y)) => x.admits_dyn(y),
                        (GenericArg::Region(_), GenericArg::Region(_)) => true,
                        (x, y) => x == y,
                    })
            }
            (x, y) => x == y,
        }
    }
    /// Whether a `dyn Trait` occurs in this term.
    pub(crate) fn contains_dyn(&self) -> bool {
        match self {
            Self::Dyn(_) => true,
            Self::Tuple(items) => items.iter().any(Self::contains_dyn),
            Self::Array(inner, _) | Self::Slice(inner) | Self::Ref(_, _, inner) => {
                inner.contains_dyn()
            }
            Self::Nominal(_, args) => args.iter().any(|arg| match arg {
                GenericArg::Type(ty) => ty.contains_dyn(),
                _ => false,
            }),
            _ => false,
        }
    }
    /// Check opaque capture arity/namespaces throughout this owned term.
    /// This structural guard does not resolve schemas or require bound terms
    /// to be closed: retained views may have a valid enclosing binder context.
    /// Measurement remains independent, so a shape error is not a size error.
    pub fn validate_opaque_shape(&self) -> Result<(), crate::core::diagnostic::Diagnostic> {
        self.measure().ok_or_else(|| {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "symbolic shape validation exceeds metadata bounds".into(),
            )
        })?;
        // Shape/depth/work were bounded above before expanding this stack.
        let mut stack = vec![self];
        while let Some(term) = stack.pop() {
            if let Self::Opaque(key, arguments) = term {
                key.validate_arguments(arguments)?;
            }
            match term {
                Self::Nominal(_, arguments) | Self::Opaque(_, arguments) => {
                    stack.extend(arguments.iter().filter_map(|arg| match arg {
                        GenericArg::Type(ty) => Some(ty),
                        _ => None,
                    }));
                }
                Self::Projection(key) => {
                    stack.push(&key.trait_ref.self_ty);
                    stack.extend(key.arguments().filter_map(|arg| match arg {
                        GenericArg::Type(ty) => Some(ty),
                        _ => None,
                    }));
                }
                Self::Function(function) => {
                    stack.extend(&function.value.parameters);
                    stack.push(&function.value.result);
                }
                Self::Tuple(fields) => stack.extend(fields),
                Self::Array(ty, _) | Self::Ref(_, _, ty) | Self::Slice(ty) => stack.push(ty),
                _ => {}
            }
        }
        Ok(())
    }
    /// Heap allocations only; the owner accounts for the inline term storage.
    pub fn heap_bytes(&self) -> Option<usize> {
        self.measure().map(|(_, bytes)| bytes)
    }
    pub(crate) fn measure(&self) -> Option<(usize, usize)> {
        let mut stack = vec![(self, 0usize)];
        let mut bytes = 0usize;
        let mut nodes = 0usize;
        while let Some((term, depth)) = stack.pop() {
            nodes = nodes.checked_add(1)?;
            if depth > 256 {
                return None;
            }
            // Capture keys own declaration metadata in addition to arguments.
            if let Self::Opaque(key, _) = term {
                bytes = bytes.checked_add(key.heap_bytes()?)?;
                nodes = nodes.checked_add(key.captures.len())?;
            }
            match term {
                Self::Projection(key) => {
                    bytes = bytes
                        .checked_add(std::mem::size_of::<ProjectionKey>())?
                        .checked_add(
                            key.trait_ref
                                .args
                                .capacity()
                                .checked_mul(std::mem::size_of::<GenericArg>())?,
                        )?
                        .checked_add(
                            key.item_args
                                .capacity()
                                .checked_mul(std::mem::size_of::<GenericArg>())?,
                        )?;
                    let arguments = key.trait_ref.args.len().checked_add(key.item_args.len())?;
                    // Both argument vectors and the trait's self type are part
                    // of the owned term. Check work before extending the stack.
                    if arguments.checked_add(1)? > 100_000usize.checked_sub(nodes)? {
                        return None;
                    }
                    nodes = nodes.checked_add(
                        key.arguments()
                            .filter(|a| !matches!(a, GenericArg::Type(_)))
                            .count(),
                    )?;
                    stack.push((&key.trait_ref.self_ty, depth + 1));
                    stack.extend(key.arguments().filter_map(|a| match a {
                        GenericArg::Type(t) => Some((t, depth + 1)),
                        _ => None,
                    }));
                }
                Self::Function(function) => {
                    bytes = bytes
                        .checked_add(std::mem::size_of::<Binder<FunctionSignature>>())?
                        .checked_add(
                            function
                                .variables
                                .capacity()
                                .checked_mul(std::mem::size_of::<BoundKind>())?,
                        )?
                        .checked_add(
                            function
                                .value
                                .parameters
                                .capacity()
                                .checked_mul(std::mem::size_of::<Self>())?,
                        )?
                        .checked_add(std::mem::size_of::<Self>())?;
                    nodes = nodes.checked_add(function.variables.len())?;
                    stack.extend(function.value.parameters.iter().map(|t| (t, depth + 1)));
                    stack.push((&function.value.result, depth + 1));
                }
                Self::Tuple(fields) => {
                    bytes = bytes
                        .checked_add(fields.capacity().checked_mul(std::mem::size_of::<Self>())?)?;
                    stack.extend(fields.iter().map(|t| (t, depth + 1)));
                }
                Self::Nominal(_, args) | Self::Opaque(_, args) => {
                    bytes = bytes.checked_add(
                        args.capacity()
                            .checked_mul(std::mem::size_of::<GenericArg>())?,
                    )?;
                    nodes = nodes.checked_add(
                        args.iter()
                            .filter(|arg| !matches!(arg, GenericArg::Type(_)))
                            .count(),
                    )?;
                    stack.extend(args.iter().filter_map(|a| {
                        if let GenericArg::Type(t) = a {
                            Some((t, depth + 1))
                        } else {
                            None
                        }
                    }));
                }
                Self::Array(t, _) | Self::Slice(t) | Self::Ref(_, _, t) => {
                    bytes = bytes.checked_add(std::mem::size_of::<Self>())?;
                    stack.push((t, depth + 1));
                }
                _ => {}
            }
        }
        Some((nodes, bytes))
    }
}

/// Exact source trait identity and typed arguments; never a selected witness.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct TraitRef {
    pub trait_key: Digest,
    pub self_ty: TyTerm,
    pub args: Vec<GenericArg>,
}
impl StableEncode for TraitRef {
    const DOMAIN: &'static str = "lrr/v2/symbolic-trait-ref";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.trait_key);
        e.record(&self.self_ty);
        e.sequence(&self.args, |e, arg| e.record(arg));
    }
}
/// A source-associated item identity and its separate use-site arguments.
/// This record is unnormalized: construction does not select an impl, certify
/// a dictionary, resolve a source name, or inspect a runtime descriptor.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ProjectionKey {
    pub trait_ref: TraitRef,
    pub item: Digest,
    pub item_args: Vec<GenericArg>,
}
impl ProjectionKey {
    pub(crate) fn arguments(&self) -> impl Iterator<Item = &GenericArg> {
        self.trait_ref.args.iter().chain(&self.item_args)
    }
}
impl StableEncode for ProjectionKey {
    const DOMAIN: &'static str = "lrr/v2/symbolic-projection";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.trait_ref);
        e.digest(self.item);
        e.sequence(&self.item_args, |e, arg| e.record(arg));
    }
}
/// Captured definition assumptions; reading this record performs no fulfillment.
/// Source/world/catalog admission and expression propagation follow in M018+.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParamEnv {
    definition: Digest,
    parameters: Vec<(BoundKind, ParamKey)>,
    predicates: Vec<Binder<TraitRef>>,
}
impl ParamEnv {
    pub fn new(
        definition: Digest,
        parameters: Vec<(BoundKind, ParamKey)>,
        predicates: Vec<Binder<TraitRef>>,
    ) -> Result<Self, crate::core::diagnostic::Diagnostic> {
        use crate::core::diagnostic::Diagnostic;
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "parameter environment exceeds metadata bounds".into(),
            )
        };
        let parameter_bytes = parameters
            .capacity()
            .checked_mul(std::mem::size_of::<(BoundKind, ParamKey)>())
            .ok_or_else(limit)?;
        let _ = parameter_bytes;
        let mut names = std::collections::BTreeSet::new();
        for key in &parameters {
            if !names.insert(*key) {
                return Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "duplicate typed definition parameter".into(),
                ));
            }
        }
        let mut bytes = parameters
            .capacity()
            .checked_mul(std::mem::size_of::<(BoundKind, ParamKey)>())
            .ok_or_else(limit)?
            .checked_add(
                predicates
                    .capacity()
                    .checked_mul(std::mem::size_of::<Binder<TraitRef>>())
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        for predicate in &predicates {
            bytes = bytes
                .checked_add(
                    predicate
                        .variables
                        .capacity()
                        .checked_mul(std::mem::size_of::<BoundKind>())
                        .ok_or_else(limit)?,
                )
                .ok_or_else(limit)?
                .checked_add(predicate.value.self_ty.heap_bytes().ok_or_else(limit)?)
                .ok_or_else(limit)?
                .checked_add(
                    predicate
                        .value
                        .args
                        .capacity()
                        .checked_mul(std::mem::size_of::<GenericArg>())
                        .ok_or_else(limit)?,
                )
                .ok_or_else(limit)?;
            for arg in &predicate.value.args {
                if let GenericArg::Type(ty) = arg {
                    bytes = bytes
                        .checked_add(ty.heap_bytes().ok_or_else(limit)?)
                        .ok_or_else(limit)?;
                }
            }
        }
        for predicate in &predicates {
            definition_parameters(&predicate.value.self_ty, &names)?;
            for arg in &predicate.value.args {
                definition_argument(arg, &names)?;
            }
        }
        // All clone sizes were bounded above before creating validation views.
        for predicate in &predicates {
            let view = TyTerm::Nominal(predicate.value.trait_key, {
                let mut args = Vec::new();
                args.try_reserve_exact(
                    predicate
                        .value
                        .args
                        .len()
                        .checked_add(1)
                        .ok_or_else(limit)?,
                )
                .map_err(|_| limit())?;
                args.push(GenericArg::Type(predicate.value.self_ty.clone()));
                args.extend_from_slice(&predicate.value.args);
                args
            });
            super::subst::validate_binder_type(&Binder {
                variables: predicate.variables.clone(),
                value: view,
            })?;
        }
        Ok(Self {
            definition,
            parameters,
            predicates,
        })
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn parameters(&self) -> &[(BoundKind, ParamKey)] {
        &self.parameters
    }
    pub fn predicates(&self) -> &[Binder<TraitRef>] {
        &self.predicates
    }
}
impl StableEncode for ParamEnv {
    const DOMAIN: &'static str = "lrr/v2/symbolic-parameter-environment";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.sequence(&self.parameters, |e, (kind, key)| {
            e.u32(kind.tag());
            e.record(key);
        });
        e.sequence(&self.predicates, |e, predicate| e.record(predicate));
    }
}

fn declared(
    kind: BoundKind,
    key: ParamKey,
    names: &std::collections::BTreeSet<(BoundKind, ParamKey)>,
) -> Result<(), crate::core::diagnostic::Diagnostic> {
    if names.contains(&(kind, key)) {
        Ok(())
    } else {
        Err(crate::core::diagnostic::Diagnostic::new(
            "LRR-E-GENERIC-BINDING",
            "unregistered definition parameter namespace".into(),
        ))
    }
}
fn definition_argument(
    arg: &GenericArg,
    names: &std::collections::BTreeSet<(BoundKind, ParamKey)>,
) -> Result<(), crate::core::diagnostic::Diagnostic> {
    match arg {
        GenericArg::Type(t) => definition_parameters(t, names),
        GenericArg::Region(RegionTerm::Param(key)) => declared(BoundKind::Region, *key, names),
        GenericArg::Const(ConstTerm::Param(key)) => declared(BoundKind::Const, *key, names),
        GenericArg::Region(RegionTerm::Placeholder(_))
        | GenericArg::Const(ConstTerm::Placeholder(_)) => {
            Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "transaction placeholder cannot become a definition assumption".into(),
            ))
        }
        _ => Ok(()),
    }
}
fn definition_parameters(
    term: &TyTerm,
    names: &std::collections::BTreeSet<(BoundKind, ParamKey)>,
) -> Result<(), crate::core::diagnostic::Diagnostic> {
    // Each caller checked term shape/depth and the combined byte limit first.
    match term {
        TyTerm::Infer(_) | TyTerm::Placeholder(_) => Err(crate::core::diagnostic::Diagnostic::new(
            "LRR-E-GENERIC-BINDING",
            "transaction term cannot become a definition assumption".into(),
        )),
        TyTerm::Param(key) => declared(BoundKind::Type, *key, names),
        TyTerm::Tuple(fields) => {
            for t in fields {
                definition_parameters(t, names)?;
            }
            Ok(())
        }
        TyTerm::Projection(key) => {
            definition_parameters(&key.trait_ref.self_ty, names)?;
            for arg in key.arguments() {
                definition_argument(arg, names)?;
            }
            Ok(())
        }
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => {
            for arg in args {
                definition_argument(arg, names)?;
            }
            Ok(())
        }
        TyTerm::Function(function) => {
            for t in &function.value.parameters {
                definition_parameters(t, names)?;
            }
            definition_parameters(&function.value.result, names)
        }
        TyTerm::Array(t, c) => {
            definition_argument(&GenericArg::Const(c.clone()), names)?;
            definition_parameters(t, names)
        }
        TyTerm::Ref(r, _, t) => {
            definition_argument(&GenericArg::Region(r.clone()), names)?;
            definition_parameters(t, names)
        }
        TyTerm::Slice(t) => definition_parameters(t, names),
        _ => Ok(()),
    }
}
