use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use atomic_refcell::AtomicRefCell;

use super::{FromResourcesMut, Res, ResMut, ResourceId, Taken};
use crate::{
    atom::Atom,
    meta::{Meta, MetaMap},
    util::DirtyVersion,
};

struct ResourceData {
    name: Cow<'static, str>,
    type_id: TypeId,
    is_send: bool,
    value: AtomicRefCell<Option<Box<dyn Any>>>,
}

// not send: should be dropped in original thread
unsafe impl Sync for ResourceData {}

impl ResourceData {
    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn borrow_any(&self) -> Option<Res<'_, dyn Any>> {
        Res::filter_map(self.value.borrow(), |v| Some(Box::deref(v.as_ref()?)))
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn borrow_any_mut(&self) -> Option<ResMut<'_, dyn Any>> {
        ResMut::filter_map(self.value.borrow_mut(), |v| {
            Some(Box::deref_mut(v.as_mut()?))
        })
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn borrow<T>(&self) -> Option<Res<'_, T>>
    where
        T: 'static,
    {
        Res::filter_map(self.value.borrow(), |v| v.as_ref()?.downcast_ref::<T>())
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn borrow_mut<T>(&self) -> Option<ResMut<'_, T>>
    where
        T: 'static,
    {
        ResMut::filter_map(self.value.borrow_mut(), |v| v.as_mut()?.downcast_mut::<T>())
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        unsafe { self.get_any()?.downcast_mut::<T>() }
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn get_any(&mut self) -> Option<&mut dyn Any> {
        Some(self.value.get_mut().as_mut()?.deref_mut())
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn take<T>(&mut self) -> Option<Box<T>>
    where
        T: 'static,
    {
        unsafe { self.take_any()?.downcast().ok() }
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn take_any(&mut self) -> Option<Box<dyn Any>> {
        self.value.get_mut().take()
    }

    /// UNSAFE: When send is false, the user must ensure that the resource is not accessed in other threads, as where the resource was created
    #[inline]
    unsafe fn put_back(&mut self, boxed: Box<dyn Any>) {
        let value = self.value.get_mut();
        assert!(value.is_none(), "Resource already has a value");
        *value = Some(boxed);
    }
}

pub struct ResourcesSend {
    resources: Vec<ResourceData>,
    by_type_id: BTreeMap<TypeId, ResourceId>,
    meta_by_type_id: MetaMap,
    modules: BTreeSet<TypeId>,
    atom: Atom,
    version: DirtyVersion,
}

#[repr(transparent)]
pub struct Resources(ResourcesSend, PhantomData<NonNull<()>>);

#[allow(unused_doc_comments)]
macro_rules! impl_getters {
    (
        // UNSAFE: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
        // User should provide a comment about safety
        unsafe $self:ident $(where T: $b1:ident $(+ $b2:ident)*)? => $resources_field:expr
    ) => {

        #[inline]
        pub fn borrow_res<T>(&self) -> Option<Res<'_, T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            self.borrow_res_id(self.id::<T>()?)
        }

        pub fn borrow_res_id<T>(&$self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            // SAFETY: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
            unsafe {
                $resources_field.get(resource_id.0)?.borrow()
            }
        }

        #[inline]
        pub fn borrow_res_mut<T>(&self) -> Option<ResMut<'_, T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            self.borrow_res_mut_id(self.id::<T>()?)
        }

        pub fn borrow_res_mut_id<T>(&$self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            // SAFETY: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
            unsafe {
                $resources_field.get(resource_id.0)?.borrow_mut()
            }
        }

        #[inline]
        pub fn get_mut<T>(&mut self) -> Option<&'_ mut T>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            self.get_mut_id(self.id::<T>()?)
        }

        pub fn get_mut_id<T>(&mut $self, resource_id: ResourceId<T>) -> Option<&'_ mut T>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            // SAFETY: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
            unsafe {
                $resources_field.get_mut(resource_id.0)?.get_mut()
            }
        }

        #[inline]
        pub fn get_copy<T>(&self) -> Option<T>
        where
            T: Copy + $($b1 $(+ $b2)* +)? 'static,
        {
            self.get_copy_id(self.id::<T>()?)
        }

        pub fn get_copy_id<T>(&self, resource_id: ResourceId<T>) -> Option<T>
        where
            T: Copy + $($b1 $(+ $b2)* +)? 'static,
        {
            Some(*self.borrow_res_id(resource_id)?)
        }

        #[inline]
        pub fn get_clone<T>(&self) -> Option<T>
        where
            T: Clone + $($b1 $(+ $b2)* +)? 'static,
        {
            self.get_clone_id(self.id::<T>()?)
        }

        pub fn get_clone_id<T>(&self, resource_id: ResourceId<T>) -> Option<T>
        where
            T: Clone + $($b1 $(+ $b2)* +)? 'static,
        {
            Some(self.borrow_res_id(resource_id)?.clone())
        }

        #[inline]
        pub fn take<T>(&mut self) -> Option<Taken<T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            self.take_id(self.id::<T>()?)
        }

        pub fn take_id<T>(&mut $self, resource_id: ResourceId<T>) -> Option<Taken<T>>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            // SAFETY: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
            let boxed = unsafe {
                $resources_field.get_mut(resource_id.0)?.take()?
            };
            Some(Taken {
                value: boxed,
                id: resource_id.untyped(),
                #[cfg(debug_assertions)]
                atom: $self.atom,
            })
        }

        pub fn put_back<T>(&mut $self, taken: Taken<T>)
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            // SAFETY: either T: Send + Sync, or Self=Resources (which is NOT Send+Sync)
            #[cfg(debug_assertions)]
            if $self.atom != taken.atom {
                panic!("put_taken called with a different atom than the one used to take the resource");
            }
            unsafe {
                $resources_field.get_mut(taken.id.0).expect("resource not defined").put_back(taken.value);
            }
        }

        #[inline]
        pub fn take_and<T, R>(&mut self, f: impl FnOnce(&mut T, &mut Self) -> R) -> Option<R>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            self.take_id_and(self.id::<T>()?, f)
        }

        #[inline]
        pub fn take_id_and<T, R>(&mut self, resource_id: ResourceId<T>, f: impl FnOnce(&mut T, &mut Self) -> R) -> Option<R>
        where
            T: $($b1 $(+ $b2)* +)? 'static,
        {
            use std::panic;
            let mut taken = self.take_id(resource_id)?;
            let r = panic::catch_unwind(panic::AssertUnwindSafe(|| f(&mut taken.value, self)));
            self.put_back(taken);
            match r {
                Err(e) => panic::resume_unwind(e),
                Ok(r) => Some(r)
            }
        }
    };
}

impl Resources {
    #[inline]
    pub fn new() -> Self {
        let mut res = Self(
            ResourcesSend {
                resources: Vec::new(),
                by_type_id: BTreeMap::new(),
                meta_by_type_id: BTreeMap::new(),
                modules: BTreeSet::new(),
                atom: Atom::new(),
                version: DirtyVersion::new(),
            },
            PhantomData,
        );
        res.init_core();
        res
    }

    fn init_core(&mut self) {
        self.init_unsend::<crate::schedule::Schedule>();
        // TODO
    }

    #[inline(always)]
    pub fn as_send(&self) -> &ResourcesSend {
        &self.0
    }

    #[inline]
    pub(crate) fn atom(&self) -> Atom {
        self.0.atom
    }

    #[inline]
    pub(crate) fn version_mut(&mut self) -> &mut DirtyVersion {
        &mut self.0.version
    }

    pub(crate) fn insert_module(&mut self, type_id: TypeId) -> bool {
        if self.0.modules.insert(type_id) {
            self.0.version.dirty();
            true
        } else {
            false
        }
    }

    /// # Safety
    /// User must ensure, that the value is Send + Sync, when is_send is true.
    unsafe fn insert_impl<T>(&mut self, is_send: bool, value: Option<Box<T>>) -> ResourceId<T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let name = Cow::Borrowed(std::any::type_name::<T>());
        unsafe {
            self.0
                ._insert_impl(type_id, name, is_send, value.map(|v| -> Box<dyn Any> { v }))
                .cast()
        }
    }

    #[inline]
    pub fn insert<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: Send + Sync + 'static,
    {
        self.insert_box(Box::new(value))
    }

    #[inline]
    pub fn insert_box<T>(&mut self, value: Box<T>) -> ResourceId<T>
    where
        T: Send + Sync + 'static,
    {
        // SAFETY: T is Send + Sync
        unsafe { self.insert_impl(true, Some(value)) }
    }

    #[inline]
    pub fn define<T>(&mut self) -> ResourceId<T>
    where
        T: Send + Sync + 'static,
    {
        // SAFETY: T is Send + Sync
        unsafe { self.insert_impl(true, None) }
    }

    #[inline]
    pub fn insert_unsend<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: 'static,
    {
        self.insert_box_unsend(Box::new(value))
    }

    #[inline]
    pub fn insert_box_unsend<T>(&mut self, value: Box<T>) -> ResourceId<T>
    where
        T: 'static,
    {
        // safety: is_send is false
        unsafe { self.insert_impl(false, Some(value)) }
    }

    #[inline]
    pub fn define_unsend<T>(&mut self) -> ResourceId<T>
    where
        T: 'static,
    {
        // safety: is_send is false
        unsafe { self.insert_impl(false, None) }
    }

    pub fn try_init<T>(&mut self) -> Result<ResourceId<T>, ResourceId<T>>
    where
        T: Send + Sync + FromResourcesMut + 'static,
    {
        if let Some(id) = self.0.id::<T>() {
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
        if let Some(id) = self.0.id::<T>() {
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

    pub fn clear(&mut self) {
        self.0.resources.clear();
        self.0.by_type_id.clear();
        self.0.meta_by_type_id.clear();
        self.0.modules.clear();
        self.init_core();
    }

    impl_getters!(
        // SAFETY: Self=Resources is not Send + Sync
        unsafe self => self.0.resources
    );

    pub(crate) fn get_meta<T: ?Sized + 'static>(&self) -> Option<&Meta<T>> {
        Self::get_meta_from_map(&self.0.meta_by_type_id)
    }

    pub(crate) fn get_meta_mut<T: ?Sized + 'static>(&mut self) -> &mut Meta<T> {
        Self::get_meta_mut_from_map(&mut self.0.meta_by_type_id)
    }

    pub fn borrow_res_meta<T>(&self, resource_id: ResourceId<T>) -> Option<Res<'_, T>>
    where
        T: ?Sized + 'static,
    {
        let r = self.0.resources.get(resource_id.0)?;
        let meta = self.get_meta::<T>()?;
        // SAFETY: Self=Resources is not Send + Sync
        unsafe { Res::filter_map(r.borrow_any()?, |v| meta.convert_ref(v)) }
    }

    pub fn borrow_res_any(&self, resource_id: ResourceId) -> Option<Res<'_, dyn Any>> {
        // SAFETY: Self=Resources is not Send + Sync
        unsafe { self.resources.get(resource_id.0)?.borrow_any() }
    }

    pub fn borrow_res_mut_meta<T>(&self, resource_id: ResourceId<T>) -> Option<ResMut<'_, T>>
    where
        T: ?Sized + 'static,
    {
        let r = self.0.resources.get(resource_id.0)?;
        let meta = self.get_meta::<T>()?;
        // SAFETY: Self=Resources is not Send + Sync
        unsafe { ResMut::filter_map(r.borrow_any_mut()?, |v| meta.convert_mut(v)) }
    }

    pub fn borrow_res_any_mut(&self, resource_id: ResourceId) -> Option<ResMut<'_, dyn Any>> {
        // SAFETY: Self=Resources is not Send + Sync
        unsafe { self.0.resources.get(resource_id.0)?.borrow_any_mut() }
    }

    pub fn get_mut_any(&mut self, resource_id: ResourceId) -> Option<&'_ mut dyn Any> {
        // SAFETY: Self=Resources is not Send + Sync
        unsafe { self.0.resources.get_mut(resource_id.0)?.get_any() }
    }
}

impl Deref for Resources {
    type Target = ResourcesSend;

    #[inline]
    fn deref(&self) -> &ResourcesSend {
        &self.0
    }
}

impl AsRef<ResourcesSend> for Resources {
    #[inline]
    fn as_ref(&self) -> &ResourcesSend {
        &self.0
    }
}

impl ResourcesSend {
    /// # Safety
    /// User must not access unsend resources in other threads.
    #[inline]
    pub unsafe fn as_unsend(&self) -> &Resources {
        let self_ptr: *const Self = self;
        // SAFETY: cast is allowed because it is a newtype-struct with #[repr(transparent)].
        // Send -> Unsend is unsafe (see doc)
        unsafe { &*(self_ptr as *const Resources) }
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

    /// # Safety
    /// User must ensure, that the value is Send + Sync, when is_send is true.
    unsafe fn _insert_impl(
        &mut self,
        type_id: TypeId,
        name: Cow<'static, str>,
        is_send: bool,
        value: Option<Box<dyn Any>>,
    ) -> ResourceId {
        let resources = &mut self.resources;
        match self.by_type_id.entry(type_id) {
            Entry::Occupied(entry) => {
                let id = *entry.get();
                let r = &mut resources[id.0];
                r.is_send = is_send;
                if value.is_some() {
                    r.value = AtomicRefCell::new(value);
                }
                id
            }
            Entry::Vacant(entry) => {
                self.version.dirty();
                let id = ResourceId::new(resources.len()); // keep positive => dense
                resources.push(ResourceData {
                    name,
                    type_id,
                    is_send,
                    value: AtomicRefCell::new(value),
                });
                entry.insert(id);
                self.version.dirty();
                id
            }
        }
    }

    impl_getters!(
        // SAFETY: T is restricted to be Send+Sync
        unsafe self where T: Send + Sync => self.resources
    );
}

impl Default for Resources {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
