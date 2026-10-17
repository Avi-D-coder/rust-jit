// `core::borrow`.
pub trait Borrow<Borrowed: ?Sized> {
    fn borrow(&self) -> &Borrowed;
}
pub trait BorrowMut<Borrowed: ?Sized>: Borrow<Borrowed> {
    fn borrow_mut(&mut self) -> &mut Borrowed;
}
// rustc's `impl<T: ?Sized> Borrow<T> for T` is a blanket the method probe only
// meets where `Borrow` is in scope. The probe here does not model traits in
// scope, so a blanket `borrow` would shadow `RefCell::borrow` behind a smart
// pointer (`Rc<RefCell<T>>`): the reference forms are written out instead.
impl<'a, T: ?Sized> Borrow<T> for &'a T {
    fn borrow(&self) -> &T {
        *self
    }
}
impl<'a, T: ?Sized> Borrow<T> for &'a mut T {
    fn borrow(&self) -> &T {
        &**self
    }
}
