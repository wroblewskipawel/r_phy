use ash::vk;

use crate::renderer::vulkan::device::framebuffer::{
    presets::AttachmentsColorDepthCombined, AttachmentReference, AttachmentReferenceBuilder,
    AttachmentTarget, AttachmentTransition, AttachmentTransitionBuilder, References, Transitions,
};

use super::{RenderPassBuilder, Subpass, SubpassNode, SubpassTerminator, TransitionList};

pub struct ColorDepthCombinedTransitions {}

impl TransitionList<AttachmentsColorDepthCombined> for ColorDepthCombinedTransitions {
    fn transitions() -> Transitions<AttachmentsColorDepthCombined> {
        AttachmentTransitionBuilder::new()
            .push_color(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .push_depth_stencil(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            })
            .push_resolve(AttachmentTransition {
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
            .push_color(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push_depth_stencil(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push_resolve(Some(AttachmentReference {
                target: AttachmentTarget::Resolve,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
    }
}

pub struct ForwardDepthPrepassTransitions {}

impl TransitionList<AttachmentsColorDepthCombined> for ForwardDepthPrepassTransitions {
    fn transitions() -> Transitions<AttachmentsColorDepthCombined> {
        AttachmentTransitionBuilder::new()
            .push_color(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .push_depth_stencil(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            })
            .push_resolve(AttachmentTransition {
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            })
    }
}

pub struct DepthPrepassSubpass {}

impl Subpass<AttachmentsColorDepthCombined> for DepthPrepassSubpass {
    fn references() -> References<AttachmentsColorDepthCombined> {
        AttachmentReferenceBuilder::new()
            .push_color(None)
            .push_depth_stencil(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push_resolve(None)
    }
}

pub struct ColorPassSubpass {}

impl Subpass<AttachmentsColorDepthCombined> for ColorPassSubpass {
    fn references() -> References<AttachmentsColorDepthCombined> {
        AttachmentReferenceBuilder::new()
            .push_color(Some(AttachmentReference {
                target: AttachmentTarget::Color,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
            .push_depth_stencil(Some(AttachmentReference {
                target: AttachmentTarget::DepthStencil,
                layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            }))
            .push_resolve(Some(AttachmentReference {
                target: AttachmentTarget::Resolve,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            }))
    }
}

// pub struct DepthDisplaySubpass {}

// impl Subpass<AttachmentsColorDepthCombined> for ColorPassSubpass {
//     fn references() -> References<AttachmentsColorDepthCombined> {
//         AttachmentReferenceBuilder::new()
//             .push_color(Some(AttachmentReference {
//                 target: AttachmentTarget::Color,
//                 layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
//                 usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
//             }))
//             .push_depth_stencil(Some(AttachmentReference {
//                 target: AttachmentTarget::DepthStencil,
//                 layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
//                 usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
//             }))
//             .push_resolve(Some(AttachmentReference {
//                 target: AttachmentTarget::Resolve,
//                 layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
//                 usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
//             }))
//     }
// }

pub type ColorDepthCombinedRenderPass = RenderPassBuilder<
    AttachmentsColorDepthCombined,
    ColorDepthCombinedTransitions,
    SubpassNode<AttachmentsColorDepthCombined, ColorDepthCombinedSubpass, SubpassTerminator>,
>;

pub type ForwardDepthPrepassRenderPass = RenderPassBuilder<
    AttachmentsColorDepthCombined,
    ColorDepthCombinedTransitions,
    SubpassNode<
        AttachmentsColorDepthCombined,
        ColorPassSubpass,
        SubpassNode<AttachmentsColorDepthCombined, DepthPrepassSubpass, SubpassTerminator>,
    >,
>;
