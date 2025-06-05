use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use atomic_refcell::AtomicRefCell;

use super::{FromResourcesMut, RemovedResource, Res, ResMut, ResourceId};

struct Resource {
    id: ResourceId,
    name: Cow<'static, str>,
    type_id: TypeId,
    is_send: bool,
    value: Option<AtomicRefCell<Box<dyn Any>>>,
}

unsafe impl Send for Resource {}
unsafe impl Sync for Resource {}

impl Resource {
    #[inline]
    fn new(id: ResourceId, type_id: TypeId, name: Cow<'static, str>) -> Self {
        Self {
            id,
            name,
            type_id,
            is_send: false,
            value: None,
        }
    }

    #[inline]
    fn borrow_any(&self) -> Option<Res<'_, dyn Any>> {
        Some(Res::map(self.value.as_ref()?.borrow(), Box::deref))
    }

    #[inline]
    fn borrow_any_mut(&self) -> Option<ResMut<'_, dyn Any>> {
        Some(ResMut::map(
            self.value.as_ref()?.borrow_mut(),
            Box::deref_mut,
        ))
    }

    #[inline]
    fn borrow<T>(&self) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        Res::filter_map(self.value.as_ref()?.borrow(), |v| v.downcast_ref::<T>())
    }

    #[inline]
    fn borrow_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        ResMut::filter_map(self.value.as_ref()?.borrow_mut(), |v| v.downcast_mut::<T>())
    }

    #[inline]
    fn get_copy<T>(&self) -> Option<T>
    where
        T: Copy + 'static,
    {
        self.borrow::<T>().map(|v| *v)
    }

    #[inline]
    fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        self.value.as_mut()?.get_mut().downcast_mut::<T>()
    }

    #[inline]
    fn get_any(&mut self) -> Option<&mut dyn Any> {
        Some(self.value.as_mut()?.get_mut().deref_mut())
    }

    #[inline]
    fn remove<T>(&mut self) -> Option<RemovedResource<T>>
    where
        T: 'static,
    {
        let value = match self.value.take()?.into_inner().downcast::<T>() {
            Ok(v) => v,
            Err(v) => {
                // put the value back into its place;
                self.value = Some(AtomicRefCell::new(v));
                return None;
            }
        };
        Some(RemovedResource { id: self.id, value })
    }

    #[inline]
    fn insert_again<T>(&mut self, taken: RemovedResource<T>)
    where
        T: 'static,
    {
        assert_eq!(self.id, taken.id, "resource id mismatch");
        assert!(self.value.is_none());
        self.value = Some(AtomicRefCell::new(taken.value));
    }
}

pub struct Resources {
    resources: Vec<Resource>,
    by_type_id: BTreeMap<TypeId, ResourceId>,
    pub(crate) meta_by_type_id: BTreeMap<TypeId, Box<dyn Any + Send + Sync>>,
    pub(crate) modules: BTreeSet<TypeId>,
    _unsend: PhantomData<NonNull<()>>,
}

impl Resources {
    #[inline]
    pub fn new() -> Self {
        let mut res = Self {
            resources: Vec::new(),
            by_type_id: BTreeMap::new(),
            meta_by_type_id: BTreeMap::new(),
            modules: BTreeSet::new(),
            _unsend: PhantomData,
        };
        res.init_unsend::<crate::schedule::Schedule>();
        res
    }

    #[inline]
    pub fn id<T>(&self) -> Option<ResourceId<T>>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        self.by_type_id.get(&type_id).copied().map(ResourceId::cast)
    }

    #[inline]
    pub fn expect_id<T>(&self) -> ResourceId<T>
    where
        T: 'static,
    {
        let Some(id) = self.id::<T>() else {
            panic!("resource {} not initialized", std::any::type_name::<T>());
        };
        id
    }

    #[inline]
    pub fn name<T>(&self, id: ResourceId<T>) -> Option<&str> {
        self.resources.get(id.0).map(|r| r.name.as_ref())
    }

    #[inline]
    pub fn type_id<T>(&self, id: ResourceId<T>) -> Option<TypeId> {
        self.resources.get(id.0).map(|r| r.type_id)
    }

    #[inline(always)]
    pub fn as_send(&self) -> &ResourcesSend {
        let self_ptr: *const Self = self;
        // SAFETY: cast is allowed because it is a newtype-struct with #[repr(transparent)].
        // Unsend -> Send is allowed, because it will restrict access to send-types
        unsafe { &*(self_ptr as *const ResourcesSend) }
    }

    fn get_resource<T>(&mut self) -> (ResourceId<T>, &mut Resource)
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let resources = &mut self.resources;
        let id = self
            .by_type_id
            .entry(type_id)
            .or_insert_with(|| {
                let id = ResourceId::new(resources.len()); // keep positive => dense
                let name = std::any::type_name::<T>();
                resources.push(Resource::new(id, type_id, Cow::Borrowed(name)));
                id
            })
            .cast();
        // SAFETY: we created the id if not available
        let res = unsafe { self.resources.get_unchecked_mut(id.0) };
        (id, res)
    }

    pub fn insert<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: Send + Sync + 'static,
    {
        let (id, res) = self.get_resource::<T>();
        res.is_send = true;
        let boxed: Box<dyn Any> = Box::new(value);
        res.value = Some(AtomicRefCell::new(boxed));
        id
    }

    pub fn insert_unsend<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: 'static,
    {
        let (id, res) = self.get_resource::<T>();
        res.is_send = false;
        res.value = Some(AtomicRefCell::new(Box::new(value)));
        id
    }

    pub fn try_init<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: Send + Sync + FromResourcesMut + 'static,
    {
        if let Some(id) = self.id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources_mut(self);
            Ok(self.insert(value))
        }
    }

    #[inline]
    pub fn init<T>(&mut self) -> ResourceId<T>
    where
        T: Send + Sync + FromResourcesMut + 'static,
    {
        match self.try_init() {
            Ok(id) | Err(id) => id,
        }
    }

    pub fn try_init_unsend<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: FromResourcesMut + 'static,
    {
        if let Some(id) = self.id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources_mut(self);
            Ok(self.insert_unsend(value))
        }
    }

    #[inline]
    pub fn init_unsend<T>(&mut self) -> ResourceId<T>
    where
        T: FromResourcesMut + 'static,
    {
        match self.try_init_unsend() {
            Ok(id) | Err(id) => id,
        }
    }

    #[inline]
    pub fn borrow_res<T>(&self) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        self.borrow_res_id(self.id::<T>()?)
    }

    pub fn borrow_res_id<T>(&self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        self.resources.get(resource_id.0)?.borrow()
    }

    pub fn borrow_res_meta<T>(&self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
    where
        T: ?Sized + 'static,
    {
        let r = self.resources.get(resource_id.0)?;
        let meta = self.get_meta::<T>()?;
        Res::filter_map(r.borrow_any()?, |v| meta.convert_ref(v))
    }

    pub fn borrow_res_any(&self, resource_id: ResourceId) -> Option<Res<'_, dyn Any>> {
        self.resources.get(resource_id.0)?.borrow_any()
    }

    #[inline]
    pub fn borrow_res_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        self.borrow_res_mut_id(self.id::<T>()?)
    }

    pub fn borrow_res_mut_id<T>(&self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        self.resources.get(resource_id.0)?.borrow_mut()
    }

    pub fn borrow_res_mut_meta<T>(&self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
    where
        T: ?Sized + 'static,
    {
        let r = self.resources.get(resource_id.0)?;
        let meta = self.get_meta::<T>()?;
        ResMut::filter_map(r.borrow_any_mut()?, |v| meta.convert_mut(v))
    }

    pub fn borrow_res_any_mut(&self, resource_id: ResourceId) -> Option<ResMut<'_, dyn Any>> {
        self.resources.get(resource_id.0)?.borrow_any_mut()
    }

    #[inline]
    pub fn get_copy<T>(&self) -> Option<T>
    where
        T: Copy + 'static,
    {
        self.get_copy_id(self.id::<T>()?)
    }

    pub fn get_copy_id<T>(&self, resource_id: ResourceId<T>) -> Option<T>
    where
        T: Copy + 'static,
    {
        self.resources
            .get(resource_id.0)
            .and_then(Resource::get_copy)
    }

    #[inline]
    pub fn get_mut<T>(&mut self) -> Option<&'_ mut T>
    where
        T: 'static,
    {
        self.get_mut_id(self.id::<T>()?)
    }

    pub fn get_mut_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<&'_ mut T>
    where
        T: 'static,
    {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::get_mut)
    }

    pub fn get_mut_any(&mut self, resource_id: ResourceId) -> Option<&'_ mut dyn Any> {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::get_any)
    }

    #[inline]
    pub fn remove<T>(&mut self) -> Option<RemovedResource<T>>
    where
        T: 'static,
    {
        self.remove_id(self.id::<T>()?)
    }

    #[inline]
    pub fn remove_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<RemovedResource<T>>
    where
        T: 'static,
    {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::remove)
    }

    pub fn insert_again<T>(&mut self, removed: RemovedResource<T>)
    where
        T: 'static,
    {
        self.resources
            .get_mut(removed.id.0)
            .unwrap()
            .insert_again(removed)
    }

    pub fn clear(&mut self) {
        self.resources.clear();
        self.by_type_id.clear();
        self.meta_by_type_id.clear();
        self.modules.clear();
    }
}

impl Default for Resources {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[repr(transparent)]
pub struct ResourcesSend(Resources);

// not send (souuld be dropped in original thread)
unsafe impl Sync for ResourcesSend {}

macro_rules! delegate_send {
    ($v:vis fn $name:ident <$T:ident $(: $($bounds:ident)+)?> ([$($mut:tt)*] self $(, $aname:ident: $atype: ty)*) $( ->  $rtype:ty )?) => {
        #[inline(always)]
        $v fn $name<$T>($($mut)* self $(, $aname: $atype)*) $( -> $rtype )?
            where $T: $( $($bounds + )+ )? Send + Sync + 'static
        {
            self.0.$name($($aname),*)
        }
    };
}

impl ResourcesSend {
    #[inline(always)]
    pub fn id<T>(&self) -> Option<ResourceId<T>>
    where
        T: 'static,
    {
        self.0.id()
    }

    /// # Safety
    /// User must ensure, that no UnSend Resources are send to an other thread.
    /// For example, it is not save, to add unsend items to resources, promote it
    /// a send-variant, send it to an other thread, promote it back to an unsend
    /// variant and acces the items there.
    #[inline(always)]
    pub unsafe fn as_unsend(&self) -> &Resources {
        let self_ptr: *const Self = self;
        // SAFETY: cast is allowed because it is a newtype-struct with #[repr(transparent)].
        // Send -> Unsend is unsafe (see doc)
        unsafe { &*(self_ptr as *const Resources) }
    }

    delegate_send!(pub fn borrow_res<T>([&]self) -> Option<Res<'_, T>>);
    delegate_send!(pub fn borrow_res_id<T>([&]self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>);
    delegate_send!(pub fn borrow_res_mut<T>([&]self) -> Option<ResMut<'_, T>>);
    delegate_send!(pub fn borrow_res_mut_id<T>([&]self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>);
    delegate_send!(pub fn get_copy<T: Copy>([&]self) -> Option<T>);
    delegate_send!(pub fn get_copy_id<T: Copy>([&]self, resource_id: ResourceId<T>) -> Option<T>);
    delegate_send!(pub fn get_mut<T: Copy>([&mut]self) -> Option<&'_ mut T>);
    delegate_send!(pub fn get_mut_id<T: Copy>([&mut]self, resource_id: ResourceId<T>) -> Option<&'_ mut T>);
}
