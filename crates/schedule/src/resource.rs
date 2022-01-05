use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use atomic_refcell::AtomicRefCell;
pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};

use crate::system::param::{SystemParam, SystemParamFetch};

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

#[doc(hidden)]
pub struct SendMarker();
#[doc(hidden)]
pub struct UnsendMarker(PhantomData<NonNull<()>>);

pub struct Resource<Marker> {
    id: ResourceId,
    name: Cow<'static, str>,
    type_id: TypeId,
    is_send: bool,
    value: Option<AtomicRefCell<Box<dyn Any>>>,
    _marker: PhantomData<Marker>,
}

unsafe impl Send for Resource<SendMarker> {}
unsafe impl Sync for Resource<SendMarker> {}

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

impl<Marker> Resource<Marker> {
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

pub type ResourcesSend = BaseResources<SendMarker>;
pub type Resources = BaseResources<UnsendMarker>;

pub struct BaseResources<Marker> {
    pub(crate) resources: Vec<Resource<Marker>>,
    by_type_id: BTreeMap<TypeId, ResourceId>,
}

impl<Marker> BaseResources<Marker> {
    #[inline]
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            by_type_id: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn get_id<T>(&self) -> Option<ResourceId<T>>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        self.by_type_id.get(&type_id).copied().map(ResourceId::cast)
    }

    #[inline]
    fn get_or_create_id<T>(&mut self) -> ResourceId<T>
    where
        T: 'static,
    {
        match self.create_id::<T>() {
            Ok(id) | Err(id) => id,
        }
    }

    fn create_id<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let resources = &mut self.resources;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = resources.len();
                let id = ResourceId(index, PhantomData); // keep positive => dense
                resources.push(Resource::<Marker> {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    is_send: true,
                    value: None,
                    _marker: PhantomData,
                });
                entry.insert(id);
                Ok(id.cast())
            }
            Entry::Occupied(entry) => Err((*entry.get()).cast()),
        }
    }

    pub fn insert<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: Send + Sync + 'static,
    {
        let id = self.get_or_create_id::<T>();
        // SAFETY: we created the id if not available
        let res = unsafe { self.resources.get_unchecked_mut(id.0) };
        res.is_send = true;
        res.value = Some(AtomicRefCell::new(Box::new(value)));
        id
    }

    #[inline]
    pub fn as_send(&self) -> &ResourcesSend {
        // SAFETY: same type but different Phantom-Data.
        // Unsend -> Send is allowed, because it will restrict access-methods even more (to only accept send+sync types)
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn as_send_mut(&mut self) -> &mut ResourcesSend {
        // SAFETY: same type but different Phantom-Data.
        // Unsend -> Send is allowed, because it will restrict access-methods even more (to only accept send+sync types)
        unsafe { std::mem::transmute(self) }
    }

    /// # Safety
    /// User must ensure, that no UnSend Resources are send to a other thread
    #[inline]
    pub unsafe fn as_unsend(&self) -> &Resources {
        // SAFETY: same type but different Phantom-Data.
        // Send -> Unsend is unsafe (see doc)
        std::mem::transmute(self)
    }
}

impl Resources {
    pub fn insert_unsend<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: 'static,
    {
        let id = self.get_or_create_id::<T>();
        // SAFETY: we created the id if not available
        let res = unsafe { self.resources.get_unchecked_mut(id.0) };
        res.is_send = false;
        res.value = Some(AtomicRefCell::new(Box::new(value)));
        id
    }

    pub fn try_init<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: 'static + Send + Sync + FromResources,
    {
        if let Some(id) = self.get_id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources(self);
            Ok(self.insert(value))
        }
    }

    #[inline]
    pub fn init<T>(&mut self) -> ResourceId<T>
    where
        T: 'static + Send + Sync + FromResources,
    {
        match self.try_init() {
            Ok(id) | Err(id) => id,
        }
    }

    pub fn try_init_unsend<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: 'static + FromResources,
    {
        if let Some(id) = self.get_id::<T>() {
            Err(id)
        } else {
            let value = T::from_resources(self);
            Ok(self.insert_unsend(value))
        }
    }

    #[inline]
    pub fn init_unsend<T>(&mut self) -> ResourceId<T>
    where
        T: 'static + FromResources,
    {
        match self.try_init_unsend() {
            Ok(id) | Err(id) => id,
        }
    }
}

macro_rules! impl_send_unsend {
    ($Marker:ident : $($bound:tt)+) => {

impl Resource<$Marker> {
    #[inline]
    fn borrow<T>(&self) -> Option<Res<'_, T>>
    where
        T: $($bound)+,
    {
        Res::filter_map(self.value.as_ref()?.borrow(), |v| v.downcast_ref::<T>())
    }

    #[inline]
    fn borrow_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: $($bound)+,
    {
        ResMut::filter_map(self.value.as_ref()?.borrow_mut(), |v| v.downcast_mut::<T>())
    }

    #[inline]
    fn get<T>(&self) -> Option<T>
    where
        T: Copy + $($bound)+,
    {
        self.borrow::<T>().map(|v| *v)
    }

    #[inline]
    fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: $($bound)+,
    {
        self.value.as_mut()?.get_mut().downcast_mut::<T>()
    }

    #[inline]
    fn remove<T>(&mut self) -> Option<TakenRes<T>>
    where
        T: $($bound)+,
    {
        let value = match self.value.take()?.into_inner().downcast::<T>() {
            Ok(v) => v,
            Err(v) => {
                // put the value back into its place;
                self.value = Some(AtomicRefCell::new(v));
                return None;
            }
        };
        Some(TakenRes{
            id: self.id,
            value,
        })
    }

    fn insert_again<T>(&mut self, taken: TakenRes<T>)
        where
        T: $($bound)+,
    {
        assert_eq!(self.id, taken.id, "resource id mismatch");
        assert!(self.value.is_none());
        self.value = Some(AtomicRefCell::new(taken.value));
    }
}

impl BaseResources<$Marker> {
    #[inline]
    pub fn borrow_res<T>(&self) -> Option<Res<'_, T>>
    where
        T: $($bound)+,
    {
        self.borrow_res_id(self.get_id::<T>()?)
    }

    #[inline]
    pub fn borrow_res_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: $($bound)+,
    {
        self.borrow_res_mut_id(self.get_id::<T>()?)
    }

    pub fn borrow_res_id<T>(&self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
    where
        T: $($bound)+,
    {
        self.resources
            .get(resource_id.0)
            .and_then(Resource::<$Marker>::borrow)
    }

    pub fn borrow_res_mut_id<T>(&self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
    where
        T: $($bound)+,
    {
        self.resources
            .get(resource_id.0)
            .and_then(Resource::<$Marker>::borrow_mut)
    }

    #[inline]
    pub fn get_copy<T>(&self) -> Option<T>
    where
        T: Copy + $($bound)+,
    {
        self.get_copy_id(self.get_id::<T>()?)
    }

    #[inline]
    pub fn get_mut<T>(&mut self) -> Option<&'_ mut T>
    where
        T: $($bound)+,
    {
        self.get_mut_id(self.get_id::<T>()?)
    }

    pub fn get_copy_id<T>(&self, resource_id: ResourceId<T>) -> Option<T>
    where
        T: Copy + $($bound)+,
    {
        self.resources
            .get(resource_id.0)
            .and_then(Resource::<$Marker>::get)
    }

    pub fn get_mut_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<&'_ mut T>
    where
        T: $($bound)+,
    {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::<$Marker>::get_mut)
    }

    #[inline]
    pub fn remove<T>(&mut self) -> Option<TakenRes<T>>
    where
        T: $($bound)+,
    {
        self.remove_id(self.get_id::<T>()?)
    }

    #[inline]
    pub fn remove_id<T>(&mut self, resource_id: ResourceId<T>) -> Option<TakenRes<T>>
    where
        T: $($bound)+,
    {
        self.resources
            .get_mut(resource_id.0)
            .and_then(Resource::<$Marker>::remove)
    }

    pub fn insert_again<T>(&mut self, taken: TakenRes<T>)
        where
        T: $($bound)+,
    {
        self.resources
            .get_mut(taken.id.0)
            .unwrap()
            .insert_again(taken)
    }
}

};
}

impl_send_unsend!(SendMarker : Send + Sync + 'static);
impl_send_unsend!(UnsendMarker : 'static);

impl<Marker> Default for BaseResources<Marker> {
    #[inline]
    fn default() -> Self {
        Self::new()
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

unsafe impl<'a, T> SystemParam for Res<'a, T>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = ResourceId<T>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>().expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for Res<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = Res<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        resources.borrow_res_id(*prepared).unwrap()
    }
}

unsafe impl<T> SystemParam for ResMut<'_, T>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = ResourceId<T>;
    type Fetch = Self;
    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>().expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for ResMut<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = ResMut<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        resources.borrow_res_mut_id(*prepared).unwrap()
    }
}

unsafe impl<T> SystemParam for Option<Res<'_, T>>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = Option<ResourceId<T>>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<Res<'_, T>>
where
    T: Send + Sync + 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = Option<Res<'a, T>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        if let Some(prepared) = *prepared {
            resources.borrow_res_id(prepared)
        } else {
            None
        }
    }
}

unsafe impl<T> SystemParam for Option<ResMut<'_, T>>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = Option<ResourceId<T>>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<ResMut<'_, T>>
where
    T: Send + Sync + 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = Option<ResMut<'a, T>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        if let Some(prepared) = *prepared {
            resources.borrow_res_mut_id(prepared)
        } else {
            None
        }
    }
}

pub struct NonSend<T>(pub T);

impl<T> Deref for NonSend<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for NonSend<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl<'a, T> SystemParam for NonSend<Res<'a, T>>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = false;
    type Prepared = ResourceId<T>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>().expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for NonSend<Res<'_, T>>
where
    T: Send + Sync + 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = NonSend<Res<'a, T>>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        NonSend(resources.borrow_res_id(*prepared).unwrap())
    }
}

unsafe impl<T> SystemParam for NonSend<ResMut<'_, T>>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = false;
    type Prepared = ResourceId<T>;
    type Fetch = Self;
    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>().expect("resource not registered")
    }
}

impl<'a, T> SystemParamFetch<'a> for NonSend<ResMut<'_, T>>
where
    T: Send + Sync + 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = NonSend<ResMut<'a, T>>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        NonSend(resources.borrow_res_mut_id(*prepared).unwrap())
    }
}

unsafe impl<T> SystemParam for Option<NonSend<Res<'_, T>>>
where
    T: 'static,
{
    const IS_SEND: bool = false;
    type Prepared = Option<ResourceId<T>>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<NonSend<Res<'_, T>>>
where
    T: 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = Option<NonSend<Res<'a, T>>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        if let Some(prepared) = *prepared {
            resources.borrow_res_id(prepared).map(NonSend)
        } else {
            None
        }
    }
}

unsafe impl<T> SystemParam for Option<NonSend<ResMut<'_, T>>>
where
    T: 'static,
{
    const IS_SEND: bool = false;
    type Prepared = Option<ResourceId<T>>;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        resources.get_id::<T>()
    }
}

impl<'a, T> SystemParamFetch<'a> for Option<NonSend<ResMut<'_, T>>>
where
    T: 'static,
{
    #[allow(clippy::use_self)] // because different lifetime
    type Output = Option<NonSend<ResMut<'a, T>>>;

    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        if let Some(prepared) = *prepared {
            resources.borrow_res_mut_id(prepared).map(NonSend)
        } else {
            None
        }
    }
}
