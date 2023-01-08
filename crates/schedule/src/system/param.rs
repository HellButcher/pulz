use crate::resource::{ResourceAccess, Resources};

/// # Safety
/// The value ov IS_SEND must be correct: when it says true, then the type must be Send!
pub unsafe trait SystemParam: Sized {
    type State: SystemParamState;
}

/// # Safety
/// update_access should mark all used resources with ther usage.
pub unsafe trait SystemParamState: Sized + Send + Sync + 'static {
    type Item<'r>: SystemParam<State = Self>;

    fn init(resources: &mut Resources) -> Self;

    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess);

    fn fetch<'r>(&'r mut self, resources: &'r Resources) -> Self::Item<'r>;
}

pub type SystemParamItem<'r, P> = <<P as SystemParam>::State as SystemParamState>::Item<'r>;

macro_rules! impl_system_param {
    ([$($(($name:ident,$index:tt)),+)?]) => (

        unsafe impl$(<$($name),+>)? SystemParam for ($($($name,)+)?)
        $(
            where
                $($name : SystemParam),+
        )?
        {
            type State = ($($($name::State,)+)?) ;
        }

        unsafe impl$(<$($name),+>)? SystemParamState for ($($($name,)+)?)
        $(
            where
                $($name : SystemParamState),+
        )?
        {
            type Item<'r> =  ($($($name::Item<'r>,)+)?);

            #[inline]
            fn init(_resources: &mut Resources) -> Self {
                $(($($name::init(_resources),)+))?
            }

            #[inline]
            fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {
                $($(self.$index.update_access(_resources, _access);)+)?
            }

            #[inline]
            fn fetch<'r>(&'r mut self, _resources: &'r Resources) -> Self::Item<'r> {
                $(($(self.$index.fetch(_resources),)+))?
            }
        }

    )
}

pulz_functional_utils::generate_variadic_array! {[T,#] impl_system_param!{}}
