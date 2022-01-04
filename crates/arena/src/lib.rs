#![warn(
    missing_docs,
    rustdoc::missing_doc_code_examples,
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
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![no_std]
#![doc = include_str!("../README.md")]

use core::{
    cmp::max,
    iter::{FromIterator, FusedIterator},
    mem::{replace, ManuallyDrop},
    num::NonZeroU32,
    ops::DerefMut,
};

use alloc::vec::Vec;

extern crate alloc;

/// A generational index into an [`Arena`]
///
/// You get a new `Index` for each element that you insert
/// into an `Arena`.
///
/// # Example
///
/// ```
/// # use pulz_arena::Arena;
/// let mut arena = Arena::new();
/// let index = arena.insert("test");
/// assert_eq!("test", arena[index]);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Index(u32, Generation);

impl Index {
    /// Deconstruct this `Index` into its _offset_ and its _generation_.
    ///
    /// The _offset_ is a position in a continuous array.
    /// When the array-cell at its _offset_ is re-used after it has been
    /// removed, the _generation_ for this cell is incremented.
    ///
    /// The _offset_ is a position in a continuous array.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Generation};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("test2");
    /// assert_eq!((0, Generation::ONE), index0.into_parts());
    /// assert_eq!((1, Generation::ONE), index1.into_parts());
    /// ```
    #[inline]
    pub const fn into_parts(self) -> (u32, Generation) {
        (self.0, self.1)
    }

    /// Returns the _offset_ of this `Index`.
    ///
    /// The _offset_ is a position in a continuous array.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Generation};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("test2");
    /// assert_eq!(0, index0.offset());
    /// assert_eq!(1, index1.offset());
    /// ```
    #[inline]
    pub const fn offset(self) -> u32 {
        self.0
    }

    /// Returns the _generation_ of this `Index`.
    ///
    /// When the array-cell at its _offset_ is re-used after it has been
    /// removed, the _generation_ for this cell is incremented.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Generation};
    /// let mut arena = Arena::new();
    /// let index = arena.insert("test");
    /// assert_eq!(Generation::ONE, index.generation());
    /// arena.remove(index);
    /// let index = arena.insert("test2");
    /// assert_eq!(2, index.generation().get());
    /// ```
    #[inline]
    pub const fn generation(self) -> Generation {
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
///
/// # Example
///
/// ```
/// # use pulz_arena::{Arena,Generation};
/// let mut arena = Arena::new();
/// let index = arena.insert("test");
/// assert_eq!(Generation::ONE, index.generation());
/// ```
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

    // SAFETY: `!1u32` is not zero!
    const NEW: Self = Self(unsafe { NonZeroU32::new_unchecked(!1u32) });

    // SAFETY: `u32::MAX >> 1` is not zero!
    #[cfg(test)]
    const MAX: Self = Self(unsafe { NonZeroU32::new_unchecked(u32::MAX >> 1) });

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
///
/// # Example
///
/// ```
/// use pulz_arena::Arena;
///
/// let mut arena = Arena::new();
/// let index = arena.insert("test");
/// assert_eq!(1, arena.len());
/// ```
#[derive(Clone)]
pub struct Arena<T> {
    storage: Storage<T>,
    next_free: u32,
}

#[derive(Clone)]
struct Storage<T> {
    data: Vec<Entry<T>>,
    len: usize,
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

impl<T> Storage<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            data: Vec::new(),
            len: 0,
        }
    }
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }
    pub fn reserve(&mut self, additional_capacity: usize) {
        let buffer_len = self.data.len() - self.len;
        if additional_capacity > buffer_len {
            self.data.reserve(additional_capacity - buffer_len);
        }
    }
    pub fn reserve_exact(&mut self, additional_capacity: usize) {
        let buffer_len = self.data.len() - self.len;
        if additional_capacity > buffer_len {
            self.data.reserve_exact(additional_capacity - buffer_len);
        }
    }
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push_new_free_entry(
        &mut self,
        alloc: bool,
        next_free: u32,
    ) -> Option<(u32, &mut Generation, &mut EntryData<T>)> {
        let next_offset = self.data.len();
        if alloc || next_offset < self.data.capacity() {
            self.data
                .push(Entry(Generation::NEW, EntryData { next_free }));
            // SAFETY: we just have created the element at next_offset
            let entry = unsafe { self.data.get_unchecked_mut(next_offset) };
            Some((next_offset as u32, &mut entry.0, &mut entry.1))
        } else {
            None
        }
    }

    pub fn remove(&mut self, index: Index, next_free: &mut u32) -> Option<T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.data.get_mut(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                entry_gen.remove();
                self.len -= 1;
                // SAFETY: user has an index with current generation: item was occupied
                let value = unsafe { ManuallyDrop::take(&mut entry.occupied) };
                entry.next_free = replace(next_free, offset);
                Some(value)
            }
            _ => None,
        }
    }

    pub fn get(&self, index: Index) -> Option<&T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.data.get(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                // SAFETY: user has an index with current generation: item was occupied
                Some(unsafe { &entry.occupied })
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        let (offset, generation) = index.into_parts();
        debug_assert!(!generation.is_removed());
        match self.data.get_mut(offset as usize) {
            Some(Entry(entry_gen, entry)) if *entry_gen == generation => {
                // SAFETY: user has an index with current generation: item was occupied
                Some(unsafe { &mut entry.occupied })
            }
            _ => None,
        }
    }

    pub fn get_by_offset(&self, offset: u32) -> Option<&T> {
        match self.data.get(offset as usize) {
            Some(Entry(entry_gen, entry)) if !entry_gen.is_removed() => {
                // SAFETY: index is not marked as removed: item was occupied
                Some(unsafe { &entry.occupied })
            }
            _ => None,
        }
    }

    pub fn get_mut_by_offset(&mut self, offset: u32) -> Option<&mut T> {
        match self.data.get_mut(offset as usize) {
            Some(Entry(entry_gen, entry)) if !entry_gen.is_removed() => {
                // SAFETY: index is not marked as removed: item was occupied
                Some(unsafe { &mut entry.occupied })
            }
            _ => None,
        }
    }

    pub fn drain(&mut self) -> Drain<'_, T> {
        let len = replace(&mut self.len, 0);
        Drain {
            len,
            inner: self.data.drain(..).enumerate(),
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            len: self.len,
            inner: self.data.iter().enumerate(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            len: self.len,
            inner: self.data.iter_mut().enumerate(),
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
    ///
    /// # Example
    ///
    /// ```
    /// use pulz_arena::Arena;
    ///
    /// let arena = Arena::<u32>::new();
    /// assert!(arena.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            storage: Storage::new(),
            next_free: u32::MAX,
        }
    }

    /// Constructs a new, empty `Arena<T>` with the specified initial capacity.
    ///
    /// The arena will be able to hold at exactly `capacity` elements without
    /// reallocating.
    ///
    /// It is important to note that although the returned arena has the
    /// capacity specified, the arena will have a zero length.
    ///
    /// Example
    ///
    /// ```
    /// use pulz_arena::Arena;
    ///
    /// let mut arena = Arena::with_capacity(15);
    /// assert_eq!(15, arena.capacity());
    ///
    /// // `try_insert` does not allocate
    /// for i in 0..15 {
    ///     assert!(arena.try_insert(i).is_ok());
    ///     assert_eq!(15, arena.capacity());
    /// }
    ///
    /// assert!(arena.try_insert(16).is_err());
    /// assert_eq!(15, arena.capacity());
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut arena = Self::new();
        arena.reserve_exact(capacity);
        arena
    }

    /// Returns the number of elements the arena can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Reserved capacity for _at least_ `additional` more elements to be
    /// inserted into this arena.
    ///
    /// The internal vector may reserve more space to avoid frequent
    /// reallocations. Does nothing, if the capacity is already sufficient.
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.capacity());
    /// arena.insert(1);
    /// arena.reserve(15);
    /// assert!(arena.capacity() >= 16);
    /// assert_eq!(1, arena.len());
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.storage.reserve(additional_capacity)
    }

    /// Reserves the minimum capacity for exactly `additional` more elements to
    /// be inserted into this arena.
    ///
    /// Does nothing, if the capacity is already sufficient.
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.capacity());
    /// arena.insert(1);
    /// arena.reserve_exact(15);
    /// assert_eq!(16, arena.capacity());
    /// assert_eq!(1, arena.len());
    /// ```
    #[inline]
    pub fn reserve_exact(&mut self, additional_capacity: usize) {
        self.storage.reserve_exact(additional_capacity)
    }

    /// Clears the arena by removing all values.
    ///
    /// Note this method has no effect on the allocated capacity of the arena.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// arena.insert("test");
    /// arena.insert("foo");
    /// arena.insert("bar");
    /// assert!(!arena.is_empty());
    /// arena.clear();
    /// assert!(arena.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.drain();
    }

    /// Returns the number of elements in this arena.
    ///
    /// Also referred to as its 'length' or 'size'.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.insert("test");
    /// assert_eq!(1, arena.len());
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!(Some("test"), arena.remove(index0));
    /// assert_eq!(1, arena.len());
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns `true` if the arena contains no elements.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert!(arena.is_empty());
    /// let index0 = arena.insert("test");
    /// assert!(!arena.is_empty());
    /// let index1 = arena.insert("test2");
    /// assert!(!arena.is_empty());
    /// arena.remove(index0);
    /// assert!(!arena.is_empty());
    /// arena.remove(index1);
    /// assert!(arena.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.storage.is_empty()
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
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// arena.reserve_exact(15);
    ///
    /// // `try_insert` does not allocate
    /// for i in 0..15 {
    ///     assert!(arena.try_insert(i).is_ok());
    ///     assert_eq!(15, arena.capacity());
    /// }
    ///
    /// assert!(arena.try_insert(16).is_err());
    /// assert_eq!(15, arena.capacity());
    /// ```
    pub fn try_insert(&mut self, value: T) -> Result<Index, T> {
        if let Some((offset, generation, entry)) = self.take_next_free(false) {
            entry.occupied = ManuallyDrop::new(value);
            generation.increment();
            let index = Index(offset, *generation);
            self.storage.len += 1;
            Ok(index)
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
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Index};
    /// struct Element {
    ///     index: Index,
    ///     value: usize,
    /// }
    /// let mut arena = Arena::new();
    /// arena.reserve_exact(3);
    ///
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.try_insert_with(|i| Element{ index: i, value: 42 }).ok().unwrap();
    /// let index1 = arena.try_insert_with(|i| Element{ index: i, value: 666 }).ok().unwrap();
    /// let index2 = arena.try_insert_with(|i| Element{ index: i, value: 42 }).ok().unwrap();
    /// assert_eq!(3, arena.len());
    ///
    /// assert!(arena.try_insert_with(|i| Element{ index: i, value: 99 }).is_err());
    /// assert_eq!(3, arena.capacity());
    ///
    /// assert_eq!(index0, arena[index0].index);
    /// assert_eq!(index1, arena[index1].index);
    /// assert_eq!(index2, arena[index2].index);
    /// ```
    pub fn try_insert_with<F>(&mut self, create: F) -> Result<Index, F>
    where
        F: FnOnce(Index) -> T,
    {
        if let Some((offset, generation, entry)) = self.take_next_free(false) {
            let new_generation = generation.next();
            let index = Index(offset, new_generation);
            let value = create(index);
            entry.occupied = ManuallyDrop::new(value);
            *generation = new_generation;
            self.storage.len += 1;
            Ok(index)
        } else {
            Err(create)
        }
    }

    /// Inserts `value` into the arena, allocating more capacity if necessary.
    ///
    /// The provided value is inserted into a free spot or into a newly
    /// allocated spot and the method returns the `Index` pointing to this spot.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!("test", arena[index0]);
    /// assert_eq!("foo", arena[index1]);
    /// ```
    #[allow(clippy::missing_panics_doc)] // doesn't panic when alloc=true
    pub fn insert(&mut self, value: T) -> Index {
        let (offset, generation, entry) = self.take_next_free(true).unwrap();
        entry.occupied = ManuallyDrop::new(value);
        generation.increment();
        let index = Index(offset, *generation);
        self.storage.len += 1;
        index
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
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Index};
    /// struct Element {
    ///     index: Index,
    ///     value: usize,
    /// }
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.insert_with(|i| Element{ index: i, value: 42 });
    /// let index1 = arena.insert_with(|i| Element{ index: i, value: 666 });
    /// assert_eq!(2, arena.len());
    /// assert_eq!(index0, arena[index0].index);
    /// assert_eq!(index1, arena[index1].index);
    /// ```
    #[allow(clippy::missing_panics_doc)] // doesn't panic when alloc=true
    pub fn insert_with<F>(&mut self, create: F) -> Index
    where
        F: FnOnce(Index) -> T,
    {
        let (offset, generation, entry) = self.take_next_free(true).unwrap();
        let new_generation = generation.next();
        let index = Index(offset, new_generation);
        let value = create(index);
        entry.occupied = ManuallyDrop::new(value);
        *generation = new_generation;
        self.storage.len += 1;
        index
    }

    #[inline]
    fn take_next_free(&mut self, alloc: bool) -> Option<(u32, &mut Generation, &mut EntryData<T>)> {
        let storage = &mut self.storage;
        let next_free_head = &mut self.next_free;
        let next_free = *next_free_head as usize;
        if next_free < storage.data.len() {
            // SAFETY: we have checked for next_free<len
            let Entry(generation, entry) = unsafe { storage.data.get_unchecked_mut(next_free) };
            // SAFETY: entry was in the free-list: so we can use `next_free`
            *next_free_head = unsafe { entry.next_free };
            return Some((next_free as u32, generation, entry));
        }
        storage.push_new_free_entry(alloc, u32::MAX)
    }

    /// Removes the element at the given `index` from this arena.
    ///
    /// The method returns the old value, if it is still in the arena.
    /// If it is not in the arena, then `None` is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!(Some("test"), arena.remove(index0));
    /// assert_eq!(1, arena.len());
    /// // removing it a second time returns `None`
    /// assert_eq!(None, arena.remove(index0));
    /// assert_eq!(1, arena.len());
    /// ```
    #[inline]
    pub fn remove(&mut self, index: Index) -> Option<T> {
        self.storage.remove(index, &mut self.next_free)
    }

    /// Checks, if the element at the given `index` is still in the arena.
    ///
    /// Returns `true` if there is a element for the given `index`.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// assert_eq!(0, arena.len());
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert!(arena.contains(index0));
    /// assert!(arena.contains(index1));
    /// assert_eq!(Some("test"), arena.remove(index0));
    /// assert_eq!(1, arena.len());
    /// assert!(!arena.contains(index0)); // element not in the arena
    /// assert!(arena.contains(index1));
    /// ```
    #[inline]
    pub fn contains(&self, index: Index) -> bool {
        self.get(index).is_some()
    }

    /// Get a shared reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!(Some(&"test"), arena.get(index0));
    /// assert_eq!(Some(&"foo"), arena.get(index1));
    /// assert_eq!(2, arena.len());
    /// assert_eq!(Some("test"), arena.remove(index0));
    /// assert_eq!(1, arena.len());
    /// assert_eq!(None, arena.get(index0));
    /// ```
    #[inline]
    pub fn get(&self, index: Index) -> Option<&T> {
        self.storage.get(index)
    }

    /// Get a exclusive reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!("test", arena[index0]);
    /// let element = arena.get_mut(index0).unwrap();
    /// *element = "bar";
    /// assert_eq!("bar", arena[index0]);
    /// ```
    #[inline]
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.storage.get_mut(index)
    }

    /// Get a shared reference to the element at the given `offset` (the offset into the arena).
    ///
    /// If there is no element at the given `offset`, None is returned.
    ///
    #[inline]
    pub fn get_by_offset(&self, offset: u32) -> Option<&T> {
        self.storage.get_by_offset(offset)
    }

    /// Get a exclusive reference to the element at the given `offset` (the offset into the arena).
    ///
    /// If there is no element at the given `offset`, None is returned.
    ///
    #[inline]
    pub fn get_mut_by_offset(&mut self, offset: u32) -> Option<&mut T> {
        self.storage.get_mut_by_offset(offset)
    }

    /// Creates a _draining_ iterator that removes all elements from this arena
    /// and yields the removed items.
    ///
    /// When the iterator is dropped, all the remaining elements are removed and
    /// dropped!
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// assert_eq!(3, arena.len());
    /// for (i, (index, element)) in arena.drain().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    assert_eq!(i, element);
    /// }
    /// assert!(arena.is_empty());
    /// ```
    #[inline]
    pub fn drain(&mut self) -> Drain<'_, T> {
        self.next_free = u32::MAX;
        self.storage.drain()
    }

    /// Creates an shared iterator over the elements of this arena.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// assert_eq!(3, arena.len());
    /// for (i, (index, element)) in arena.iter().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    assert_eq!(i, *element);
    /// }
    /// assert_eq!(3, arena.len());
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.storage.iter()
    }

    /// Creates an exclusive iterator over the elements of this arena.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::Arena;
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// assert_eq!(3, arena.len());
    /// for (i, (index, element)) in arena.iter_mut().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    *element *= 3;
    /// }
    /// assert_eq!(3, arena.len());
    /// assert_eq!(0, arena[indices[0]]);
    /// assert_eq!(3, arena[indices[1]]);
    /// assert_eq!(6, arena[indices[2]]);
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.storage.iter_mut()
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

impl<T> core::ops::Index<u32> for Arena<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: u32) -> &T {
        self.get_by_offset(index).expect("invalid offset")
    }
}

impl<T> core::ops::IndexMut<u32> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, index: u32) -> &mut T {
        self.get_mut_by_offset(index).expect("invalid offset")
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
        let mut arena = Self::with_capacity(cap);
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

/// A `Arena`-like collection-type that shares the Indices with an `Arena`.
///
/// After inserting elements into a `Arena`, you can use the returned
/// [`Index`] of the Arena to insert an element into a `Mirror` and to refer to
/// the newly inserted element in `get` or `remove` operations.
///
/// # Example
///
/// ```
/// use pulz_arena::{Arena,Mirror};
///
/// let mut arena: Arena<&str> = Arena::new();
/// let index = arena.insert("test");
///
/// let mut mirror: Mirror<u32> = Mirror::new();
/// mirror.insert(index, 123);
/// assert_eq!(1, mirror.len());
///
/// assert_eq!("test", arena[index]);
/// assert_eq!(123, mirror[index]);
/// ```
#[derive(Clone)]
pub struct Mirror<T>(Storage<T>);

impl<T> Mirror<T> {
    /// Constructs a new, empty `Mirror<T>`.
    ///
    /// The internal vector will not allocate until elements are inserted into
    /// it.
    ///
    /// # Example
    ///
    /// ```
    /// use pulz_arena::Mirror;
    ///
    /// let mirror = Mirror::<u32>::new();
    /// assert!(mirror.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self(Storage::new())
    }

    /// Constructs a new, empty `Mirror<T>` with the specified initial capacity.
    ///
    /// The mirror will be able to hold at exactly `capacity` elements without
    /// reallocating.
    ///
    /// It is important to note that although the returned mirror has the
    /// capacity specified, the mirror will have a zero length.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut mirror = Self::new();
        mirror.reserve_exact(capacity);
        mirror
    }

    /// Returns the number of elements the arena can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Reserved capacity for _at least_ `additional` more elements to be
    /// inserted into this mirror.
    ///
    /// The internal vector may reserve more space to avoid frequent
    /// reallocations. Does nothing, if the capacity is already sufficient.
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert(42);
    ///
    /// let mut mirror = Mirror::new();
    /// assert_eq!(0, mirror.capacity());
    /// mirror.insert(index0, "foo");
    /// mirror.reserve_exact(15);
    /// assert!(mirror.capacity() >= 16);
    /// assert_eq!(1, mirror.len());
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.0.reserve(additional_capacity)
    }

    /// Reserves the minimum capacity for exactly `additional` more elements to
    /// be inserted into this mirror.
    ///
    /// Does nothing, if the capacity is already sufficient.
    ///
    /// Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert(42);
    ///
    /// let mut mirror = Mirror::new();
    /// assert_eq!(0, mirror.capacity());
    /// mirror.insert(index0, "foo");
    /// mirror.reserve_exact(15);
    /// assert_eq!(16, mirror.capacity());
    /// assert_eq!(1, mirror.len());
    /// ```
    #[inline]
    pub fn reserve_exact(&mut self, additional_capacity: usize) {
        self.0.reserve_exact(additional_capacity)
    }

    /// Clears the mirror by removing all values.
    ///
    /// Note this method has no effect on the allocated capacity of the mirror.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert(42);
    /// let index1 = arena.insert(123);
    /// let index2 = arena.insert(456);
    ///
    /// let mut mirror = Mirror::new();
    /// mirror.insert(index0, "test");
    /// mirror.insert(index1, "foo");
    /// mirror.insert(index2, "bar");
    /// assert!(!mirror.is_empty());
    /// mirror.clear();
    /// assert!(mirror.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.drain();
    }

    /// Returns the number of elements in this mirror.
    ///
    /// Also referred to as its 'length' or 'size'.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert(42);
    /// let index1 = arena.insert(123);
    ///
    /// let mut mirror = Mirror::new();
    /// assert_eq!(0, mirror.len());
    /// mirror.insert(index0, "test");
    /// assert_eq!(1, mirror.len());
    /// mirror.insert(index1, "foo");
    /// assert_eq!(2, mirror.len());
    /// assert_eq!(Some("test"), mirror.remove(index0));
    /// assert_eq!(1, mirror.len());
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the mirror contains no elements.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let index0 = arena.insert(42);
    ///
    /// let mut mirror = Mirror::new();
    /// assert!(mirror.is_empty());
    /// mirror.insert(index0, "test");
    /// assert!(!mirror.is_empty());
    /// mirror.remove(index0);
    /// assert!(mirror.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Inserts `value` into the arena at the given index,
    /// allocating more capacity if necessary.
    ///
    /// * If the spot is not occupied, the value is inserted and `Ok(None)` is
    ///   returned.
    /// * If the spot is occupied by an entry with the an older or equal
    ///   `Generation`, the entry is replaced and the old value is returned
    ///   as `Ok(Some(T))`
    ///
    /// # Errors
    ///
    /// * If the spot at the index is already occupied by an entry with a newer
    ///   `Generation`, a reference pointing to the existing entry is returned
    ///   as `Err(&mut T)`.
    ///
    /// This function can be used to create multiple `Arena`s of different entries,
    /// that share the same identity:
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena: Arena<&str> = Arena::new();
    /// let mut mirror: Mirror<u32> = Mirror::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(2, arena.len());
    /// assert_eq!(0, mirror.len());
    /// assert_eq!(Ok(None), mirror.insert(index0, 1));
    /// assert_eq!(Ok(None), mirror.insert(index1, 2));
    /// assert_eq!(2, mirror.len());
    /// assert_eq!("test", arena[index0]);
    /// assert_eq!(1, mirror[index0]);
    /// assert_eq!("foo", arena[index1]);
    /// assert_eq!(2, mirror[index1]);
    /// ```
    pub fn insert(&mut self, index: Index, value: T) -> Result<Option<T>, &mut T> {
        let offset = index.offset() as usize;
        // allocate free entries until offset
        if self.0.data.len() <= offset {
            self.0.data.resize_with(offset + 1, || {
                Entry(
                    Generation::NEW,
                    EntryData {
                        next_free: u32::MAX,
                    },
                )
            });
        }

        // SAFETY: we allocated free entries until offset
        let entry = unsafe { self.0.data.get_unchecked_mut(offset) };

        let new_generation = index.generation();
        if entry.is_removed() {
            // replace (including generation)
            entry.0 = new_generation;
            entry.1.occupied = ManuallyDrop::new(value);
            self.0.len += 1;
            Ok(None)
        } else if entry.0 <= new_generation {
            entry.0 = new_generation;
            // SAFETY: entry is occupied (not removed)
            let old_value = replace(unsafe { entry.1.occupied.deref_mut() }, value);
            Ok(Some(old_value))
        } else {
            // SAFETY: entry is occupied (not removed)
            let old_ref = unsafe { entry.1.occupied.deref_mut() };
            Err(old_ref)
        }
    }

    /// Removes the element at the given `index` from this arena.
    ///
    /// The method returns the old value, if it is still in the arena.
    /// If it is not in the arena, then `None` is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena: Arena<&str> = Arena::new();
    /// let mut mirror: Mirror<u32> = Mirror::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// mirror.insert(index0, 1);
    /// mirror.insert(index1, 2);
    /// assert_eq!(2, mirror.len());
    /// assert_eq!(1, mirror[index0]);
    /// assert_eq!(2, mirror[index1]);
    ///
    /// assert_eq!(Some(2), mirror.remove(index1));
    /// assert_eq!(1, mirror.len());
    /// ```
    #[inline]
    pub fn remove(&mut self, index: Index) -> Option<T> {
        let mut dummy = u32::MAX;
        self.0.remove(index, &mut dummy)
    }

    /// Checks, if the element at the given `index` is still in the arena.
    ///
    /// Returns `true` if there is a element for the given `index`.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena: Arena<&str> = Arena::new();
    /// let mut mirror: Mirror<u32> = Mirror::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert!(!mirror.contains(index0));
    /// assert!(!mirror.contains(index1));
    /// mirror.insert(index0, 1);
    /// mirror.insert(index1, 2);
    /// assert!(mirror.contains(index0));
    /// assert!(mirror.contains(index1));
    /// assert_eq!(Some(1), mirror.remove(index0));
    /// assert!(!mirror.contains(index0)); // element not in the mirror
    /// assert!(mirror.contains(index1));
    /// ```
    #[inline]
    pub fn contains(&self, index: Index) -> bool {
        self.get(index).is_some()
    }

    /// Get a shared reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena: Arena<&str> = Arena::new();
    /// let mut mirror: Mirror<u32> = Mirror::new();
    /// let index0 = arena.insert("test");
    /// let index1 = arena.insert("foo");
    /// assert_eq!(None, mirror.get(index0));
    /// assert_eq!(None, mirror.get(index1));
    /// mirror.insert(index0, 1);
    /// mirror.insert(index1, 2);
    /// assert_eq!(Some(&1), mirror.get(index0));
    /// assert_eq!(Some(&2), mirror.get(index1));
    /// ```
    #[inline]
    pub fn get(&self, index: Index) -> Option<&T> {
        self.0.get(index)
    }

    /// Get a exclusive reference to the element at the given `index`.
    ///
    /// If there is no element at the given `index`, None is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena: Arena<&str> = Arena::new();
    /// let mut mirror: Mirror<u32> = Mirror::new();
    /// let index = arena.insert("test");
    /// assert_eq!(None, mirror.get_mut(index));
    /// mirror.insert(index, 1);
    /// assert_eq!(1, mirror[index]);
    /// let entry = mirror.get_mut(index).unwrap();
    /// *entry = 123;
    /// assert_eq!(123, mirror[index]);
    /// ```
    #[inline]
    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.0.get_mut(index)
    }

    /// Get a shared reference to the element at the given `offset` (the offset into the arena).
    ///
    /// If there is no element at the given `offset`, None is returned.
    ///
    #[inline]
    pub fn get_by_offset(&self, offset: u32) -> Option<&T> {
        self.0.get_by_offset(offset)
    }

    /// Get a exclusive reference to the element at the given `offset` (the offset into the arena).
    ///
    /// If there is no element at the given `offset`, None is returned.
    ///
    #[inline]
    pub fn get_mut_by_offset(&mut self, offset: u32) -> Option<&mut T> {
        self.0.get_mut_by_offset(offset)
    }

    /// Creates a _draining_ iterator that removes all elements from this arena
    /// and yields the removed items.
    ///
    /// When the iterator is dropped, all the remaining elements are removed and
    /// dropped!
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// assert_eq!(3, arena.len());
    /// let mut mirror = Mirror::new();
    /// for (i, &index) in indices.iter().enumerate() {
    ///    mirror.insert(index, i*2);
    /// }
    /// assert_eq!(3, mirror.len());
    /// for (i, (index, element)) in mirror.drain().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    assert_eq!(i * 2, element);
    /// }
    /// assert!(mirror.is_empty());
    /// assert_eq!(3, arena.len());
    /// ```
    #[inline]
    pub fn drain(&mut self) -> Drain<'_, T> {
        self.0.drain()
    }

    /// Creates an shared iterator over the elements of this arena.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};;
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// let mut mirror = Mirror::new();
    /// for (i, &index) in indices.iter().enumerate() {
    ///    mirror.insert(index, i*2);
    /// }
    /// assert_eq!(3, mirror.len());
    /// for (i, (index, &element)) in mirror.iter().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    assert_eq!(i * 2, element);
    /// }
    /// assert_eq!(3, mirror.len());
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    /// Creates an exclusive iterator over the elements of this arena.
    ///
    /// # Example
    ///
    /// ```
    /// # use pulz_arena::{Arena,Mirror};;
    /// let mut arena = Arena::new();
    /// let indices = [
    ///     arena.insert(0),
    ///     arena.insert(1),
    ///     arena.insert(2),
    /// ];
    /// let mut mirror = Mirror::new();
    /// for (i, &index) in indices.iter().enumerate() {
    ///    mirror.insert(index, i*2);
    /// }
    /// assert_eq!(3, mirror.len());
    /// for (i, (index, element)) in mirror.iter_mut().enumerate() {
    ///    assert_eq!(indices[i], index);
    ///    *element *= 3;
    /// }
    /// assert_eq!(3, mirror.len());
    /// assert_eq!(0, mirror[indices[0]]);
    /// assert_eq!(6, mirror[indices[1]]);
    /// assert_eq!(12, mirror[indices[2]]);
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.0.iter_mut()
    }
}

impl<T> Default for Mirror<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> core::ops::Index<Index> for Mirror<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: Index) -> &T {
        self.get(index).expect("invalid index")
    }
}

impl<T> core::ops::IndexMut<Index> for Mirror<T> {
    #[inline]
    fn index_mut(&mut self, index: Index) -> &mut T {
        self.get_mut(index).expect("invalid index")
    }
}

impl<T> core::ops::Index<u32> for Mirror<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: u32) -> &T {
        self.get_by_offset(index).expect("invalid offset")
    }
}

impl<T> core::ops::IndexMut<u32> for Mirror<T> {
    #[inline]
    fn index_mut(&mut self, index: u32) -> &mut T {
        self.get_mut_by_offset(index).expect("invalid offset")
    }
}

impl<T> Extend<(Index, T)> for Mirror<T> {
    fn extend<I: IntoIterator<Item = (Index, T)>>(&mut self, iter: I) {
        for (index, value) in iter {
            let _ = self.insert(index, value);
        }
    }
}

impl<T> FromIterator<(Index, T)> for Mirror<T> {
    fn from_iter<I: IntoIterator<Item = (Index, T)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let cap = upper.unwrap_or(lower);
        let cap = max(cap, 1);
        debug_assert!(cap <= u32::MAX as usize);
        let mut arena = Self::with_capacity(cap);
        arena.extend(iter);
        arena
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, sync::Arc, vec};

    #[test]
    fn test_index_accessors() {
        let index = Index(2, Generation::ONE);
        let (offset, generation) = index.into_parts();
        assert_eq!(2, offset);
        assert_eq!(Generation::ONE, generation);

        assert_eq!(2, index.offset());

        assert_eq!(Generation::ONE, index.generation());
    }
    #[test]
    fn test_index_debug() {
        let index = Index(2, Generation::ONE);
        assert_eq!("2v1", format!("{:?}", index));
    }

    #[test]
    fn test_generation() {
        let gen_one = Generation::ONE;
        let gen_two = gen_one.next();
        assert_eq!(1, gen_one.get());
        assert!(!gen_one.is_removed());
        assert_eq!(2, gen_two.get());
        assert!(!gen_two.is_removed());
        let gen_three_rm = gen_two.removed();
        assert_eq!(!3, gen_three_rm.get());
        assert!(gen_three_rm.is_removed());
    }

    #[test]
    fn test_generation_overflow() {
        assert_eq!(Generation::NEW, Generation::MAX.removed());
        assert_eq!(Generation::ONE, Generation::MAX.next());
    }

    #[test]
    fn test_generation_debug() {
        let gen_one = Generation::ONE;
        let gen_two = gen_one.next();
        assert_eq!("v1", format!("{:?}", gen_one));
        assert_eq!("v2", format!("{:?}", gen_two));
    }

    #[test]
    fn test_arena_new() {
        let a = Arena::<usize>::new();
        assert!(a.is_empty());
        assert_eq!(0, a.len());
        assert_eq!(0, a.capacity());
    }

    #[test]
    fn test_arena_default() {
        let a = Arena::<usize>::default();
        assert!(a.is_empty());
        assert_eq!(0, a.len());
        assert_eq!(0, a.capacity());
    }

    #[test]
    fn test_arena_with_capacity() {
        for capacity in [0usize, 1, 5, 13, 47] {
            let a = Arena::<usize>::with_capacity(capacity);
            assert!(a.is_empty());
            assert_eq!(0, a.len());
            assert_eq!(capacity, a.capacity());
        }
    }

    #[test]
    fn test_arena_reserve() {
        for capacity in [0usize, 1, 5, 13, 47] {
            let mut a = Arena::<usize>::new();
            a.reserve(capacity);
            assert!(a.is_empty());
            assert_eq!(0, a.len());
            assert!(a.capacity() >= capacity);
        }
    }

    #[test]
    fn test_arena_reserve_exact() {
        for capacity in [0usize, 1, 5, 13, 47] {
            let mut a = Arena::<usize>::new();
            a.reserve_exact(capacity);
            assert!(a.is_empty());
            assert_eq!(0, a.len());
            assert_eq!(capacity, a.capacity());
        }
    }

    #[test]
    fn test_arena_clear() {
        let mut arena = Arena::new();
        arena.insert("test");
        arena.insert("foo");
        arena.insert("bar");
        assert_eq!(3, arena.len());
        arena.clear();
        assert!(arena.is_empty());
    }

    #[test]
    fn test_arena_len() {
        let mut arena = Arena::new();
        assert_eq!(0, arena.len());
        arena.insert("test");
        assert_eq!(1, arena.len());
        arena.insert("foo");
        assert_eq!(2, arena.len());
        arena.insert("bar");
        assert_eq!(3, arena.len());
    }

    #[test]
    fn test_arena_is_empty() {
        let mut arena = Arena::new();
        assert!(arena.is_empty());
        arena.insert("test");
        assert!(!arena.is_empty());
    }

    #[test]
    fn test_arena_try_insert() {
        let mut arena = Arena::new();
        arena.reserve_exact(15);
        // `try_insert` does not allocate
        for i in 0..15 {
            assert!(arena.try_insert(i).is_ok());
            assert_eq!(15, arena.capacity());
        }
        assert!(arena.try_insert(16).is_err());
        assert_eq!(15, arena.capacity());
    }

    #[test]
    fn test_arena_try_insert_with() {
        struct Element {
            index: Index,
            value: usize,
        }
        let mut arena = Arena::new();
        arena.reserve_exact(3);
        assert_eq!(0, arena.len());
        let index0 = arena
            .try_insert_with(|i| Element {
                index: i,
                value: 42,
            })
            .ok()
            .unwrap();
        let index1 = arena
            .try_insert_with(|i| Element {
                index: i,
                value: 666,
            })
            .ok()
            .unwrap();
        let index2 = arena
            .try_insert_with(|i| Element {
                index: i,
                value: 42,
            })
            .ok()
            .unwrap();
        assert_eq!(3, arena.len());
        assert!(arena
            .try_insert_with(|i| Element {
                index: i,
                value: 99
            })
            .is_err());
        assert_eq!(3, arena.capacity());
        assert_eq!(index0, arena[index0].index);
        assert_eq!(index1, arena[index1].index);
        assert_eq!(index2, arena[index2].index);
        assert_eq!(42, arena[index0].value);
        assert_eq!(666, arena[index1].value);
        assert_eq!(42, arena[index2].value);
    }

    #[test]
    fn test_arena_insert() {
        let mut arena = Arena::new();
        assert_eq!(0, arena.len());
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!("test", arena[index0]);
        assert_eq!("foo", arena[index1]);
    }

    #[test]
    fn test_arena_insert_with() {
        struct Element {
            index: Index,
            value: usize,
        }
        let mut arena = Arena::new();
        assert_eq!(0, arena.len());
        let index0 = arena.insert_with(|i| Element {
            index: i,
            value: 42,
        });
        let index1 = arena.insert_with(|i| Element {
            index: i,
            value: 666,
        });
        assert_eq!(2, arena.len());
        assert_eq!(index0, arena[index0].index);
        assert_eq!(42, arena[index0].value);
        assert_eq!(index1, arena[index1].index);
        assert_eq!(666, arena[index1].value);
    }

    #[test]
    fn test_arena_remove() {
        let mut arena = Arena::new();
        assert_eq!(0, arena.len());
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!(Some("test"), arena.remove(index0));
        assert_eq!(1, arena.len());
        // removing it a second time returns `None`
        assert_eq!(None, arena.remove(index0));
        assert_eq!(1, arena.len());
        assert_eq!(Some("foo"), arena.remove(index1));
        assert!(arena.is_empty());
    }

    #[test]
    fn test_arena_contains() {
        let mut arena = Arena::new();
        assert_eq!(0, arena.len());
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert!(arena.contains(index0));
        assert!(arena.contains(index1));
        assert_eq!(Some("test"), arena.remove(index0));
        assert_eq!(1, arena.len());
        assert!(!arena.contains(index0)); // element not in the arena
        assert!(arena.contains(index1));
    }

    #[test]
    fn test_arena_get() {
        let mut arena = Arena::new();
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!(Some(&"test"), arena.get(index0));
        assert_eq!(Some(&"foo"), arena.get(index1));
        assert_eq!(2, arena.len());
        assert_eq!(Some("test"), arena.remove(index0));
        assert_eq!(1, arena.len());
        assert_eq!(None, arena.get(index0));
    }

    #[test]
    fn test_arena_get_mut() {
        let mut arena = Arena::new();
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!("test", arena[index0]);
        let element = arena.get_mut(index0).unwrap();
        *element = "bar";
        assert_eq!("bar", arena[index0]);
        assert_eq!("foo", arena[index1]);
        assert_eq!(Some("bar"), arena.remove(index0));
        assert!(arena.get_mut(index0).is_none())
    }

    #[test]
    fn test_arena_drain() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.drain().enumerate() {
            assert_eq!(indices[i], index);
            assert_eq!(i, element);
        }
        assert!(arena.is_empty());
    }

    #[test]
    fn test_arena_drain_reverse() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.drain().rev().enumerate() {
            assert_eq!(indices[2 - i], index);
            assert_eq!(2 - i, element);
        }
        assert!(arena.is_empty());
    }

    #[test]
    fn test_arena_iter() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.iter().enumerate() {
            assert_eq!(indices[i], index);
            assert_eq!(i, *element);
        }
        assert_eq!(3, arena.len());
    }

    #[test]
    fn test_arena_iter_reverse() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.iter().rev().enumerate() {
            assert_eq!(indices[2 - i], index);
            assert_eq!(2 - i, *element);
        }
        assert_eq!(3, arena.len());
    }

    #[test]
    fn test_arena_iter_mut() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.iter_mut().enumerate() {
            assert_eq!(indices[i], index);
            *element *= 3 * i;
        }
        assert_eq!(3, arena.len());
        assert_eq!(0, arena[indices[0]]);
        assert_eq!(3, arena[indices[1]]);
        assert_eq!(12, arena[indices[2]]);
    }

    #[test]
    fn test_arena_iter_mut_reverse() {
        let mut arena = Arena::new();
        let indices = [arena.insert(0), arena.insert(1), arena.insert(2)];
        assert_eq!(3, arena.len());
        for (i, (index, element)) in arena.iter_mut().rev().enumerate() {
            assert_eq!(indices[2 - i], index);
            *element *= 3 * i;
        }
        assert_eq!(3, arena.len());
        assert_eq!(0, arena[indices[0]]);
        assert_eq!(3, arena[indices[1]]);
        assert_eq!(0, arena[indices[2]]);
    }

    #[test]
    fn test_arena_index() {
        let mut arena = Arena::new();
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!("test", arena[index0]);
        assert_eq!("foo", arena[index1]);
        assert_eq!(2, arena.len());
        assert_eq!(Some("test"), arena.remove(index0));
        assert_eq!(1, arena.len());
    }

    #[test]
    fn test_arena_index_mut() {
        let mut arena = Arena::new();
        let index0 = arena.insert("test");
        let index1 = arena.insert("foo");
        assert_eq!(2, arena.len());
        assert_eq!("test", arena[index0]);
        let element = &mut arena[index0];
        *element = "bar";
        assert_eq!("bar", arena[index0]);
        assert_eq!("foo", arena[index1]);
    }

    #[test]
    fn test_arena_extend() {
        let a = vec!["test", "foo", "bar"];
        let mut b = Arena::new();
        b.extend(a);
        assert_eq!(3, b.len());
    }

    #[test]
    fn test_arena_from_iter() {
        let a = vec!["test", "foo", "bar"];
        let b: Arena<_> = a.iter().collect();
        assert_eq!(3, b.len());
    }

    #[test]
    fn test_arena_drop() {
        let refcounter = Arc::new(());
        let mut a = Arena::new();
        for _ in 0..5 {
            a.insert(refcounter.clone());
        }
        let i1 = a.insert(refcounter.clone());
        for _ in 0..5 {
            a.insert(refcounter.clone());
        }
        let i2 = a.insert(refcounter.clone());
        assert_eq!(13, Arc::strong_count(&refcounter));
        assert!(a.remove(i1).is_some());
        assert_eq!(12, Arc::strong_count(&refcounter));
        assert!(a.remove(i2).is_some());
        assert_eq!(11, Arc::strong_count(&refcounter));
        assert!(a.remove(i1).is_none());
        assert!(a.remove(i2).is_none());
        assert_eq!(11, Arc::strong_count(&refcounter));
        drop(a);
        assert_eq!(1, Arc::strong_count(&refcounter));
    }

    #[test]
    fn test_arena_clone() {
        let refcounter = Arc::new(());
        let mut a = Arena::new();
        let i1 = a.insert(refcounter.clone());
        for _ in 0..5 {
            a.insert(refcounter.clone());
        }
        assert_eq!(7, Arc::strong_count(&refcounter));
        assert!(a.remove(i1).is_some());
        assert_eq!(6, Arc::strong_count(&refcounter));
        let b = a.clone();
        assert_eq!(11, Arc::strong_count(&refcounter));
        drop(a);
        assert_eq!(6, Arc::strong_count(&refcounter));
        drop(b);
        assert_eq!(1, Arc::strong_count(&refcounter));
    }
}
