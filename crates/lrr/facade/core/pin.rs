// `core::pin`.
pub struct Pin<Ptr> {
    pub __pointer: Ptr,
}

impl<Ptr: crate::ops::Deref> Pin<Ptr> {
    pub fn new(pointer: Ptr) -> Pin<Ptr> {
        Pin { __pointer: pointer }
    }
    pub unsafe fn new_unchecked(pointer: Ptr) -> Pin<Ptr> {
        Pin { __pointer: pointer }
    }
    pub fn as_ref(&self) -> Pin<&Ptr::Target> {
        Pin {
            __pointer: &*self.__pointer,
        }
    }
    pub fn into_inner(pin: Pin<Ptr>) -> Ptr {
        pin.__pointer
    }
}

impl<Ptr: crate::ops::Deref> crate::ops::Deref for Pin<Ptr> {
    type Target = Ptr::Target;
    fn deref(&self) -> &Ptr::Target {
        &*self.__pointer
    }
}

