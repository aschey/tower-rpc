use std::fmt::{Debug, Formatter};

use background_service::ServiceContext;

#[derive(Clone)]
pub struct Request<T> {
    pub context: ServiceContext,
    pub value: T,
}

impl<T> Debug for Request<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("value", &self.value)
            .finish()
    }
}
