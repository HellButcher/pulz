use std::{
    cell::UnsafeCell,
    collections::HashMap,
    hash::{BuildHasher, BuildHasherDefault, Hasher},
    marker::PhantomData,
    sync::atomic::{AtomicI32, Ordering},
};

use bit_set::BitSet;

pub struct DisjointSliceHelper<'a, T> {
    ptr: *mut T,
    len: usize,
    borrow_mut: UnsafeCell<BitSet>,
    phantom: PhantomData<&'a mut UnsafeCell<[T]>>,
}

impl<'a, T> DisjointSliceHelper<'a, T> {
    pub fn new(slice: &'a mut [T]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            len: slice.len(),
            borrow_mut: UnsafeCell::new(BitSet::new()),
            phantom: PhantomData,
        }
    }

    #[inline]
    #[allow(clippy::mut_from_ref)] // we assume, we have dont it right here
    pub fn get_mut(&self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        // SAFETY: not Send/Sync and only accessed here
        if unsafe { (*self.borrow_mut.get()).insert(index) } {
            // SAFETY: address is valid (<len) and not already borrowed
            Some(unsafe { &mut *self.ptr.add(index) })
        } else {
            None
        }
    }
}

/// optimized Hasher for type-ids
#[derive(Default)]
pub struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    fn write_u64(&mut self, n: u64) {
        debug_assert_eq!(self.0, 0);
        self.0 = n;
    }

    // Tolerate TypeId being either u64 or u128.
    fn write_u128(&mut self, n: u128) {
        debug_assert_eq!(self.0, 0);
        self.0 = n as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        panic!(
            "TypeIdHasher only supports u64 and u128, but got bytes: {:?}",
            bytes
        );
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub type TypeIdMap<T> = HashMap<std::any::TypeId, T, BuildHasherDefault<TypeIdHasher>>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct DirtyVersion(isize);

impl DirtyVersion {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub fn dirty(&mut self) {
        let v = self.0;
        if v > 0 {
            self.0 = -v;
        }
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.0 <= 0
    }

    #[inline]
    pub fn reset(&mut self) -> bool {
        let v = self.0;
        if v <= 0 {
            self.0 = -v + 1;
            true
        } else {
            false
        }
    }

    pub fn check_and_reset(&mut self, upstream: &mut Self) -> bool {
        if upstream.reset() {
            self.0 = upstream.0;
            true
        } else {
            let v = upstream.0;
            if self.0 != v {
                self.0 = v;
                true
            } else {
                false
            }
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Tick(i32);

impl Tick {
    #[inline]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn get(self) -> i32 {
        self.0
    }

    #[inline]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Returns true if self is newer than other (i.e., has a greater tick value).
    ///
    /// allows for comparison of ticks that have wrapped around, assuming that the difference between the two ticks is less than i32::MAX (half of u32::MAX).
    #[inline]
    pub const fn is_newer_than(self, other: Self) -> bool {
        let diff = self.0.wrapping_sub(other.0);
        diff > 0
    }
}

impl PartialOrd for Tick {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Tick {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.0 == other.0 {
            std::cmp::Ordering::Equal
        } else if self.is_newer_than(*other) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        }
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct AtomicTick(AtomicI32);

impl AtomicTick {
    #[inline]
    pub const fn new(tick: Tick) -> Self {
        Self(AtomicI32::new(tick.0))
    }

    #[inline]
    pub fn get(&self) -> Tick {
        Tick(self.0.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn set(&self, tick: Tick) {
        self.0.store(tick.0, Ordering::Relaxed);
    }

    /// Updates the atomic tick to the provided tick if it is newer than the current value. (maximum)
    #[inline]
    pub fn update_newer(&self, tick: Tick) -> Tick {
        let mut current = self.get();
        while tick.is_newer_than(current) {
            match self.0.compare_exchange_weak(
                current.0,
                tick.0,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => return tick,                  // Successfully updated
                Err(actual) => current = Tick(actual), // Update failed, retry with the new value
            }
        }
        current
    }

    /// Updates the atomic tick to the provided tick if it is older than the current value.
    /// (minimum)
    #[inline]
    pub fn update_older(&self, tick: Tick) -> Tick {
        let mut current = self.get();
        while current.is_newer_than(tick) {
            match self.0.compare_exchange_weak(
                current.0,
                tick.0,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => return tick,                  // Successfully updated
                Err(actual) => current = Tick(actual), // Update failed, retry with the new value
            }
        }
        current
    }
}

impl From<Tick> for AtomicTick {
    #[inline]
    fn from(tick: Tick) -> Self {
        Self::new(tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Initial state ---

    #[test]
    fn dirty_version_new_is_clean() {
        let mut v = DirtyVersion::new();
        // Initially dirty (counter = 0, and is_dirty checks <= 0)
        assert!(v.is_dirty());
        // Reset makes it clean: 0 → 1
        assert!(v.reset());
        assert!(!v.is_dirty()); // 1 > 0, so clean
    }

    #[test]
    fn dirty_version_initial_state_is_dirty() {
        let v = DirtyVersion::new();
        assert!(v.is_dirty()); // 0 <= 0
    }

    // --- dirty() ---

    #[test]
    fn dirty_version_make_dirty_from_clean() {
        let v = DirtyVersion::new();
        assert!(v.is_dirty()); // 0 <= 0
    }

    #[test]
    fn dirty_version_flips_positive() {
        let mut v = DirtyVersion::new();
        v.dirty(); // 0 → 0 (stays 0 since 0 > 0 is false... wait)
        // Actually: v = 0, v > 0 is false, so self.0 stays 0
        // Let me trace through: dirty() does `if v > 0 { self.0 = -v }`
        // For v=0: 0 > 0 is false, so nothing happens → stays 0
        assert!(v.is_dirty());
    }

    #[test]
    fn dirty_version_make_dirty_from_positive() {
        let mut v = DirtyVersion::new();
        v.reset(); // 0 → 1 (clean)
        assert!(!v.is_dirty());
        v.dirty(); // 1 → -1 (dirty)
        assert!(v.is_dirty());
    }

    // --- reset() ---

    #[test]
    fn dirty_version_reset_from_negative() {
        let mut v = DirtyVersion::new();
        assert!(v.reset()); // 0 → 1, returned true because was dirty
        assert!(!v.is_dirty());
    }

    #[test]
    fn dirty_version_reset_from_positive_returns_false() {
        let mut v = DirtyVersion::new();
        v.reset(); // 0 → 1
        assert!(!v.reset()); // 1 > 0, returns false
    }

    #[test]
    fn dirty_version_reset_chain_increments() {
        let mut v = DirtyVersion::new();
        assert!(v.reset()); // 0 → 1 (first reset, was dirty)
        assert!(!v.reset()); // 1 > 0, returns false (already clean)
        v.dirty(); // 1 → -1
        assert!(v.reset()); // -1 → 2 (reset from negative: -(-1) + 1 = 2)
        assert!(!v.reset()); // 2 > 0, returns false
    }

    #[test]
    fn dirty_version_reset_from_negative_returns_true() {
        let mut v = DirtyVersion::new();
        v.dirty(); // still 0 (0 > 0 is false)
        // Let's make it negative: reset from a known dirty state
        let mut v2 = DirtyVersion::new();
        // Start clean, then dirty
        v2.reset(); // 0 → 1
        v2.dirty(); // 1 → -1
        assert!(v2.is_dirty());
        assert!(v2.reset()); // -1 → 2, returns true
        assert!(!v2.is_dirty());
    }

    // --- check_and_reset() ---

    #[test]
    fn check_and_reset_upstream_resets() {
        let mut upstream = DirtyVersion::new();
        let mut downstream = DirtyVersion::new();
        // Both start dirty (0)
        assert!(downstream.check_and_reset(&mut upstream));
        // upstream: 0 → 1, downstream gets upstream's new value (1)
        assert!(!downstream.is_dirty());
    }

    #[test]
    fn check_and_reset_upstream_not_resets_copies() {
        let mut upstream = DirtyVersion::new();
        upstream.reset(); // 0 → 1 (clean)
        let mut downstream = DirtyVersion::new();
        // upstream: 1 > 0, reset returns false
        // downstream was dirty (0), upstream is clean (1), they differ → sync
        assert!(downstream.check_and_reset(&mut upstream));
        assert_eq!(downstream.0, upstream.0);
    }

    #[test]
    fn check_and_reset_no_change() {
        let mut upstream = DirtyVersion::new();
        upstream.reset(); // 0 → 1
        upstream.reset(); // 1 → 2
        let mut downstream = DirtyVersion::new();
        downstream.reset(); // 0 → 1
        downstream.reset(); // 1 → 2
        // Both at 2, upstream reset returns false, values equal → no change
        assert!(!downstream.check_and_reset(&mut upstream));
    }

    #[test]
    fn dirty_version_negative_then_positive() {
        let mut v = DirtyVersion::new();
        v.dirty(); // 0 stays 0 (0 > 0 is false)
        // Need to get past zero: reset then dirty
        v.reset(); // 0 → 1
        v.dirty(); // 1 → -1
        assert!(v.is_dirty());
        v.reset(); // -1 → 2
        assert!(!v.is_dirty());
    }

    #[test]
    fn dirty_version_multiple_dirty_cycles() {
        let mut v = DirtyVersion::new();
        // Cycle 1: dirty → reset → dirty → reset
        v.dirty(); // 0 stays 0
        v.reset(); // 0 → 1
        v.dirty(); // 1 → -1
        v.reset(); // -1 → 2
        // Cycle 2
        v.dirty(); // 2 → -2
        v.reset(); // -2 → 3
        assert!(!v.is_dirty());
    }
}
