use std::{
    any::Any,
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    fmt::Debug,
    hash::{Hash, Hasher},
};

use downcast_rs::DowncastSync;
use tinybox::{TinyBox, tinybox};

pub trait AnyLabel: DowncastSync + Send + Sync + Debug {
    fn any_clone_systemlabel(&self) -> SystemLabel;
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
    fn any_clone_systemlabel(&self) -> SystemLabel {
        self.clone().into()
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
struct SystemLabelInner<T: ?Sized = dyn AnyLabel>(T);

#[derive(Debug,PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemLabel(TinyBox<SystemLabelInner>);

impl<T: AnyLabel> From<T> for SystemLabel {
    #[inline]
    fn from(any_label: T) -> Self {
        let system_label = SystemLabelInner(any_label);
        SystemLabel(tinybox!(SystemLabelInner => system_label))
    }
}

impl Clone for SystemLabel {
  #[inline]
  fn clone(&self) -> Self {
      self.0.0.any_clone_systemlabel()
  }
}

impl PartialEq for SystemLabelInner {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.any_eq(&other.0)
    }
}
impl Eq for SystemLabelInner {}
impl PartialOrd for SystemLabelInner {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.any_cmp(&other.0))
    }
}
impl Ord for SystemLabelInner {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.any_cmp(&other.0)
    }
}
impl Hash for SystemLabelInner {
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
