use crate::resource::{ResourceAccess, Resources};

pub trait SystemParam: Sized {
    type State: SystemParamState;
    type Fetch<'r>: SystemParamFetch<'r, State = Self::State>;
    type Item<'a>;
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a>;
}

/// # Safety
/// update_access should mark all used resources with ther usage.
pub unsafe trait SystemParamState: Sized + Send + Sync + 'static {
    fn init(resources: &mut Resources) -> Self;

    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess);
}

/// # Safety
/// update_access should mark all used resources with ther usage.
pub trait SystemParamFetch<'r> {
    type State: SystemParamState;

    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self;
}

macro_rules! impl_system_param {
    ([$($(($name:ident,$index:tt)),+)?]) => (

        impl$(<$($name),+>)? SystemParam for ($($($name,)+)?)
        $(
            where
                $($name : SystemParam),+
        )?
        {
            type State = ($($($name::State,)+)?) ;
            type Fetch<'r> = ($($($name::Fetch<'r>,)+)?) ;
            type Item<'a> =  ($($($name::Item<'a>,)+)?);

            #[inline]
            fn get<'a>(_fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
                $(($($name::get(&mut _fetch.$index),)+))?
            }
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


        impl<'r $($(,$name)+)?> SystemParamFetch<'r> for ($($($name,)+)?)
        $(
            where
                $($name : SystemParamFetch<'r>),+
        )?
        {
            type State = ($($($name::State,)+)?) ;

            #[inline]
            fn fetch(_res: &'r Resources, _state: &'r mut Self::State) -> Self {
                $(($($name::fetch(_res, &mut _state.$index),)+))?
            }
        }
    )
}

pulz_functional_utils::generate_variadic_array! {[T,#] impl_system_param!{}}
