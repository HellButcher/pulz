use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, ComponentSet, Components},
    entity::Entity,
    get_or_init_component,
    query::{QueryBorrow, QueryFetch, QueryPrepare},
    resource::{Res, ResMut, ResourceId, Resources, ResourcesSend},
    storage::Storage,
};

impl<T: Component> QueryPrepare for &'_ T {
    type Prepared = (ResourceId<T::Storage>, ComponentId<T>);
    type State = ();
    type Borrow = Self;

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        get_or_init_component::<T>(res, components)
    }

    #[inline]
    fn update_access(
        (_storage_id, component_id): Self::Prepared,
        shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
        shared.insert(component_id);
    }

    #[inline]
    fn matches_archetype(
        (_storage_id, component_id): Self::Prepared,
        archetype: &Archetype,
    ) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl<'w, T: Component> QueryBorrow<'w> for &'_ T {
    type Borrowed = Res<'w, T::Storage>;
    type Fetch = Self;

    #[inline]
    fn borrow(
        res: &'w ResourcesSend,
        (storage_id, _component_id): Self::Prepared,
    ) -> Self::Borrowed {
        res.borrow_res_id(storage_id)
            .expect("unable to borrow component")
    }
}

impl<'w, 'a, T: Component> QueryFetch<'w, 'a> for &'_ T {
    type Item = &'a T;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        _state: (),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item {
        this.get(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl<T: Component> QueryPrepare for &'_ mut T {
    type Prepared = (ResourceId<T::Storage>, ComponentId<T>);
    type State = ();
    type Borrow = Self;

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        get_or_init_component::<T>(res, components)
    }

    #[inline]
    fn update_access(
        (_storage_id, component_id): Self::Prepared,
        _shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        exclusive.insert(component_id);
    }

    #[inline]
    fn matches_archetype(
        (_storage_id, component_id): Self::Prepared,
        archetype: &Archetype,
    ) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl<'w, T: Component> QueryBorrow<'w> for &'_ mut T {
    type Borrowed = ResMut<'w, T::Storage>;
    type Fetch = Self;

    #[inline]
    fn borrow(
        res: &'w ResourcesSend,
        (storage_id, _component_id): Self::Prepared,
    ) -> Self::Borrowed {
        res.borrow_res_mut_id(storage_id)
            .expect("unable to borrow mut component")
    }
}

impl<'w, 'a, T: Component> QueryFetch<'w, 'a> for &'_ mut T {
    type Item = &'a mut T;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        _state: (),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item {
        this.get_mut(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl QueryPrepare for Entity {
    type Prepared = ();
    type State = ();
    type Borrow = Self;

    #[inline(always)]
    fn prepare(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn update_access(_prepared: (), _shared: &mut ComponentSet, _exclusive: &mut ComponentSet) {}

    #[inline(always)]
    fn matches_archetype(_prepared: (), _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl QueryBorrow<'_> for Entity {
    type Borrowed = ();
    type Fetch = Self;

    #[inline(always)]
    fn borrow(_res: &ResourcesSend, _prepared: ()) {}
}

impl QueryFetch<'_, '_> for Entity {
    type Item = Self;

    #[inline(always)]
    fn get(_this: &mut Self::Borrowed, _state: (), archetype: &Archetype, index: usize) -> Self {
        archetype.entities[index]
    }
}

impl<Q> QueryPrepare for Option<Q>
where
    Q: QueryPrepare,
{
    type Prepared = Q::Prepared;
    type State = (bool, Q::State);
    type Borrow = Option<Q::Borrow>;

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        Q::prepare(res, components)
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        Q::update_access(prepared, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(_prepared: Self::Prepared, _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn state(prepared: Self::Prepared, archetype: &Archetype) -> (bool, Q::State) {
        (
            Q::matches_archetype(prepared, archetype),
            Q::state(prepared, archetype),
        )
    }
}

impl<'w, Q> QueryBorrow<'w> for Option<Q>
where
    Q: QueryBorrow<'w>,
{
    type Borrowed = Q::Borrowed;
    type Fetch = Option<Q::Fetch>;

    #[inline]
    fn borrow(res: &'w ResourcesSend, prepared: Self::Prepared) -> Self::Borrowed {
        Q::borrow(res, prepared)
    }
}

impl<'w, 'a, F> QueryFetch<'w, 'a> for Option<F>
where
    F: QueryFetch<'w, 'a>,
{
    type Item = Option<F::Item>;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        state: (bool, F::State),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        let (state, sub_state) = state;
        if state {
            Some(F::get(this, sub_state, archetype, index))
        } else {
            None
        }
    }
}

impl QueryPrepare for () {
    type Prepared = ();
    type State = ();
    type Borrow = Self;

    #[inline(always)]
    fn prepare(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn update_access(
        _prepared: Self::Prepared,
        _shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
    }

    #[inline(always)]
    fn matches_archetype(_prepared: Self::Prepared, _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl QueryBorrow<'_> for () {
    type Borrowed = ();
    type Fetch = ();

    #[inline(always)]
    fn borrow(_res: &ResourcesSend, _prepared: Self::Prepared) {}
}

impl QueryFetch<'_, '_> for () {
    type Item = ();

    #[inline(always)]
    fn get(_this: &mut Self::Borrowed, _state: (), _archetype: &Archetype, _index: usize) {}
}

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

impl<$($name),+> QueryPrepare for ($($name,)+)
where
    $($name: QueryPrepare,)+
{
    type Prepared = ($($name::Prepared,)+);
    type State = ($($name::State,)+);
    type Borrow = ($($name::Borrow,)+);

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::Prepared {
        ($($name::prepare(res, components),)+)
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        $($name::update_access(prepared.$index, shared, exclusive);)+
    }

    #[inline]
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool {
        $($name::matches_archetype(prepared.$index, archetype))&&+
    }

    #[inline]
    fn state(prepared: Self::Prepared, archetype: &Archetype) -> Self::State {
        ($($name::state(prepared.$index, archetype),)+)
    }
}

impl<'w, $($name),+> QueryBorrow<'w> for ($($name,)+)
where
    $($name: QueryBorrow<'w>,)+
{
    type Borrowed = ($($name::Borrowed,)+);
    type Fetch = ($($name::Fetch,)+);

    #[inline]
    fn borrow(res: &'w ResourcesSend, prepared: Self::Prepared) -> Self::Borrowed {
        ($($name::borrow(res, prepared.$index),)+)
    }
}

impl<'w, 'a, $($name),+> QueryFetch<'w, 'a> for ($($name,)+)
where
    $($name: QueryFetch<'w,'a>,)+
{
    type Item = ($($name::Item,)+);

    #[inline(always)]
    fn get(this: &'a mut Self::Borrowed, state: Self::State, archetype: &Archetype, index: usize) -> Self::Item where 'w: 'a {
        ($($name::get(&mut this.$index, state.$index, archetype, index),)+)
    }
}


        peel! { tuple [] $($name.$index,)+ }
    )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }
