use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tagged<T> {
    pub(crate) tag: usize,
    pub(crate) value: T,
}
