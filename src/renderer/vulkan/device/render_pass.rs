use super::VulkanDevice;
use ash::vk;
use std::error::Error;

pub struct VulkanRenderPass {
    handle: vk::RenderPass,
}

impl VulkanDevice {
    fn get_attachment_descriptions(&self) -> Vec<vk::AttachmentDescription> {
        vec![
            vk::AttachmentDescription {
                format: self
                    .physical_device
                    .surface_properties
                    .surface_format
                    .format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            vk::AttachmentDescription {
                format: self.physical_device.attachment_formats.depth_stencil,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ]
    }

    fn get_subpass_descriptions() -> (Vec<vk::SubpassDescription>, Vec<vk::AttachmentReference>) {
        let attachment_references = vec![
            vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            },
            vk::AttachmentReference {
                attachment: 1,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            },
        ];
        let subpasses = vec![vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: 1,
            p_color_attachments: &attachment_references[0],
            p_depth_stencil_attachment: &attachment_references[1],
            ..Default::default()
        }];
        (subpasses, attachment_references)
    }

    fn get_subpass_dependencies() -> Vec<vk::SubpassDependency> {
        vec![
            vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::empty(),
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                ..Default::default()
            },
            vk::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                dst_stage_mask: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                dst_access_mask: vk::AccessFlags::MEMORY_READ,
                ..Default::default()
            },
        ]
    }

    pub fn create_render_pass(&self) -> Result<VulkanRenderPass, Box<dyn Error>> {
        let attachment_descriptions = self.get_attachment_descriptions();
        let (subpasses, _attachment_references) = Self::get_subpass_descriptions();
        let subpass_dependencies = Self::get_subpass_dependencies();
        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descriptions)
            .subpasses(&subpasses)
            .dependencies(&subpass_dependencies);
        let render_pass = unsafe { self.device.create_render_pass(&create_info, None)? };
        Ok(VulkanRenderPass {
            handle: render_pass,
        })
    }

    pub fn destory_render_pass(&self, render_pass: &mut VulkanRenderPass) {
        unsafe {
            self.device.destroy_render_pass(render_pass.handle, None);
        }
    }
}
