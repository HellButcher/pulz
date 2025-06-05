use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom(usize);

impl Atom {
    pub const DIRTY: Self = Self(0);

    pub fn new() -> Self {
        static GLOBAL_EPOCH: AtomicUsize = AtomicUsize::new(1);
        Self(GLOBAL_EPOCH.fetch_add(1, Ordering::SeqCst))
    }

    #[inline]
    pub const fn is_dirty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn set_dirty(&mut self) {
        self.0 = 0;
    }

    #[inline]
    pub fn reset_dirty(&mut self) -> bool {
        if self.0 == 0 {
            *self = Self::new();
            true
        } else {
            false
        }
    }

    #[inline]
    pub const fn inherit(&mut self, other: &Self) -> bool {
        if self.0 == 0 || self.0 != other.0 {
            assert!(other.0 != 0, "can not inherit dirty atom (other is dirty)");
            self.0 = other.0;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn inherit_reset(&mut self, other: &mut Self) -> bool {
        if other.reset_dirty() {
            self.0 = other.0;
            true
        } else {
            self.inherit(other)
        }
    }
}
