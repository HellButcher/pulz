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

use std::ops::Range;

/// Bit-Set like structure
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitSet(Vec<u64>);
const SHIFT_DIV64: usize = 6;
const MASK_MOD64: usize = 0x3f;

impl BitSet {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// creates a new bitset with a reserved capacity for items up to the given index.
    pub fn with_capacity_for(max_item: usize) -> Self {
        Self(Vec::with_capacity((max_item >> SHIFT_DIV64) + 1))
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn from_range(range: Range<usize>) -> Self {
        let words_from = range.start >> SHIFT_DIV64;
        let words_to = range.end >> SHIFT_DIV64;
        let words_from_rest = range.start & MASK_MOD64;
        let words_to_rest = range.end & MASK_MOD64;
        let mut result = Vec::with_capacity(words_to + 1);
        result.resize(words_from, 0); // fill with zeros
        match words_from.cmp(&words_to) {
            std::cmp::Ordering::Equal => {
                let mut value = !0u64;
                value <<= words_from_rest;
                value &= !((!0u64) << words_to_rest);
                if value != 0 {
                    result.push(value);
                }
            }
            std::cmp::Ordering::Less => {
                if words_from_rest != 0 {
                    let value = (!0u64) << words_from_rest;
                    result.push(value);
                }
                result.resize(words_to, !0u64); // fill with ones
                if words_to_rest != 0 {
                    let value = !((!0u64) << words_to_rest);
                    result.push(value);
                }
            }
            std::cmp::Ordering::Greater => (),
        }
        Self(result)
    }

    #[inline]
    fn split_value(value: usize) -> (usize, u64) {
        let index = value >> SHIFT_DIV64;
        let subindex = value & MASK_MOD64;
        let bits = 1u64 << subindex;
        (index, bits)
    }

    #[inline]
    pub fn contains(&self, value: usize) -> bool {
        let (index, bits) = Self::split_value(value);
        if let Some(word) = self.0.get(index) {
            *word & bits != 0
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || !self.0.iter().any(|i| *i != 0)
    }

    pub fn contains_all(&self, other: &Self) -> bool {
        if other.0.len() > self.0.len() {
            return false;
        }
        for i in 0..other.0.len() {
            let l = self.0[i];
            let r = other.0[i];
            if l & r != r {
                return false;
            }
        }
        true
    }

    pub fn insert(&mut self, value: usize) -> bool {
        let (index, bits) = Self::split_value(value);
        if index >= self.0.len() {
            self.0.resize(index + 1, 0);
        }
        // SAFETY: vec was extended to contain index
        let word = unsafe { self.0.get_unchecked_mut(index) };
        let was_unset = (*word & bits) == 0;
        *word |= bits;
        was_unset
    }

    pub fn remove(&mut self, value: usize) -> bool {
        let (index, bits) = Self::split_value(value);
        let was_set = if let Some(word) = self.0.get_mut(index) {
            let was_set = (*word & bits) != 0;
            *word &= !bits;
            was_set
        } else {
            false
        };
        if index + 1 == self.0.len() {
            self.normalize_after_remove();
        }
        was_set
    }

    fn normalize_after_remove(&mut self) {
        // shrink (for Eq)
        while let Some(0) = self.0.last() {
            self.0.pop();
        }
    }

    pub fn first(&self) -> Option<usize> {
        for (i, word) in self.0.iter().copied().enumerate() {
            if word != 0 {
                let mut value = i * 64;
                let mut bit = 1;
                while bit != 0 {
                    if word & bit != 0 {
                        return Some(value);
                    }
                    value += 1;
                    bit <<= 1;
                }
            }
        }
        None
    }

    pub fn find_next(&self, mut value: usize) -> Option<usize> {
        value += 1;
        let (mut index, mut bits) = Self::split_value(value);
        while let Some(word) = self.0.get(index) {
            if *word & bits != 0 {
                return Some(value);
            }
            value += 1;
            if bits > (!0 >> 1) {
                index += 1;
                bits = 1;
            } else {
                bits <<= 1;
            }
        }
        None
    }

    pub fn iter(&self) -> BitSetIter<'_> {
        BitSetIter::new(&self.0, 0, !0)
    }

    #[inline]
    fn bounds_inclusive(range: impl std::ops::RangeBounds<usize>) -> (usize, usize) {
        let start = match range.start_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => (*i).saturating_add(1),
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => (*i).saturating_sub(1),
            std::ops::Bound::Unbounded => 0,
        };
        (start, end)
    }

    pub fn iter_range(&self, range: impl std::ops::RangeBounds<usize>) -> BitSetIter<'_> {
        let (start, end) = Self::bounds_inclusive(range);
        BitSetIter::new(&self.0, start, end)
    }

    pub fn drain(&mut self, range: impl std::ops::RangeBounds<usize>) -> BitSetDrain<'_> {
        let (start, end) = Self::bounds_inclusive(range);
        BitSetDrain::new(self, start, end)
    }

    /// Add items to this bitset. (union)
    /// This is an ptimized version of `extend`.
    pub fn extend_bitset(&mut self, other: &Self) {
        let len = other.0.len();
        if self.0.len() < len {
            self.0.resize(len, 0u64);
        }
        for i in 0..len {
            // SAFETY: we have resized the vector, to be at least `len`
            unsafe {
                *self.0.get_unchecked_mut(i) |= *other.0.get_unchecked(i);
            }
        }
    }

    /// remove items from this bitset (difference)
    pub fn remove_bitset(&mut self, other: &Self) {
        let len = usize::min(self.0.len(), other.0.len());
        for i in 0..len {
            // SAFETY: we have checked the vector, to be at least `len`
            unsafe {
                *self.0.get_unchecked_mut(i) &= !*other.0.get_unchecked(i);
            }
        }
        if len == self.0.len() {
            self.normalize_after_remove();
        }
    }

    /// only retain the elements from other (intersection)
    pub fn retain_bitset(&mut self, other: &Self) {
        let len = usize::min(self.0.len(), other.0.len());
        self.0.resize(len, 0u64);
        for i in 0..len {
            // SAFETY: we have resized the vector, to be at least `len`
            unsafe {
                *self.0.get_unchecked_mut(i) &= *other.0.get_unchecked(i);
            }
        }
        self.normalize_after_remove();
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        let len = usize::min(self.0.len(), other.0.len());
        for i in 0..len {
            // SAFETY: we have checked the size
            unsafe {
                if *self.0.get_unchecked(i) & *other.0.get_unchecked(i) != 0 {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for BitSet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Extend<usize> for BitSet {
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<T> FromIterator<T> for BitSet
where
    Self: Extend<T>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut bitset = Self::new();
        bitset.extend(iter);
        bitset
    }
}

impl std::fmt::Debug for BitSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[inline]
fn iter_next(slice: &[u64], index: &mut usize, end: usize) -> Option<usize> {
    if *index > end {
        return None;
    }
    let mut major = *index >> SHIFT_DIV64;
    let mut minor = *index & MASK_MOD64;
    let major_end = end >> SHIFT_DIV64;
    while let Some(mut word) = slice.get(major).copied() {
        if major > major_end {
            break;
        }
        word >>= minor;
        while word != 0 {
            if word & 1 == 1 {
                let result = major << 6 | minor;
                *index = result + 1; // set next
                return Some(result);
            }
            minor += 1;
            word >>= 1;
        }
        // skip complete word
        major += 1;
        minor = 0;
    }
    *index = (major << SHIFT_DIV64) + 1;
    None
}

#[derive(Clone)]
pub struct BitSetIter<'l> {
    slice: &'l [u64],
    index: usize,
    end: usize,
}

impl<'l> BitSetIter<'l> {
    #[inline]
    fn new(slice: &'l [u64], start: usize, end: usize) -> Self {
        Self {
            slice,
            index: start,
            end,
        }
    }
}

impl<'l> Iterator for BitSetIter<'l> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        iter_next(self.slice, &mut self.index, self.end)
    }
}

impl<'l> IntoIterator for &'l BitSet {
    type Item = usize;
    type IntoIter = BitSetIter<'l>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct BitSetDrain<'l> {
    bitset: &'l mut BitSet,
    index: usize,
    end: usize,
}

impl<'l> BitSetDrain<'l> {
    #[inline]
    fn new(bitset: &'l mut BitSet, start: usize, end: usize) -> Self {
        Self {
            bitset,
            index: start,
            end,
        }
    }
}

impl<'l> Iterator for BitSetDrain<'l> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        let i = iter_next(&self.bitset.0, &mut self.index, self.end)?;
        self.bitset.remove(i);
        Some(i)
    }
}

#[cfg(test)]
mod tests {
    use crate::BitSet;

    #[test]
    fn test_insert_contains_remove() {
        let mut subject = BitSet::new();

        // everything unset
        assert!(!subject.contains(0));
        assert!(!subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(!subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(!subject.contains(1337));

        // insert
        assert!(subject.insert(1));
        assert!(subject.insert(63));
        assert!(subject.insert(1337));

        // check setted values
        assert!(!subject.contains(0));
        assert!(subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(subject.contains(1337));

        // insert again
        assert!(!subject.insert(1));

        // insert new
        assert!(subject.insert(128));

        // check setted values
        assert!(!subject.contains(0));
        assert!(subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(subject.contains(63));
        assert!(!subject.contains(64));
        assert!(subject.contains(128));
        assert!(subject.contains(1337));

        // remove
        assert!(subject.remove(63));
        assert!(subject.remove(1337));

        // check setted values
        assert!(!subject.contains(0));
        assert!(subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(!subject.contains(63));
        assert!(!subject.contains(64));
        assert!(subject.contains(128));
        assert!(!subject.contains(1337));

        // remove again
        assert!(!subject.remove(63));

        // remove more
        assert!(subject.remove(128));
        assert!(subject.remove(1));

        // check setted values
        assert!(!subject.contains(0));
        assert!(!subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(!subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(!subject.contains(1337));
    }

    #[test]
    fn test_clear() {
        let mut subject = BitSet::new();

        // everything unset
        assert!(!subject.contains(0));
        assert!(!subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(!subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(!subject.contains(1337));

        // insert
        assert!(subject.insert(1));
        assert!(subject.insert(63));
        assert!(subject.insert(1337));

        // check setted values
        assert!(!subject.contains(0));
        assert!(subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(subject.contains(1337));

        subject.clear();

        // everything unset
        assert!(!subject.contains(0));
        assert!(!subject.contains(1));
        assert!(!subject.contains(2));
        assert!(!subject.contains(62));
        assert!(!subject.contains(63));
        assert!(!subject.contains(64));
        assert!(!subject.contains(128));
        assert!(!subject.contains(1337));
    }

    #[test]
    fn test_from_range() {
        let subject = BitSet::from_range(126..1337);

        // everything unset
        assert!(!subject.contains(124));
        assert!(!subject.contains(125));
        assert!(subject.contains(126));
        assert!(subject.contains(127));
        assert!(subject.contains(128));
        assert!(subject.contains(129));
        assert!(subject.contains(1335));
        assert!(subject.contains(1336));
        assert!(!subject.contains(1337));
        assert!(!subject.contains(1338));
    }

    #[test]
    fn test_from_range_2() {
        let subject = BitSet::from_range(100..110);

        // everything unset
        assert!(!subject.contains(98));
        assert!(!subject.contains(99));
        assert!(subject.contains(100));
        assert!(subject.contains(101));
        assert!(subject.contains(102));
        assert!(subject.contains(108));
        assert!(subject.contains(109));
        assert!(!subject.contains(110));
        assert!(!subject.contains(111));
        assert!(!subject.contains(112));
    }

    #[test]
    fn test_iter() {
        let mut subject = BitSet::new();

        // insert
        assert!(subject.insert(1));
        assert!(subject.insert(2));
        assert!(subject.insert(63));
        assert!(subject.insert(1337));

        let mut iter = subject.into_iter();
        assert_eq!(Some(1), iter.next());
        assert_eq!(Some(2), iter.next());
        assert_eq!(Some(63), iter.next());
        assert_eq!(Some(1337), iter.next());
        assert_eq!(None, iter.next());
    }
}
