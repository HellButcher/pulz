use crate::{
    archetype::{ArchetypeId, ArchetypeSet},
    component::{ComponentId, ComponentSet},
    Entity, World,
};

mod exec;
mod one;
mod traits;

use self::exec::{QueryExecution, QueryIter};
pub use self::traits::*;

pub struct PreparedQuery<Q>
where
    Q: Query,
{
    prepared: Q::Prepare,
    shared_access: ComponentSet,
    exclusive_access: ComponentSet,

    sparse_only: bool,
    last_archetype_index: usize,
    matching_archetypes: ArchetypeSet,
}

impl<Q> PreparedQuery<Q>
where
    Q: Query,
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
        let archetypes = &world.archetypes;
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

impl<'w, Q> PreparedQuery<Q>
where
    Q: Query + QueryFetch<'w>,
{
    #[inline]
    pub fn one(&'w mut self, world: &'w World, entity: Entity) -> Option<self::one::One<'w, Q>> {
        self.update_archetypes(world);
        let location = *world.entities.get(entity)?;
        Some(self::one::One::new(self, world, location))
    }

    #[inline]
    pub fn exec(&mut self, world: &'w World) -> QueryExecution<'w, '_, Q> {
        self.update_archetypes(world);
        QueryExecution::new(self, world)
    }

    #[inline]
    pub fn iter(&mut self, world: &'w World) -> QueryIter<'w, '_, '_, Q> {
        self.update_archetypes(world);
        QueryExecution::new(self, world).into_iter()
    }

    pub fn get_with<G, R>(&mut self, world: &'w World, entity: Entity, mut apply: G) -> Option<R>
    where
        G: for<'l> ItemFn<<Q::Fetch as FetchGet<'l>>::Target, R>,
    {
        self.update_archetypes(world);
        let location = world.entities.get(entity)?;
        if !self.matching_archetypes.contains(location.archetype_id) {
            return None;
        }
        let mut fetched = Q::fetch(self.prepared, world);
        let archetype = &world.archetypes[location.archetype_id];
        // TODO: filter?
        let item = fetched.get(archetype, location.index);
        let result = apply.call(item);
        Some(result)
    }

    #[inline]
    pub fn for_each<G>(&mut self, world: &'w World, mut apply: G)
    where
        G: for<'l> ItemFn<<Q::Fetch as FetchGet<'l>>::Target, ()>,
    {
        self.update_archetypes(world);
        if self.sparse_only {
            unimplemented!();
        } else {
            let mut fetched = Q::fetch(self.prepared, world);
            let archetypes = &world.archetypes;
            for archetype_id in self.matching_archetypes.iter() {
                let archetype = &archetypes[archetype_id];
                for i in 0..archetype.len() {
                    // TODO: filter
                    apply.call(fetched.get(archetype, i));
                }
            }
        }
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
        let r = q1.one(&world, entities[1]).map(|mut v| *v.get());
        assert_eq!(Some(A(1)), r);

        let mut counter1 = 0;
        let mut sum1 = 0;
        q1.for_each(&world, |a: &A| {
            counter1 += 1;
            sum1 += a.0;
        });
        assert_eq!(750, counter1);
        assert_eq!(374500, sum1);

        let mut q2 = PreparedQuery::<(&A, &B)>::new(&mut world);
        let mut counter2 = 0;
        let mut sum2a = 0;
        let mut sum2b = 0;
        q2.for_each(&world, |(a, b): (&A, &B)| {
            counter2 += 1;
            sum2a += a.0;
            sum2b += b.0;
        });
        assert_eq!(500, counter2);
        assert_eq!(249750, sum2a);
        assert_eq!(249750, sum2b);

        let mut q3 = PreparedQuery::<(&B,)>::new(&mut world);
        let mut counter3 = 0;
        let mut sum3 = 0;
        q3.for_each(&world, |(b,): (&B,)| {
            counter3 += 1;
            sum3 += b.0;
        });
        assert_eq!(750, counter3);
        assert_eq!(374750, sum3);

        let mut counter4 = 0;
        let mut sum4 = 0;
        for a in q1.exec(&world).into_iter() {
            counter4 += 1;
            sum4 += a.0;
        }
        assert_eq!(750, counter4);
        assert_eq!(374500, sum4);
    }
}
