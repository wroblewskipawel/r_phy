use ash::vk;

use crate::renderer::vulkan::device::AttachmentProperties;

use super::{
    Attachment, AttachmentFormatInfo, AttachmentNode, AttachmentTerminator, ClearColor,
    ClearDeptStencil, ClearNone,
};

pub struct ColorMultisampled {}

impl Attachment for ColorMultisampled {
    type Clear = ClearColor;

    fn get_format(properties: &AttachmentProperties) -> AttachmentFormatInfo {
        AttachmentFormatInfo {
            format: properties.formats.color,
            samples: properties.msaa_samples,
        }
    }
}

pub struct DepthStencilMultisampled {}

impl Attachment for DepthStencilMultisampled {
    type Clear = ClearDeptStencil;

    fn get_format(properties: &AttachmentProperties) -> AttachmentFormatInfo {
        AttachmentFormatInfo {
            format: properties.formats.depth_stencil,
            samples: properties.msaa_samples,
        }
    }
}

pub struct Resolve {}

impl Attachment for Resolve {
    type Clear = ClearNone;

    fn get_format(properties: &AttachmentProperties) -> AttachmentFormatInfo {
        AttachmentFormatInfo {
            format: properties.formats.color,
            samples: vk::SampleCountFlags::TYPE_1,
        }
    }
}

pub type AttachmentsEmpty = AttachmentTerminator;
pub type AttachmentsColorDepthCombined = AttachmentNode<
    ColorMultisampled,
    AttachmentNode<DepthStencilMultisampled, AttachmentNode<Resolve, AttachmentTerminator>>,
>;

pub type AttachmentsDepthPrepass = AttachmentNode<
    ColorMultisampled,
    AttachmentNode<
        ColorMultisampled,
        AttachmentNode<DepthStencilMultisampled, AttachmentNode<Resolve, AttachmentTerminator>>,
    >,
>;

// deferred.rs

pub type AttachmentsGBuffer = AttachmentNode<
    ColorMultisampled, // Combined
    AttachmentNode<
        ColorMultisampled, // Albedo
        AttachmentNode<
            ColorMultisampled, // Normal
            AttachmentNode<
                ColorMultisampled, // Position
                AttachmentNode<
                    DepthStencilMultisampled,
                    AttachmentNode<Resolve, AttachmentTerminator>,
                >,
            >,
        >,
    >,
>;
