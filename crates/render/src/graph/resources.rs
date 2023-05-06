use std::{
    collections::VecDeque,
    hash::{self, Hash},
    marker::PhantomData,
    ops::Deref,
};

use pulz_assets::Handle;
use pulz_bitset::BitSet;
use pulz_window::WindowId;

use super::{
    access::{ResourceAccess, Stage},
    builder::{GraphExport, GraphImport},
    deps::DependencyMatrix,
    PassIndex, RenderGraph, RenderGraphAssignments, ResourceIndex, SubPassIndex, PASS_UNDEFINED,
    SUBPASS_UNDEFINED,
};
use crate::{
    buffer::Buffer,
    camera::RenderTarget,
    texture::{Image, Texture, TextureDimensions, TextureFormat},
};

#[derive(Copy, Clone)]
pub struct Slot<R> {
    pub(crate) index: ResourceIndex,
    pub(crate) last_written_by: SubPassIndex,
    _phantom: PhantomData<fn() -> R>,
}

impl<R> std::fmt::Debug for Slot<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typename = std::any::type_name::<R>();
        f.debug_tuple(&format!("Slot<{typename}>"))
            .field(&self.index)
            .finish()
    }
}

// Not Copy by intention!
pub struct WriteSlot<R>(Slot<R>);

impl<R> std::fmt::Debug for WriteSlot<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typename = std::any::type_name::<R>();
        f.debug_tuple(&format!("WriteSlot<{typename}>"))
            .field(&self.0.index)
            .finish()
    }
}

pub trait SlotAccess {
    const WRITE: bool;
    fn index(&self) -> ResourceIndex;
}

impl<R> SlotAccess for Slot<R> {
    const WRITE: bool = false;
    #[inline]
    fn index(&self) -> ResourceIndex {
        self.index
    }
}

impl<R> SlotAccess for WriteSlot<R> {
    const WRITE: bool = true;
    #[inline]
    fn index(&self) -> ResourceIndex {
        self.0.index
    }
}

impl<R> Deref for WriteSlot<R> {
    type Target = Slot<R>;
    #[inline]
    fn deref(&self) -> &Slot<R> {
        &self.0
    }
}

impl<R> Slot<R> {
    const fn new(index: ResourceIndex, last_written_by: SubPassIndex) -> Self {
        Self {
            index,
            last_written_by,
            _phantom: PhantomData,
        }
    }
}

impl<R> WriteSlot<R> {
    #[inline]
    const fn new(index: ResourceIndex, last_written_by: SubPassIndex) -> Self {
        Self(Slot::new(index, last_written_by))
    }
    #[inline]
    pub const fn read(self) -> Slot<R> {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ResourceVariant {
    Transient,
    Import,
    Export,
}

struct Resource<R: ResourceAccess> {
    first_written: SubPassIndex,
    last_written: SubPassIndex,
    first_topo_group: u16,
    last_topo_group: u16,
    usage: R::Usage,
    format: Option<R::Format>,
    size: Option<R::Size>,
    variant: ResourceVariant,
    extern_res: Option<Box<dyn GetExternalResource<R>>>,
}

#[derive(Hash)]
pub(super) struct ResourceSet<R: ResourceAccess>(Vec<Resource<R>>);

impl<R: ResourceAccess> ResourceSet<R> {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<R: ResourceAccess> Resource<R> {
    #[inline]
    fn is_active(&self) -> bool {
        self.first_topo_group <= self.last_topo_group
    }

    #[inline]
    fn format_or_default(&self) -> R::Format {
        if let Some(f) = self.format {
            f
        } else {
            R::default_format(self.usage)
        }
    }
}

impl<R: ResourceAccess> Hash for Resource<R> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.first_written.hash(state);
        self.last_written.hash(state);
        self.variant.hash(state);
        // ignore extern_res!
    }
}

pub struct ResourceDeps<R: ResourceAccess>(Vec<ResourceDep<R>>);

impl<R: ResourceAccess> Hash for ResourceDeps<R> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash_slice(&self.0, state);
    }
}

#[derive(Hash)]
pub struct ResourceDep<R: ResourceAccess> {
    index: ResourceIndex,
    last_written_by_pass: PassIndex,
    write_access: bool,
    stages: Stage,
    usage: R::Usage,
}

impl<R: ResourceAccess> ResourceSet<R> {
    #[inline]
    pub(super) const fn new() -> Self {
        Self(Vec::new())
    }

    pub(super) fn reset(&mut self) {
        self.0.clear();
    }

    pub(super) fn create(&mut self) -> WriteSlot<R> {
        let index = self.0.len() as ResourceIndex;
        self.0.push(Resource {
            first_written: SUBPASS_UNDEFINED,
            last_written: SUBPASS_UNDEFINED,
            first_topo_group: !0,
            last_topo_group: 0,
            usage: Default::default(),
            format: None,
            size: None,
            variant: ResourceVariant::Transient,
            extern_res: None,
        });
        WriteSlot::new(index, SUBPASS_UNDEFINED)
    }

    pub(super) fn set_format(&mut self, slot: &Slot<R>, format: R::Format) {
        let slot = &mut self.0[slot.index as usize];
        if let Some(old_format) = &slot.format {
            assert_eq!(old_format, &format, "incompatible format");
        }
        slot.format = Some(format);
    }

    pub(super) fn set_size(&mut self, slot: &Slot<R>, size: R::Size) {
        let slot = &mut self.0[slot.index as usize];
        slot.size = Some(size);
    }

    pub(super) fn writes(&mut self, slot: WriteSlot<R>, new_pass: SubPassIndex) -> WriteSlot<R> {
        let r = &mut self.0[slot.0.index as usize];
        let last_written_by_pass = r.last_written;
        assert_eq!(
            last_written_by_pass, slot.0.last_written_by,
            "resource also written by an other pass (slot out of sync)"
        );
        if new_pass != last_written_by_pass {
            r.last_written = new_pass;
            if r.first_written.0 == PASS_UNDEFINED {
                r.first_written = new_pass
            }
        }
        WriteSlot::new(slot.0.index, new_pass)
    }

    pub(super) fn reads(&mut self, slot: Slot<R>) {
        assert_ne!(
            slot.last_written_by.0, PASS_UNDEFINED,
            "resource was not yet written!"
        );
        let r = &self.0[slot.index as usize];
        let last_written_by_pass = r.last_written;
        // TODO: allow usage of older slots for reading (Write>Read>Write)
        assert_eq!(
            last_written_by_pass, slot.last_written_by,
            "resource also written by an other pass (slot out of sync)"
        );
    }

    pub(super) fn import(&mut self, f: Box<dyn GetExternalResource<R>>) -> Slot<R> {
        let slot = self.create();
        let r = &mut self.0[slot.index as usize];
        r.variant = ResourceVariant::Import;
        r.extern_res = Some(f);
        slot.read()
    }

    pub(super) fn export(&mut self, slot: Slot<R>, f: Box<dyn GetExternalResource<R>>) {
        let r = &mut self.0[slot.index as usize];
        assert_eq!(
            ResourceVariant::Transient,
            r.variant,
            "resource can be exported only once"
        );
        // TODO: allow multiple exports by copying resource?
        r.variant = ResourceVariant::Export;
        r.extern_res = Some(f);
    }
}

impl<R: ResourceAccess> ResourceDeps<R> {
    #[inline]
    pub fn deps(&self) -> &[ResourceDep<R>] {
        &self.0
    }

    pub fn find_by_resource_index(&self, resource_index: ResourceIndex) -> Option<&ResourceDep<R>> {
        if let Ok(i) = self.0.binary_search_by_key(&resource_index, |d| d.index) {
            Some(&self.0[i])
        } else {
            None
        }
    }

    #[inline]
    pub(super) const fn new() -> Self {
        Self(Vec::new())
    }

    pub(super) fn mark_deps(&self, marks: &mut BitSet, todo: &mut VecDeque<u16>) {
        for dep in &self.0 {
            let pass_index = dep.src_pass();
            if pass_index != !0 && marks.insert(pass_index as usize) {
                todo.push_back(pass_index);
            }
        }
    }

    pub(super) fn mark_pass_dependency_matrix(&self, m: &mut DependencyMatrix, to_pass: PassIndex) {
        for dep in &self.0 {
            let pass_index = dep.src_pass();
            if pass_index != !0 {
                m.insert(pass_index as usize, to_pass as usize);
            }
        }
    }

    pub(super) fn update_resource_topo_group_range(
        &self,
        res: &mut ResourceSet<R>,
        group_index: u16,
    ) {
        for dep in &self.0 {
            let r = &mut res.0[dep.resource_index() as usize];
            if r.first_topo_group > group_index {
                r.first_topo_group = group_index;
            }
            if r.last_topo_group < group_index {
                r.last_topo_group = group_index;
            }
        }
    }

    pub(super) fn access(
        &mut self,
        slot: &Slot<R>,
        write_access: bool,
        stages: Stage,
        usage: R::Usage,
    ) -> bool {
        match self.0.binary_search_by_key(&slot.index, |e| e.index) {
            Ok(i) => {
                let entry = &mut self.0[i];
                assert_eq!(entry.last_written_by_pass, slot.last_written_by.0);
                entry.write_access |= write_access;
                entry.stages |= stages;
                entry.usage |= usage;
                if entry.write_access {
                    R::check_usage_is_pass_compatible(entry.usage);
                }
                false
            }
            Err(i) => {
                self.0.insert(
                    i,
                    ResourceDep {
                        index: slot.index,
                        last_written_by_pass: slot.last_written_by.0,
                        write_access,
                        stages,
                        usage,
                    },
                );
                true
            }
        }
    }
}

impl<R: ResourceAccess> Deref for ResourceDeps<R> {
    type Target = [ResourceDep<R>];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: ResourceAccess> ResourceDep<R> {
    #[inline]
    pub fn resource_index(&self) -> ResourceIndex {
        self.index
    }

    #[inline]
    pub fn src_pass(&self) -> PassIndex {
        self.last_written_by_pass
    }

    #[inline]
    pub fn stages(&self) -> Stage {
        self.stages
    }

    #[inline]
    pub fn usage(&self) -> R::Usage {
        self.usage
    }

    #[inline]
    pub fn is_read(&self) -> bool {
        self.last_written_by_pass != !0
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        self.write_access
    }
}

enum ResourceAssignment<R: ResourceAccess> {
    Undefined,
    Inactive,
    Extern(R, R::Format, R::Size),
    Transient(R::Format, u16),
}

pub(super) struct ResourceAssignments<R: ResourceAccess> {
    assignments: Vec<ResourceAssignment<R>>,
    transient_physical: Vec<(R::Format, u16)>,
}

impl<R: ResourceAccess + Copy> ResourceAssignments<R> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            assignments: Vec::new(),
            transient_physical: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.assignments.clear();
        self.transient_physical.clear()
    }

    fn get(&self, idx: ResourceIndex) -> Option<(R, R::Format, R::Size)> {
        match self.assignments.get(idx as usize)? {
            ResourceAssignment::Extern(r, f, s) => Some((*r, *f, *s)),
            ResourceAssignment::Transient(_, p) => {
                let (f, _) = self.transient_physical[*p as usize];
                todo!("implement get physical ({f:?})")
            }
            _ => None,
        }
    }

    fn assign_resources(&mut self, res: &ResourceSet<R>, backend: &mut dyn GraphBackend) -> bool {
        let mut changed = false;
        if self.assignments.len() < res.0.len() {
            changed = true;
            self.assignments
                .resize_with(res.0.len(), || ResourceAssignment::Undefined);
        }
        for (a, r) in self.assignments.iter_mut().zip(res.0.iter()) {
            if !r.is_active() {
                *a = ResourceAssignment::Inactive;
            } else if let Some(ext_res) = &r.extern_res {
                let (id, format, size) = ext_res.get(backend);
                if !changed {
                    if let ResourceAssignment::Extern(_, old_format, old_size) = *a {
                        changed = old_format != format || old_size != size;
                    } else {
                        changed = true;
                    }
                }
                *a = ResourceAssignment::Extern(id, format, size);
            } else if let ResourceAssignment::Transient(f, _) = a {
                let format = r.format_or_default();
                changed |= f != &format;
                *f = format;
            } else {
                changed = true;
                let format = r.format_or_default();
                *a = ResourceAssignment::Transient(format, !0);
            }
        }
        changed
    }

    fn assign_physical(&mut self, res: &ResourceSet<R>) {
        self.transient_physical.clear();
        let mut res_sorted: Vec<_> = res.0.iter().enumerate().collect();
        res_sorted.sort_by_key(|&(_, r)| r.first_topo_group);
        for (i, r) in res_sorted {
            if let ResourceAssignment::Transient(format, p) = &mut self.assignments[i] {
                *p = !0;
                for (j, (phys_format, last_topo_group)) in
                    self.transient_physical.iter_mut().enumerate()
                {
                    if *format == *phys_format && *last_topo_group < r.first_topo_group {
                        *last_topo_group = r.last_topo_group;
                        *p = j as u16;
                    }
                }
                if *p != !0 {
                    *p = self.transient_physical.len() as u16;
                    self.transient_physical.push((*format, r.last_topo_group));
                }
                // TODO: calc. max size!
            }
        }
    }
}

impl RenderGraphAssignments {
    pub fn clear(&mut self) {
        self.hash = 0;
        self.was_updated = false;
        self.texture_assignments.clear();
        self.buffer_assignments.clear();
    }

    pub fn update(&mut self, graph: &RenderGraph, backend: &mut dyn GraphBackend) -> bool {
        self.was_updated = graph.was_updated;
        if self.hash != graph.hash {
            self.hash = graph.hash;
            self.was_updated = true;
        }
        self.was_updated |= self
            .texture_assignments
            .assign_resources(&graph.textures, backend);
        self.was_updated |= self
            .buffer_assignments
            .assign_resources(&graph.buffers, backend);

        if self.was_updated {
            self.texture_assignments.assign_physical(&graph.textures);
            self.buffer_assignments.assign_physical(&graph.buffers);
        }

        self.was_updated
    }

    pub(crate) fn get_texture(
        &self,
        idx: ResourceIndex,
    ) -> Option<(Texture, TextureFormat, u8, TextureDimensions)> {
        let (r, f, s) = self.texture_assignments.get(idx)?;
        Some((r, f, 1, s))
    }

    pub(crate) fn get_buffer(&self, idx: ResourceIndex) -> Option<(Buffer, usize)> {
        let (r, _, s) = self.buffer_assignments.get(idx)?;
        Some((r, s))
    }
}

pub trait GetExternalResource<R: ResourceAccess> {
    fn get(&self, backend: &mut dyn GraphBackend) -> (R, R::Format, R::Size);
}

impl<R, F> GetExternalResource<R> for F
where
    R: ResourceAccess,
    F: for<'l, 'r> Fn(&'l mut dyn GraphBackend) -> (R, R::Format, R::Size),
{
    fn get(&self, backend: &mut dyn GraphBackend) -> (R, R::Format, R::Size) {
        self(backend)
    }
}

pub trait GraphBackend {
    fn get_surface(&mut self, window: WindowId) -> (Texture, TextureFormat, TextureDimensions);
}

impl GraphImport for Handle<Image> {
    type Resource = Texture;

    fn import(&self) -> Box<dyn GetExternalResource<Texture>> {
        let handle = *self;
        Box::new(move |_backend: &mut dyn GraphBackend| todo!("import image handle {:?}", handle))
    }
}

impl GraphExport for Handle<Image> {
    type Resource = Texture;

    fn export(&self) -> Box<dyn GetExternalResource<Texture>> {
        let handle = *self;
        Box::new(move |_backend: &mut dyn GraphBackend| todo!("export image handle {:?}", handle))
    }
}

impl GraphExport for WindowId {
    type Resource = Texture;

    fn export(&self) -> Box<dyn GetExternalResource<Texture>> {
        let window = *self;
        Box::new(move |backend: &mut dyn GraphBackend| backend.get_surface(window))
    }
}

impl GraphExport for RenderTarget {
    type Resource = Texture;

    fn export(&self) -> Box<dyn GetExternalResource<Texture>> {
        match self {
            Self::Image(i) => i.export(),
            Self::Window(w) => w.export(),
        }
    }
}
