//! A compact bitset of component ids, used to describe archetype membership.

use roaring::RoaringBitmap;

use crate::component::{ComponentData, ComponentId, Components};

/// A compact bitset that tracks which components are present in an archetype.
///
/// Backed by a [`RoaringBitmap`] for memory-efficient storage of sparse component ids.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ComponentSet(RoaringBitmap);

// We can derive Eq because although RoaringBitmap does not implement Eq, its actually reflexive:
// see https://github.com/RoaringBitmap/roaring-rs/issues/302
impl Eq for ComponentSet {}

// Simple implementation of Hash for ComponentSet
// Note: This is not the most efficient way to hash a RoaringBitmap, but it currently doesn't
// a Has impl itself. See https://github.com/RoaringBitmap/roaring-rs/issues/231
impl std::hash::Hash for ComponentSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for id in self.0.iter() {
            id.hash(state);
        }
    }
}

impl ComponentSet {
    /// Creates an empty component set.
    #[inline]
    pub fn new() -> Self {
        Self(RoaringBitmap::new())
    }

    /// Removes all component ids from the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Returns `true` if the set contains `id`.
    #[inline]
    pub fn contains<X>(&self, id: ComponentId<X>) -> bool {
        self.0.contains(id.0)
    }

    /// Inserts `id`, returning `true` if it was not already present.
    pub fn insert<X>(&mut self, id: ComponentId<X>) -> bool {
        self.0.insert(id.0)
    }

    /// Removes `id`, returning `true` if it was present.
    pub fn remove<X>(&mut self, id: ComponentId<X>) -> bool {
        self.0.remove(id.0)
    }

    /// Returns `true` if this set and `other` share no elements.
    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }

    /// Adds all elements of `other` to this set (set union in place).
    #[inline]
    pub fn union_with(&mut self, other: &Self) {
        self.0 |= &other.0
    }

    /// Removes all elements of `other` from this set (set difference in place).
    #[inline]
    pub fn difference_with(&mut self, other: &Self) {
        self.0 -= &other.0
    }

    /// Retains only elements that are also in `other` (set intersection in place).
    #[inline]
    pub fn intersect_with(&mut self, other: &Self) {
        self.0 &= &other.0
    }

    /// Optimises the internal bitmap representation.
    #[inline]
    pub fn optimize(&mut self) {
        self.0.optimize();
    }

    /// Returns `true` if the set contains no components.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns number of distinct component ids in the set.
    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    /// Returns an iterator over all component ids in the set.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter())
    }

    /// Returns an iterator over the [`ComponentData`] metadata for each id in the set.
    pub fn iter_infos<'l>(
        &'l self,
        components: &'l Components,
    ) -> impl Iterator<Item = &'l ComponentData> + 'l {
        self.iter().map(move |id| &components[id])
    }

    /// Returns a new set containing only the ids for which `f` returns `true`.
    #[allow(clippy::missing_panics_doc)] // from_sorted_iter cannot fail: filtering a sorted iterator yields sorted values
    pub fn filter(&self, mut f: impl FnMut(ComponentId) -> bool) -> Self {
        Self(
            RoaringBitmap::from_sorted_iter(self.0.iter().filter(|i| f(ComponentId::new(*i))))
                .unwrap(),
        )
    }
}

impl Extend<ComponentId> for ComponentSet {
    fn extend<T: IntoIterator<Item = ComponentId>>(&mut self, iter: T) {
        for id in iter {
            self.insert(id);
        }
    }
}

/// Borrowing iterator over a [`ComponentSet`].
pub struct Iter<'a>(roaring::bitmap::Iter<'a>);
/// Owning iterator over a [`ComponentSet`].
pub struct IntoIter(roaring::bitmap::IntoIter);

impl Iterator for Iter<'_> {
    type Item = ComponentId;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(ComponentId::new)
    }
}

impl Iterator for IntoIter {
    type Item = ComponentId;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(ComponentId::new)
    }
}

impl IntoIterator for ComponentSet {
    type Item = ComponentId;
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}
impl<'a> IntoIterator for &'a ComponentSet {
    type Item = ComponentId;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentId;

    #[test]
    fn component_set_new_is_empty_then_filled_then_cleared() {
        let mut s = ComponentSet::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        // insert c1
        let c1 = ComponentId::<u8>::new(1);
        s.insert(c1);
        assert!(!s.is_empty());
        assert_eq!(s.len(), 1);
        // insert c2
        let c2 = ComponentId::<u16>::new(2);
        assert!(s.insert(c2));
        assert_eq!(s.len(), 2);
        // insert c1 again
        assert!(!s.insert(c1));
        assert_eq!(s.len(), 2);
        // remove c1
        assert!(s.remove(c1));
        assert_eq!(s.len(), 1);
        // remove c1 again
        assert!(!s.remove(c1));
        assert_eq!(s.len(), 1);
        // clear
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn component_set_clone() {
        let s1 = ComponentSet::new();
        let s2 = s1.clone();
        assert_eq!(s1, s2);

        let mut s3 = ComponentSet::new();
        let c1 = ComponentId::<u8>::new(1);
        s3.insert(c1);
        let s4 = s3.clone();
        assert_eq!(s3, s4);
    }

    #[test]
    fn component_set_iter() {
        let mut s = ComponentSet::new();
        let mut iter = s.iter();
        assert!(iter.next().is_none());
        let c1 = ComponentId::<u8>::new(1);
        s.insert(c1);
        let mut iter = s.iter();
        assert_eq!(iter.next(), Some(c1.untyped()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn component_set_into_iter() {
        let mut s = ComponentSet::new();
        let c1 = ComponentId::<u8>::new(1);
        s.insert(c1);
        let mut iter = s.into_iter();
        assert_eq!(iter.next(), Some(c1.untyped()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn component_set_operations() {
        let mut s1 = ComponentSet::new();
        let s2 = ComponentSet::new();
        assert!(s1.is_disjoint(&s2));
        s1.union_with(&s2);
        assert!(s1.is_empty());
        s1.difference_with(&s2);
        assert!(s1.is_empty());
        s1.intersect_with(&s2);
        assert!(s1.is_empty());
    }
}
