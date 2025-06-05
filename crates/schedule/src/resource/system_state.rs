use super::{Res, ResMut, ResourceAccess, ResourceId, Resources};
use crate::system::data::{SystemData, SystemDataFetch, SystemDataState};

#[doc(hidden)]
pub struct ResState<T>(pub ResourceId<T>);

impl<T> SystemData for &'_ T
where
    T: 'static,
{
    type State = ResState<T>;
    type Fetch<'r> = Res<'r, T>;
    type Item<'a> = &'a T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch
    }
}

unsafe impl<T> SystemDataState for ResState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_shared_checked(self.0);
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for Res<'r, T> {
    type State = ResState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        res.borrow_res_id(state.0).unwrap()
    }
}

#[doc(hidden)]
pub struct ResMutState<T>(pub ResourceId<T>);

impl<T> SystemData for &'_ mut T
where
    T: 'static,
{
    type State = ResMutState<T>;
    type Fetch<'r> = ResMut<'r, T>;
    type Item<'a> = &'a mut T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch
    }
}

unsafe impl<T> SystemDataState for ResMutState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_exclusive_checked(self.0);
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for ResMut<'r, T> {
    type State = ResMutState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        res.borrow_res_mut_id(state.0).unwrap()
    }
}

#[doc(hidden)]
pub struct OptionResState<T>(pub Option<ResourceId<T>>);

impl<T> SystemData for Option<&'_ T>
where
    T: 'static,
{
    type State = OptionResState<T>;
    type Fetch<'r> = Option<Res<'r, T>>;
    type Item<'a> = Option<&'a T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch.as_deref()
    }
}

unsafe impl<T> SystemDataState for OptionResState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        if let Some(resource) = self.0 {
            access.add_shared_checked(resource);
        }
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for Option<Res<'r, T>> {
    type State = OptionResState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        if let Some(resource_id) = state.0 {
            res.borrow_res_id(resource_id)
        } else {
            None
        }
    }
}

#[doc(hidden)]
pub struct OptionResMutState<T>(pub Option<ResourceId<T>>);

impl<T> SystemData for Option<&'_ mut T>
where
    T: 'static,
{
    type State = OptionResMutState<T>;
    type Fetch<'r> = Option<ResMut<'r, T>>;
    type Item<'a> = Option<&'a mut T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch.as_deref_mut()
    }
}

unsafe impl<T> SystemDataState for OptionResMutState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        if let Some(resource) = self.0 {
            access.add_exclusive_checked(resource);
        }
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for Option<ResMut<'r, T>> {
    type State = OptionResMutState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        if let Some(resource_id) = state.0 {
            res.borrow_res_mut_id(resource_id)
        } else {
            None
        }
    }
}
