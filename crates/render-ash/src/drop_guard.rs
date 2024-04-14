use std::{collections::VecDeque, mem::ManuallyDrop};

use ash::vk;

use crate::{instance::AshInstance, AshDevice, Result};

pub trait Destroy {
    type Context;
    unsafe fn destroy(self, context: &Self::Context);
}

pub trait CreateWithInfo: Destroy + Sized {
    type CreateInfo<'a>;
    unsafe fn create(context: &Self::Context, create_info: &Self::CreateInfo<'_>) -> Result<Self>;
}

impl AshDevice {
    #[inline]
    pub unsafe fn create<C>(&self, create_info: &C::CreateInfo<'_>) -> Result<Guard<'_, C>>
    where
        C: CreateWithInfo<Context = Self>,
    {
        Ok(Guard::new(self, unsafe { C::create(self, create_info)? }))
    }

    #[inline]
    pub unsafe fn destroy<D>(&self, handle: D)
    where
        D: Destroy<Context = Self>,
    {
        handle.destroy(self)
    }

    #[inline]
    pub(crate) fn hold<D>(&self, item: D) -> Guard<'_, D>
    where
        D: Destroy<Context = Self>,
    {
        Guard::new(self, item)
    }
}

impl AshInstance {
    #[inline]
    pub unsafe fn create<C>(&self, create_info: &C::CreateInfo<'_>) -> Result<Guard<'_, C>>
    where
        C: CreateWithInfo<Context = Self>,
    {
        Ok(Guard::new(self, unsafe { C::create(self, create_info)? }))
    }

    #[inline]
    pub unsafe fn destroy<D>(&self, handle: D)
    where
        D: Destroy<Context = Self>,
    {
        handle.destroy(self)
    }

    #[inline]
    pub(crate) fn hold<D>(&self, item: D) -> Guard<'_, D>
    where
        D: Destroy<Context = Self>,
    {
        Guard::new(self, item)
    }
}

pub struct Guard<'a, D: Destroy> {
    item: ManuallyDrop<D>,
    context: &'a D::Context,
}

impl<'a, D: Destroy> Guard<'a, D> {
    #[inline]
    pub fn new(context: &'a D::Context, item: D) -> Self {
        Self {
            item: ManuallyDrop::new(item),
            context,
        }
    }

    #[inline]
    pub fn take(mut self) -> D {
        let item = unsafe { ManuallyDrop::take(&mut self.item) };
        std::mem::forget(self);
        item
    }

    #[inline]
    pub fn as_ref(&self) -> &D {
        &self.item
    }

    #[inline]
    pub fn as_mut(&mut self) -> &mut D {
        &mut self.item
    }
}

impl<'a, I, D: Destroy + std::ops::Index<I>> std::ops::Index<I> for Guard<'a, D> {
    type Output = D::Output;
    #[inline]
    fn index(&self, i: I) -> &D::Output {
        &self.item[i]
    }
}

impl<'a, D: Destroy + Copy> Guard<'a, D> {
    #[inline]
    pub fn raw(&self) -> D {
        *self.item
    }
}

impl<D: Destroy> Drop for Guard<'_, D> {
    fn drop(&mut self) {
        unsafe {
            let item = ManuallyDrop::take(&mut self.item);
            item.destroy(self.context);
        }
    }
}

impl<D: Destroy> Destroy for Vec<D> {
    type Context = D::Context;
    #[inline]
    unsafe fn destroy(self, device: &D::Context) {
        self.into_iter().for_each(|d| d.destroy(device))
    }
}

impl<D: Destroy> Destroy for VecDeque<D> {
    type Context = D::Context;
    #[inline]
    unsafe fn destroy(self, device: &D::Context) {
        self.into_iter().for_each(|d| d.destroy(device))
    }
}

impl<D: Destroy> Destroy for std::vec::Drain<'_, D> {
    type Context = D::Context;
    #[inline]
    unsafe fn destroy(self, device: &D::Context) {
        self.for_each(|d| d.destroy(device))
    }
}

impl<D: Destroy> Destroy for std::collections::vec_deque::Drain<'_, D> {
    type Context = D::Context;
    #[inline]
    unsafe fn destroy(self, device: &D::Context) {
        self.for_each(|d| d.destroy(device))
    }
}

macro_rules! impl_create_destroy {
    ($ctx:ty {
        <$lt:tt>
        $(
            $vktype:ty : ($destroy:ident $(, $create:ident $createinfo:ty)?)
        ),* $(,)?
    }) => {
        $(

            impl Destroy for $vktype {
                type Context = $ctx;
                #[inline]
                unsafe fn destroy(self, ctx: &Self::Context) {
                    ctx.$destroy(self, None);
                }
            }

            $(
                impl CreateWithInfo for $vktype {
                    type CreateInfo<$lt> = $createinfo;
                    #[inline]
                    unsafe fn create(ctx: &Self::Context, create_info: &Self::CreateInfo<'_>) -> Result<Self> {
                        Ok(ctx.$create(create_info, None)?)
                    }
                }
            )?
        )*
    };
}

impl_create_destroy! {
    AshDevice { <'a>
        vk::Fence : (destroy_fence, create_fence vk::FenceCreateInfo<'a>),
        vk::Semaphore : (destroy_semaphore, create_semaphore vk::SemaphoreCreateInfo<'a>),
        vk::Event : (destroy_event, create_event vk::EventCreateInfo<'a>),
        vk::CommandPool : (destroy_command_pool, create_command_pool vk::CommandPoolCreateInfo<'a>),
        vk::Buffer : (destroy_buffer, create_buffer vk::BufferCreateInfo<'a>),
        vk::Image : (destroy_image, create_image vk::ImageCreateInfo<'a>),
        vk::ImageView : (destroy_image_view, create_image_view vk::ImageViewCreateInfo<'a>),
        vk::Framebuffer : (destroy_framebuffer, create_framebuffer vk::FramebufferCreateInfo<'a>),
        vk::RenderPass : (destroy_render_pass, create_render_pass vk::RenderPassCreateInfo<'a>),
        vk::ShaderModule : (destroy_shader_module, create_shader_module vk::ShaderModuleCreateInfo<'a>),
        vk::DescriptorSetLayout : (destroy_descriptor_set_layout, create_descriptor_set_layout vk::DescriptorSetLayoutCreateInfo<'a>),
        vk::PipelineLayout : (destroy_pipeline_layout, create_pipeline_layout vk::PipelineLayoutCreateInfo<'a>),
        vk::Pipeline : (destroy_pipeline),
    }
}
