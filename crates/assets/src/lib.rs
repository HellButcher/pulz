#![warn(
    // missing_docs,
    // rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::{
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use pulz_schedule::{
    define_label_enum,
    event::{EventWriter, Events},
    label::{CoreSystemPhase, SystemPhase},
    prelude::*,
};
use slotmap::{Key, KeyData, SlotMap};

#[repr(transparent)]
pub struct Handle<T>(KeyData, PhantomData<fn() -> T>);

impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<T> Default for Handle<T> {
    #[inline]
    fn default() -> Self {
        Self(KeyData::default(), PhantomData)
    }
}
impl<T> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("Handle<{}>", ::std::any::type_name::<T>()))
            .field(&self.0)
            .finish()
    }
}
impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for Handle<T> {}
impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}
impl<T> From<KeyData> for Handle<T> {
    #[inline]
    fn from(k: KeyData) -> Self {
        Self(k, PhantomData)
    }
}
unsafe impl<T> Key for Handle<T> {
    #[inline]
    fn data(&self) -> KeyData {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetEvent<T> {
    Created(Handle<T>),
    Modified(Handle<T>),
    Removed(Handle<T>),
}

struct AssetEntry<T> {
    asset: T,
    changed_since_last_update: bool,
}

pub struct Assets<T> {
    map: SlotMap<Handle<T>, AssetEntry<T>>,
    events: Vec<AssetEvent<T>>,
}

impl<T> Assets<T> {
    pub fn new() -> Self {
        Self {
            map: SlotMap::with_key(),
            events: Vec::new(),
        }
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.map.reserve(additional_capacity)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        for (handle, _) in self.map.drain() {
            self.events.push(AssetEvent::Removed(handle))
        }
    }

    #[inline]
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        let handle = self.map.insert(AssetEntry {
            asset,
            changed_since_last_update: true,
        });
        self.events.push(AssetEvent::Created(handle));
        handle
    }

    #[inline]
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.map.contains_key(handle)
    }

    #[inline]
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        Some(&self.map.get(handle)?.asset)
    }

    #[inline]
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let entry = self.map.get_mut(handle)?;
        if !entry.changed_since_last_update {
            entry.changed_since_last_update = true;
            self.events.push(AssetEvent::Modified(handle));
        }
        Some(&mut entry.asset)
    }

    #[inline]
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let entry = self.map.remove(handle)?;
        self.events.push(AssetEvent::Removed(handle));
        Some(entry.asset)
    }

    pub fn update(&mut self, mut events_writer: EventWriter<'_, AssetEvent<T>>) {
        for (_, entry) in self.map.iter_mut() {
            entry.changed_since_last_update = false;
        }
        events_writer.send_batch(self.events.drain(..))
    }

    pub fn install_into(res: &mut Resources)
    where
        T: Send + Sync + 'static,
    {
        if res.try_init::<Self>().is_ok() {
            Events::<AssetEvent<T>>::install_into(res);
            let mut schedule = res.borrow_res_mut::<Schedule>().unwrap();
            // update assets after FIRST(events), and before UPDATE
            schedule.add_phase_chain([
                AssetSystemPhase::LoadAssets.as_label(),
                AssetSystemPhase::UpdateAssets.as_label(),
                CoreSystemPhase::Update.as_label(),
            ]);
            schedule
                .add_system(Self::update)
                .into_phase(AssetSystemPhase::UpdateAssets);
        }
    }
}

define_label_enum! {
    pub enum AssetSystemPhase: SystemPhase {
        LoadAssets,
        UpdateAssets,
    }
}

impl<T> Default for Assets<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::ops::Index<Handle<T>> for Assets<T> {
    type Output = T;
    #[inline]
    fn index(&self, handle: Handle<T>) -> &T {
        self.get(handle).expect("invalid handle")
    }
}

impl<T> std::ops::IndexMut<Handle<T>> for Assets<T> {
    #[inline]
    fn index_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.get_mut(handle).expect("invalid handle")
    }
}
