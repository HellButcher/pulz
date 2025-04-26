use pulz_ecs::prelude::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Component)]
pub struct Parent(pub Entity);

impl std::ops::Deref for Parent {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Parent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
