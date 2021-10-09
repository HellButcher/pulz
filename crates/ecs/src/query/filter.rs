use std::marker::PhantomData;

use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, ComponentSet, Components},
    get_or_init_component,
    query::{QueryBorrow, QueryFetch, QueryPrepare},
    resource::{Resources, ResourcesSend},
};

pub trait Filter {
    type Prepared: Send + Sync + Sized + Copy + 'static;
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared;

    /// Checks if the archetype matches the query
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool;
}

impl<T> Filter for &'_ T
where
    T: Component,
{
    type Prepared = ComponentId<T>;
    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        get_or_init_component::<T>(res, components).1
    }

    #[inline]
    fn matches_archetype(component_id: ComponentId<T>, archetype: &Archetype) -> bool {
        archetype.contains_component_id(component_id)
    }
}

impl<T> Filter for &'_ mut T
where
    T: Component,
{
    type Prepared = ComponentId<T>;
    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        get_or_init_component::<T>(res, components).1
    }

    #[inline]
    fn matches_archetype(component_id: ComponentId<T>, archetype: &Archetype) -> bool {
        archetype.contains_component_id(component_id)
    }
}

impl Filter for () {
    type Prepared = ();
    #[inline(always)]
    fn prepare(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn matches_archetype(_prepared: (), _archetype: &Archetype) -> bool {
        true
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Or<T>(pub T);

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

impl<$($name),+> Filter for ($($name,)+)
where
    $($name: Filter,)+
{
    type Prepared = ($($name::Prepared,)+);

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        ($($name::prepare(res, components),)+)
    }

    #[inline(always)]
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool {
        $($name::matches_archetype(prepared.$index, archetype))&&+
    }
}

impl<$($name),+> Filter for Or<($($name,)+)>
where
    $($name: Filter,)+
{
    type Prepared = ($($name::Prepared,)+);

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        ($($name::prepare(res, components),)+)
    }

    #[inline(always)]
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool {
        $($name::matches_archetype(prepared.$index, archetype))||+
    }
}

    peel! { tuple [] $($name.$index,)+ }
)
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }

pub struct Without<F, Q>(PhantomData<(Q, fn(F))>);

impl<F, Q> QueryPrepare for Without<F, Q>
where
    F: Filter,
    Q: QueryPrepare,
{
    type Prepared = (F::Prepared, Q::Prepared);
    type State = Q::State;
    type Borrow = Without<F, Q::Borrow>;

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        (F::prepare(res, components), Q::prepare(res, components))
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        Q::update_access(prepared.1, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool {
        !F::matches_archetype(prepared.0, archetype) && Q::matches_archetype(prepared.1, archetype)
    }

    #[inline(always)]
    fn state(prepared: Self::Prepared, archetype: &Archetype) -> Q::State {
        Q::state(prepared.1, archetype)
    }
}

impl<'w, F, Q> QueryBorrow<'w> for Without<F, Q>
where
    F: Filter,
    Q: QueryBorrow<'w>,
{
    type Borrowed = Q::Borrowed;
    type Fetch = Without<F, Q::Fetch>;

    #[inline(always)]
    fn borrow(res: &'w ResourcesSend, prepared: Self::Prepared) -> Self::Borrowed {
        Q::borrow(res, prepared.1)
    }
}

impl<'w, 'a, F, Q> QueryFetch<'w, 'a> for Without<F, Q>
where
    F: Filter,
    Q: QueryFetch<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(
        this: &'a mut Self::Borrowed,
        state: Q::State,
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        Q::get(this, state, archetype, index)
    }
}

pub struct With<F, Q>(PhantomData<(Q, fn(F))>);

impl<F, Q> QueryPrepare for With<F, Q>
where
    F: Filter,
    Q: QueryPrepare,
{
    type Prepared = (F::Prepared, Q::Prepared);
    type State = Q::State;
    type Borrow = With<F, Q::Borrow>;

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        (F::prepare(res, components), Q::prepare(res, components))
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        Q::update_access(prepared.1, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool {
        F::matches_archetype(prepared.0, archetype) && Q::matches_archetype(prepared.1, archetype)
    }

    #[inline(always)]
    fn state(prepared: Self::Prepared, archetype: &Archetype) -> Q::State {
        Q::state(prepared.1, archetype)
    }
}

impl<'w, F, Q> QueryBorrow<'w> for With<F, Q>
where
    F: Filter,
    Q: QueryBorrow<'w>,
{
    type Borrowed = Q::Borrowed;
    type Fetch = With<F, Q::Fetch>;

    #[inline(always)]
    fn borrow(res: &'w ResourcesSend, prepared: Self::Prepared) -> Self::Borrowed {
        Q::borrow(res, prepared.1)
    }
}

impl<'w, 'a, F, Q> QueryFetch<'w, 'a> for With<F, Q>
where
    F: Filter,
    Q: QueryFetch<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(
        this: &'a mut Self::Borrowed,
        state: Q::State,
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        Q::get(this, state, archetype, index)
    }
}
