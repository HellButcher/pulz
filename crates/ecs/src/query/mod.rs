use std::sync::{
    Mutex,
    atomic::{AtomicPtr, AtomicUsize, Ordering},
};

pub use self::exec::Query;
use crate::{
    WorldInner,
    archetype::{Archetype, ArchetypeId, ArchetypeSet},
    component::Components,
};

// mostly based on `hecs` (https://github.com/Ralith/hecs/blob/9a2405c703ea0eb6481ad00d55e74ddd226c1494/src/query.rs)

/// A collection of component types to fetch from a [`World`](crate::World)
pub trait QueryParam {
    /// The type of the data which can be cached to speed up retrieving
    /// the relevant type states from a matching [`Archetype`]
    type State: QueryParamState;
    type Fetch<'w>: QueryParamFetch<'w, State = Self::State>;
}

/// # Safety
/// update_access should mark all used resources with ther usage.
pub unsafe trait QueryParamState: Send + Sync + Sized + 'static {
    /// Looks up data that can be re-used between multiple query invocations
    fn init(resources: &Resources, components: &Components) -> Self;

    fn update_access(&self, access: &mut ResourceAccess);

    /// Checks if the archetype matches the query
    fn matches_archetype(&self, archetype: &Archetype) -> bool;
}

pub trait QueryParamFetch<'w>: Send {
    type State: QueryParamState;

    /// Type of value to be fetched
    type Item<'a>
    where
        Self: 'a;

    /// Acquire dynamic borrows from `archetype`
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self;

    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype);

    /// Access the given item in this archetype
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_>;
}

/// Type of values yielded by a query
pub type QueryItem<'w, 'a, Q> = <<Q as QueryParam>::Fetch<'w> as QueryParamFetch<'w>>::Item<'a>;

pub mod exec;
mod fetch;
mod filter;
pub use fetch::*;
pub use filter::*;
use pulz_schedule::resource::{
    FromResourcesMut, ResourceAccess, ResourceId, Resources, ResourcesSend,
};

struct QueryState<S>
where
    S: QueryParamState,
{
    world_resource_id: ResourceId<WorldInner>,
    param_state: S,

    last_archetype_index: AtomicUsize,
    updating_archetypes: Mutex<()>,
    matching_archetypes_p: AtomicPtr<ArchetypeSet>,
}

impl<S> QueryState<S>
where
    S: QueryParamState,
{
    pub fn new(resources: &mut Resources) -> Self {
        let world_id = resources.init::<WorldInner>();
        let world = resources.borrow_res_id(world_id).unwrap();
        Self::from_world(resources, &world, world_id)
    }

    fn from_world(
        resources: &Resources,
        world: &WorldInner,
        resource_id: ResourceId<WorldInner>,
    ) -> Self {
        let state = S::init(resources, &world.components);
        // TODO: detect if only sparse components are used, and handle this seperately

        let query = Self {
            world_resource_id: resource_id,
            param_state: state,
            last_archetype_index: AtomicUsize::new(0),
            updating_archetypes: Mutex::new(()),
            matching_archetypes_p: AtomicPtr::new(std::ptr::null_mut()),
        };
        query.update_archetypes(world);
        query
    }

    fn update_archetypes(&self, world: &WorldInner) {
        let archetypes = &world.archetypes;
        let last_archetype_index = archetypes.len();
        let old_archetype_index = self.last_archetype_index.load(Ordering::Relaxed);
        if old_archetype_index >= last_archetype_index {
            // no new archetypes
            return;
        }
        let lock = self.updating_archetypes.lock();

        let mut archetypes_scratch: Option<Box<ArchetypeSet>> = None;

        for index in old_archetype_index..last_archetype_index {
            let id = ArchetypeId::new(index);
            let archetype = &archetypes[id];
            if self.param_state.matches_archetype(archetype) {
                // init scratch
                let scratch = archetypes_scratch.get_or_insert_with(|| {
                    let ptr = self.matching_archetypes_p.load(Ordering::Relaxed);
                    if ptr.is_null() {
                        Default::default()
                    } else {
                        unsafe { Box::new((*ptr).clone()) }
                    }
                });
                // indert archetype
                scratch.insert(id);
            }
        }

        if let Some(new) = archetypes_scratch {
            // replace
            let old = self
                .matching_archetypes_p
                .swap(Box::into_raw(new), Ordering::Relaxed);
            if !old.is_null() {
                unsafe { drop(Box::from_raw(old)) }
            }
        }

        self.last_archetype_index
            .store(last_archetype_index, Ordering::Relaxed);

        drop(lock);
    }

    fn matching_archetypes(&self) -> &ArchetypeSet {
        static EMPTY: ArchetypeSet = ArchetypeSet::new();
        let ptr = self.matching_archetypes_p.load(Ordering::Relaxed);
        if ptr.is_null() {
            &EMPTY
        } else {
            unsafe { &*ptr }
        }
    }
}

impl<S: QueryParamState> FromResourcesMut for QueryState<S> {
    fn from_resources_mut(resources: &mut Resources) -> Self {
        Self::new(resources)
    }
}

#[cfg(test)]
mod test {

    use std::sync::{Arc, Mutex};

    use pulz_schedule::resource::Resources;

    use crate::{WorldExt, component::Component, prelude::Query};

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Component)]
    struct A(usize);

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Component)]
    #[component(storage = "crate::storage::DenseStorage")]
    struct B(usize);

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Component)]
    #[component(sparse)]
    struct C(usize);

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Component)]
    #[component(storage = "DenseStorage")] // shortcut for `pulz_ecs::storage::DenseStorage`
    struct D(usize);

    #[test]
    fn test_query() {
        let mut resources = Resources::new();
        let mut entities = Vec::new();
        {
            let mut world = resources.world_mut();
            for i in 0..1000 {
                let entity = match i % 4 {
                    1 => world.spawn().insert(A(i)).id(),
                    2 => world.spawn().insert(B(i)).id(),
                    _ => world.spawn().insert(A(i)).insert(B(i)).id(),
                };
                entities.push(entity);
            }
        }

        let mut q1 = Query::<&A>::new(&mut resources);
        //let r = q1.one(&world, entities[1]).map(|mut o| *o.get());
        let r = q1.get(entities[1]).copied();
        assert_eq!(Some(A(1)), r);

        let mut counter1 = 0;
        let mut sum1 = 0;
        for a in q1.iter() {
            counter1 += 1;
            sum1 += a.0;
        }

        assert_eq!(750, counter1);
        assert_eq!(374500, sum1);
        drop(q1);

        let mut q2 = Query::<(&A, &B)>::new(&mut resources);
        let mut counter2 = 0;
        let mut sum2a = 0;
        let mut sum2b = 0;
        for (a, b) in q2.iter() {
            counter2 += 1;
            sum2a += a.0;
            sum2b += b.0;
        }
        assert_eq!(500, counter2);
        assert_eq!(249750, sum2a);
        assert_eq!(249750, sum2b);
        drop(q2);

        let mut q3 = Query::<(&B,)>::new(&mut resources);
        let mut counter3 = 0;
        let mut sum3 = 0;
        for (b,) in q3.iter() {
            counter3 += 1;
            sum3 += b.0;
        }
        assert_eq!(750, counter3);
        assert_eq!(374750, sum3);
        drop(q3);

        let mut q1 = Query::<&A>::new(&mut resources);
        let mut counter4 = 0;
        let mut sum4 = 0;
        for a in q1.iter() {
            counter4 += 1;
            sum4 += a.0;
        }
        assert_eq!(750, counter4);
        assert_eq!(374500, sum4);
    }

    #[test]
    fn test_query_sys() {
        let mut resources = Resources::new();
        let mut entities = Vec::new();
        {
            let mut world = resources.world_mut();
            for i in 0..1000 {
                let entity = match i % 4 {
                    1 => world.spawn().insert(A(i)).id(),
                    2 => world.spawn().insert(B(i)).id(),
                    _ => world.spawn().insert(A(i)).insert(B(i)).id(),
                };
                entities.push(entity);
            }
        }

        let data = Arc::new(Mutex::new((0, 0, 0)));
        let data1 = data.clone();
        let f1 = move |mut q1: Query<'_, &A>| {
            let mut counter1 = 0;
            let mut sum1 = 0;
            for a in q1.iter() {
                counter1 += 1;
                sum1 += a.0;
            }
            *data1.lock().unwrap() = (counter1, sum1, 0);
        };
        resources.run(f1);
        let (counter1, sum1, _) = *data.lock().unwrap();
        assert_eq!(750, counter1);
        assert_eq!(374500, sum1);

        let data2 = data.clone();
        let f2 = move |mut q2: Query<'_, (&A, &B)>| {
            let mut counter2 = 0;
            let mut sum2a = 0;
            let mut sum2b = 0;
            for (a, b) in q2.iter() {
                counter2 += 1;
                sum2a += a.0;
                sum2b += b.0;
            }
            *data2.lock().unwrap() = (counter2, sum2a, sum2b);
        };
        resources.run(f2);
        let (counter2, sum2a, sum2b) = *data.lock().unwrap();
        assert_eq!(500, counter2);
        assert_eq!(249750, sum2a);
        assert_eq!(249750, sum2b);

        let data3 = data.clone();
        let f3 = move |mut q3: Query<'_, (&B,)>| {
            let mut counter3 = 0;
            let mut sum3 = 0;
            for (b,) in q3.iter() {
                counter3 += 1;
                sum3 += b.0;
            }
            *data3.lock().unwrap() = (counter3, sum3, 0);
        };
        resources.run(f3);
        let (counter3, sum3, _) = *data.lock().unwrap();
        assert_eq!(750, counter3);
        assert_eq!(374750, sum3);
    }
}
