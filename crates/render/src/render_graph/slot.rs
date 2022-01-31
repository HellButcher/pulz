use std::borrow::Cow;

use crate::render_resource::{BufferId, TextureId};
use ecs::Entity;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotAccess {
    Input,
    Output,
    Both,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotType {
    Buffer,
    Texture,
    Entity,
}

#[derive(Debug, Copy, Clone)]
pub enum SlotBinding {
    Buffer(BufferId),
    Texture(TextureId),
    Entity(Entity),
}

#[derive(Clone, Debug)]
pub struct SlotDescriptor {
    pub name: Cow<'static, str>,
    pub access: SlotAccess,
    pub slot_type: SlotType,
    pub optional: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum SlotLabel<'a> {
    Index(u32),
    Name(&'a str),
}

impl SlotBinding {
    pub const fn slot_type(&self) -> SlotType {
        match self {
            Self::Buffer(_) => SlotType::Buffer,
            Self::Texture(_) => SlotType::Texture,
            Self::Entity(_) => SlotType::Entity,
        }
    }
}

impl From<SlotBinding> for SlotType {
    #[inline]
    fn from(value: SlotBinding) -> Self {
        value.slot_type()
    }
}

impl From<BufferId> for SlotBinding {
    fn from(value: BufferId) -> Self {
        Self::Buffer(value)
    }
}

impl From<TextureId> for SlotBinding {
    fn from(value: TextureId) -> Self {
        Self::Texture(value)
    }
}

impl From<Entity> for SlotBinding {
    fn from(value: Entity) -> Self {
        Self::Entity(value)
    }
}

impl SlotDescriptor {
    #[inline]
    pub fn input(name: impl Into<Cow<'static, str>>, slot_type: SlotType) -> Self {
        Self {
            name: name.into(),
            access: SlotAccess::Input,
            slot_type,
            optional: false,
        }
    }

    #[inline]
    pub fn output(name: impl Into<Cow<'static, str>>, slot_type: SlotType) -> Self {
        Self {
            name: name.into(),
            access: SlotAccess::Output,
            slot_type,
            optional: true,
        }
    }
}

impl From<u32> for SlotLabel<'_> {
    #[inline]
    fn from(value: u32) -> Self {
        Self::Index(value)
    }
}

impl<'a> From<&'a str> for SlotLabel<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Name(value)
    }
}
