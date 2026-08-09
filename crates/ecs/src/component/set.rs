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
