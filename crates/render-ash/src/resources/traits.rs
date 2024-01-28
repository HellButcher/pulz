use std::hash::{Hash, Hasher};

use pulz_render::backend::GpuResource;
use slotmap::SlotMap;

use super::{replay::AsResourceRecord, AshFrameGarbage, AshResources, U64HashMap};
use crate::{alloc::AshAllocator, Result};

pub trait AshGpuResource: GpuResource + 'static {
    type Raw;
    unsafe fn create_raw(
        alloc: &mut AshResources,
        descriptor: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw>;
    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw);

    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw>;
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw>;
}

fn hash_one<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(super) trait AshGpuResourceCached: AshGpuResource
where
    for<'l> Self::Descriptor<'l>: Hash,
{
    fn hash_descriptor(descr: &Self::Descriptor<'_>) -> u64 {
        hash_one(descr)
    }

    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self>;
}

pub trait AshGpuResourceCreate: AshGpuResource {
    #[inline]
    fn create(res: &mut AshResources, descr: &Self::Descriptor<'_>) -> Result<Self> {
        unsafe {
            let raw = Self::create_raw(res, descr)?;
            let key = Self::slotmap_mut(res).insert(raw);
            Ok(key)
        }
    }
}

pub trait AshGpuResourceCollection {
    type Resource: AshGpuResource;
    unsafe fn clear_destroy(&mut self, alloc: &mut AshAllocator);
    unsafe fn destroy(&mut self, key: Self::Resource, alloc: &mut AshAllocator) -> bool;
}
impl<R: AshGpuResource> AshGpuResourceCollection for SlotMap<R, R::Raw> {
    type Resource = R;
    unsafe fn clear_destroy(&mut self, alloc: &mut AshAllocator) {
        for (_key, raw) in self.drain() {
            unsafe {
                R::destroy_raw(alloc, raw);
            }
        }
    }
    unsafe fn destroy(&mut self, key: Self::Resource, alloc: &mut AshAllocator) -> bool {
        if let Some(raw) = self.remove(key) {
            R::destroy_raw(alloc, raw);
            true
        } else {
            false
        }
    }
}

pub trait AshGpuResourceRemove: AshGpuResource {
    fn put_to_garbage(garbage: &mut AshFrameGarbage, raw: Self::Raw);
    fn destroy(res: &mut AshResources, key: Self) -> bool {
        if let Some(raw) = Self::slotmap_mut(res).remove(key) {
            let garbage = res.current_frame_garbage_mut();
            Self::put_to_garbage(garbage, raw);
            true
        } else {
            false
        }
    }
}

impl<R> AshGpuResourceCreate for R
where
    R: AshGpuResourceCached + AsResourceRecord,
    for<'l> R::Descriptor<'l>: Hash,
{
    fn create(res: &mut AshResources, descr: &Self::Descriptor<'_>) -> Result<Self> {
        let hash = Self::hash_descriptor(descr);
        if let Some(key) = Self::get_hashs_mut(res).get(&hash) {
            return Ok(*key);
        }
        let key = unsafe {
            let raw = Self::create_raw(res, descr)?;
            Self::slotmap_mut(res).insert(raw)
        };
        Self::get_hashs_mut(res).insert(hash, key);
        if let Some(record) = &mut res.record {
            record.record(key.as_record(descr))?;
        }
        Ok(key)
    }
}
