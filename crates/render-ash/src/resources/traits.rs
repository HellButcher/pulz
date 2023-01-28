use std::hash::{Hash, Hasher};

use pulz_render::backend::GpuResource;
use slotmap::SlotMap;

use super::{replay::AsResourceRecord, AshResources, PreHashedU64Map};
use crate::{device::AshDevice, Result};

pub trait AshGpuResource: GpuResource + 'static {
    type Raw: Copy;

    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw>;
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw>;

    unsafe fn create_raw(
        res: &AshResources,
        descriptor: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw>;
    unsafe fn destroy_raw(device: &AshDevice, raw: Self::Raw);
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

    fn get_hashs_mut(res: &mut AshResources) -> &mut PreHashedU64Map<Self>;
}

pub trait AshGpuResourceCreate: AshGpuResource {
    #[inline]
    fn get_raw(res: &AshResources, key: Self) -> Option<&Self::Raw> {
        Self::slotmap(res).get(key)
    }
    #[inline]
    fn create(res: &mut AshResources, descr: &Self::Descriptor<'_>) -> Result<Self> {
        unsafe {
            let raw = Self::create_raw(res, descr)?;
            let key = Self::slotmap_mut(res).insert(raw);
            Ok(key)
        }
    }
    fn clear(res: &mut AshResources) {
        let device = res.device.clone();
        for (_key, raw) in Self::slotmap_mut(res).drain() {
            unsafe {
                Self::destroy_raw(&device, raw);
            }
        }
    }
}

pub trait AshGpuResourceRemove: AshGpuResource {
    fn remove(res: &mut AshResources, key: Self) -> bool {
        if let Some(raw) = Self::slotmap_mut(res).remove(key) {
            unsafe { Self::destroy_raw(&res.device, raw) }
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
    fn clear(res: &mut AshResources) {
        Self::get_hashs_mut(res).clear();
        let device = res.device.clone();
        for (_key, raw) in Self::slotmap_mut(res).drain() {
            unsafe {
                Self::destroy_raw(&device, raw);
            }
        }
    }
}
