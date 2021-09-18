use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
};

use atomic_refcell::AtomicRefCell;
pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};

use crate::{
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
