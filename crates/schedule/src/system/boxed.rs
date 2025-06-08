use std::fmt::Debug;

use super::{ExclusiveSystem, SendSystem, System};
use crate::{
    resource::{ResourceAccess, Resources},
    system::SystemInit,
};

pub enum BoxedSystem {
    Unsend(Box<dyn System>),
    Send(Box<dyn SendSystem>),
    Exclusive(Box<dyn ExclusiveSystem>),
}

impl BoxedSystem {
    pub fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        match self {
            Self::Unsend(system) => system.update_access(resources, access),
            Self::Send(system) => system.update_access(resources, access),
            Self::Exclusive(_) => {}
        }
    }

    #[inline]
    pub fn is_exclusive(&self) -> bool {
        matches!(self, Self::Exclusive(_))
    }
}

impl SystemInit for BoxedSystem {
    fn init(&mut self, resources: &mut Resources) {
        match self {
            Self::Unsend(system) => system.init(resources),
            Self::Send(system) => system.init(resources),
            Self::Exclusive(system) => system.init(resources),
        }
    }
    fn system_type_name(&self) -> &'static str {
        match self {
            Self::Unsend(system) => system.system_type_name(),
            Self::Send(system) => system.system_type_name(),
            Self::Exclusive(system) => system.system_type_name(),
        }
    }
    fn system_type_id(&self) -> std::any::TypeId {
        match self {
            Self::Unsend(system) => system.system_type_id(),
            Self::Send(system) => system.system_type_id(),
            Self::Exclusive(system) => system.system_type_id(),
        }
    }
}

impl ExclusiveSystem for BoxedSystem {
    fn run_exclusive(&mut self, resources: &mut Resources) {
        match self {
            Self::Unsend(system) => system.run_exclusive(resources),
            Self::Send(system) => system.run_exclusive(resources),
            Self::Exclusive(system) => system.run_exclusive(resources),
        }
    }
}

impl Debug for BoxedSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_tuple(match self {
            Self::Unsend(_) => "BoxedSystem::Unsend",
            Self::Send(_) => "BoxedSystem::Send",
            Self::Exclusive(_) => "BoxedSystem::Exclusive",
        });
        s.field(&self.system_type_name());
        s.finish()
    }
}
