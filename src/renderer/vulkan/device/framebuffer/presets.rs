use ash::vk;

use crate::renderer::vulkan::device::AttachmentProperties;

use super::{
    Attachment, AttachmentFormatInfo, AttachmentNode, AttachmentTerminator, Attachments,
    ClearColor, ClearDeptStencil, ClearNone,
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

pub struct AttachmentsEmpty {}

impl Attachments for AttachmentsEmpty {
    type Color = AttachmentTerminator;
    type DepthStencil = AttachmentTerminator;
    type Resolve = AttachmentTerminator;
}

pub struct AttachmentsColorDepthCombined {}

impl Attachments for AttachmentsColorDepthCombined {
    type Color = AttachmentNode<ColorMultisampled, AttachmentTerminator>;
    type DepthStencil = AttachmentNode<DepthStencilMultisampled, AttachmentTerminator>;
    type Resolve = AttachmentNode<Resolve, AttachmentTerminator>;
}
