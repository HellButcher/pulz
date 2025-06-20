use std::{collections::VecDeque, marker::PhantomData};

use pulz_schedule_macros::system_module;

use crate::{
    label::CoreSystemSet,
    local::Local,
    prelude::ResourceId,
    resource::{Res, ResMut, Resources},
    schedule::Schedule,
    system::SystemData,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId<T>(usize, PhantomData<fn() -> T>);

pub struct Events<T> {
    events: VecDeque<T>,
    first_id: usize,
    frame_start_id: usize,
}

impl<T> Events<T> {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            first_id: 0,
            frame_start_id: 0,
        }
    }

    pub fn last(&self) -> Option<&T> {
        self.events.back()
    }

    pub fn send(&mut self, event: T) {
        self.events.push_back(event);
    }

    pub fn send_batch(&mut self, events: impl Iterator<Item = T>) {
        self.events.extend(events);
    }

    #[inline]
    pub fn clear(&mut self) {
        let next_id = self.first_id + self.events.len();
        self.first_id = next_id;
        self.frame_start_id = next_id;
        self.events.clear();
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn install_into(res: &mut Resources) -> ResourceId<Self>
    where
        T: Send + Sync + 'static,
    {
        match res.try_init::<Self>() {
            Err(id) => id,
            Ok(id) => {
                let mut schedule = res.borrow_res_mut::<Schedule>().unwrap();
                Self::install_systems(&mut schedule);
                id
            }
        }
    }
}

#[system_module]
#[__crate_path(crate)]
impl<T: Send + Sync + 'static> Events<T> {
    #[system(into = CoreSystemSet::First)]
    pub fn update(&mut self) {
        while self.first_id != self.frame_start_id {
            self.first_id += 1;
            self.events.pop_front();
        }

        self.frame_start_id = self.first_id + self.events.len();
    }
}

impl<T> Default for Events<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Extend<T> for Events<T> {
    #[inline]
    fn extend<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.send_batch(events.into_iter())
    }
}

pub type Iter<'a, T> = std::collections::vec_deque::Iter<'a, T>;

pub struct IdIter<'a, T> {
    base: Iter<'a, T>,
    next_id: usize,
}

impl<T> Clone for IdIter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            next_id: self.next_id,
        }
    }
}

impl<'a, T> Iterator for IdIter<'a, T> {
    type Item = (EventId<T>, &'a T);

    #[inline]
    fn next(&mut self) -> Option<(EventId<T>, &'a T)> {
        let value = self.base.next()?;
        let id = EventId(self.next_id, PhantomData);
        self.next_id += 1;
        Some((id, value))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
}

impl<T> ExactSizeIterator for IdIter<'_, T> {}

impl<T> std::iter::FusedIterator for IdIter<'_, T> {}

#[derive(SystemData)]
#[__crate_path(crate)]
pub struct EventSubscriber<'r, T: 'static> {
    next_id: Local<'r, usize>,
    events: Res<'r, Events<T>>,
}

impl<T> EventSubscriber<'_, T> {
    #[inline]
    fn offset(&self) -> usize {
        self.next_id.saturating_sub(self.events.first_id)
    }

    pub fn iter(&mut self) -> Iter<'_, T> {
        let offset = self.offset();
        *self.next_id += self.events.events.len();
        self.events.events.range(offset..)
    }

    pub fn iter_with_id(&mut self) -> IdIter<'_, T> {
        let next_id = *self.next_id;
        IdIter {
            base: self.iter(),
            next_id,
        }
    }
}

#[derive(SystemData)]
#[__crate_path(crate)]
pub struct EventWriter<'r, T: 'static> {
    events: ResMut<'r, Events<T>>,
}

impl<T> EventWriter<'_, T> {
    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }

    pub fn send_batch(&mut self, events: impl Iterator<Item = T>) {
        self.events.send_batch(events);
    }
}

impl<T> Extend<T> for EventWriter<'_, T> {
    fn extend<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.events.send_batch(events.into_iter())
    }
}

impl Resources {
    #[inline]
    pub fn init_event<T>(&mut self) -> ResourceId<Events<T>>
    where
        T: Send + Sync + 'static,
    {
        Events::<T>::install_into(self)
    }
}
