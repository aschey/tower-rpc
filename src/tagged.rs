use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tagged<T> {
    pub(crate) tag: usize,
    pub(crate) object: T,
}

impl<T> From<T> for Tagged<T> {
    fn from(value: T) -> Self {
        Self {
            tag: 0,
            object: value,
        }
    }
}

impl<T> Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl<T> DerefMut for Tagged<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.object
    }
}
