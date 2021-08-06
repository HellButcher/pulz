#![warn(
    missing_docs,
    //missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports,
)]
#![doc(html_no_source)]
#![no_std]
#![doc = include_str!("../README.md")]

use core::{
    cmp::max,
    iter::{FromIterator, FusedIterator},
    mem::{replace, ManuallyDrop},
    num::NonZeroU32,
};

use alloc::vec::Vec;

extern crate alloc;

/// A generational index into an [`Arena`]
///
/// You get a new `Index` for each element that you insert
/// into an `Arena`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Index(u32, Generation);

impl Index {
    /// Deconstruct this `Index` into its _offset_ and its _generation_.
    ///
    /// The _offset_ is a position in a continuous array.
    /// When the array-cell at its _offset_ is re-used after it has been
    /// removed, the _generation_ for this cell is incremented.
    #[inline]
    pub const fn into_parts(self) -> (u32, Generation) {
        (self.0, self.1)
    }

    /// Returns the _offset_ of this `Index`.
    ///
    /// The _offset_ is a position in a continuous array.
    #[inline]
    pub fn offset(self) -> u32 {
        self.0
    }

    /// Returns the _generation_ of this `Index`.
    ///
    /// When the array-cell at its _offset_ is re-used after it has been
    /// removed, the _generation_ for this cell is incremented.
    #[inline]
    pub fn generation(self) -> Generation {
        self.1
    }
}

impl core::fmt::Debug for Index {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}v{}", self.0, self.1.get())
    }
}

/// A value type denoting the version or generation of an [`Index`].
///
/// It has an increasing integral and non-zero value. This makes it a candidate
// for _niche_ optimizations.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Generation(NonZeroU32);

impl core::fmt::Debug for Generation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "v{}", self.0.get())
    }
}

impl Generation {
    /// The first/initial generation of an [`Index`].
    // SAFETY: `1` is not zero!
    pub const ONE: Self = Self(unsafe { NonZeroU32::new_unchecked(1) });

    /// Retrieves the value of this `Generation`
    #[inline]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    #[inline]
    const fn is_removed(self) -> bool {
        self.0.get() > (u32::MAX >> 1)
    }

    #[inline]
    const fn next(self) -> Self {
        let value = self.0.get();
        if value > (u32::MAX >> 1) {
            // removed: reactivate by negating
            // SAFETY: generations range from 1 to (u32::MAX-1)
            Self(unsafe { NonZeroU32::new_unchecked(!value) })
        } else if value == (u32::MAX >> 1) {
            Self::ONE // overflow (and skipping 0)
        } else {
            // SAFETY: generations range from 1 to (u32::MAX-1)
            Self(unsafe { NonZeroU32::new_unchecked(value + 1) })
        }
    }

    #[inline]
    const fn removed(self) -> Self {
        let value = self.0.get();
        if value > (u32::MAX >> 1) {
            // already removed
            self
        } else if value == (u32::MAX >> 1) {
            // SAFETY: this is not zero
            Self(unsafe { NonZeroU32::new_unchecked(!1) })
        } else {
            // SAFETY: generations range from 1 to (u32::MAX-1)
            Self(unsafe { NonZeroU32::new_unchecked(!(value + 1)) })
        }
    }

    #[inline]
    fn remove(&mut self) {
        *self = self.removed();
    }

    #[inline]
    fn increment(&mut self) {
        *self = self.next();
    }
}

/// A collection-type with constant insert and remove operations.
///
/// After inserting elements into the Arena, you can use the returned
/// [`Index`] to refer to the newly inserted element in `get` or `remove`
/// operations.
#[derive(Clone)]
pub struct Arena<T> {
    storage: Vec<Entry<T>>,
    next_free: u32,
    len: u32,
}

struct Entry<T>(Generation, EntryData<T>);

union EntryData<T> {
    next_free: u32,
    occupied: ManuallyDrop<T>,
}

impl<T> Entry<T> {
    #[inline]
    pub fn is_removed(&self) -> bool {
        self.0.is_removed()
    }
}

impl<T: Clone> Clone for Entry<T> {
    fn clone(&self) -> Self {
        if self.is_removed() {
            unsafe {
                Self(
                    self.0,
                    EntryData {
                        next_free: self.1.next_free,
                    },
                )
            }
        } else {
            unsafe {
                Self(
                    self.0,
                    EntryData {
                        occupied: self.1.occupied.clone(),
                    },
                )
            }
        }
    }
}

impl<T> Default for Arena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    /// Constructs a new, empty `Arena<T>`.
    ///
    /// The internal vector will not allocate until elements are inserted into
    /// it.
    #[inline]
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            next_free: u32::MAX,
            len: 0,
        }
    }

    /// Constructs a new, empty `Arena<T>` with the specified initial capacity.
    ///
    /// The arena will be able to hold at least `capacity` elements without
    /// reallocating.
    ///
    /// It is important to note that although the returned arena has the
    /// capacity specified, the arena will have a zero length.
    #[inline]
    pub fn with_capacity(capacity: u32) -> Self {
        let mut arena = Self::new();
        arena.reserve(capacity);
        arena
    }

    /// Returns the number of elements the arena can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    /// Reserved capacity for _at least_ `additional` more elements to be
    /// inserted into this arena.
    ///
    /// The internal vector may reserve more space to avoid frequent
    /// reallocations. Does nothing, if the capacity is already sufficient.
    pub fn reserve(&mut self, additional_capacity: u32) {
        let buffer_len = self.storage.len() - self.len as usize;
        if additional_capacity as usize > buffer_len {
            self.storage
                .reserve(additional_capacity as usize - buffer_len);
        }
    }

    /// Reserves the minimum capacity for exactly `additional` more elements to
    /// be inserted into this arena.
    ///
    /// Does nothing, if the capacity is already sufficient.
    pub fn reserve_exact(&mut self, additional_capacity: u32) {
        let buffer_len = self.storage.len() - self.len as usize;
        if additional_capacity as usize > buffer_len {
            self.storage
                .reserve_exact(additional_capacity as usize - buffer_len);
        }
    }

    /// Clears the arena by removing all values.
    ///
    /// Note this method has no effect on the allocated capacity of the arena.
    #[inline]
    pub fn clear(&mut self) {
        self.drain();
    }

    /// Returns the number of elements in this arena.
    ///
    /// Also referred to as its 'length' or 'size'.
    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Returns `true` if the arena contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Attempts to insert `value` into the arena at a free spot.
    ///
    /// If there is a free spot, the provided value is inserted into this spot
    /// and the method returns the `Index` pointing to this spot.
    ///
    /// # Errors
    ///
    /// If there is no free spot, it will not try to allocate new capacity.
    /// In this case, it returns `Err(value)` with the provided
    /// `value`, to give back ownership to the caller.
    pub fn try_insert(&mut self, value: T) -> Result<Index, T> {
        if let Some((offset, generation, entry)) = self.take_next_free() {
            entry.occupied = ManuallyDrop::new(value);
            generation.increment();
            Ok(Index(offset, *generation))
        } else {
            Err(value)
        }
    }

    /// Attempts to insert a new value returned by `create` into the arena at
    /// a free spot.
    ///
    /// If there is a free spot, the created value is inserted into this spot
    /// and the method returns the `Index` pointing to this spot.
    ///
    /// The `create` method is called with the `Index` of the spot, where the
    /// created value will be inserted. This allows the value to be aware of
    /// it's own index.
    ///
    /// # Errors
    ///
    /// If there is no free spot, it will not try to allocate new capacity.
    /// In this case, it returns `Err(create)` with the provided
    /// `create` function, to give back ownership to the caller.
    pub fn try_insert_with<F>(&mut self, create: F) -> Result<Index, F>
    where
        F: FnOnce(Index) -> T,
    {
        if let Some((offset, generation, entry)) = self.take_next_free() {
            let new_generation = generation.next();
            let index = Index(offset, new_generation);

            let value = create(index);
            entry.occupied = ManuallyDrop::new(value);
            *generation = new_generation;
            Ok(index)
        } else {
            Err(create)
        }
    }

    /// Inserts `value` into the arena, allocating more capacity if necessary.
    ///
    /// The provided value is inserted into a free spot or into a newly
    /// allocated spot and the method returns the `Index` pointing to this spot.
    pub fn insert(&mut self, value: T) -> Index {
        if let Some((offset, generation, entry)) = self.take_next_free() {
            entry.occupied = ManuallyDrop::new(value);
            generation.increment();
            Index(offset, *generation)
        } else {
            let (offset, generation, entry) = self.alloc_new_entry();
            entry.occupied = ManuallyDrop::new(value);
            generation.increment();
            Index(offset, *generation)
        }
    }

    /// Inserts a new value returned by `create` into the arena , allocating
    /// more capacity if necessary.
    ///
    /// The provided value is inserted into a free spot or into a newly
    /// allocated spot and the method returns the `Index` pointing to this spot.
    ///
    /// The `create` method is called with the `Index` of the spot, where the
    /// created value will be inserted. This allows the value to be aware of
    /// it's own index.
    pub fn insert_with<F>(&mut self, create: F) -> Index
    where
        F: FnOnce(Index) -> T,
    {
        if let Some((offset, generation, entry)) = self.take_next_free() {
            let new_generation = generation.next();
            let index = Index(offset, new_generation);

            let value = create(index);
            entry.occupied = ManuallyDrop::new(value);
            *generation = new_generation;
            index
        } else {
            let (offset, generation, entry) = self.alloc_new_entry();
            let new_generation = generation.next();
            let index = Index(offset, new_generation);

            let value = create(index);
            entry.occupied = ManuallyDrop::new(value);
            *generation = new_generation;
            index
        }
    }

    #[inline]
    fn take_next_free(&mut self) -> Option<(u32, &mut Generation, &mut EntryData<T>)> {
        let next_free_head = &mut self.next_free;
        let next_free = *next_free_head;
        self.storage
            .get_mut(next_free as usize)
            .map(|Entry(generation, entry)| {
                // SAFETY: entry was in the free-list: so we can use `next_free`
                *next_free_head = unsafe { entry.next_free };
                *generation = generation.next();
                (next_free, generation, entry)
            })
    }

    #[inline]
    fn alloc_new_entry(&mut self) -> (u32, &mut Generation, &mut EntryData<T>) {
        let next_offset = self.storage.len();
        self.storage.push(Entry(
            Generation::ONE,
            EntryData {
                next_free: u32::MAX,
            },
        ));
        let entry = unsafe { self.storage.get_unchecked_mut(next_offset) };
        (next_offset as u32, &mut entry.0, &mut entry.1)
    }

    /// Removes the element at the given `index` from this arena.
    ///
    /// The method returns the old value, if it is still in the arena.
    /// If it is not in the arena, then `None` is returned.
    pub fn remove(&mut self, index: Index) -> Option<T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.storage.get_mut(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                entry_gen.remove();
                // SAFETY: user has an index with current generation: item was occupied
                let value = unsafe { ManuallyDrop::take(&mut entry.occupied) };
                entry.next_free = replace(&mut self.next_free, offset);
                Some(value)
            }
            _ => None,
        }
    }

    /// Checks, if the element at the given `index` is still in the arena.
    ///
    /// Returns `true` if there is a element for the given `index`.
    #[inline]
    pub fn contains(&self, index: Index) -> bool {
        self.get(index).is_some()
    }

    /// Get a shared reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    pub fn get(&self, index: Index) -> Option<&T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.storage.get(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                // SAFETY: user has an index with current generation: item was occupied
                Some(unsafe { &entry.occupied })
            }
            _ => None,
        }
    }

    /// Get a exclusive reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.storage.get_mut(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                // SAFETY: user has an index with current generation: item was occupied
                Some(unsafe { &mut entry.occupied })
            }
            _ => None,
        }
    }

    /// Creates a _draining_ iterator that removes all elements from this arena
    /// and yields the removed items.
    ///
    /// When the iterator is dropped, all the remaining elements are removed and
    /// dropped!
    pub fn drain(&mut self) -> Drain<'_, T> {
        let len = replace(&mut self.len, 0);
        self.next_free = u32::MAX;
        Drain {
            len: len as usize,
            inner: self.storage.drain(..).enumerate(),
        }
    }

    /// Creates an shared iterator over the elements of this arena.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            len: self.len as usize,
            inner: self.storage.iter().enumerate(),
        }
    }

    /// Creates an exclusive iterator over the elements of this arena.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            len: self.len as usize,
            inner: self.storage.iter_mut().enumerate(),
        }
    }
}

impl<T> core::ops::Index<Index> for Arena<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: Index) -> &T {
        self.get(index).expect("invalid index")
    }
}

impl<T> core::ops::IndexMut<Index> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, index: Index) -> &mut T {
        self.get_mut(index).expect("invalid index")
    }
}

impl<T> Extend<T> for Arena<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<T> FromIterator<T> for Arena<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let cap = upper.unwrap_or(lower);
        let cap = max(cap, 1);
        debug_assert!(cap <= u32::MAX as usize);
        let mut arena = Self::with_capacity(cap as u32);
        arena.extend(iter);
        arena
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// A draining iterator for `Arena<T>` created by [`Arena::drain`].
pub struct Drain<'a, T> {
    len: usize,
    inner: core::iter::Enumerate<alloc::vec::Drain<'a, Entry<T>>>,
}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = (Index, T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, mut entry))) => {
                    let idx = Index(offset as u32, gen);
                    // SAFETY: entry was not marked as removed, so it is occupied
                    let value = unsafe { ManuallyDrop::take(&mut entry.occupied) };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for Drain<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, mut entry))) => {
                    let idx = Index(offset as u32, gen);
                    // SAFETY: entry was not marked as removed, so it is occupied
                    let value = unsafe { ManuallyDrop::take(&mut entry.occupied) };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> FusedIterator for Drain<'a, T> {}

impl<'a, T> ExactSizeIterator for Drain<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        for item in self {
            drop(item);
        }
    }
}

/// An immutable iterator for `Arena<T>` created by [`Arena::iter`].
pub struct Iter<'a, T> {
    len: usize,
    inner: core::iter::Enumerate<core::slice::Iter<'a, Entry<T>>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Index, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, entry))) => {
                    let idx = Index(offset as u32, *gen);
                    // SAFETY: entry was not removed: so it is occupied
                    let value = unsafe { &entry.occupied };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, entry))) => {
                    let idx = Index(offset as u32, *gen);
                    // SAFETY: entry was not removed: so it is occupied
                    let value = unsafe { &entry.occupied };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

/// An mutable iterator for `Arena<T>` created by [`Arena::iter_mut`].
pub struct IterMut<'a, T> {
    len: usize,
    inner: core::iter::Enumerate<core::slice::IterMut<'a, Entry<T>>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, entry))) => {
                    let idx = Index(offset as u32, *gen);
                    // SAFETY: entry was not removed: so it is occupied
                    let value = unsafe { &mut entry.occupied };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, entry)) if entry.is_removed() => continue,
                Some((offset, Entry(gen, entry))) => {
                    let idx = Index(offset as u32, *gen);
                    // SAFETY: entry was not removed: so it is occupied
                    let value = unsafe { &mut entry.occupied };
                    self.len -= 1;
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}
