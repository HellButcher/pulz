use crate::resource::{ResourceAccess, Resources};

/// # Safety
/// The value ov IS_SEND must be correct: when it says true, then the type must be Send!
pub unsafe trait SystemParam: Sized {
    type Fetch: for<'r> SystemParamFetch<'r>;
}

pub unsafe trait SystemParamState: Send + Sync {
    fn init(resources: &mut Resources) -> Self;

    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess);
}

pub unsafe trait SystemParamFetch<'r>: SystemParamState {
    type Item: SystemParam<Fetch = Self>;
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item;
}

unsafe impl SystemParam for () {
    type Fetch = ();
}

unsafe impl SystemParamState for () {
    #[inline]
    fn init(_resources: &mut Resources) {}

    #[inline]
    fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
}

unsafe impl SystemParamFetch<'_> for () {
    type Item = ();
    #[inline]
    fn fetch(&mut self, _resources: &Resources) {}
}

pub type SystemParamItem<'r, P> = <<P as SystemParam>::Fetch as SystemParamFetch<'r>>::Item;

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

        unsafe impl<$($name),+> SystemParam for ($($name,)+)
        where
            $($name : SystemParam),+
        {
            type Fetch = ($($name::Fetch,)+) ;
        }

        unsafe impl<$($name),+> SystemParamState for ($($name,)+)
        where
            $($name : SystemParamState),+
        {
            #[inline]
            fn init(resources: &mut Resources) -> Self {
                ($($name::init(resources),)+)
            }

            #[inline]
            fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
                $(self.$index.update_access(resources, access);)+
            }
        }

        unsafe impl<'r $(,$name)+> SystemParamFetch<'r> for ($($name,)+)
        where
            $($name : SystemParamFetch<'r>,)+
        {
            type Item =  ($($name::Item,)+);
            #[inline]
            fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
                ($(self.$index.fetch(resources),)+)
            }
        }

        peel! { tuple [] $($name.$index,)+ }
    )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }
