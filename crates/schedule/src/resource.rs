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

use crate::system::data::{SystemData, SystemDataFetch, SystemDataState};

#[repr(transparent)]
pub struct ResourceId<T: ?Sized = crate::Void>(usize, PhantomData<fn(&T)>);

impl<T: ?Sized> std::fmt::Debug for ResourceId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ComponentId").field(&self.0).finish()
    }
}
impl<T: ?Sized> Copy for ResourceId<T> {}
impl<T: ?Sized> Clone for ResourceId<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Eq for ResourceId<T> {}
impl<T: ?Sized> Ord for ResourceId<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T: ?Sized> PartialEq<Self> for ResourceId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: ?Sized> PartialOrd<Self> for ResourceId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T: ?Sized> Hash for ResourceId<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T: ?Sized> ResourceId<T> {
    #[inline(always)]
    fn cast<X: ?Sized>(self) -> ResourceId<X> {
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
        T: ?Sized + 'static,
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

pub struct RemovedResource<T> {
    id: ResourceId,
    value: Box<T>,
}
impl<T> RemovedResource<T> {
    #[inline]
    pub fn id(&self) -> ResourceId<T> {
        self.id.cast()
    }

    #[inline]
    pub fn into_inner(self) -> T {
        *self.value
    }
}
impl<T> Deref for RemovedResource<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> DerefMut for RemovedResource<T> {
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

pub trait FromResources {
    fn from_resources(resources: &Resources) -> Self;
}
pub trait FromResourcesMut {
    fn from_resources_mut(resources: &mut Resources) -> Self;
}

impl<T: Default> FromResources for T {
    #[inline]
    fn from_resources(_resources: &Resources) -> Self {
        T::default()
    }
}

impl<T: FromResources> FromResourcesMut for T {
    #[inline]
    fn from_resources_mut(resources: &mut Resources) -> Self {
        T::from_resources(resources)
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
pub struct ResState<T>(pub ResourceId<T>);

impl<T> SystemData for &'_ T
where
    T: 'static,
{
    type State = ResState<T>;
    type Fetch<'r> = Res<'r, T>;
    type Item<'a> = &'a T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch
    }
}

unsafe impl<T> SystemDataState for ResState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_shared_checked(self.0);
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for Res<'r, T> {
    type State = ResState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        res.borrow_res_id(state.0).unwrap()
    }
}

#[doc(hidden)]
pub struct ResMutState<T>(pub ResourceId<T>);

impl<T> SystemData for &'_ mut T
where
    T: 'static,
{
    type State = ResMutState<T>;
    type Fetch<'r> = ResMut<'r, T>;
    type Item<'a> = &'a mut T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch
    }
}

unsafe impl<T> SystemDataState for ResMutState<T>
where
    T: 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<T>())
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_exclusive_checked(self.0);
    }
}

impl<'r, T: 'static> SystemDataFetch<'r> for ResMut<'r, T> {
    type State = ResMutState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        res.borrow_res_mut_id(state.0).unwrap()
    }
}

#[doc(hidden)]
pub struct OptionResState<T>(pub Option<ResourceId<T>>);

impl<T> SystemData for Option<&'_ T>
where
    T: 'static,
{
    type State = OptionResState<T>;
    type Fetch<'r> = Option<Res<'r, T>>;
    type Item<'a> = Option<&'a T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch.as_deref()
    }
}

unsafe impl<T> SystemDataState for OptionResState<T>
where
    T: 'static,
{
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
}

impl<'r, T: 'static> SystemDataFetch<'r> for Option<Res<'r, T>> {
    type State = OptionResState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        if let Some(resource_id) = state.0 {
            res.borrow_res_id(resource_id)
        } else {
            None
        }
    }
}

#[doc(hidden)]
pub struct OptionResMutState<T>(pub Option<ResourceId<T>>);

impl<T> SystemData for Option<&'_ mut T>
where
    T: 'static,
{
    type State = OptionResMutState<T>;
    type Fetch<'r> = Option<ResMut<'r, T>>;
    type Item<'a> = Option<&'a mut T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        fetch.as_deref_mut()
    }
}

unsafe impl<T> SystemDataState for OptionResMutState<T>
where
    T: 'static,
{
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
}

impl<'r, T: 'static> SystemDataFetch<'r> for Option<ResMut<'r, T>> {
    type State = OptionResMutState<T>;

    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        if let Some(resource_id) = state.0 {
            res.borrow_res_mut_id(resource_id)
        } else {
            None
        }
    }
}
