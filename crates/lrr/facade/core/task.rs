// `core::task`.
pub enum Poll<T> {
    Ready(T),
    Pending,
}

pub struct Context<'a> {
    waker: &'a Waker,
}

pub struct Waker {
    data: *const (),
}

pub struct RawWaker {
    data: *const (),
}
