use std::hash::Hash;

use hashbrown::{hash_map::Entry, HashMap};

use crate::backend::RenderBackend;

pub trait Cacheable: Hash + Eq + PartialEq + 'static {
    type Target: Clone;

    fn create(&self, renderer: &mut dyn RenderBackend) -> Self::Target;
    fn destroy(&self, value: Self::Target, renderer: &mut dyn RenderBackend);
}

struct Cached<T> {
    value: T,
    taken: bool,
    frames_since_last_use: usize,
}

pub struct Cache<C: Cacheable>(HashMap<C, Vec<Cached<C::Target>>>);

impl<C: Cacheable> Cache<C> {
    #[inline]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&mut self, renderer: &mut dyn RenderBackend, descriptor: C) -> C::Target {
        match self.0.entry(descriptor) {
            Entry::Occupied(mut entry) => {
                for item in entry.get_mut().iter_mut() {
                    if !item.taken {
                        item.frames_since_last_use = 0;
                        item.taken = true;
                        return item.value.clone();
                    }
                }

                let value = entry.key().create(renderer);
                let items = entry.get_mut();
                items.push(Cached {
                    value: value.clone(),
                    frames_since_last_use: 0,
                    taken: true,
                });
                value
            }
            Entry::Vacant(entry) => {
                let value = entry.key().create(renderer);
                let items = entry.insert(Vec::new());
                items.push(Cached {
                    value: value.clone(),
                    frames_since_last_use: 0,
                    taken: true,
                });
                value
            }
        }
    }

    pub fn update(&mut self, backend: &mut dyn RenderBackend) {
        for (descriptor, items) in self.0.iter_mut() {
            for item in items.iter_mut() {
                item.frames_since_last_use += 1;
                item.taken = false;
            }

            items.retain(|item| {
                if item.frames_since_last_use < 3 {
                    true
                } else {
                    descriptor.destroy(item.value.clone(), backend);
                    false
                }
            });
        }
    }
}

impl<C: Cacheable> Default for Cache<C> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
