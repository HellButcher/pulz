use std::marker::PhantomData;

use crate::Entity;

// tracks removed components
pub struct RemovedComponents<C> {
    removed: Vec<Entity>,
    _phantom: PhantomData<fn(C)>,
}

impl<C> RemovedComponents<C> {
    #[inline]
    pub fn reset(&mut self) {
        self.removed.clear();
    }
}

impl<C> Default for RemovedComponents<C> {
    #[inline]
    fn default() -> Self {
        Self {
            removed: Vec::new(),
            _phantom: PhantomData,
        }
    }
}
