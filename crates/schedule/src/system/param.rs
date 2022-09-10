use crate::resource::{ResourceAccess, Resources};

/// # Safety
/// The value ov IS_SEND must be correct: when it says true, then the type must be Send!
pub unsafe trait SystemParam: Sized {
    type Fetch: for<'r> SystemParamFetch<'r>;
}

/// # Safety
/// update_access should mark all used resources with ther usage.
pub unsafe trait SystemParamState: Sized + Send + Sync {
    fn init(resources: &mut Resources) -> Self;

    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess);
}

/// # Safety
/// fetch should only access resources, marked in update_access
pub unsafe trait SystemParamFetch<'r>: SystemParamState {
    type Item: SystemParam<Fetch = Self>;
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item;
}

pub type SystemParamItem<'r, P> = <<P as SystemParam>::Fetch as SystemParamFetch<'r>>::Item;

macro_rules! impl_system_param {
    ([$($(($name:ident,$index:tt)),+)?]) => (

        unsafe impl$(<$($name),+>)? SystemParam for ($($($name,)+)?)
        $(
            where
                $($name : SystemParam),+
        )?
        {
            type Fetch = ($($($name::Fetch,)+)?) ;
        }

        unsafe impl$(<$($name),+>)? SystemParamState for ($($($name,)+)?)
        $(
            where
                $($name : SystemParamState),+
        )?
        {
            #[inline]
            fn init(_resources: &mut Resources) -> Self {
                $(($($name::init(_resources),)+))?
            }

            #[inline]
            fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {
                $($(self.$index.update_access(_resources, _access);)+)?
            }
        }

        unsafe impl<'r $($(,$name)+)?> SystemParamFetch<'r> for ($($($name,)+)?)
        $(
            where
                $($name : SystemParamFetch<'r>,)+
        )?
        {
            type Item =  ($($($name::Item,)+)?);
            #[inline]
            fn fetch(&'r mut self, _resources: &'r Resources) -> Self::Item {
                $(($(self.$index.fetch(_resources),)+))?
            }
        }

    )
}

pulz_functional_utils::generate_variadic_array! {[T,#] impl_system_param!{}}
