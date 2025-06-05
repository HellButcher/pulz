use pulz_bitset::BitSet;

use super::ResourceId;

pub struct ResourceAccess {
    pub(crate) shared: BitSet,
    pub(crate) exclusive: BitSet,
}

impl ResourceAccess {
    #[inline]
    pub fn new() -> Self {
        Self {
            shared: BitSet::new(),
            exclusive: BitSet::new(),
        }
    }
    #[inline]
    pub fn add_shared_checked<T>(&mut self, resource: ResourceId<T>) -> bool {
        self._add_shared_checked(resource.0)
    }
    fn _add_shared_checked(&mut self, index: usize) -> bool {
        if self.exclusive.contains(index) {
            panic!("resource {index} is already used as exclusive");
        }
        self.shared.insert(index)
    }
    #[inline]
    pub fn add_shared<T>(&mut self, resource: ResourceId<T>) -> bool {
        self.shared.insert(resource.0)
    }
    #[inline]
    pub fn add_exclusive_checked<T>(&mut self, resource: ResourceId<T>) -> bool {
        self._add_exclusive_checked(resource.0)
    }
    fn _add_exclusive_checked(&mut self, index: usize) -> bool {
        if self.shared.contains(index) {
            panic!("resource {index} is already used as exclusive");
        }
        self.exclusive.insert(index)
    }
    #[inline]
    pub fn add_exclusive<T>(&mut self, resource: ResourceId<T>) -> bool {
        self.exclusive.insert(resource.0)
    }
    #[inline]
    pub fn is_shared<T>(&self, resource: ResourceId<T>) -> bool {
        self.shared.contains(resource.0)
    }
    #[inline]
    pub fn is_exclusive<T>(&self, resource: ResourceId<T>) -> bool {
        self.shared.contains(resource.0)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.shared.clear();
        self.exclusive.clear();
    }
    #[inline]
    pub fn extend(&mut self, other: &Self) {
        self.shared.extend_bitset(&other.shared);
        self.exclusive.extend_bitset(&other.exclusive);
    }
    #[inline]
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.shared.is_disjoint(&other.exclusive)
            && self.exclusive.is_disjoint(&other.shared)
            && self.exclusive.is_disjoint(&other.exclusive)
    }
}

impl Default for ResourceAccess {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
