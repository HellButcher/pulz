use std::{cell::Cell, collections::VecDeque, sync::Arc};

use ash::{prelude::VkResult, vk};
use pulz_render::backend::CommandEncoder;

use crate::{device::AshDevice, Result};

pub struct AshCommandPool {
    device: Arc<AshDevice>,
    queue_family_index: u32,
    pool: vk::CommandPool,
    fresh_buffers: VecDeque<vk::CommandBuffer>,
    done_buffers: Vec<vk::CommandBuffer>,
    new_allocation_count: u32,
    semaphores_pool: VecDeque<vk::Semaphore>,
    used_semaphores: Vec<vk::Semaphore>, // semaphores to return to pool after frame finished
}

impl AshDevice {
    pub(crate) fn new_command_pool(
        self: &Arc<Self>,
        queue_family_index: u32,
    ) -> VkResult<AshCommandPool> {
        let pool = unsafe {
            self.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )?
        };
        Ok(AshCommandPool {
            device: self.clone(),
            queue_family_index,
            pool,
            fresh_buffers: VecDeque::new(),
            done_buffers: Vec::new(),
            new_allocation_count: 1,
            semaphores_pool: VecDeque::new(),
            used_semaphores: Vec::new(),
        })
    }
}

impl AshCommandPool {
    #[inline]
    pub fn device(&self) -> &AshDevice {
        &self.device
    }

    pub unsafe fn reset(&mut self) -> VkResult<()> {
        self.device
            .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())?;
        self.fresh_buffers.extend(self.done_buffers.drain(..));

        // return all semaphores to pool
        self.semaphores_pool.extend(self.used_semaphores.drain(..));

        Ok(())
    }

    pub fn request_semaphore(&mut self) -> Result<vk::Semaphore> {
        // TODO: drop guard
        let s = if let Some(s) = self.semaphores_pool.pop_front() {
            s
        } else {
            unsafe {
                self.device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?
            }
        };
        self.used_semaphores.push(s);
        Ok(s)
    }

    pub fn encoder(&mut self) -> Result<AshCommandEncoder<'_>, vk::Result> {
        if self.fresh_buffers.is_empty() {
            let new_buffers = unsafe {
                self.device.allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(self.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(self.new_allocation_count),
                )?
            };
            self.fresh_buffers.extend(new_buffers);
        }
        let buffer = self.fresh_buffers.pop_front().unwrap();
        unsafe {
            self.device.begin_command_buffer(
                buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        Ok(AshCommandEncoder {
            pool: self,
            buffer,
            debug_levels: Cell::new(0),
        })
    }

    unsafe fn free_command_buffers(&self, buffers: &[vk::CommandBuffer]) {
        if !buffers.is_empty() {
            self.device.free_command_buffers(self.pool, buffers);
        }
    }
}

impl Drop for AshCommandPool {
    fn drop(&mut self) {
        self.semaphores_pool.extend(self.used_semaphores.drain(..));
        for semaphore in self.semaphores_pool.drain(..) {
            unsafe {
                self.device.destroy_semaphore(semaphore, None);
            }
        }

        unsafe {
            let (a, b) = self.fresh_buffers.as_slices();
            self.free_command_buffers(a);
            self.free_command_buffers(b);
            self.fresh_buffers.clear();
            self.free_command_buffers(&self.done_buffers);
            self.done_buffers.clear();
        }
        if self.pool != vk::CommandPool::null() {
            unsafe {
                self.device.destroy_command_pool(self.pool, None);
            }
            self.pool = vk::CommandPool::null();
        }
    }
}
pub struct AshCommandEncoder<'l> {
    pool: &'l mut AshCommandPool,
    buffer: vk::CommandBuffer,
    debug_levels: Cell<usize>,
}

impl AshCommandEncoder<'_> {
    pub fn submit(self, submission: &SubmissionGroup) -> VkResult<()> {
        self.end_remaining_debug_labels();
        unsafe {
            self.pool.device.end_command_buffer(self.buffer)?;
        }
        submission.queue.push(self.buffer);
        Ok(())
    }

    #[inline]
    pub fn request_semaphore(&mut self) -> Result<vk::Semaphore> {
        self.pool.request_semaphore()
    }

    pub fn insert_debug_label(&self, label: &str) {
        if let Ok(debug_utils) = self.pool.device.debug_utils() {
            unsafe {
                debug_utils.cmd_begin_debug_label(self.buffer, label);
            }
        }
    }

    pub fn begin_debug_label(&self, label: &str) {
        if let Ok(debug_utils) = self.pool.device.debug_utils() {
            unsafe {
                debug_utils.cmd_begin_debug_label(self.buffer, label);
            }
            let debug_levels = self.debug_levels.get();
            self.debug_levels.set(debug_levels + 1);
        }
    }

    pub fn end_debug_label(&self) {
        if let Ok(debug_utils) = self.pool.device.debug_utils() {
            unsafe {
                debug_utils.cmd_end_debug_label(self.buffer);
            }
            let debug_levels = self.debug_levels.get();
            if debug_levels > 0 {
                self.debug_levels.set(debug_levels - 1);
            }
        }
    }

    fn end_remaining_debug_labels(&self) {
        let debug_levels = self.debug_levels.get();
        if debug_levels > 0 {
            self.debug_levels.set(0);
            if let Ok(debug_utils) = self.pool.device.debug_utils() {
                for _i in 0..debug_levels {
                    unsafe {
                        debug_utils.cmd_end_debug_label(self.buffer);
                    }
                }
            }
        }
    }

    pub unsafe fn clear_color_image(
        &self,
        image: vk::Image,
        image_layout: vk::ImageLayout,
        clear_value: &vk::ClearColorValue,
        ranges: &[vk::ImageSubresourceRange],
    ) {
        self.pool.device().cmd_clear_color_image(
            self.buffer,
            image,
            image_layout,
            clear_value,
            ranges,
        )
    }

    pub unsafe fn clear_depth_stencil_image(
        &self,
        image: vk::Image,
        image_layout: vk::ImageLayout,
        clear_value: &vk::ClearDepthStencilValue,
        ranges: &[vk::ImageSubresourceRange],
    ) {
        self.pool.device().cmd_clear_depth_stencil_image(
            self.buffer,
            image,
            image_layout,
            clear_value,
            ranges,
        )
    }

    pub unsafe fn pipeline_barrier(
        &self,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
        memory_barriers: &[vk::MemoryBarrier<'_>],
        buffer_memory_barriers: &[vk::BufferMemoryBarrier<'_>],
        image_memory_barriers: &[vk::ImageMemoryBarrier<'_>],
    ) {
        self.pool.device().cmd_pipeline_barrier(
            self.buffer,
            src_stage_mask,
            dst_stage_mask,
            vk::DependencyFlags::empty(),
            memory_barriers,
            buffer_memory_barriers,
            image_memory_barriers,
        )
    }

    pub unsafe fn begin_render_pass(
        &self,
        create_info: &vk::RenderPassBeginInfo<'_>,
        contents: vk::SubpassContents,
    ) {
        self.pool
            .device()
            .cmd_begin_render_pass(self.buffer, create_info, contents);
    }

    pub unsafe fn next_subpass(&self, contents: vk::SubpassContents) {
        self.pool.device().cmd_next_subpass(self.buffer, contents);
    }

    pub unsafe fn end_render_pass(&self) {
        self.pool.device().cmd_end_render_pass(self.buffer);
    }
}

impl CommandEncoder for AshCommandEncoder<'_> {
    #[inline]
    fn insert_debug_marker(&mut self, label: &str) {
        self.insert_debug_label(label);
    }
    #[inline]
    fn push_debug_group(&mut self, label: &str) {
        self.begin_debug_label(label)
    }
    #[inline]
    fn pop_debug_group(&mut self) {
        self.end_debug_label();
    }
}

impl Drop for AshCommandEncoder<'_> {
    fn drop(&mut self) {
        if self.buffer != vk::CommandBuffer::null() {
            self.pool.done_buffers.push(self.buffer);
            self.buffer = vk::CommandBuffer::null();
        }
    }
}

pub struct SubmissionGroup {
    wait_semaphores: Vec<vk::Semaphore>,
    wait_semaphores_dst_stages: Vec<vk::PipelineStageFlags>,
    command_buffers: Vec<vk::CommandBuffer>,
    signal_semaphores: Vec<vk::Semaphore>,
    queue: crossbeam_queue::SegQueue<vk::CommandBuffer>,
}

impl SubmissionGroup {
    #[inline]
    pub fn new() -> Self {
        Self {
            wait_semaphores: Vec::new(),
            wait_semaphores_dst_stages: Vec::new(),
            command_buffers: Vec::new(),
            signal_semaphores: Vec::new(),
            queue: crossbeam_queue::SegQueue::new(),
        }
    }

    #[inline]
    pub fn wait(&mut self, sem: vk::Semaphore, dst_stages: vk::PipelineStageFlags) -> &mut Self {
        self.wait_semaphores.push(sem);
        self.wait_semaphores_dst_stages.push(dst_stages);
        self
    }

    #[inline]
    pub(crate) fn get_wait_semaphores(&self) -> &[vk::Semaphore] {
        &self.wait_semaphores
    }

    #[inline]
    pub fn push(&mut self, buf: vk::CommandBuffer) -> &mut Self {
        self.command_buffers.push(buf);
        self
    }

    #[inline]
    pub fn flush_queue(&mut self) -> &mut Self {
        while let Some(buf) = self.queue.pop() {
            self.command_buffers.push(buf);
        }
        self
    }

    #[inline]
    pub fn signal(&mut self, sem: vk::Semaphore) -> &mut Self {
        self.signal_semaphores.push(sem);
        self
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.command_buffers.is_empty()
    }

    pub fn submit_info(&self) -> vk::SubmitInfo<'_> {
        vk::SubmitInfo::default()
            .wait_semaphores(&self.wait_semaphores)
            .wait_dst_stage_mask(&self.wait_semaphores_dst_stages)
            .command_buffers(&self.command_buffers)
            .signal_semaphores(&self.signal_semaphores)
    }

    pub fn submit(&mut self, device: &AshDevice, fence: vk::Fence) -> VkResult<&mut Self> {
        unsafe {
            device.queue_submit(device.queues().graphics, &[self.submit_info()], fence)?;
        }
        self.reset();
        Ok(self)
    }

    #[inline]
    pub fn reset(&mut self) -> &mut Self {
        self.wait_semaphores.clear();
        self.wait_semaphores_dst_stages.clear();
        self.command_buffers.clear();
        self.signal_semaphores.clear();
        self
    }
}
