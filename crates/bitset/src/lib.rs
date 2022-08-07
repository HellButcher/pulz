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
        // shrink (for Eq)
        if index + 1 == self.0.len() {
            while let Some(0) = self.0.last() {
                self.0.pop();
            }
        }
        was_set
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
        BitSetIter::new(&self.0)
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

#[derive(Clone)]
pub struct BitSetIter<'l> {
    slice: &'l [u64],
    index: usize,
}

impl<'l> BitSetIter<'l> {
    #[inline]
    fn new(slice: &'l [u64]) -> Self {
        Self { slice, index: 0 }
    }
}

impl<'l> Iterator for BitSetIter<'l> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        let mut major = self.index >> SHIFT_DIV64;
        let mut minor = self.index & MASK_MOD64;
        while let Some(mut word) = self.slice.get(major).copied() {
            word >>= minor;
            while word != 0 {
                if word & 1 == 1 {
                    let index = major << 6 | minor;
                    self.index = index + 1; // set next
                    return Some(index);
                }
                minor += 1;
                word >>= 1;
            }
            // skip complete word
            major += 1;
            minor = 0;
        }
        self.index = (major << SHIFT_DIV64) + 1;
        None
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
