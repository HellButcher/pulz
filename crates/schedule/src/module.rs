use crate::{resource::Resources, schedule::Schedule};

pub trait Module<'l>: Sized {
    type Output: 'l;

    fn install_into(self, resources: &'l mut Resources, schedule: &mut Schedule) -> Self::Output;

    #[inline]
    fn install(self, resources: &'l mut Resources) -> Self::Output {
        let resources_reborrow: *mut _ = resources;
        let schedule_id = resources.init_unsend::<Schedule>();
        let mut schedule = resources.remove_id(schedule_id).unwrap();
        let result = Self::install_into(self, resources, &mut schedule);
        // SAFETY: `insert_again` only touches the schedule resource., all other resources can be returned
        unsafe { (*resources_reborrow).insert_again(schedule) };
        result
    }
}

impl<'l, F, O: 'l> Module<'l> for F
where
    F: FnOnce(&mut Resources, &mut Schedule) -> O,
{
    type Output = O;

    fn install_into(self, resources: &mut Resources, schedule: &mut Schedule) -> O {
        self(resources, schedule)
    }
}

impl Resources {
    #[inline]
    pub fn install<'l, M>(&'l mut self, module: M) -> M::Output
    where
        M: Module<'l>,
    {
        module.install(self)
    }
}
