use ash::vk;

use crate::renderer::vulkan::device::framebuffer::{
    presets::{AttachmentsColorDepthCombined, AttachmentsDepthPrepass, AttachmentsGBuffer},
    AttachmentReference, AttachmentReferenceBuilder, AttachmentTarget, AttachmentTransition,
    AttachmentTransitionBuilder, References, Transitions,
};

use super::{RenderPassBuilder, Subpass, SubpassNode, SubpassTerminator, TransitionList};

pub struct ColorDepthCombinedTransitions {}

impl TransitionList<AttachmentsColorDepthCombined> for ColorDepthCombinedTransitions {
    fn transitions() -> Transitions<AttachmentsColorDepthCombined> {
        AttachmentTransitionBuilder::new()
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            })
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            })
    }
}

pub struct ColorDepthCombinedSubpass {}

impl Subpass<AttachmentsColorDepthCombined> for ColorDepthCombinedSubpass {
    fn references() -> References<AttachmentsColorDepthCombined> {
        AttachmentReferenceBuilder::new()
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Resolve,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
    }
}

pub struct ForwardDepthPrepassTransitions {}

impl TransitionList<AttachmentsDepthPrepass> for ForwardDepthPrepassTransitions {
    fn transitions() -> Transitions<AttachmentsDepthPrepass> {
        AttachmentTransitionBuilder::new()
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            })
    }
}

pub struct DepthPrepassSubpass {}

impl Subpass<AttachmentsDepthPrepass> for DepthPrepassSubpass {
    fn references() -> References<AttachmentsDepthPrepass> {
        AttachmentReferenceBuilder::new()
            .push(None)
            .push(None)
            .push(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push(None)
    }
}

pub struct ColorPassSubpass {}

impl Subpass<AttachmentsDepthPrepass> for ColorPassSubpass {
    fn references() -> References<AttachmentsDepthPrepass> {
        AttachmentReferenceBuilder::new()
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(None)
            .push(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push(None)
    }
}

pub struct DepthDisplaySubpass {}

impl Subpass<AttachmentsDepthPrepass> for DepthDisplaySubpass {
    fn references() -> References<AttachmentsDepthPrepass> {
        AttachmentReferenceBuilder::new()
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Resolve,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
    }
}

pub type ColorDepthCombinedRenderPass = RenderPassBuilder<
    AttachmentsDepthPrepass,
    ColorDepthCombinedTransitions,
    SubpassNode<AttachmentsDepthPrepass, ColorDepthCombinedSubpass, SubpassTerminator>,
>;

pub type ForwardDepthPrepassRenderPass = RenderPassBuilder<
    AttachmentsDepthPrepass,
    ForwardDepthPrepassTransitions,
    SubpassNode<
        AttachmentsDepthPrepass,
        DepthDisplaySubpass,
        SubpassNode<
            AttachmentsDepthPrepass,
            ColorPassSubpass,
            SubpassNode<AttachmentsDepthPrepass, DepthPrepassSubpass, SubpassTerminator>,
        >,
    >,
>;

// defered.rs

pub struct DeferedRenderPassTransitions {}

impl TransitionList<AttachmentsGBuffer> for DeferedRenderPassTransitions {
    fn transitions() -> Transitions<AttachmentsGBuffer> {
        AttachmentTransitionBuilder::new()
            .push(AttachmentTransition {
                // Combined
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .push(AttachmentTransition {
                // Albedo
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                // Normal
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                // Position
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                // Depth
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            })
            .push(AttachmentTransition {
                // Resolve
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            })
    }
}

pub struct GBufferDepthPrepas {}

impl Subpass<AttachmentsGBuffer> for GBufferDepthPrepas {
    fn references() -> References<AttachmentsGBuffer> {
        AttachmentReferenceBuilder::new()
            .push(None)
            .push(None)
            .push(None)
            .push(None)
            .push(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push(None)
    }
}

pub struct GBufferWritePass {}

impl Subpass<AttachmentsGBuffer> for GBufferWritePass {
    fn references() -> References<AttachmentsGBuffer> {
        AttachmentReferenceBuilder::new()
            .push(None)
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push(None)
    }
}

pub struct GBufferShadingPass {}

impl Subpass<AttachmentsGBuffer> for GBufferShadingPass {
    fn references() -> References<AttachmentsGBuffer> {
        AttachmentReferenceBuilder::new()
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Input,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::INPUT_ATTACHMENT,
            }))
            .push(Some(AttachmentReference {
                target: AttachmentTarget::Resolve,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
    }
}

pub type DeferedRenderPass = RenderPassBuilder<
    AttachmentsGBuffer,
    DeferedRenderPassTransitions,
    SubpassNode<
        AttachmentsGBuffer,
        GBufferShadingPass,
        SubpassNode<
            AttachmentsGBuffer,
            GBufferWritePass,
            SubpassNode<AttachmentsGBuffer, GBufferDepthPrepas, SubpassTerminator>,
        >,
    >,
>;
