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
