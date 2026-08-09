use std::sync::{
    Mutex,
    atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use pulz_schedule::resource::{FromResourcesMut, ResourceId, Resources};

use crate::{
    WorldInner,
    archetype::ArchetypeSet,
    query::data::{QueryData, QueryFetchState, QueryFilter},
};

pub(super) struct QueryState<Q: QueryData, F: QueryFilter = ()> {
    pub(super) world_id: ResourceId<WorldInner>,
    pub(super) q_state: Q::State,
    pub(super) f_state: F::State,

    last_archetype_count: AtomicUsize,
    updating: Mutex<()>,
    // Box<ArchetypeSet> owned behind a raw pointer so readers need no lock.
    // Written only under `updating`; swapped atomically on each update.
    matching_archetypes_p: AtomicPtr<ArchetypeSet>,
}

impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    fn new(res: &mut Resources) -> Self {
        let world_id = res.init::<WorldInner>();
        let world = res
            .borrow_res_id(world_id)
            .expect("WorldInner not initialized");
        let components = &world.components;
        let q_state = Q::State::init(res, components);
        let f_state = F::State::init(res, components);
        drop(world);

        let state = Self {
            world_id,
            q_state,
            f_state,
            last_archetype_count: AtomicUsize::new(0),
            updating: Mutex::new(()),
            matching_archetypes_p: AtomicPtr::new(std::ptr::null_mut()),
        };

        let world = res
            .borrow_res_id(world_id)
            .expect("WorldInner not initialized");
        state.update_archetypes_inner(&world.archetypes);
        drop(world);
        state
    }

    /// Extends the cached `ArchetypeSet` with any archetypes added since the last call.
    pub(super) fn update_archetypes(&self, world: &WorldInner) {
        self.update_archetypes_inner(&world.archetypes);
    }

    fn update_archetypes_inner(&self, archetypes: &crate::archetype::Archetypes) {
        let current_count = archetypes.len();
        let last = self.last_archetype_count.load(Ordering::Relaxed);
        if last >= current_count {
            return;
        }

        let _lock = self.updating.lock();
        let mut scratch: Option<Box<ArchetypeSet>> = None;

        for archetype in archetypes.iter().skip(last) {
            if self.q_state.matches_archetype(archetype)
                && self.f_state.matches_archetype(archetype)
            {
                let set = scratch.get_or_insert_with(|| {
                    let p = self.matching_archetypes_p.load(Ordering::Relaxed);
                    if p.is_null() {
                        Box::default()
                    } else {
                        // SAFETY: valid while we hold the mutex
                        unsafe { Box::new((*p).clone()) }
                    }
                });
                set.insert(archetype.id);
            }
        }

        if let Some(new) = scratch {
            let old = self
                .matching_archetypes_p
                .swap(Box::into_raw(new), Ordering::Relaxed);
            if !old.is_null() {
                // SAFETY: swapped out under the mutex; no other writer can race
                unsafe { drop(Box::from_raw(old)) }
            }
        }

        self.last_archetype_count
            .store(current_count, Ordering::Relaxed);
    }

    pub(super) fn matching_archetypes(&self) -> &ArchetypeSet {
        static EMPTY: std::sync::OnceLock<ArchetypeSet> = std::sync::OnceLock::new();
        let p = self.matching_archetypes_p.load(Ordering::Relaxed);
        if p.is_null() {
            EMPTY.get_or_init(ArchetypeSet::new)
        } else {
            // SAFETY: pointer is valid until the next update swap under the mutex;
            // callers that read this hold a Res<QueryState> borrow, preventing mutation
            unsafe { &*p }
        }
    }
}

impl<Q: QueryData, F: QueryFilter> Drop for QueryState<Q, F> {
    fn drop(&mut self) {
        let p = *self.matching_archetypes_p.get_mut();
        if !p.is_null() {
            // SAFETY: exclusive access via &mut self
            unsafe { drop(Box::from_raw(p)) }
        }
    }
}

// SAFETY: States are Send+Sync; raw pointer is protected by the mutex
unsafe impl<Q: QueryData, F: QueryFilter> Send for QueryState<Q, F> {}
unsafe impl<Q: QueryData, F: QueryFilter> Sync for QueryState<Q, F> {}

impl<Q: QueryData + 'static, F: QueryFilter + 'static> FromResourcesMut for QueryState<Q, F> {
    fn from_resources_mut(res: &mut Resources) -> Self {
        Self::new(res)
    }
}
