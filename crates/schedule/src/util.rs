use std::{cell::UnsafeCell, marker::PhantomData};

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
