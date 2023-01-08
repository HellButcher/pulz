use std::{
    hash::{self, Hash},
    marker::PhantomData,
    ops::Deref,
};

use pulz_assets::Handle;
use pulz_window::WindowId;

use super::{
    access::{ResourceAccess, Stage},
    builder::{GraphExport, GraphImport},
    deps::DependencyMatrix,
    PassDescription,
};
pub use crate::{
    buffer::{Buffer, BufferUsage},
    texture::{Texture, TextureUsage},
};
use crate::{camera::RenderTarget, texture::Image};

#[derive(Copy, Clone)]
pub struct Slot<R> {
    pub(crate) index: u16,
    pub(crate) last_written_by_pass: u16,
    _phantom: PhantomData<fn() -> R>,
}

// Not Copy by intention!
pub struct WriteSlot<R>(Slot<R>);

pub trait SlotAccess {
    const WRITE: bool;
    fn index(&self) -> u16;
}

impl<R> SlotAccess for Slot<R> {
    const WRITE: bool = false;
    #[inline]
    fn index(&self) -> u16 {
        self.index
    }
}

impl<R> SlotAccess for WriteSlot<R> {
    const WRITE: bool = true;
    #[inline]
    fn index(&self) -> u16 {
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
    const fn new(index: usize, last_written_by_pass: u16) -> Self {
        Self {
            index: index as u16,
            last_written_by_pass,
            _phantom: PhantomData,
        }
    }
}

impl<R> WriteSlot<R> {
    #[inline]
    const fn new(index: usize, last_writing_pass: u16) -> Self {
        Self(Slot::new(index, last_writing_pass))
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

pub(super) struct ResourceSet<R> {
    last_written: Vec<u16>,
    first_written: Vec<u16>,
    variant: Vec<ResourceVariant>,
    _phantom: PhantomData<fn() -> R>,
}

impl<R> ResourceSet<R> {
    pub fn len(&self) -> usize {
        self.last_written.len()
    }
}

impl<R> Hash for ResourceSet<R> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash_slice(&self.last_written, state);
        Hash::hash_slice(&self.first_written, state);
        Hash::hash_slice(&self.variant, state);
    }
}

pub struct ResourceDeps<R: ResourceAccess> {
    deps: Vec<ResourceDep<R::Usage>>,
}

impl<R: ResourceAccess> Hash for ResourceDeps<R> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash_slice(&self.deps, state);
    }
}

#[derive(Hash)]
pub struct ResourceDep<U> {
    index: u16,
    last_written_by_pass: u16,
    write_access: bool,
    stages: Stage,
    usage: U,
}

impl<R> ResourceSet<R> {
    #[inline]
    pub(super) const fn new() -> Self {
        Self {
            last_written: Vec::new(),
            first_written: Vec::new(),
            variant: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub(super) fn reset(&mut self) {
        self.last_written.clear();
        self.first_written.clear();
        self.variant.clear();
    }

    pub(super) fn create(&mut self) -> WriteSlot<R> {
        let index = self.last_written.len();
        self.last_written.push(!0);
        self.first_written.push(!0);
        self.variant.push(ResourceVariant::Transient);
        WriteSlot::new(index, !0)
    }

    pub(super) fn writes(&mut self, slot: WriteSlot<R>, new_pass: usize) -> WriteSlot<R> {
        let new_pass = new_pass as u16;
        let index = slot.0.index as usize;
        let last_written_by_pass = self.last_written[index];
        assert_eq!(
            last_written_by_pass, slot.0.last_written_by_pass,
            "resource also written by an other pass (slot out of sync)"
        );
        if new_pass != last_written_by_pass {
            self.last_written[index] = new_pass;
            if self.first_written[index] == !0 {
                self.first_written[index] = new_pass
            }
        }
        WriteSlot::new(index, new_pass)
    }

    pub(super) fn reads(&mut self, slot: Slot<R>) {
        assert_ne!(
            slot.last_written_by_pass, !0,
            "resource was not yet written!"
        );
        let index = slot.index as usize;
        let last_written_by_pass = self.last_written[index];
        // TODO: allow usage of older slots for reading
        assert_eq!(
            last_written_by_pass, slot.last_written_by_pass,
            "resource also written by an other pass (slot out of sync), TODO!"
        );
    }

    pub(super) fn import(&mut self) -> Slot<R> {
        let s = self.create();
        let index = s.index as usize;
        self.variant[index] = ResourceVariant::Import;
        s.read()
    }

    pub(super) fn export(&mut self, slot: Slot<R>) {
        let index = slot.index as usize;
        assert_eq!(ResourceVariant::Transient, self.variant[index]);
        self.variant[index] = ResourceVariant::Export;
    }
}

impl<R: ResourceAccess> ResourceDeps<R> {
    #[inline]
    pub fn deps(&self) -> &[ResourceDep<R::Usage>] {
        &self.deps
    }

    pub fn find_by_resource_index(&self, resource_index: usize) -> Option<&ResourceDep<R::Usage>> {
        if let Ok(i) = self
            .deps
            .binary_search_by_key(&resource_index, |d| d.index as usize)
        {
            Some(&self.deps[i])
        } else {
            None
        }
    }

    #[inline]
    pub(super) const fn new() -> Self {
        Self { deps: Vec::new() }
    }

    pub(super) fn mark_pass_dependency_matrix(&self, m: &mut DependencyMatrix, to_pass: usize) {
        for dep in &self.deps {
            let pass_index = dep.src_pass();
            if pass_index != !0 {
                m.insert(pass_index, to_pass);
            }
        }
    }

    pub(super) fn mark_group_dependency_matrix(
        &self,
        m: &mut DependencyMatrix,
        passes: &[PassDescription],
        to_group: usize,
    ) {
        for dep in &self.deps {
            let pass_index = dep.src_pass();
            if pass_index != !0 {
                m.insert(passes[pass_index].group_index, to_group);
            }
        }
    }

    pub(super) fn access(
        &mut self,
        slot: &Slot<R>,
        write_access: bool,
        stages: Stage,
        usage: R::Usage,
    ) {
        match self.deps.binary_search_by_key(&slot.index, |e| e.index) {
            Ok(i) => {
                let entry = &mut self.deps[i];
                assert_eq!(entry.last_written_by_pass, slot.last_written_by_pass);
                entry.write_access |= write_access;
                entry.stages |= stages;
                entry.usage |= usage;
            }
            Err(i) => {
                self.deps.insert(
                    i,
                    ResourceDep {
                        index: slot.index,
                        last_written_by_pass: slot.last_written_by_pass,
                        write_access,
                        stages,
                        usage,
                    },
                );
            }
        }
    }
}

impl<U: Copy> ResourceDep<U> {
    #[inline]
    pub fn resource_index(&self) -> usize {
        self.index as usize
    }

    #[inline]
    pub fn src_pass(&self) -> usize {
        if self.last_written_by_pass == !0 {
            !0
        } else {
            self.last_written_by_pass as usize
        }
    }

    #[inline]
    pub fn stages(&self) -> Stage {
        self.stages
    }

    #[inline]
    pub fn usage(&self) -> U {
        self.usage
    }

    #[inline]
    pub fn write_access(&self) -> bool {
        self.write_access
    }
}

impl GraphImport for Handle<Image> {
    type Resource = Texture;
}

impl GraphExport for Handle<Image> {
    type Resource = Texture;
}

impl GraphExport for WindowId {
    type Resource = Texture;
}

impl GraphExport for RenderTarget {
    type Resource = Texture;
}
