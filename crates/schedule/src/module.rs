use crate::{resource::Resources, schedule::Schedule};

pub trait Module {
    type Output;

    fn install(self, resources: &mut Resources, schedule: &mut Schedule) -> Self::Output;
}

impl<F, O> Module for F
where
    F: FnOnce(&mut Resources, &mut Schedule) -> O,
{
    type Output = O;

    fn install(self, resources: &mut Resources, schedule: &mut Schedule) -> O {
        self(resources, schedule)
    }
}
