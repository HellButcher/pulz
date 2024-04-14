use std::{
    any::{Any, TypeId},
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
};

use atomic_refcell::AtomicRefCell;
use dynsequence::{dyn_sequence, DynSequence};
use fnv::FnvHashMap as HashMap;
use pulz_ecs::{prelude::*, resource::ResState, system::data::SystemData};

use crate::{backend::CommandEncoder, utils::hash::TypeIdHashMap, RenderSystemPhase};

pub type DrawContext<'a> = &'a mut (dyn CommandEncoder + 'a);

pub trait Drawable {
    fn draw(&self, cmds: DrawContext<'_>);
}
impl<D: ?Sized + Drawable> Drawable for &D {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        D::draw(self, cmds)
    }
}
impl<D: ?Sized + Drawable> Drawable for &mut D {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        D::draw(self, cmds)
    }
}
impl<D: Drawable> Drawable for [D] {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        for d in self {
            D::draw(d, cmds)
        }
    }
}
impl<D: ?Sized + Drawable> Drawable for Box<D> {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        D::draw(self.as_ref(), cmds)
    }
}
impl<D: Drawable> Drawable for Vec<D> {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        <[D]>::draw(self.as_slice(), cmds)
    }
}

impl<D: ?Sized + Drawable> Drawable for DynSequence<D> {
    #[inline]
    fn draw(&self, cmds: DrawContext<'_>) {
        <[&D]>::draw(self.as_slice(), cmds)
    }
}

pub type DynDrawables = DynSequence<dyn Drawable + Send + Sync + 'static>;

pub trait PhaseItem: Send + Sync + Sized + 'static {
    type TargetKey: Copy + Clone + Hash + Ord + Eq + Send + Sync;
    fn sort<E>(items: &mut [E])
    where
        E: Deref<Target = Self>;
}

#[doc(hidden)]
pub struct DrawQueue<I: PhaseItem>(crossbeam_queue::SegQueue<(I::TargetKey, PhaseData<I>)>);

struct PhaseDataMap<I: PhaseItem>(AtomicRefCell<HashMap<I::TargetKey, PhaseData<I>>>);

impl<I: PhaseItem> PhaseDataMap<I> {
    fn new() -> Self {
        Self(AtomicRefCell::new(HashMap::default()))
    }

    fn new_any() -> Box<dyn Any + Send + Sync> {
        Box::new(Self::new())
    }

    fn get(&self, target_key: I::TargetKey) -> Option<atomic_refcell::AtomicRef<'_, PhaseData<I>>> {
        atomic_refcell::AtomicRef::filter_map(self.0.borrow(), |v| v.get(&target_key))
    }
}
pub struct PhaseData<I: PhaseItem> {
    drawables: DynDrawables,
    items: Vec<PhaseDataItem<I>>,
}

struct PhaseDataItem<I: PhaseItem> {
    item: I,
    draw_offset: usize,
    draw_count: usize,
}

impl<I: PhaseItem> Deref for PhaseDataItem<I> {
    type Target = I;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<I: PhaseItem> PhaseData<I> {
    #[inline]
    const fn new() -> Self {
        Self {
            drawables: DynDrawables::new(),
            items: Vec::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn push(&mut self, item: I) -> PhaseDraw<'_> {
        let draw_offset = self.drawables.len();
        let index = self.items.len();
        self.items.push(PhaseDataItem {
            draw_offset,
            draw_count: 0,
            item,
        });
        let item = &mut self.items[index];
        PhaseDraw {
            drawables: &mut self.drawables,
            count: &mut item.draw_count,
        }
    }

    fn clear(&mut self) {
        self.drawables.clear();
        self.items.clear();
    }

    fn extend(&mut self, mut other: Self) {
        let drawables_offset = self.drawables.len();
        self.drawables
            .extend_dynsequence(std::mem::take(&mut other.drawables));
        let mut other_items = std::mem::take(&mut other.items);
        for other_item in &mut other_items {
            other_item.draw_offset += drawables_offset;
        }
        self.items.extend(other_items);
    }

    fn sort(&mut self) {
        I::sort(self.items.as_mut_slice());
    }

    pub(crate) fn draw(&self, cmds: DrawContext<'_>) {
        for item in &self.items {
            for draw in
                &self.drawables.as_slice()[item.draw_offset..item.draw_offset + item.draw_count]
            {
                draw.draw(cmds);
            }
        }
    }
}

pub struct PhaseDraw<'l> {
    drawables: &'l mut DynDrawables,
    count: &'l mut usize,
}

impl PhaseDraw<'_> {
    pub fn draw<D>(&mut self, draw: D)
    where
        D: Drawable + Send + Sync + 'static,
    {
        dyn_sequence![dyn Drawable + Send + Sync + 'static | &mut self.drawables => {
            push(draw);
        }];
        *self.count += 1;
    }
}

pub struct DrawPhases(TypeIdHashMap<Box<dyn Any + Send + Sync>>);

impl DrawPhases {
    pub fn new() -> Self {
        Self(TypeIdHashMap::default())
    }
}

impl Default for DrawPhases {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl DrawPhases {
    fn get_map<I: PhaseItem>(&self) -> Option<&PhaseDataMap<I>> {
        self.0
            .get(&TypeId::of::<PhaseDataMap<I>>())?
            .downcast_ref::<PhaseDataMap<I>>()
    }

    pub fn get<I: PhaseItem>(
        &self,
        target_key: I::TargetKey,
    ) -> Option<atomic_refcell::AtomicRef<'_, PhaseData<I>>> {
        self.get_map::<I>()?.get(target_key)
    }

    fn register<I: PhaseItem>(&mut self) {
        self.0
            .entry(TypeId::of::<PhaseDataMap<I>>())
            .or_insert_with(PhaseDataMap::<I>::new_any);
    }
}

pub struct Draw<'l, I: PhaseItem> {
    destination: &'l DrawQueue<I>,
}

impl<I: PhaseItem> Draw<'_, I> {
    #[inline]
    pub fn draw(&mut self, target_key: I::TargetKey) -> DrawTarget<'_, I> {
        DrawTarget {
            draw: self,
            data: PhaseData::new(),
            target_key,
        }
    }
}

pub struct DrawTarget<'l, I: PhaseItem> {
    draw: &'l Draw<'l, I>,
    data: PhaseData<I>,
    target_key: I::TargetKey,
}

impl<I: PhaseItem> Default for DrawQueue<I> {
    #[inline]
    fn default() -> Self {
        Self(crossbeam_queue::SegQueue::new())
    }
}

fn collect_and_sort_draws_system<I: PhaseItem>(queue: &mut DrawQueue<I>, phases: &DrawPhases) {
    let mut phase_map = phases.get_map::<I>().unwrap().0.borrow_mut();

    // clear sequences
    for phase_data in phase_map.values_mut() {
        phase_data.clear();
    }

    // TODO: optimize with a variant of merge-sort with pre-sorted chunks.
    // pre-sort chunks inside Draw::flush, where it could utilize other threads.
    for (target_key, chunk) in std::mem::take(&mut queue.0) {
        phase_map
            .entry(target_key)
            .or_insert_with(PhaseData::new)
            .extend(chunk);
    }

    // remove empty sequences
    phase_map.retain(|_, v| !v.items.is_empty());

    // sort remaining sequences
    for phase_data in phase_map.values_mut() {
        phase_data.sort();
    }
}

impl<I: PhaseItem> DrawTarget<'_, I> {
    pub fn flush(&mut self) {
        if !self.data.is_empty() {
            // move commands into queue
            self.draw.destination.0.push((
                self.target_key,
                std::mem::replace(&mut self.data, PhaseData::new()),
            ));
        }
    }
    pub fn push(&mut self, item: I) -> PhaseDraw<'_> {
        if self.data.len() >= 64 {
            self.flush();
        }
        self.data.push(item)
    }
}

impl<I: PhaseItem> Drop for DrawTarget<'_, I> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl<I: PhaseItem> SystemData for Draw<'_, I> {
    type State = ResState<DrawQueue<I>>;
    type Fetch<'r> = Res<'r, DrawQueue<I>>;
    type Item<'a> = Draw<'a, I>;

    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        Draw { destination: fetch }
    }
}

pub struct PhaseModule<I>(PhantomData<fn(&I)>);

impl<I: PhaseItem + Sync> Default for PhaseModule<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: PhaseItem + Sync> PhaseModule<I> {
    #[inline]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<I: PhaseItem + Sync> Module for PhaseModule<I> {
    fn install_once(&self, res: &mut Resources) {
        let phases = res.init::<DrawPhases>();
        res.init::<DrawQueue<I>>();
        res.get_mut_id(phases).unwrap().register::<I>();
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(collect_and_sort_draws_system::<I>)
            .into_phase(RenderSystemPhase::Sorting);
    }
}
