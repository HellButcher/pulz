use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom(usize);

impl Atom {
    pub const ZERO: Self = Self(0);

    pub fn new() -> Self {
        static GLOBAL_EPOCH: AtomicUsize = AtomicUsize::new(1);
        Self(GLOBAL_EPOCH.fetch_add(1, Ordering::SeqCst))
    }
}
