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

impl BitSet {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn from_range(range: Range<usize>) -> Self {
        let words_from = range.start / 64;
        let words_to = range.end / 64;
        let words_from_rest = range.end % 64;
        let words_to_rest = range.end % 64;
        let mut result = Vec::with_capacity(words_to + 1);
        result.resize(words_from, u64::MAX); // fill with zeros
        if words_from == words_to {
            let mut value = !0u64;
            if words_from_rest != 0 {
                value <<= words_from_rest;
            }
            if words_to_rest != 0 {
                value &= !(!0u64 << words_to_rest);
            }
            if value != 0 {
                result.push(value);
            }
        } else {
            if words_from_rest != 0 {
                result.push(!0u64 << words_from_rest);
            }
            result.resize(words_to, u64::MAX); // fill with ones
            if words_to_rest != 0 {
                result.push(!(!0u64 << words_to_rest));
            }
        }
        Self(result)
    }

    #[inline]
    fn split_value(value: usize) -> (usize, u64) {
        let index = value / 64;
        let bits = 1u64 << (value % 64);
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

    fn ones_inside_word(start: usize, mut word: u64) -> impl Iterator<Item = usize> {
        let mut i = start;
        std::iter::from_fn(move || {
            while word != 0 {
                if word & 1 == 1 {
                    let result = i;
                    i += 1;
                    word >>= 1;
                    return Some(result);
                }
                i += 1;
                word >>= 1;
            }
            None
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(i, word)| Self::ones_inside_word(i * 64, word))
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
