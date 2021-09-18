use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeSet},
    component::{ComponentId, ComponentSet},
    World,
};

use self::exec::Query;

// mostly based on `hecs` (https://github.com/Ralith/hecs/blob/9a2405c703ea0eb6481ad00d55e74ddd226c1494/src/query.rs)

/// A collection of component types to fetch from a [`World`](crate::World)
pub trait QueryPrepare {
    /// The type of the data which can be cached to speed up retrieving
    /// the relevant type states from a matching [`Archetype`]
    type Prepared: Send + Sync + Sized + Copy + 'static;

    type State: Sized + Copy + 'static;

    type Borrow: for<'w> QueryBorrow<'w, Prepared = Self::Prepared>;

    /// Looks up data that can be re-used between multiple query invocations
    fn prepare(world: &mut World) -> Self::Prepared;

    fn update_access(
        prepared: Self::Prepared,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    );

    /// Checks if the archetype matches the query
    fn matches_archetype(prepared: Self::Prepared, archetype: &Archetype) -> bool;

    fn state(prepared: Self::Prepared, archetype: &Archetype) -> Self::State;
}

pub trait QueryBorrow<'w>: QueryPrepare<Borrow = Self> {
    type Borrowed: Send;

    #[doc(hidden)]
    type Fetch: for<'a> QueryFetch<'w, 'a>;

    /// Acquire dynamic borrows from `archetype`
    fn borrow(world: &'w World, prepared: Self::Prepared) -> Self::Borrowed;
}

/// Type of values yielded by a query
pub type QueryItem<'w, 'a, Q> = <<Q as QueryBorrow<'w>>::Fetch as QueryFetch<'w, 'a>>::Item;

pub trait QueryFetch<'w, 'a>: QueryBorrow<'w, Fetch = Self> {
    /// Type of value to be fetched
    type Item;

    /// Access the given item in this archetype
    fn get(
        this: &'a mut Self::Borrowed,
        state: Self::State,
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a;
}

pub mod exec;
mod fetch;
mod filter;
pub use fetch::*;
pub use filter::*;

pub struct PreparedQuery<Q>
where
    Q: QueryPrepare,
{
    prepared: Q::Prepared,
    shared_access: ComponentSet,
    exclusive_access: ComponentSet,

    sparse_only: bool,
    last_archetype_index: usize,
    matching_archetypes: ArchetypeSet,
}

impl<Q> PreparedQuery<Q>
where
    Q: QueryPrepare,
{
    pub fn new(world: &mut World) -> Self {
        let prepared = Q::prepare(world);
        let mut shared_access = ComponentSet::new();
        let mut exclusive_access = ComponentSet::new();
        Q::update_access(prepared, &mut shared_access, &mut exclusive_access);

        let sparse_only = shared_access
            .iter(world.components())
            .chain(exclusive_access.iter(world.components()))
            .all(ComponentId::is_sparse);

        let mut query = Self {
            prepared,
            shared_access,
            exclusive_access,
            sparse_only,
            last_archetype_index: 0,
            matching_archetypes: ArchetypeSet::new(),
        };
        query.update_archetypes(world);
        query
    }

    pub fn update_archetypes(&mut self, world: &World) {
        let archetypes = world.archetypes();
        let last_archetype_index = archetypes.len();
        let old_archetype_index =
            std::mem::replace(&mut self.last_archetype_index, last_archetype_index);

        for index in old_archetype_index..last_archetype_index {
            let id = ArchetypeId::new(index);
            let archetype = &archetypes[id];
            if Q::matches_archetype(self.prepared, archetype) {
                self.matching_archetypes.insert(id);
            }
        }
    }
}

impl<Q> Clone for PreparedQuery<Q>
where
    Q: QueryPrepare,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            prepared: self.prepared.clone(),
            shared_access: self.shared_access.clone(),
            exclusive_access: self.exclusive_access.clone(),
            sparse_only: self.sparse_only,
            last_archetype_index: self.last_archetype_index,
            matching_archetypes: self.matching_archetypes.clone(),
        }
    }
}

impl<Q> PreparedQuery<Q>
where
    Q: QueryPrepare,
{
    #[inline]
    pub fn query<'w>(&'w mut self, world: &'w World) -> Query<'w, Q> {
        self.update_archetypes(world);
        Query::new_prepared(self, world)
    }
}

#[cfg(test)]
mod test {
    use crate::World;

    use super::PreparedQuery;

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct A(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct B(usize);

    #[test]
    fn test_query() {
        let mut world = World::new();
        let mut entities = Vec::new();
        for i in 0..1000 {
            let entity = match i % 4 {
                1 => world.spawn().insert(A(i)).id(),
                2 => world.spawn().insert(B(i)).id(),
                _ => world.spawn().insert(A(i)).insert(B(i)).id(),
            };
            entities.push(entity);
        }

        let mut q1 = PreparedQuery::<&A>::new(&mut world);
        //let r = q1.one(&world, entities[1]).map(|mut o| *o.get());
        let r = q1.query(&world).get(entities[1]).copied();
        assert_eq!(Some(A(1)), r);

        let mut counter1 = 0;
        let mut sum1 = 0;
        for a in q1.query(&world).iter() {
            counter1 += 1;
            sum1 += a.0;
        }
        assert_eq!(750, counter1);
        assert_eq!(374500, sum1);

        let mut q2 = PreparedQuery::<(&A, &B)>::new(&mut world);
        let mut counter2 = 0;
        let mut sum2a = 0;
        let mut sum2b = 0;
        for (a, b) in q2.query(&world).iter() {
            counter2 += 1;
            sum2a += a.0;
            sum2b += b.0;
        }
        assert_eq!(500, counter2);
        assert_eq!(249750, sum2a);
        assert_eq!(249750, sum2b);

        let mut q3 = PreparedQuery::<(&B,)>::new(&mut world);
        let mut counter3 = 0;
        let mut sum3 = 0;
        for (b,) in q3.query(&world).iter() {
            counter3 += 1;
            sum3 += b.0;
        }
        assert_eq!(750, counter3);
        assert_eq!(374750, sum3);

        let mut counter4 = 0;
        let mut sum4 = 0;
        for a in q1.query(&world).iter() {
            counter4 += 1;
            sum4 += a.0;
        }
        assert_eq!(750, counter4);
        assert_eq!(374500, sum4);
    }
}
