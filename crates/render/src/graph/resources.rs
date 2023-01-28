use std::{
    hash::{self, Hash},
    marker::PhantomData,
    ops::Deref,
};

use pulz_assets::Handle;
use pulz_window::WindowId;

use super::{
    access::{ResourceAccess, Stage},
    builder::{GetExternalResource, GraphExport, GraphImport},
    deps::DependencyMatrix,
    PassIndex, ResourceIndex, SubPassIndex, PASS_UNDEFINED, SUBPASS_UNDEFINED,
};
use crate::{
    camera::RenderTarget,
    texture::{Image, Texture},
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

struct Resource<R> {
    first_written: SubPassIndex,
    last_written: SubPassIndex,
    variant: ResourceVariant,
    extern_res: Option<Box<dyn GetExternalResource<R>>>,
}

#[derive(Hash)]
pub(super) struct ResourceSet<R>(Vec<Resource<R>>);

impl<R> ResourceSet<R> {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<R> Hash for Resource<R> {
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

impl<R> ResourceSet<R> {
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
            variant: ResourceVariant::Transient,
            extern_res: None,
        });
        WriteSlot::new(index, SUBPASS_UNDEFINED)
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

    pub(super) fn mark_pass_dependency_matrix(&self, m: &mut DependencyMatrix, to_pass: PassIndex) {
        for dep in &self.0 {
            let pass_index = dep.src_pass();
            if pass_index != !0 {
                m.insert(pass_index as usize, to_pass as usize);
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

pub struct ResourceAssignments<R: ResourceAccess>(Vec<Option<(R, R::Meta)>>);

impl<R: ResourceAccess + Copy> ResourceAssignments<R> {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn get(&self, resource_index: ResourceIndex) -> Option<(R, &R::Meta)> {
        if let Some(Some((r, m))) = self.0.get(resource_index as usize) {
            Some((*r, m))
        } else {
            None
        }
    }

    pub(super) fn assign_external_resources(&mut self, res: &ResourceSet<R>) {
        assert_eq!(res.0.len(), self.0.len());
        for (i, r) in res.0.iter().enumerate() {
            if let Some(ext_res) = &r.extern_res {
                self.0[i] = Some(ext_res.get_external_resource());
            }
        }
    }
}

impl GraphImport for Handle<Image> {
    type Resource = Texture;

    fn import(&self) -> Box<dyn GetExternalResource<Texture>> {
        todo!("import image handle")
    }
}

impl GraphExport for Handle<Image> {
    type Resource = Texture;

    fn export(&self) -> Box<dyn GetExternalResource<Texture>> {
        todo!("export image handle")
    }
}

impl GraphExport for WindowId {
    type Resource = Texture;

    fn export(&self) -> Box<dyn GetExternalResource<Texture>> {
        todo!("export swapchain image")
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
