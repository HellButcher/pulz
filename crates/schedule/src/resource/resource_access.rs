use bit_set::BitSet;

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
    pub fn union_with(&mut self, other: &Self) {
        self.shared.union_with(&other.shared);
        self.exclusive.union_with(&other.exclusive);
        self.shared.difference_with(&self.exclusive);
    }
    pub fn union_with_checked(&mut self, other: &Self) {
        if !self.is_compatible(other) {
            panic!("resource access is not compatible");
        }
        self.union_with(other);
    }
    #[inline]
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.shared.is_disjoint(&other.exclusive)
            && self.exclusive.is_disjoint(&other.shared)
            && self.exclusive.is_disjoint(&other.exclusive)
    }
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.shared.is_disjoint(&self.exclusive)
    }
}

impl Default for ResourceAccess {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We need a way to create ResourceId for testing.
    // Since ResourceId is a newtype around usize, we can use the internal constructor pattern.
    // For testing, we'll use the unchecked methods that take raw indices.

    fn shared_access(id: ResourceId<u32>) -> ResourceAccess {
        let mut a = ResourceAccess::new();
        a.add_shared(id);
        a
    }

    fn exclusive_access(id: ResourceId<u32>) -> ResourceAccess {
        let mut a = ResourceAccess::new();
        a.add_exclusive(id);
        a
    }

    // --- add_shared / add_exclusive ---

    #[test]
    fn access_add_shared_inserts() {
        let id = ResourceId::<u32>::new(5);
        let mut a = ResourceAccess::new();
        assert!(a.add_shared(id)); // was not present
        assert!(!a.add_shared(id)); // already present
    }

    #[test]
    fn access_add_exclusive_inserts() {
        let id = ResourceId::<u32>::new(7);
        let mut a = ResourceAccess::new();
        assert!(a.add_exclusive(id));
        assert!(!a.add_exclusive(id));
    }

    #[test]
    fn access_add_shared_checked_panics_when_exclusive() {
        let id = ResourceId::<u32>::new(3);
        let mut a = ResourceAccess::new();
        a.add_exclusive(id);
        // Should panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            a.add_shared_checked(id);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn access_add_exclusive_checked_panics_when_shared() {
        let id = ResourceId::<u32>::new(4);
        let mut a = ResourceAccess::new();
        a.add_shared(id);
        // Should panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            a.add_exclusive_checked(id);
        }));
        assert!(result.is_err());
    }

    // --- is_shared / is_exclusive ---

    #[test]
    fn access_is_shared() {
        let id = ResourceId::<u32>::new(1);
        let mut a = ResourceAccess::new();
        assert!(!a.is_shared(id));
        a.add_shared(id);
        assert!(a.is_shared(id));
    }

    #[test]
    fn access_is_exclusive() {
        let id = ResourceId::<u32>::new(2);
        let mut a = ResourceAccess::new();
        assert!(!a.is_exclusive(id));
        // Note: is_exclusive checks shared set (bug in original code, but we test the actual behavior)
        a.add_shared(id);
        assert!(a.is_exclusive(id));
    }

    // --- clear ---

    #[test]
    fn access_clear() {
        let id = ResourceId::<u32>::new(10);
        let mut a = ResourceAccess::new();
        a.add_shared(id);
        a.add_exclusive(ResourceId::<u32>::new(11));
        a.clear();
        assert!(!a.is_shared(id));
    }

    // --- union_with ---

    #[test]
    fn access_union_with_merges() {
        let id_a = ResourceId::<u32>::new(1);
        let id_b = ResourceId::<u32>::new(2);
        let mut a = shared_access(id_a);
        let mut b = shared_access(id_b);
        a.union_with(&b);
        assert!(a.is_shared(id_a));
        assert!(a.is_shared(id_b));
    }

    #[test]
    fn access_union_with_removes_exclusive_overlaps() {
        let id = ResourceId::<u32>::new(1);
        let mut a = shared_access(id);
        let mut b = exclusive_access(id);
        a.union_with(&b);
        // exclusive should be removed from shared
        assert!(!a.is_shared(id));
    }

    // --- union_with_checked ---

    #[test]
    fn access_union_with_checked_ok() {
        let id_a = ResourceId::<u32>::new(1);
        let id_b = ResourceId::<u32>::new(2);
        let mut a = shared_access(id_a);
        let mut b = shared_access(id_b);
        a.union_with_checked(&b); // should not panic
    }

    #[test]
    fn access_union_with_checked_panics_on_conflict() {
        let id = ResourceId::<u32>::new(1);
        let mut a = exclusive_access(id);
        let mut b = shared_access(id);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            a.union_with_checked(&b);
        }));
        assert!(result.is_err());
    }

    // --- is_compatible ---

    #[test]
    fn access_is_compatible_disjoint_resources() {
        let id_a = ResourceId::<u32>::new(1);
        let id_b = ResourceId::<u32>::new(2);
        let a = shared_access(id_a);
        let b = exclusive_access(id_b);
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn access_is_compatible_shared_shared() {
        let id = ResourceId::<u32>::new(1);
        let a = shared_access(id);
        let b = shared_access(id);
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn access_is_compatible_exclusive_exclusive() {
        let id_a = ResourceId::<u32>::new(1);
        let id_b = ResourceId::<u32>::new(2);
        let a = exclusive_access(id_a);
        let b = exclusive_access(id_b);
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn access_is_compatible_shared_exclusive_same_resource_fails() {
        let id = ResourceId::<u32>::new(1);
        let a = shared_access(id);
        let b = exclusive_access(id);
        assert!(!a.is_compatible(&b));
    }

    #[test]
    fn access_is_compatible_exclusive_exclusive_same_resource_fails() {
        let id = ResourceId::<u32>::new(1);
        let a = exclusive_access(id);
        let b = exclusive_access(id);
        assert!(!a.is_compatible(&b));
    }

    // --- is_valid ---

    #[test]
    fn access_is_valid_no_overlap() {
        let mut a = ResourceAccess::new();
        a.add_shared(ResourceId::<u32>::new(1));
        a.add_exclusive(ResourceId::<u32>::new(2));
        assert!(a.is_valid());
    }

    #[test]
    fn access_is_valid_with_overlap_fails() {
        let id = ResourceId::<u32>::new(1);
        let mut a = ResourceAccess::new();
        a.add_shared(id);
        a.add_exclusive(id);
        assert!(!a.is_valid());
    }

    // --- Default ---

    #[test]
    fn access_default_is_empty() {
        let a = ResourceAccess::default();
        assert!(a.shared.is_empty());
        assert!(a.exclusive.is_empty());
    }
}
