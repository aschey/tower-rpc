use std::fmt::Debug;

#[cfg_attr(feature = "codec", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Tagged<T> {
    pub(crate) tag: usize,
    pub(crate) value: T,
}
