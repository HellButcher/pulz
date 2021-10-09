use std::{collections::VecDeque, marker::PhantomData};

use crate::{
    resource::{Res, ResMut, Resources},
    schedule::Schedule,
    system::param::{SystemParam, SystemParamFetch},
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

    pub fn update(&mut self) {
        while self.first_id != self.frame_start_id {
            self.first_id += 1;
            self.events.pop_front();
        }
    }

    pub fn update_system(mut events: ResMut<'_, Self>) {
        events.update()
    }

    pub fn install_into(resources: &mut Resources, schedule: &mut Schedule)
    where
        T: Send + Sync + 'static,
    {
        if resources.try_init::<Self>().is_ok() {
            schedule.add_system(Self::update_system);
        }
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

pub struct EventSubscriber<'w, T> {
    next_id: usize,
    events: Res<'w, Events<T>>,
}

impl<'w, T> EventSubscriber<'w, T> {
    #[inline]
    fn offset(&self) -> usize {
        if self.next_id > self.events.first_id {
            self.next_id - self.events.first_id
        } else {
            0
        }
    }

    pub fn iter(&mut self) -> Iter<'_, T> {
        let offset = self.offset();
        self.next_id += self.events.events.len();
        self.events.events.range(offset..)
    }

    pub fn iter_with_id(&mut self) -> IdIter<'_, T> {
        let next_id = self.next_id;
        IdIter {
            base: self.iter(),
            next_id,
        }
    }
}

pub struct EventWriter<'w, T>(ResMut<'w, Events<T>>);

impl<'w, T> EventWriter<'w, T> {
    pub fn send(&mut self, event: T) {
        self.0.send(event);
    }

    pub fn send_batch(&mut self, events: impl Iterator<Item = T>) {
        self.0.send_batch(events);
    }
}

impl<'w, T> Extend<T> for EventWriter<'w, T> {
    fn extend<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.0.send_batch(events.into_iter())
    }
}

unsafe impl<'a, T> SystemParam for EventSubscriber<'a, T>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = <Res<'a, Events<T>> as SystemParam>::Prepared;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        <Res<'a, Events<T>> as SystemParam>::prepare(resources)
    }
}

impl<'a, T> SystemParamFetch<'a> for EventSubscriber<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = EventSubscriber<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        EventSubscriber {
            next_id: 0, // TODO: keep state
            events: <Res<'a, Events<T>> as SystemParamFetch<'a>>::get(prepared, resources),
        }
    }
}

unsafe impl<'a, T> SystemParam for EventWriter<'a, T>
where
    T: Send + Sync + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = <ResMut<'a, Events<T>> as SystemParam>::Prepared;
    type Fetch = Self;

    #[inline]
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        <ResMut<'a, Events<T>> as SystemParam>::prepare(resources)
    }
}

impl<'a, T> SystemParamFetch<'a> for EventWriter<'_, T>
where
    T: Send + Sync + 'static,
{
    type Output = EventWriter<'a, T>;
    #[inline]
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output {
        EventWriter(<ResMut<'a, Events<T>> as SystemParamFetch<'a>>::get(
            prepared, resources,
        ))
    }
}
