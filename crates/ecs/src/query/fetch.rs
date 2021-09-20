use super::{QueryBorrow, QueryFetch, QueryPrepare};
use crate::archetype::Archetype;
use crate::world::{Ref, RefMut, WorldSend};
use crate::Entity;
use crate::{
    component::{ComponentId, ComponentSet},
    storage::Storage,
    World,
};

impl<T: Send + Sync + 'static> QueryPrepare for &'_ T {
    type Prepared = ComponentId<T>;
    type State = ();
    type Borrow = Self;

    #[inline]
    fn prepare(world: &mut WorldSend) -> ComponentId<T> {
        world.components_mut().get_or_insert_id::<T>()
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
        shared.insert(prepared);
    }

    #[inline]
    fn matches_archetype(component_id: ComponentId<T>, archetype: &Archetype) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl<'w, T: Send + Sync + 'static> QueryBorrow<'w> for &'_ T {
    type Borrowed = Ref<'w, Storage<T>>;
    type Fetch = Self;

    #[inline]
    fn borrow(world: &'w WorldSend, component_id: Self::Prepared) -> Self::Borrowed {
        world
            .storage()
            .borrow(component_id)
            .expect("unable to borrow component")
    }
}

impl<'w, 'a, T: Send + Sync + 'static> QueryFetch<'w, 'a> for &'_ T {
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

impl<T: Send + Sync + 'static> QueryPrepare for &'_ mut T {
    type Prepared = ComponentId<T>;
    type State = ();
    type Borrow = Self;

    #[inline]
    fn prepare(world: &mut WorldSend) -> ComponentId<T> {
        world.components_mut().get_or_insert_id::<T>()
    }

    #[inline]
    fn update_access(
        prepared: Self::Prepared,
        _shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        exclusive.insert(prepared);
    }

    #[inline]
    fn matches_archetype(component_id: ComponentId<T>, archetype: &Archetype) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }

    #[inline(always)]
    fn state(_prepared: Self::Prepared, _archetype: &Archetype) {}
}

impl<'w, T: Send + Sync + 'static> QueryBorrow<'w> for &'_ mut T {
    type Borrowed = RefMut<'w, Storage<T>>;
    type Fetch = Self;

    #[inline]
    fn borrow(world: &'w WorldSend, component_id: Self::Prepared) -> Self::Borrowed {
        world
            .storage()
            .borrow_mut(component_id)
            .expect("unable to borrow mut component")
    }
}

impl<'w, 'a, T: Send + Sync + 'static> QueryFetch<'w, 'a> for &'_ mut T {
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
    fn prepare(_world: &mut WorldSend) {}

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
    fn borrow(_world: &WorldSend, _prepared: ()) {}
}

impl QueryFetch<'_, '_> for Entity {
    type Item = Entity;

    #[inline(always)]
    fn get(_this: &mut Self::Borrowed, _state: (), archetype: &Archetype, index: usize) -> Entity {
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
    fn prepare(world: &mut WorldSend) -> Self::Prepared {
        Q::prepare(world)
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
    fn borrow(world: &'w WorldSend, prepared: Self::Prepared) -> Self::Borrowed {
        Q::borrow(world, prepared)
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
    fn prepare(_world: &mut WorldSend) {}

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
    fn borrow(_world: &WorldSend, _prepared: Self::Prepared) {}
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
    fn prepare(world: &mut WorldSend) -> Self::Prepared {
        ($($name::prepare(world),)+)
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
    fn borrow(world: &'w WorldSend, prepared: Self::Prepared) -> Self::Borrowed {
        ($($name::borrow(world, prepared.$index),)+)
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
