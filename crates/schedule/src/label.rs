use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    fmt::Debug,
    hash::{Hash, Hasher},
};

use downcast_rs::DowncastSync;

pub trait AnyLabel: DowncastSync + Send + Sync + Debug {
    fn any_clone(&self) -> Box<dyn AnyLabel>;
    fn any_eq(&self, other: &dyn AnyLabel) -> bool;
    fn any_cmp(&self, other: &dyn AnyLabel) -> Ordering;
    fn any_hash(&self) -> u64;
}

downcast_rs::impl_downcast!(sync AnyLabel);

impl<T> AnyLabel for T
where
    T: Any
        + Send
        + Sync
        + Debug
        + Copy
        + Clone
        + Hash
        + Eq
        + PartialEq
        + Ord
        + PartialOrd
        + 'static,
{
    fn any_clone(&self) -> Box<dyn AnyLabel> {
        Box::new(self.clone())
    }

    fn any_eq(&self, other: &dyn AnyLabel) -> bool {
        if let Some(other) = other.downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }

    fn any_cmp(&self, other: &dyn AnyLabel) -> Ordering {
        match self.type_id().cmp(&other.type_id()) {
            Ordering::Equal => self.cmp(other.downcast_ref::<Self>().unwrap()),
            ord => ord,
        }
    }

    fn any_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug)]
pub struct SystemLabel<T: ?Sized = dyn AnyLabel>(T);

impl<T: AnyLabel> From<T> for Box<SystemLabel> {
    #[inline]
    fn from(any_label: T) -> Self {
        Box::new(SystemLabel(any_label))
    }
}

// impl Clone for SystemLabel {
//   #[inline]
//   fn clone(&self) -> Self {
//       SystemLabel(self.0.any_clone())
//   }
// }

impl PartialEq for SystemLabel {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.any_eq(&other.0)
    }
}
impl Eq for SystemLabel {}
impl PartialOrd for SystemLabel {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.any_cmp(&other.0))
    }
}
impl Ord for SystemLabel {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.any_cmp(&other.0)
    }
}
impl Hash for SystemLabel {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.any_hash())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreSystemLabel {
    First,
    Update,
    Last,
}
