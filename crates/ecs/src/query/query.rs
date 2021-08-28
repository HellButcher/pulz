use std::cell::{Ref, RefMut};

use crate::{
    archetype::{Archetype, ArchetypeId},
    component::{ComponentId, ComponentSet},
    storage::Storage,
    Entity, World,
};

pub trait Query {
    type Prepare: Copy;

    fn prepare(world: &mut World) -> Self::Prepare;
    fn update_access(
        prepared: Self::Prepare,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    );
    fn matches_archetype(prepared: Self::Prepare, archetype: &Archetype) -> bool;
}

pub trait QueryFetch<'w>: Query {
    type Fetch: 'w + for<'i> FetchGet<'i>;
    fn fetch(prepared: Self::Prepare, world: &'w World) -> Self::Fetch;
}

pub trait FetchGet<'l> {
    type Target: 'l;
    fn get(&'l mut self, archetype: &Archetype, index: usize) -> Self::Target;
}

impl<T> Query for &'_ T
where
    T: 'static,
{
    type Prepare = ComponentId;

    #[inline]
    fn prepare(world: &mut World) -> ComponentId {
        world.components_mut().get_or_insert_id::<T>()
    }

    #[inline]
    fn update_access(
        component_id: ComponentId,
        shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
        shared.insert(component_id)
    }

    #[inline]
    fn matches_archetype(component_id: ComponentId, archetype: &Archetype) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }
}

impl<'w, T> QueryFetch<'w> for &'_ T
where
    T: 'static,
{
    type Fetch = Ref<'w, Storage<T>>;
    #[inline]
    fn fetch(component_id: ComponentId, world: &'w World) -> Self::Fetch {
        world
            .storage
            .borrow(component_id)
            .expect("unable to borrow component")
    }
}

impl<'l, T> FetchGet<'l> for Ref<'_, Storage<T>>
where
    T: 'static,
{
    type Target = &'l T;
    fn get(&'l mut self, archetype: &Archetype, index: usize) -> Self::Target {
        let storage: &Storage<T> = self;
        storage
            .get(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl<T> Query for &'_ mut T
where
    T: 'static,
{
    type Prepare = ComponentId;

    #[inline]
    fn prepare(world: &mut World) -> ComponentId {
        world.components_mut().get_or_insert_id::<T>()
    }
    #[inline]
    fn update_access(
        component_id: ComponentId,
        _shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        exclusive.insert(component_id)
    }
    #[inline]
    fn matches_archetype(component_id: ComponentId, archetype: &Archetype) -> bool {
        component_id.is_sparse() || archetype.contains_component_id(component_id)
    }
}

impl<'w, T> QueryFetch<'w> for &'_ mut T
where
    T: 'static,
{
    type Fetch = RefMut<'w, Storage<T>>;

    #[inline]
    fn fetch(component_id: ComponentId, world: &'w World) -> Self::Fetch {
        world
            .storage
            .borrow_mut(component_id)
            .expect("unable to borrow mut component")
    }
}

impl<'l, T> FetchGet<'l> for RefMut<'_, Storage<T>>
where
    T: 'static,
{
    type Target = &'l mut T;
    fn get(&'l mut self, archetype: &Archetype, index: usize) -> Self::Target {
        let storage: &mut Storage<T> = self;
        storage
            .get_mut(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl Query for Entity {
    type Prepare = ();

    #[inline]
    fn prepare(_world: &mut World) {}

    #[inline]
    fn update_access(_prepared: (), _shared: &mut ComponentSet, _exclusive: &mut ComponentSet) {}

    #[inline]
    fn matches_archetype(_prepared: (), _archetype: &Archetype) -> bool {
        true
    }
}

impl QueryFetch<'_> for Entity {
    type Fetch = FetchEntity;

    #[inline]
    fn fetch(_prepared: Self::Prepare, _world: &World) -> FetchEntity {
        FetchEntity
    }
}

pub struct FetchEntity;

impl FetchGet<'_> for FetchEntity {
    type Target = Entity;
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Target {
        archetype.entities[index]
    }
}

impl<Q> Query for Option<Q>
where
    Q: Query,
{
    type Prepare = Q::Prepare;

    #[inline]
    fn prepare(world: &mut World) -> Q::Prepare {
        Q::prepare(world)
    }
    #[inline]
    fn update_access(
        prepared: Q::Prepare,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        Q::update_access(prepared, shared, exclusive)
    }

    #[inline]
    fn matches_archetype(_prepared: Q::Prepare, _archetype: &Archetype) -> bool {
        true
    }
}

impl<'w, Q> QueryFetch<'w> for Option<Q>
where
    Q: QueryFetch<'w> + 'w,
{
    type Fetch = OptionFetch<'w, Q>;

    #[inline]
    fn fetch(prepared: Self::Prepare, world: &'w World) -> Self::Fetch {
        OptionFetch {
            prepared,
            fetch: Q::fetch(prepared, world),
            check: None,
        }
    }
}

pub struct OptionFetch<'w, Q>
where
    Q: QueryFetch<'w>,
{
    prepared: Q::Prepare,
    fetch: Q::Fetch,
    check: Option<(ArchetypeId, bool)>,
}

impl<'w, 'l, Q> FetchGet<'l> for OptionFetch<'w, Q>
where
    Q: QueryFetch<'w>,
    Q::Fetch: FetchGet<'l>,
{
    type Target = Option<<Q::Fetch as FetchGet<'l>>::Target>;
    fn get(&'l mut self, archetype: &Archetype, index: usize) -> Self::Target {
        let value = match self.check {
            Some((id, value)) if id == archetype.id => value,
            _ => {
                let id = archetype.id;
                let value = Q::matches_archetype(self.prepared, archetype);
                self.check = Some((id, value));
                value
            }
        };
        if value {
            Some(self.fetch.get(archetype, index))
        } else {
            None
        }
    }
}

macro_rules! peel {
  ($macro:tt [$($args:tt)*] ) => ($macro! { $($args)* });
  ($macro:tt [$($args:tt)*] $name:ident.$index:tt, ) => ($macro! { $($args)* });
  ($macro:tt [$($args:tt)*] $name:ident.$index:tt, $($other:tt)+) => (peel!{ $macro [$($args)* $name.$index, ] $($other)+ } );
}

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

        impl<$($name),+> Query for ($($name,)+)
        where
            $($name : Query),+
        {
            type Prepare = ($($name::Prepare,)+) ;

            #[inline]
            fn prepare(world: &mut World) -> Self::Prepare {
                ($($name::prepare(world),)+)
            }
            #[inline]
            fn update_access(prepared: Self::Prepare, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
                $($name::update_access(prepared.$index, shared, exclusive);)+
            }

            #[inline]
            fn matches_archetype(prepared: Self::Prepare, archetype: &Archetype) -> bool {
                $($name::matches_archetype(prepared.$index, archetype))&&+
            }
        }

        impl<'w $(,$name)+> QueryFetch<'w> for ($($name,)+)
        where
            $($name : QueryFetch<'w>,)+
        {
            type Fetch = ($($name::Fetch,)+) ;

            #[inline]
            fn fetch(prepared: Self::Prepare, world: &'w World) -> Self::Fetch {
                ($($name::fetch(prepared.$index, world),)+)
            }
        }

        impl<'l $(,$name)+> FetchGet<'l>  for ($($name,)+)
        where
            $($name : FetchGet<'l>,)+
        {
            type Target = ($($name::Target,)+) ;
            #[inline]
            fn get(&'l mut self, archetype: &Archetype, index: usize) -> Self::Target {
                ($(self.$index.get(archetype,index),)+)
            }
        }

        peel! { tuple [] $($name.$index,)+ }
    )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }

impl Query for () {
    type Prepare = ();

    #[inline]
    fn prepare(_world: &mut World) -> Self::Prepare {
        ()
    }
    #[inline]
    fn update_access(
        _prepared: Self::Prepare,
        _shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
    }

    #[inline]
    fn matches_archetype(_prepared: Self::Prepare, _archetype: &Archetype) -> bool {
        true
    }
}

impl QueryFetch<'_> for () {
    type Fetch = ();

    #[inline]
    fn fetch(_prepared: Self::Prepare, _world: &World) -> Self {
        ()
    }
}

impl FetchGet<'_> for () {
    type Target = ();

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> () {
        ()
    }
}

// macro_rules! query_fn {
//     () => ();
//     //( $name:ident, ) => (); // ignore single argument fn
//     ( $($name:ident,)* ) => (

//         impl<R,F $(,$name)*> ItemFn<($($name,)*),R> for F
//         where
//             F: FnMut($($name),*) -> R,
//             $($name : Query,)*
//         {
//             fn call(&mut self, item: ($($name,)*)) -> R {
//                 #[allow(non_snake_case)]
//                 let ($($name,)*) = item;
//                 self($($name),*)
//             }
//         }

//         peel! { query_fn : $($name,)* }
//     )
// }

// query_fn! { T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, }

pub trait ItemFn<I, R> {
    fn call(&mut self, item: I) -> R;
}

impl<I, R, F> ItemFn<I, R> for F
where
    I: Query,
    F: FnMut(I) -> R,
{
    fn call(&mut self, item: I) -> R {
        self(item)
    }
}
