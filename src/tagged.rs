use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tagged<T> {
    pub(crate) tag: usize,
    pub(crate) value: T,
}

impl<T> Tagged<T> {
    pub fn tag(&self) -> usize {
        self.tag
    }

    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> From<T> for Tagged<T> {
    fn from(value: T) -> Self {
        Self { tag: 0, value }
    }
}

impl<T> Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Tagged<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
