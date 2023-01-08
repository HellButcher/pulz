use ash::vk;
use bitflags::bitflags;

bitflags!(
    pub struct AttachmentOps: u8 {
        const LOAD = 1 << 0;
        const STORE = 1 << 1;
    }
);

impl AttachmentOps {
    #[inline]
    pub fn load_op(self) -> vk::AttachmentLoadOp {
        if self.contains(Self::LOAD) {
            vk::AttachmentLoadOp::LOAD
        } else {
            vk::AttachmentLoadOp::CLEAR
        }
    }
    #[inline]
    pub fn store_op(self) -> vk::AttachmentStoreOp {
        if self.contains(Self::STORE) {
            vk::AttachmentStoreOp::STORE
        } else {
            vk::AttachmentStoreOp::DONT_CARE
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColorAttachmentKey {
    pub format: vk::Format,
    pub layout: vk::ImageLayout,
    pub samples: u8,
    pub ops: AttachmentOps,
    pub resolve_ops: AttachmentOps,
}

impl ColorAttachmentKey {
    pub fn attachment_base_desc(&self) -> vk::AttachmentDescription {
        vk::AttachmentDescription::builder()
            .format(self.format)
            .samples(vk::SampleCountFlags::from_raw(self.samples as u32))
            .load_op(self.ops.load_op())
            .store_op(self.ops.store_op())
            .initial_layout(self.layout)
            .final_layout(self.layout)
            .build()
    }
    pub fn attachment_resolve_desc(&self) -> Option<vk::AttachmentDescription> {
        if self.resolve_ops.is_empty() {
            None
        } else {
            Some(
                vk::AttachmentDescription::builder()
                    .format(self.format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(self.resolve_ops.load_op())
                    .store_op(self.resolve_ops.store_op())
                    .initial_layout(self.layout)
                    .final_layout(self.layout)
                    .build(),
            )
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DepthStencilAttachmentKey {
    pub format: vk::Format,
    pub layout: vk::ImageLayout,
    pub samples: u8,
    pub depth_ops: AttachmentOps,
    pub stencil_ops: AttachmentOps,
}

impl DepthStencilAttachmentKey {
    pub fn attachment_desc(&self) -> vk::AttachmentDescription {
        vk::AttachmentDescription::builder()
            .format(self.format)
            .samples(vk::SampleCountFlags::from_raw(self.samples as u32))
            .load_op(self.depth_ops.load_op())
            .store_op(self.depth_ops.store_op())
            .stencil_load_op(self.stencil_ops.load_op())
            .stencil_store_op(self.stencil_ops.store_op())
            .initial_layout(self.layout)
            .final_layout(self.layout)
            .build()
    }
}
