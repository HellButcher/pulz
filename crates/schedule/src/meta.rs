use std::{
    any::{type_name, Any, TypeId},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use fnv::{FnvHashMap, FnvHashSet};

use crate::{
    prelude::*,
    resource::ResourceAccess,
    system::data::{SystemData, SystemDataFetch, SystemDataState},
};

pub trait AnyCast<T> {
    fn any_cast(from: &T) -> &Self;
}

pub trait AnyCastMut<T>: AnyCast<T> {
    fn any_cast_mut(from: &mut T) -> &mut Self;
}

#[macro_export]
macro_rules! impl_any_cast {
    ($(dyn $T:path),+) => {$(
        impl<T: $T> $crate::meta::AnyCast<T> for dyn $T {
            #[inline]
            fn any_cast(from_any: &T) -> &Self {
                from_any
            }
        }
        impl<T: $T> $crate::meta::AnyCastMut<T> for dyn $T {
            #[inline]
            fn any_cast_mut(from_any: &mut T) -> &mut Self {
                from_any
            }
        }
    )+};
}

/// # Safety
/// argument must actually be of correct type
pub unsafe fn any_cast_ref_unchecked<T, B>(any: &dyn Any) -> &T
where
    T: AnyCast<B> + ?Sized + 'static,
    B: 'static,
{
    AnyCast::any_cast(if cfg!(debug_assertions) {
        let Some(v) = any.downcast_ref::<B>() else {
            panic!(
                "unable to cast Any ({:?}) to {:?}, expected {:?}({:?})",
                any.type_id(),
                type_name::<T>(),
                type_name::<B>(),
                TypeId::of::<B>(),
            );
        };
        v
    } else {
        let any: *const dyn Any = any;
        unsafe { &*(any as *const B) }
    })
}

/// # Safety
/// argument must actually be of correct type
pub unsafe fn any_cast_mut_unchecked<T, B>(any: &mut dyn Any) -> &mut T
where
    T: AnyCastMut<B> + ?Sized + 'static,
    B: 'static,
{
    let tid = Any::type_id(any);
    AnyCastMut::any_cast_mut(if cfg!(debug_assertions) {
        let Some(v) = any.downcast_mut::<B>() else {
            panic!(
                "unable to cast Any ({:?}) to {:?}, expected {:?}({:?})",
                tid,
                type_name::<T>(),
                type_name::<B>(),
                TypeId::of::<B>(),
            );
        };
        v
    } else {
        let any: *mut dyn Any = any;
        unsafe { &mut *(any as *mut B) }
    })
}

pub struct Meta<T: ?Sized> {
    conv_ref: FnvHashMap<TypeId, unsafe fn(&dyn Any) -> &T>,
    conv_mut: FnvHashMap<TypeId, unsafe fn(&mut dyn Any) -> &mut T>,
    resources: FnvHashSet<ResourceId>,
}

impl<T: ?Sized> Meta<T> {
    pub fn convert_ref<'a>(&self, any: &'a dyn Any) -> Option<&'a T> {
        let typeid = Any::type_id(any);
        let conv_ref = self.conv_ref.get(&typeid)?;
        unsafe { Some(conv_ref(any)) }
    }

    pub fn convert_mut<'a>(&self, any: &'a mut dyn Any) -> Option<&'a mut T> {
        let typeid = Any::type_id(any);
        let conv_mut = self.conv_mut.get(&typeid)?;
        unsafe { Some(conv_mut(any)) }
    }

    fn init_ref<B: 'static>(&mut self)
    where
        T: AnyCast<B> + 'static,
    {
        let typeid = TypeId::of::<B>();
        self.conv_ref
            .entry(typeid)
            .or_insert(any_cast_ref_unchecked::<T, B>);
    }

    fn init_mut<B: 'static>(&mut self)
    where
        T: AnyCastMut<B> + 'static,
    {
        let typeid = TypeId::of::<B>();
        self.conv_ref
            .entry(typeid)
            .or_insert(any_cast_ref_unchecked::<T, B>);
        self.conv_mut
            .entry(typeid)
            .or_insert(any_cast_mut_unchecked::<T, B>);
    }
}

impl<T: ?Sized> Default for Meta<T> {
    fn default() -> Self {
        Self {
            conv_ref: FnvHashMap::default(),
            conv_mut: FnvHashMap::default(),
            resources: FnvHashSet::default(),
        }
    }
}

impl Resources {
    pub fn init_meta<M, R>(&mut self)
    where
        R: 'static,
        M: AnyCastMut<R> + ?Sized + 'static,
    {
        self.init_meta_id::<M, R>(self.expect_id::<R>())
    }
    pub fn init_meta_readonly<M, R>(&mut self)
    where
        R: 'static,
        M: AnyCast<R> + ?Sized + 'static,
    {
        self.init_meta_readonly_id::<M, R>(self.expect_id::<R>())
    }

    pub(crate) fn get_meta<T: ?Sized + 'static>(&self) -> Option<&Meta<T>> {
        self.meta_by_type_id
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<Meta<T>>())
    }
    fn get_meta_mut<T: ?Sized + 'static>(&mut self) -> &mut Meta<T> {
        self.meta_by_type_id
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::<Meta<T>>::default())
            .downcast_mut::<Meta<T>>()
            .unwrap()
    }

    pub fn init_meta_id<M, R>(&mut self, id: ResourceId<R>)
    where
        R: 'static,
        M: AnyCastMut<R> + ?Sized + 'static,
    {
        let meta = self.get_meta_mut::<M>();
        meta.init_mut::<R>();
        meta.resources.insert(id.untyped());
    }
    pub fn init_meta_readonly_id<M, R>(&mut self, id: ResourceId<R>)
    where
        R: 'static,
        M: AnyCast<R> + ?Sized + 'static,
    {
        let meta = self.get_meta_mut::<M>();
        meta.init_ref::<R>();
        meta.resources.insert(id.untyped());
    }

    pub fn foreach_meta<T: ?Sized + 'static>(&self, mut f: impl FnMut(&T)) {
        if let Some(meta) = self.get_meta::<T>() {
            for resource_id in meta.resources.iter().copied() {
                if let Some(r) = self.borrow_res_any(resource_id) {
                    if let Some(r) = meta.convert_ref(r.deref()) {
                        f(r);
                    }
                }
            }
        }
    }

    pub fn foreach_meta_mut<T: ?Sized + 'static>(&self, mut f: impl FnMut(&mut T)) {
        if let Some(meta) = self.get_meta::<T>() {
            for resource_id in meta.resources.iter().copied() {
                if let Some(mut r) = self.borrow_res_any_mut(resource_id) {
                    if let Some(r) = meta.convert_mut(r.deref_mut()) {
                        f(r);
                    }
                }
            }
        }
    }
}

pub struct Metas<'a, T: ?Sized>(&'a [Res<'a, T>]);
pub struct MetasMut<'a, T: ?Sized>(&'a [ResMut<'a, T>]);

impl<'a, T: ?Sized> Deref for Metas<'a, T> {
    type Target = [Res<'a, T>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, T: ?Sized> Deref for MetasMut<'a, T> {
    type Target = [ResMut<'a, T>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[doc(hidden)]
pub struct MetasState<T: ?Sized>(PhantomData<fn(&T)>);

#[doc(hidden)]
pub struct MetasMutState<T: ?Sized>(PhantomData<fn(&T)>);

impl<T> SystemData for Metas<'_, T>
where
    T: ?Sized + 'static,
{
    type State = MetasState<T>;
    type Fetch<'r> = Box<[Res<'r, T>]>;
    type Item<'a> = Metas<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        Metas(fetch)
    }
}

impl<T> SystemData for MetasMut<'_, T>
where
    T: ?Sized + 'static,
{
    type State = MetasMutState<T>;
    type Fetch<'r> = Box<[ResMut<'r, T>]>;
    type Item<'a> = MetasMut<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        MetasMut(fetch)
    }
}

unsafe impl<T> SystemDataState for MetasState<T>
where
    T: ?Sized + 'static,
{
    #[inline]
    fn init(_resources: &mut Resources) -> Self {
        Self(PhantomData)
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        if let Some(meta) = resources.get_meta::<T>() {
            for r in meta.resources.iter().copied() {
                access.add_shared_checked(r);
            }
        }
    }
}

unsafe impl<T> SystemDataState for MetasMutState<T>
where
    T: ?Sized + 'static,
{
    #[inline]
    fn init(_resources: &mut Resources) -> Self {
        Self(PhantomData)
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        if let Some(meta) = resources.get_meta::<T>() {
            for r in meta.resources.iter().copied() {
                access.add_exclusive_checked(r);
            }
        }
    }
}

impl<'r, T: ?Sized + 'static> SystemDataFetch<'r> for Box<[Res<'r, T>]> {
    type State = MetasState<T>;

    #[inline]
    fn fetch(res: &'r Resources, _state: &'r mut Self::State) -> Self {
        if let Some(meta) = res.get_meta::<T>() {
            meta.resources
                .iter()
                .copied()
                .map(|id| {
                    Res::map(res.borrow_res_any(id).unwrap(), |r| {
                        meta.convert_ref(r).unwrap()
                    })
                })
                .collect()
        } else {
            Box::new([])
        }
    }
}

impl<'r, T: ?Sized + 'static> SystemDataFetch<'r> for Box<[ResMut<'r, T>]> {
    type State = MetasMutState<T>;

    #[inline]
    fn fetch(res: &'r Resources, _state: &'r mut Self::State) -> Self {
        if let Some(meta) = res.get_meta::<T>() {
            meta.resources
                .iter()
                .copied()
                .map(|id| {
                    ResMut::map(res.borrow_res_any_mut(id).unwrap(), |r| {
                        meta.convert_mut(r).unwrap()
                    })
                })
                .collect()
        } else {
            Box::new([])
        }
    }
}
