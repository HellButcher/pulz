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
    access::{Access, ResourceAccess, Stage},
    builder::{GraphExport, GraphImport},
    deps::DependencyMatrix,
    PassDescription, PassIndex, RenderGraph, ResourceIndex, SubPassIndex, PASS_UNDEFINED,
    SUBPASS_UNDEFINED,
};
use crate::{
    backend::PhysicalResourceResolver,
    buffer::Buffer,
    camera::RenderTarget,
    texture::{Image, Texture, TextureDimensions, TextureFormat},
};

#[derive(Copy, Clone)]
pub struct SlotRaw {
    pub(crate) index: ResourceIndex,
    pub(crate) last_written_by: SubPassIndex,
}

#[derive(Copy, Clone)]
pub struct Slot<R> {
    raw: SlotRaw,
    _phantom: PhantomData<fn() -> R>,
}

impl<R> std::fmt::Debug for Slot<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typename = std::any::type_name::<R>();
        f.debug_tuple(&format!("Slot<{typename}>"))
            .field(&self.raw.index)
            .finish()
    }
}

impl<R> Deref for Slot<R> {
    type Target = SlotRaw;
    #[inline]
    fn deref(&self) -> &SlotRaw {
        &self.raw
    }
}

// Not Copy by intention!
pub struct WriteSlot<R>(Slot<R>);

impl<R> std::fmt::Debug for WriteSlot<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typename = std::any::type_name::<R>();
        f.debug_tuple(&format!("WriteSlot<{typename}>"))
            .field(&self.raw.index)
            .finish()
    }
}

impl<R> Deref for WriteSlot<R> {
    type Target = Slot<R>;
    #[inline]
    fn deref(&self) -> &Slot<R> {
        &self.0
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
        self.raw.index
    }
}

impl<R> SlotAccess for WriteSlot<R> {
    const WRITE: bool = true;
    #[inline]
    fn index(&self) -> ResourceIndex {
        self.raw.index
    }
}

impl<R> Slot<R> {
    const fn new(index: ResourceIndex, last_written_by: SubPassIndex) -> Self {
        Self {
            raw: SlotRaw {
                index,
                last_written_by,
            },
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
pub enum ResourceVariant {
    Transient,
    Import,
    Export,
}

#[derive(Debug)]
pub(crate) struct Resource<R: ResourceAccess> {
    first_written: SubPassIndex,
    last_written: SubPassIndex,
    is_read_after_last_written: bool,
    access: Access,
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
            R::default_format(self.access)
        }
    }

    #[inline]
    pub fn access(&self) -> Access {
        self.access
    }

    #[inline]
    pub fn size(&self) -> Option<R::Size> {
        self.size
    }

    #[inline]
    pub fn variant(&self) -> ResourceVariant {
        self.variant
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
        self.access.hash(state);
        self.variant.hash(state);
        self.format.hash(state);
        // ignore size and extern assignment!
    }
}

#[derive(Debug)]
pub struct ResourceDeps(Vec<ResourceDep>);

impl Hash for ResourceDeps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash_slice(&self.0, state);
    }
}

#[derive(Hash, Debug)]
pub struct ResourceDep {
    index: ResourceIndex,
    last_written_by_pass: PassIndex,
    access: Access,
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
            is_read_after_last_written: false,
            access: Access::empty(),
            format: None,
            size: None,
            variant: ResourceVariant::Transient,
            extern_assignment: None,
        });
        WriteSlot::new(index, SUBPASS_UNDEFINED)
    }

    pub(super) fn get(&self, index: usize) -> Option<&Resource<R>> {
        self.resources.get(index)
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
        access: Access,
    ) -> WriteSlot<R> {
        assert!(access.is_empty() || access.is_write());
        let r = &mut self.resources[slot.0.index as usize];
        let last_written_by_pass = r.last_written;
        assert_eq!(
            last_written_by_pass, slot.0.last_written_by,
            "resource also written by an other pass (slot out of sync)"
        );
        r.access |= access;
        if new_pass != last_written_by_pass {
            r.is_read_after_last_written = false;
            r.last_written = new_pass;
            if r.first_written.0 == PASS_UNDEFINED {
                r.first_written = new_pass
            }
        }
        WriteSlot::new(slot.0.index, new_pass)
    }

    pub(super) fn reads(&mut self, slot: Slot<R>, access: Access) {
        assert!(access.is_empty() || access.is_read());
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
        r.is_read_after_last_written = true;
        r.access |= access;
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

impl ResourceDeps {
    #[inline]
    pub fn deps(&self) -> &[ResourceDep] {
        &self.0
    }

    pub fn find_by_resource_index(&self, resource_index: ResourceIndex) -> Option<&ResourceDep> {
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

    pub(super) fn access(&mut self, slot: &SlotRaw, access: Access) -> bool {
        match self.0.binary_search_by_key(&slot.index, |e| e.index) {
            Ok(i) => {
                let entry = &mut self.0[i];
                assert_eq!(entry.last_written_by_pass, slot.last_written_by.0);
                entry.access |= access;
                if access.is_write() {
                    //TODO: R::check_usage_is_pass_compatible(entry.usage);
                }
                false
            }
            Err(i) => {
                self.0.insert(
                    i,
                    ResourceDep {
                        index: slot.index,
                        last_written_by_pass: slot.last_written_by.0,
                        access,
                    },
                );
                true
            }
        }
    }
}

impl Deref for ResourceDeps {
    type Target = [ResourceDep];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ResourceDep {
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
        self.access.as_stage()
    }

    #[inline]
    pub fn access(&self) -> Access {
        self.access
    }

    #[inline]
    pub fn is_read(&self) -> bool {
        self.access.is_read()
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        self.access.is_write()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PhysicalResource<R: ResourceAccess> {
    pub resource: R,
    pub format: R::Format,
    pub size: R::Size,
    pub access: Access,
}

impl<R: ResourceAccess> Hash for PhysicalResource<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.format.hash(state);
        self.access.hash(state);
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
    fn create_transient(&mut self, format: R::Format, size: R::Size, access: Access) -> Option<R>;
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
        access: Access,
        first_topo_group: u16,
        last_topo_group: u16,
    ) -> u16 {
        for (j, p) in transients.iter_mut().enumerate() {
            if format == p.physical.format && p.last_topo_group < first_topo_group {
                if let Some(s) = R::merge_size_max(p.physical.size, size) {
                    p.physical.size = s;
                    p.physical.access |= access;
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
                access,
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
                if r.access.is_empty() {
                    panic!(
                        "transient usage is empty, {:?}, {:?}, {}, {}, {:?}, {:?}",
                        r.size,
                        r.format,
                        d.first_topo_group,
                        d.last_topo_group,
                        r.first_written,
                        r.access
                    );
                }
                let transient_index = Self::get_or_create_transient(
                    &mut self.transients,
                    r.format_or_default(),
                    self.assignment_sizes[i].expect("missing size"),
                    r.access,
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
                    trans.physical.access,
                )
                .expect("unable to create transient"); // TODO: error
        }
    }
}

impl PhysicalResourceSet<Texture> {
    fn derive_framebuffer_sizes(&mut self, passes: &[PassDescription]) {
        for pass in passes {
            if pass.active {
                self.derive_framebuffer_size_for_pass(pass);
            }
        }
    }

    fn derive_framebuffer_size_for_pass(&mut self, pass: &PassDescription) {
        let mut pass_size = None;
        let mut empty = true;
        for r in pass.textures().iter() {
            if r.access().is_graphics_attachment() {
                empty = false;
                if let Some(s) = self.assignment_sizes[r.index as usize] {
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

        for r in pass.textures().iter() {
            if r.access().is_graphics_attachment() {
                let s = &mut self.assignment_sizes[r.index as usize];
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

        self.textures.derive_framebuffer_sizes(&graph.passes);

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

    pub fn get_texture(&self, idx: ResourceIndex) -> Option<&PhysicalResource<Texture>> {
        self.textures.get_physical(idx)
    }

    pub fn get_buffer(&self, idx: ResourceIndex) -> Option<&PhysicalResource<Buffer>> {
        self.buffers.get_physical(idx)
    }
}

struct PhysicalResourceMap<R, T> {
    external: Vec<T>,
    transient: Vec<T>,
    _phanton: PhantomData<fn() -> R>,
}

impl<R: ResourceAccess, T: Default + Copy> PhysicalResourceMap<R, T> {
    pub const fn new() -> Self {
        Self {
            external: Vec::new(),
            transient: Vec::new(),
            _phanton: PhantomData,
        }
    }
    fn clear(&mut self) {
        self.external.clear();
        self.transient.clear();
    }
    pub fn reset(&mut self, p: &PhysicalResourceSet<R>) {
        self.clear();
        self.external.resize(p.externals.len(), T::default());
        self.transient.resize(p.externals.len(), T::default());
    }
    pub fn get(&self, p: &PhysicalResourceSet<R>, i: ResourceIndex) -> Option<&T> {
        match p.assignments.get(i as usize)? {
            ExternalOrTransient::None => None,
            ExternalOrTransient::External(i) => self.external.get(*i as usize),
            ExternalOrTransient::Transient(i) => self.transient.get(*i as usize),
        }
    }
    pub fn get_mut(&mut self, p: &PhysicalResourceSet<R>, i: ResourceIndex) -> Option<&mut T> {
        match p.assignments.get(i as usize)? {
            ExternalOrTransient::None => None,
            ExternalOrTransient::External(i) => self.external.get_mut(*i as usize),
            ExternalOrTransient::Transient(i) => self.transient.get_mut(*i as usize),
        }
    }
}

pub struct PhysicalResourceAccessTracker {
    textures: PhysicalResourceMap<Texture, Access>,
    buffers: PhysicalResourceMap<Buffer, Access>,
    total: Access,
}

impl PhysicalResourceAccessTracker {
    pub const fn new() -> Self {
        Self {
            textures: PhysicalResourceMap::new(),
            buffers: PhysicalResourceMap::new(),
            total: Access::empty(),
        }
    }

    pub fn reset(&mut self, r: &PhysicalResources) {
        self.textures.reset(&r.textures);
        self.buffers.reset(&r.buffers);
        self.total = Access::empty();
    }

    pub(crate) fn get_current_texture_access(
        &self,
        p: &PhysicalResources,
        resource_index: ResourceIndex,
    ) -> Access {
        *self
            .textures
            .get(&p.textures, resource_index)
            .expect("no valid physical buffer resource")
    }
    pub(crate) fn get_current_buffer_access(
        &self,
        p: &PhysicalResources,
        resource_index: ResourceIndex,
    ) -> Access {
        *self
            .buffers
            .get(&p.buffers, resource_index)
            .expect("no valid physical buffer resource")
    }

    pub(crate) fn update_texture_access(
        &mut self,
        p: &PhysicalResources,
        resource_index: ResourceIndex,
        new_access: Access,
    ) -> Access {
        let dest = self
            .textures
            .get_mut(&p.textures, resource_index)
            .expect("no valid physical texture resource");
        self.total |= new_access;
        std::mem::replace(dest, new_access)
    }
    pub(crate) fn update_buffer_access(
        &mut self,
        p: &PhysicalResources,
        resource_index: ResourceIndex,
        new_access: Access,
    ) -> Access {
        let dest = self
            .buffers
            .get_mut(&p.buffers, resource_index)
            .expect("no valid physical buffer resource");
        self.total |= new_access;
        std::mem::replace(dest, new_access)
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
    fn export(&self) -> Self {
        *self
    }
}

impl GraphImport<Buffer> for Handle<Buffer> {
    fn import(&self) -> Self {
        *self
    }
}

impl GraphExport<Buffer> for Handle<Buffer> {
    fn export(&self) -> Self {
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
        access: Access,
    ) -> Option<Texture> {
        self.create_transient_texture(format, size, access)
    }
}
impl<B> CreateTransient<Buffer> for B
where
    B: PhysicalResourceResolver,
{
    fn create_transient(&mut self, _format: (), size: usize, access: Access) -> Option<Buffer> {
        self.create_transient_buffer(size, access)
    }
}
