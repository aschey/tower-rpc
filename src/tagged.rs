pub struct Tagged<T> {
    pub(crate) tag: usize,
    pub(crate) object: T,
}
