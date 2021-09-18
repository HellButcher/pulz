use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
};

use atomic_refcell::AtomicRefCell;
pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};

use crate::{
    system::param::{SystemParam, SystemParamFetch},
    World,
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct ResourceId(usize);

pub struct Resource {
    id: ResourceId,
    name: Cow<'static, str>,
    type_id: TypeId,
    value: Option<AtomicRefCell<Box<dyn Any + Send + Sync>>>,
}

impl Resource {
    #[inline]
    pub fn id(&self) -> ResourceId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

pub struct Resources {
    pub(crate) resources: Vec<Resource>,
    by_type_id: BTreeMap<TypeId, ResourceId>,
}

impl Resources {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            resources: Vec::new(),
            by_type_id: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn borrow<T>(&self) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        self.borrow_id(self.get_id::<T>()?)
    }

    #[inline]
    pub fn borrow_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        self.borrow_mut_id(self.get_id::<T>()?)
    }

    pub fn borrow_id<T>(&self, resource_id: ResourceId) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        if let Some(res) = self
            .resources
            .get(resource_id.0)
            .and_then(|v| v.value.as_ref())
        {
            Res::filter_map(res.borrow(), |v| v.downcast_ref::<T>())
        } else {
            None
        }
    }

    pub fn borrow_mut_id<T>(&self, resource_id: ResourceId) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        if let Some(res) = self
            .resources
            .get(resource_id.0)
            .and_then(|v| v.value.as_ref())
        {
            ResMut::filter_map(res.borrow_mut(), |v| v.downcast_mut::<T>())
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut<T>(&mut self) -> Option<&'_ mut T>
    where
        T: 'static,
    {
        self.get_mut_id(self.get_id::<T>()?)
    }

    pub fn get_mut_id<T>(&mut self, resource_id: ResourceId) -> Option<&'_ mut T>
    where
        T: 'static,
    {
        if let Some(res) = self
            .resources
            .get_mut(resource_id.0)
            .and_then(|v| v.value.as_mut())
        {
            res.get_mut().downcast_mut::<T>()
        } else {
            None
        }
    }

    #[inline]
    pub fn get_id<T>(&self) -> Option<ResourceId>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        self.by_type_id.get(&type_id).copied()
    }

    #[inline]
    fn get_or_create_id<T>(&mut self) -> ResourceId
    where
        T: Send + Sync + 'static,
    {
        match self.create_id::<T>() {
            Ok(id) | Err(id) => id,
        }
    }

    fn create_id<T>(&mut self) -> Result<ResourceId, ResourceId>
    where
        T: Send + Sync + 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let resources = &mut self.resources;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = resources.len();
                let id = ResourceId(index); // keep positive => dense
                resources.push(Resource {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    value: None,
                });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(entry) => Err(*entry.get()),
        }
    }

    pub fn insert<T>(&mut self, value: T) -> ResourceId
    where
        T: Send + Sync + 'static,
    {
        let id = self.get_or_create_id::<T>();
        // SAFETY: we created the id if not available
        unsafe { self.resources.get_unchecked_mut(id.0) }.value =
            Some(AtomicRefCell::new(Box::new(value)));
        id
    }
}

impl<'a, T> SystemParam for Res<'a, T>
where
    T: Send + Sync + 'static,
{
    type Prepared = ResourceId;
    type Fetch = Self;

    #[inline]
    fn prepare(world: &mut World) -> Self::Prepared {
        world
            .resources_mut()
            .get_id::<T>()
            .expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for Res<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = Res<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, world: &'a World) -> Self::Output {
        world.resources().borrow_id(*prepared).unwrap()
    }
}

impl<T> SystemParam for ResMut<'_, T>
where
    T: Send + Sync + 'static,
{
    type Prepared = ResourceId;
    type Fetch = Self;
    #[inline]
    fn prepare(world: &mut World) -> Self::Prepared {
        world
            .resources_mut()
            .get_id::<T>()
            .expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for ResMut<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = ResMut<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, world: &'a World) -> Self::Output {
        world.resources().borrow_mut_id(*prepared).unwrap()
    }
}

impl<T> SystemParam for Option<Res<'_, T>>
where
    T: Sync + 'static,
{
    type Prepared = Option<ResourceId>;
    type Fetch = Self;

    #[inline]
    fn prepare(world: &mut World) -> Self::Prepared {
        world.resources_mut().get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<Res<'_, T>>
where
    T: Sync + 'static,
{
    type Output = Option<Res<'a, T>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, world: &'a World) -> Self::Output {
        if let Some(prepared) = *prepared {
            world.resources().borrow_id(prepared)
        } else {
            None
        }
    }
}

impl<T> SystemParam for Option<ResMut<'_, T>>
where
    T: Send + 'static,
{
    type Prepared = Option<ResourceId>;
    type Fetch = Self;

    #[inline]
    fn prepare(world: &mut World) -> Self::Prepared {
        world.resources_mut().get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<ResMut<'_, T>>
where
    T: Send + 'static,
{
    type Output = Option<ResMut<'a, T>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, world: &'a World) -> Self::Output {
        if let Some(prepared) = *prepared {
            world.resources().borrow_mut_id(prepared)
        } else {
            None
        }
    }
}
