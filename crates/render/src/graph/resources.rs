use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::Deref,
    usize,
};

use pulz_assets::Handle;
use pulz_bitset::BitSet;
use pulz_window::WindowId;

use super::{
    access::{ResourceAccess, Stage},
    builder::{GraphExport, GraphImport},
    deps::DependencyMatrix,
    PassDescription, PassIndex, RenderGraph, ResourceIndex, SubPassDescription, SubPassIndex,
    PASS_UNDEFINED, SUBPASS_UNDEFINED,
};
use crate::{
    backend::PhysicalResourceResolver,
    buffer::{Buffer, BufferUsage},
    camera::RenderTarget,
    texture::{Image, Texture, TextureDimensions, TextureFormat, TextureUsage},
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

#[derive(Debug)]
struct Resource<R: ResourceAccess> {
    first_written: SubPassIndex,
    last_written: SubPassIndex,
    usage: R::Usage,
    format: Option<R::Format>,
    size: Option<R::Size>,
    variant: ResourceVariant,
    extern_assignment: Option<R::ExternHandle>,
}

#[derive(Debug)]
pub(super) struct ExtendedResourceData {
    pub first_topo_group: u16,
    pub last_topo_group: u16,
}

#[derive(Debug)]
pub(super) struct ResourceSet<R: ResourceAccess> {
    resources: Vec<Resource<R>>,
}

impl<R: ResourceAccess> Resource<R> {
    #[inline]
    fn format_or_default(&self) -> R::Format {
        if let Some(f) = self.format {
            f
        } else {
            R::default_format(self.usage)
        }
    }
}

impl ExtendedResourceData {
    #[inline]
    pub const fn new() -> Self {
        Self {
            first_topo_group: !0,
            last_topo_group: 0,
        }
    }

    #[inline]
    fn is_active(&self) -> bool {
        self.first_topo_group <= self.last_topo_group
    }
}

impl<R: ResourceAccess> Hash for ResourceSet<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.resources.hash(state);
        // ignore transients
    }
}

impl<R: ResourceAccess> Hash for Resource<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.first_written.hash(state);
        self.last_written.hash(state);
        self.variant.hash(state);
        self.format.hash(state);
        // ignore size and extern assignment!
    }
}

#[derive(Debug)]
pub struct ResourceDeps<R: ResourceAccess>(Vec<ResourceDep<R>>);

impl<R: ResourceAccess> Hash for ResourceDeps<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash_slice(&self.0, state);
    }
}

#[derive(Hash, Debug)]
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
        Self {
            resources: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub(super) fn reset(&mut self) {
        self.resources.clear();
    }

    pub(super) fn create(&mut self) -> WriteSlot<R> {
        let index = self.resources.len() as ResourceIndex;
        self.resources.push(Resource {
            first_written: SUBPASS_UNDEFINED,
            last_written: SUBPASS_UNDEFINED,
            usage: Default::default(),
            format: None,
            size: None,
            variant: ResourceVariant::Transient,
            extern_assignment: None,
        });
        WriteSlot::new(index, SUBPASS_UNDEFINED)
    }

    pub(super) fn set_format(&mut self, slot: &Slot<R>, format: R::Format) {
        let slot = &mut self.resources[slot.index as usize];
        if let Some(old_format) = &slot.format {
            assert_eq!(old_format, &format, "incompatible format");
        }
        slot.format = Some(format);
    }

    pub(super) fn set_size(&mut self, slot: &Slot<R>, size: R::Size) {
        let slot = &mut self.resources[slot.index as usize];
        slot.size = Some(size);
    }

    pub(super) fn writes(
        &mut self,
        slot: WriteSlot<R>,
        new_pass: SubPassIndex,
        usage: R::Usage,
    ) -> WriteSlot<R> {
        let r = &mut self.resources[slot.0.index as usize];
        let last_written_by_pass = r.last_written;
        assert_eq!(
            last_written_by_pass, slot.0.last_written_by,
            "resource also written by an other pass (slot out of sync)"
        );
        r.usage |= usage;
        if new_pass != last_written_by_pass {
            r.last_written = new_pass;
            if r.first_written.0 == PASS_UNDEFINED {
                r.first_written = new_pass
            }
        }
        WriteSlot::new(slot.0.index, new_pass)
    }

    pub(super) fn reads(&mut self, slot: Slot<R>, usage: R::Usage) {
        assert_ne!(
            slot.last_written_by.0, PASS_UNDEFINED,
            "resource was not yet written!"
        );
        let r = &mut self.resources[slot.index as usize];
        let last_written_by_pass = r.last_written;
        // TODO: allow usage of older slots for reading (Write>Read>Write)
        assert_eq!(
            last_written_by_pass, slot.last_written_by,
            "resource also written by an other pass (slot out of sync)"
        );
        r.usage |= usage;
    }

    pub(super) fn import(&mut self, extern_resource: R::ExternHandle) -> Slot<R> {
        let slot = self.create();
        let r = &mut self.resources[slot.index as usize];
        r.variant = ResourceVariant::Import;
        r.extern_assignment = Some(extern_resource);
        slot.read()
    }

    pub(super) fn export(&mut self, slot: Slot<R>, extern_resource: R::ExternHandle) {
        let r = &mut self.resources[slot.index as usize];
        assert_eq!(
            ResourceVariant::Transient,
            r.variant,
            "resource can be exported only once"
        );
        assert_eq!(
            None, r.format,
            "format of slot must be undefined for exports. Export target format will be used."
        );
        assert_eq!(
            None, r.size,
            "size of slot must be undefined for exports. Export target size will be used."
        );
        // TODO: allow multiple exports by copying resource?
        r.variant = ResourceVariant::Export;
        r.extern_assignment = Some(extern_resource);
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
        res: &mut [ExtendedResourceData],
        group_index: u16,
    ) {
        for dep in &self.0 {
            let d = &mut res[dep.resource_index() as usize];
            if d.first_topo_group > group_index {
                d.first_topo_group = group_index;
            }
            if d.last_topo_group < group_index {
                d.last_topo_group = group_index;
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PhysicalResource<R: ResourceAccess> {
    pub resource: R,
    pub format: R::Format,
    pub size: R::Size,
    pub usage: R::Usage,
}

impl<R: ResourceAccess> Hash for PhysicalResource<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.format.hash(state);
        // ignore resource and size
    }
}

#[derive(Hash, Debug)]
struct TransientResource<R: ResourceAccess> {
    physical: PhysicalResource<R>,
    first_topo_group: u16,
    last_topo_group: u16,
}

#[derive(Copy, Clone, Hash, Debug)]
enum ExternalOrTransient {
    None,
    External(u16),
    Transient(u16),
}

#[derive(Hash, Debug)]
struct PhysicalResourceSet<R: ResourceAccess> {
    assignments: Vec<ExternalOrTransient>,
    assignment_sizes: Vec<Option<R::Size>>,
    externals: Vec<PhysicalResource<R>>,
    transients: Vec<TransientResource<R>>,
}

trait ResolveExtern<R: ResourceAccess> {
    fn resolve_extern(&mut self, handle: &R::ExternHandle) -> Option<PhysicalResource<R>>;
}
trait CreateTransient<R: ResourceAccess> {
    fn create_transient(&mut self, format: R::Format, size: R::Size, usage: R::Usage) -> Option<R>;
}

impl<R: ResourceAccess> PhysicalResourceSet<R> {
    #[inline]
    pub(super) const fn new() -> Self {
        Self {
            assignments: Vec::new(),
            assignment_sizes: Vec::new(),
            externals: Vec::new(),
            transients: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    fn reset(&mut self, resources: &ResourceSet<R>) {
        self.assignments.clear();
        self.assignment_sizes.clear();
        self.externals.clear();
        self.transients.clear();
        self.assignments
            .resize_with(resources.len(), || ExternalOrTransient::None);
        for res in resources.resources.iter() {
            self.assignment_sizes.push(res.size);
        }
    }

    fn get_physical(&self, idx: ResourceIndex) -> Option<&PhysicalResource<R>> {
        match self.assignments.get(idx as usize).copied()? {
            ExternalOrTransient::None => None,
            ExternalOrTransient::External(e) => self.externals.get(e as usize),
            ExternalOrTransient::Transient(t) => {
                self.transients.get(t as usize).map(|t| &t.physical)
            }
        }
    }

    fn get_or_create_transient(
        transients: &mut Vec<TransientResource<R>>,
        format: R::Format,
        size: R::Size,
        usage: R::Usage,
        first_topo_group: u16,
        last_topo_group: u16,
    ) -> u16 {
        for (j, p) in transients.iter_mut().enumerate() {
            if format == p.physical.format && p.last_topo_group < first_topo_group {
                if let Some(s) = R::merge_size_max(p.physical.size, size) {
                    p.physical.size = s;
                    p.physical.usage |= usage;
                    p.last_topo_group = last_topo_group;
                    return j as u16;
                }
            }
        }
        let index = transients.len();
        transients.push(TransientResource {
            physical: PhysicalResource {
                resource: R::default(),
                format,
                size,
                usage,
            },
            first_topo_group,
            last_topo_group,
        });
        index as u16
    }

    fn assign_externals<B: ResolveExtern<R>>(
        &mut self,
        resources: &ResourceSet<R>,
        resources_data: &[ExtendedResourceData],
        backend: &mut B,
    ) {
        // assign externals
        for (i, r) in resources.resources.iter().enumerate() {
            if !resources_data[i].is_active() {
                continue;
            }
            if let Some(extern_handle) = &r.extern_assignment {
                if let Some(ext) = backend.resolve_extern(extern_handle) {
                    // TODO: check usage compatible?
                    let external_index = self.externals.len() as u16;
                    self.assignments[i] = ExternalOrTransient::External(external_index);
                    self.assignment_sizes[i] = Some(ext.size);
                    self.externals.push(ext);
                } else {
                    panic!(
                        "unable to resolve external resource {:?}, first_written={:?}",
                        i, r.first_written
                    );
                }
            }
        }
    }

    fn assign_transient<B: CreateTransient<R>>(
        &mut self,
        resources: &ResourceSet<R>,
        resources_data: &[ExtendedResourceData],
        backend: &mut B,
    ) {
        let mut res_sorted: Vec<_> = (0..resources.len()).collect();
        res_sorted.sort_by_key(|&r| resources_data[r].first_topo_group);

        // pre-assign transients
        for &i in res_sorted.iter() {
            let r = &resources.resources[i];
            let d = &resources_data[i];
            if d.is_active() && matches!(self.assignments[i], ExternalOrTransient::None) {
                if r.usage == R::Usage::default() {
                    panic!(
                        "transient usage is empty, {:?}, {:?}, {}, {}, {:?}, {:?}",
                        r.size,
                        r.format,
                        d.first_topo_group,
                        d.last_topo_group,
                        r.first_written,
                        r.usage
                    );
                }
                let transient_index = Self::get_or_create_transient(
                    &mut self.transients,
                    r.format_or_default(),
                    self.assignment_sizes[i].expect("missing size"),
                    r.usage,
                    d.first_topo_group,
                    d.last_topo_group,
                );
                self.assignments[i] = ExternalOrTransient::Transient(transient_index);
            }
        }

        for trans in self.transients.iter_mut() {
            trans.physical.resource = backend
                .create_transient(
                    trans.physical.format,
                    trans.physical.size,
                    trans.physical.usage,
                )
                .expect("unable to create transient"); // TODO: error
        }
    }
}

impl PhysicalResourceSet<Texture> {
    fn derive_framebuffer_sizes(
        &mut self,
        passes: &[PassDescription],
        subpasses: &[SubPassDescription],
    ) {
        for pass in passes {
            if pass.active {
                self.derive_framebuffer_size_for_pass(pass, &subpasses[pass.sub_pass_range()]);
            }
        }
    }

    fn derive_framebuffer_size_for_pass(
        &mut self,
        pass: &PassDescription,
        subpasses: &[SubPassDescription],
    ) {
        let mut pass_size = None;
        let mut empty = true;
        for p in subpasses {
            for r_index in p
                .color_attachments
                .iter()
                .copied()
                .chain(p.depth_stencil_attachment.iter().copied())
                .chain(p.input_attachments.iter().copied())
            {
                empty = false;
                if let Some(s) = self.assignment_sizes[r_index as usize] {
                    if let Some(s2) = pass_size {
                        assert_eq!(s2, s, "pass attachments have to be the same size");
                    } else {
                        pass_size = Some(s);
                    }
                }
            }
        }
        if empty {
            return;
        }
        if pass_size.is_none() {
            panic!(
                "unable to derive framebuffer size for pass {:?}, physical_resource_set={:?}",
                pass.name, self
            );
        }

        for p in subpasses {
            for r_index in p
                .color_attachments
                .iter()
                .copied()
                .chain(p.depth_stencil_attachment.iter().copied())
                .chain(p.input_attachments.iter().copied())
            {
                let s = &mut self.assignment_sizes[r_index as usize];
                if s.is_none() {
                    *s = pass_size;
                }
            }
        }
    }
}

#[derive(Hash, Debug)]
pub struct PhysicalResources {
    textures: PhysicalResourceSet<Texture>,
    buffers: PhysicalResourceSet<Buffer>,
    hash: u64,
}

impl PhysicalResources {
    pub const fn new() -> Self {
        Self {
            textures: PhysicalResourceSet::new(),
            buffers: PhysicalResourceSet::new(),
            hash: 0,
        }
    }

    pub fn assign_physical<B: PhysicalResourceResolver>(
        &mut self,
        graph: &RenderGraph,
        backend: &mut B,
    ) -> bool {
        self.textures.reset(&graph.textures);
        self.buffers.reset(&graph.buffers);

        self.textures
            .assign_externals(&graph.textures, &graph.textures_ext, backend);
        self.buffers
            .assign_externals(&graph.buffers, &graph.buffers_ext, backend);

        self.textures
            .derive_framebuffer_sizes(&graph.passes, &graph.subpasses);

        self.textures
            .assign_transient(&graph.textures, &graph.textures_ext, backend);
        self.buffers
            .assign_transient(&graph.buffers, &graph.buffers_ext, backend);

        let new_hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.textures.hash(&mut hasher);
            self.buffers.hash(&mut hasher);
            hasher.finish()
        };
        let changed = self.hash != new_hash;
        self.hash = new_hash;
        changed
    }

    pub(crate) fn get_texture(
        &self,
        idx: ResourceIndex,
    ) -> Option<(Texture, TextureFormat, u8, TextureDimensions)> {
        let r = self.textures.get_physical(idx)?;
        Some((r.resource, r.format, 1, r.size))
    }

    pub(crate) fn get_buffer(&self, idx: ResourceIndex) -> Option<(Buffer, usize)> {
        let r = self.buffers.get_physical(idx)?;
        Some((r.resource, r.size))
    }
}

impl<R: ResourceAccess, T: GraphImport<R>> GraphImport<R> for &T {
    fn import(&self) -> R::ExternHandle {
        T::import(self)
    }
}

impl<R: ResourceAccess, T: GraphExport<R>> GraphExport<R> for &T {
    fn export(&self) -> R::ExternHandle {
        T::export(self)
    }
}

impl<R: ResourceAccess, T: GraphImport<R>> GraphImport<R> for &mut T {
    fn import(&self) -> R::ExternHandle {
        T::import(self)
    }
}

impl<R: ResourceAccess, T: GraphExport<R>> GraphExport<R> for &mut T {
    fn export(&self) -> R::ExternHandle {
        T::export(self)
    }
}

impl GraphImport<Texture> for Handle<Image> {
    fn import(&self) -> RenderTarget {
        RenderTarget::Image(*self)
    }
}

impl GraphExport<Texture> for Handle<Image> {
    fn export(&self) -> RenderTarget {
        RenderTarget::Image(*self)
    }
}

impl GraphExport<Texture> for WindowId {
    fn export(&self) -> RenderTarget {
        RenderTarget::Window(*self)
    }
}

impl GraphExport<Texture> for RenderTarget {
    fn export(&self) -> RenderTarget {
        *self
    }
}

impl GraphImport<Buffer> for Handle<Buffer> {
    fn import(&self) -> Handle<Buffer> {
        *self
    }
}

impl GraphExport<Buffer> for Handle<Buffer> {
    fn export(&self) -> Handle<Buffer> {
        *self
    }
}

impl<B> ResolveExtern<Texture> for B
where
    B: PhysicalResourceResolver,
{
    fn resolve_extern(&mut self, handle: &RenderTarget) -> Option<PhysicalResource<Texture>> {
        self.resolve_render_target(handle)
    }
}

impl<B> ResolveExtern<Buffer> for B
where
    B: PhysicalResourceResolver,
{
    fn resolve_extern(&mut self, handle: &Handle<Buffer>) -> Option<PhysicalResource<Buffer>> {
        self.resolve_buffer(handle)
    }
}

impl<B> CreateTransient<Texture> for B
where
    B: PhysicalResourceResolver,
{
    fn create_transient(
        &mut self,
        format: TextureFormat,
        size: TextureDimensions,
        usage: TextureUsage,
    ) -> Option<Texture> {
        self.create_transient_texture(format, size, usage)
    }
}
impl<B> CreateTransient<Buffer> for B
where
    B: PhysicalResourceResolver,
{
    fn create_transient(&mut self, _format: (), size: usize, usage: BufferUsage) -> Option<Buffer> {
        self.create_transient_buffer(size, usage)
    }
}
