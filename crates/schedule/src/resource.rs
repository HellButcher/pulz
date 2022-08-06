use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::BTreeMap,
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use atomic_refcell::AtomicRefCell;
pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};

use crate::{
    module::Module,
    schedule::Schedule,
    system::param::{SystemParam, SystemParamFetch, SystemParamState},
};

#[repr(transparent)]
pub struct ResourceId<T = crate::Void>(usize, PhantomData<fn() -> T>);

impl<T> std::fmt::Debug for ResourceId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ComponentId").field(&self.0).finish()
    }
}
impl<T> Copy for ResourceId<T> {}
impl<T> Clone for ResourceId<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<T> Eq for ResourceId<T> {}
impl<T> Ord for ResourceId<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T> PartialEq<Self> for ResourceId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> PartialOrd<Self> for ResourceId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T> Hash for ResourceId<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T> ResourceId<T> {
    #[inline(always)]
    fn cast<X>(self) -> ResourceId<X> {
        ResourceId(self.0, PhantomData)
    }

    #[inline]
    pub fn untyped(self) -> ResourceId {
        self.cast()
    }
}

impl ResourceId {
    #[inline]
    pub fn typed<T>(self) -> ResourceId<T>
    where
        T: 'static,
    {
        self.cast()
    }
}

struct Resource {
    id: ResourceId,
    name: Cow<'static, str>,
    type_id: TypeId,
    is_send: bool,
    value: Option<AtomicRefCell<Box<dyn Any>>>,
}

unsafe impl Send for Resource {}
unsafe impl Sync for Resource {}

pub struct TakenRes<T> {
    id: ResourceId,
    value: Box<T>,
}
impl<T> TakenRes<T> {
    #[inline]
    pub fn id(&self) -> ResourceId<T> {
        self.id.cast()
    }

    #[inline]
    pub fn into_inner(self) -> T {
        *self.value
    }
}
impl<T> Deref for TakenRes<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> DerefMut for TakenRes<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

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
    fn remove<T>(&mut self) -> Option<TakenRes<T>>
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
        Some(TakenRes { id: self.id, value })
    }

    #[inline]
    fn insert_again<T>(&mut self, taken: TakenRes<T>)
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
    _unsend: PhantomData<NonNull<()>>,
}

impl Resources {
    #[inline]
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            by_type_id: BTreeMap::new(),
            _unsend: PhantomData,
        }
    }

    #[inline]
    pub fn id<T>(&self) -> Option<ResourceId<T>>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        self.by_type_id.get(&type_id).copied().map(ResourceId::cast)
    }

    #[inline(always)]
    pub fn as_send(&self) -> &ResourcesSend {
        // SAFETY: transmute is allowed because it is a newtype-struct with #[repr(transparent)].
        // Unsend -> Send is allowed, because it will restrict access to send-types
        unsafe { std::mem::transmute(self) }
    }

    fn get_resource<T>(&mut self) -> (ResourceId<T>, &mut Resource)
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let resources = &mut self.resources;
        let id = self
            .by_type_id
            .entry(type_id)
            .or_insert_with(|| {
                let id = ResourceId(resources.len(), PhantomData); // keep positive => dense
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
        res.value = Some(AtomicRefCell::new(Box::new(value)));
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
        T: Send + Sync + FromResources + 'static,
    {
        if let Some(id) = self.id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources(self);
            Ok(self.insert(value))
        }
    }

    #[inline]
    pub fn init<T>(&mut self) -> ResourceId<T>
    where
        T: Send + Sync + FromResources + 'static,
    {
        match self.try_init() {
            Ok(id) | Err(id) => id,
        }
    }

    pub fn try_init_unsend<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: FromResources + 'static,
    {
        if let Some(id) = self.id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources(self);
            Ok(self.insert_unsend(value))
        }
    }

    #[inline]
    pub fn init_unsend<T>(&mut self) -> ResourceId<T>
    where
        T: FromResources + 'static,
    {
        match self.try_init_unsend() {
            Ok(id) | Err(id) => id,
        }
    }

    #[inline]
    pub fn install<M>(&mut self, module: M) -> M::Output
    where
        M: Module,
    {
        let schedule_id = self.init_unsend::<Schedule>();
        let mut schedule = self.remove_id(schedule_id).unwrap();
        let result = module.install(self, &mut schedule);
        self.insert_again(schedule);
        result
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
        self.resources.get(resource_id.0).and_then(Resource::borrow)
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
        self.resources
            .get(resource_id.0)
            .and_then(Resource::borrow_mut)
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

    #[inline]
    pub fn remove<T>(&mut self) -> Option<TakenRes<T>>
    where
        T: 'static,
    {
        self.remove_id(self.id::<T>()?)
    }

    #[inline]
    pub fn remove_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<TakenRes<T>>
    where
        T: 'static,
    {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::remove)
    }

    pub fn insert_again<T>(&mut self, taken: TakenRes<T>)
    where
        T: 'static,
    {
        self.resources
            .get_mut(taken.id.0)
            .unwrap()
            .insert_again(taken)
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
        // SAFETY: transmute is allowed because it is a newtype-struct with #[repr(transparent)].
        // Send -> Unsend is unsafe (see doc)
        std::mem::transmute(self)
    }

    #[inline(always)]
    pub fn borrow_res<T>(&self) -> Option<Res<'_, T>>
    where
        T: Send + Sync + 'static,
    {
        self.0.borrow_res()
    }

    #[inline(always)]
    pub fn borrow_res_id<T>(&self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
    where
        T: Send + Sync + 'static,
    {
        self.0.borrow_res_id(resource_id)
    }

    #[inline(always)]
    pub fn borrow_res_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: Send + Sync + 'static,
    {
        self.0.borrow_res_mut()
    }

    #[inline(always)]
    pub fn borrow_res_mut_id<T>(&self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
    where
        T: Send + Sync + 'static,
    {
        self.0.borrow_res_mut_id(resource_id)
    }

    #[inline(always)]
    pub fn get_copy<T>(&self) -> Option<T>
    where
        T: Copy + Send + Sync + 'static,
    {
        self.0.get_copy()
    }

    #[inline(always)]
    pub fn get_copy_id<T>(&self, resource_id: ResourceId<T>) -> Option<T>
    where
        T: Copy + Send + Sync + 'static,
    {
        self.0.get_copy_id(resource_id)
    }

    #[inline(always)]
    pub fn get_mut<T>(&mut self) -> Option<&'_ mut T>
    where
        T: Send + Sync + 'static,
    {
        self.0.get_mut()
    }

    #[inline(always)]
    pub fn get_mut_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<&'_ mut T>
    where
        T: Send + Sync + 'static,
    {
        self.0.get_mut_id(resource_id)
    }
}

pub trait FromResources {
    fn from_resources(resources: &mut Resources) -> Self;
}

impl<T: Default> FromResources for T {
    #[inline]
    fn from_resources(_resources: &mut Resources) -> Self {
        T::default()
    }
}

#[doc(hidden)]
pub struct FetchRes<T>(ResourceId<T>);

unsafe impl<T> SystemParam for Res<'_, T>
where
    T: 'static,
{
    type Fetch = FetchRes<T>;
}

unsafe impl<T> SystemParamState for FetchRes<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>().expect("resource not registered"))
    }
}

unsafe impl<'r, T> SystemParamFetch<'r> for FetchRes<T>
where
    T: 'static,
{
    type Item = Res<'r, T>;
    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        resources.borrow_res_id(self.0).unwrap()
    }
}

#[doc(hidden)]
pub struct FetchResMut<T>(ResourceId<T>);

unsafe impl<T> SystemParam for ResMut<'_, T>
where
    T: 'static,
{
    type Fetch = FetchResMut<T>;
}

unsafe impl<T> SystemParamState for FetchResMut<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>().expect("resource not registered"))
    }
}

unsafe impl<'r, T> SystemParamFetch<'r> for FetchResMut<T>
where
    T: 'static,
{
    type Item = ResMut<'r, T>;
    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        resources.borrow_res_mut_id(self.0).unwrap()
    }
}

#[doc(hidden)]
pub struct FetchOptionRes<T>(Option<ResourceId<T>>);

unsafe impl<T> SystemParam for Option<Res<'_, T>>
where
    T: 'static,
{
    type Fetch = FetchOptionRes<T>;
}

unsafe impl<T> SystemParamState for FetchOptionRes<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }
}

unsafe impl<'r, T> SystemParamFetch<'r> for FetchOptionRes<T>
where
    T: 'static,
{
    type Item = Option<Res<'r, T>>;

    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        if let Some(resource_id) = self.0 {
            resources.borrow_res_id(resource_id)
        } else {
            None
        }
    }
}

#[doc(hidden)]
pub struct FetchOptionResMut<T>(Option<ResourceId<T>>);

unsafe impl<T> SystemParam for Option<ResMut<'_, T>>
where
    T: 'static,
{
    type Fetch = FetchOptionResMut<T>;
}

unsafe impl<T> SystemParamState for FetchOptionResMut<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }
}

unsafe impl<'r, T> SystemParamFetch<'r> for FetchOptionResMut<T>
where
    T: 'static,
{
    type Item = Option<ResMut<'r, T>>;

    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        if let Some(resource_id) = self.0 {
            resources.borrow_res_mut_id(resource_id)
        } else {
            None
        }
    }
}
