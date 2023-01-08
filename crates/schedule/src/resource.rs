use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use atomic_refcell::AtomicRefCell;
pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};
use pulz_bitset::BitSet;

use crate::system::param::{SystemParam, SystemParamState};

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
    pub(crate) modules: BTreeSet<TypeId>,
    _unsend: PhantomData<NonNull<()>>,
}

impl Resources {
    #[inline]
    pub fn new() -> Self {
        let mut res = Self {
            resources: Vec::new(),
            by_type_id: BTreeMap::new(),
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
        let type_id = std::any::TypeId::of::<T>();
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

pub struct ResourceAccess {
    pub(crate) shared: BitSet,
    pub(crate) exclusive: BitSet,
}

impl ResourceAccess {
    #[inline]
    pub fn new() -> Self {
        Self {
            shared: BitSet::new(),
            exclusive: BitSet::new(),
        }
    }
    #[inline]
    pub fn add_shared_checked<T>(&mut self, resource: ResourceId<T>) -> bool {
        self._add_shared_checked(resource.0)
    }
    fn _add_shared_checked(&mut self, index: usize) -> bool {
        if self.exclusive.contains(index) {
            panic!("resource {index} is already used as exclusive");
        }
        self.shared.insert(index)
    }
    #[inline]
    pub fn add_shared<T>(&mut self, resource: ResourceId<T>) -> bool {
        self.shared.insert(resource.0)
    }
    #[inline]
    pub fn add_exclusive_checked<T>(&mut self, resource: ResourceId<T>) -> bool {
        self._add_exclusive_checked(resource.0)
    }
    fn _add_exclusive_checked(&mut self, index: usize) -> bool {
        if self.shared.contains(index) {
            panic!("resource {index} is already used as exclusive");
        }
        self.exclusive.insert(index)
    }
    #[inline]
    pub fn add_exclusive<T>(&mut self, resource: ResourceId<T>) -> bool {
        self.exclusive.insert(resource.0)
    }
    #[inline]
    pub fn is_shared<T>(&self, resource: ResourceId<T>) -> bool {
        self.shared.contains(resource.0)
    }
    #[inline]
    pub fn is_exclusive<T>(&self, resource: ResourceId<T>) -> bool {
        self.shared.contains(resource.0)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.shared.clear();
        self.exclusive.clear();
    }
    #[inline]
    pub fn extend(&mut self, other: &Self) {
        self.shared.extend_bitset(&other.shared);
        self.exclusive.extend_bitset(&other.exclusive);
    }
    #[inline]
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.shared.is_disjoint(&other.exclusive)
            && self.exclusive.is_disjoint(&other.shared)
            && self.exclusive.is_disjoint(&other.exclusive)
    }
}

impl Default for ResourceAccess {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
pub struct FetchRes<T>(ResourceId<T>);

unsafe impl<T> SystemParam for Res<'_, T>
where
    T: 'static,
{
    type State = FetchRes<T>;
}

unsafe impl<T> SystemParamState for FetchRes<T>
where
    T: 'static,
{
    type Item<'r> = Res<'r, T>;

    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_shared_checked(self.0);
    }

    #[inline]
    fn fetch<'r>(&'r mut self, resources: &'r Resources) -> Self::Item<'r> {
        resources.borrow_res_id(self.0).unwrap()
    }
}

#[doc(hidden)]
pub struct FetchResMut<T>(ResourceId<T>);

unsafe impl<T> SystemParam for ResMut<'_, T>
where
    T: 'static,
{
    type State = FetchResMut<T>;
}

unsafe impl<T> SystemParamState for FetchResMut<T>
where
    T: 'static,
{
    type Item<'r> = ResMut<'r, T>;

    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_exclusive_checked(self.0);
    }

    #[inline]
    fn fetch<'r>(&'r mut self, resources: &'r Resources) -> Self::Item<'r> {
        resources.borrow_res_mut_id(self.0).unwrap()
    }
}

#[doc(hidden)]
pub struct FetchOptionRes<T>(Option<ResourceId<T>>);

unsafe impl<T> SystemParam for Option<Res<'_, T>>
where
    T: 'static,
{
    type State = FetchOptionRes<T>;
}

unsafe impl<T> SystemParamState for FetchOptionRes<T>
where
    T: 'static,
{
    type Item<'r> = Option<Res<'r, T>>;

    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        if let Some(resource) = self.0 {
            access.add_shared_checked(resource);
        }
    }

    #[inline]
    fn fetch<'r>(&'r mut self, resources: &'r Resources) -> Self::Item<'r> {
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
    type State = FetchOptionResMut<T>;
}

unsafe impl<T> SystemParamState for FetchOptionResMut<T>
where
    T: 'static,
{
    type Item<'r> = Option<ResMut<'r, T>>;

    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        if let Some(resource) = self.0 {
            access.add_exclusive_checked(resource);
        }
    }

    #[inline]
    fn fetch<'r>(&'r mut self, resources: &'r Resources) -> Self::Item<'r> {
        if let Some(resource_id) = self.0 {
            resources.borrow_res_mut_id(resource_id)
        } else {
            None
        }
    }
}
