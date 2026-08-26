//! A double-buffered event queue with per-subscriber read cursors.
//!
//! [`Events<T>`] stores events across two frames; a system clears the older half each frame.
//! [`EventSubscriber`] tracks a per-system read cursor; [`EventWriter`] sends new events.

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

/// A double-buffered queue of events of type `T`.
///
/// Events are retained for two frames so that systems which run once per frame always see them.
/// Use [`EventSubscriber`] to read and [`EventWriter`] to write from systems.
pub struct Events<T> {
    events: VecDeque<T>,
    first_id: usize,
    frame_start_id: usize,
}

impl<T> Events<T> {
    /// Creates an empty event queue.
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            first_id: 0,
            frame_start_id: 0,
        }
    }

    /// Returns a reference to the most recently sent event, if any.
    pub fn last(&self) -> Option<&T> {
        self.events.back()
    }

    /// Enqueues a single event.
    pub fn send(&mut self, event: T) {
        self.events.push_back(event);
    }

    /// Enqueues multiple events from an iterator.
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

/// System parameter that reads events from an [`Events<T>`] queue with a persistent read cursor.
///
/// Each subscriber tracks how far it has read so events are only delivered once per subscriber.
#[derive(SystemData)]
pub struct EventSubscriber<'r, T: 'static> {
    next_id: Local<'r, usize>,
    events: Res<'r, Events<T>>,
}

impl<T> EventSubscriber<'_, T> {
    #[inline]
    fn offset(&self) -> usize {
        self.next_id.saturating_sub(self.events.first_id)
    }

    /// Returns an iterator over all events not yet read by this subscriber.
    pub fn iter(&mut self) -> Iter<'_, T> {
        let offset = self.offset();
        *self.next_id += self.events.events.len();
        self.events.events.range(offset..)
    }

    /// Like [`iter`](Self::iter) but also yields the [`EventId`] for each event.
    pub fn iter_with_id(&mut self) -> IdIter<'_, T> {
        let next_id = *self.next_id;
        IdIter {
            base: self.iter(),
            next_id,
        }
    }
}

/// System parameter for sending events into an [`Events<T>`] queue.
#[derive(SystemData)]
pub struct EventWriter<'r, T: 'static> {
    events: ResMut<'r, Events<T>>,
}

impl<T> EventWriter<'_, T> {
    /// Sends a single event.
    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }

    /// Sends multiple events.
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- new / is_empty ---

    #[test]
    fn events_new_is_empty() {
        let events: Events<i32> = Events::new();
        assert!(events.is_empty());
    }

    // --- send / send_batch ---

    #[test]
    fn events_send_single() {
        let mut events = Events::new();
        events.send(42);
        assert!(!events.is_empty());
        assert_eq!(events.last(), Some(&42));
    }

    #[test]
    fn events_send_batch() {
        let mut events = Events::new();
        events.send_batch([1, 2, 3].into_iter());
        assert_eq!(events.last(), Some(&3));
    }

    #[test]
    fn events_last_empty() {
        let events: Events<i32> = Events::new();
        assert!(events.last().is_none());
    }

    // --- clear ---

    #[test]
    fn events_clear() {
        let mut events = Events::new();
        events.send(1);
        events.send(2);
        events.clear();
        assert!(events.is_empty());
        assert!(events.last().is_none());
    }

    // --- Extend ---

    #[test]
    fn events_extend() {
        let mut events = Events::new();
        events.extend([10, 20, 30]);
        assert_eq!(events.last(), Some(&30));
    }
}
