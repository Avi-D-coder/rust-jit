// `core::any`.
pub trait Any {
    fn type_id(&self) -> TypeId;
}

pub struct TypeId {
    t: (u64, u64),
}

impl TypeId {
    pub fn of<T: ?Sized>() -> TypeId {
        core::intrinsics::lrr_unsupported("core::any::TypeId::of")
    }
}

pub fn type_name<T: ?Sized>() -> &'static str {
    core::intrinsics::lrr_unsupported("core::any::type_name")
}

pub fn type_name_of_val<T: ?Sized>(_val: &T) -> &'static str {
    core::intrinsics::lrr_unsupported("core::any::type_name_of_val")
}
