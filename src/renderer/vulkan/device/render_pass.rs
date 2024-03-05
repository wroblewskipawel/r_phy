use super::{swapchain::SwapchainFrame, VulkanDevice};
use ash::vk;
use std::error::Error;

pub struct VulkanRenderPass {
    handle: vk::RenderPass,
}

impl Into<vk::RenderPass> for &VulkanRenderPass {
    fn into(self) -> vk::RenderPass {
        self.handle
    }
}

impl VulkanRenderPass {
    fn get_attachment_descriptions(
        color_attachment_format: vk::Format,
        depth_stencil_attachment_format: vk::Format,
    ) -> Vec<vk::AttachmentDescription> {
        vec![
            vk::AttachmentDescription {
                format: color_attachment_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            vk::AttachmentDescription {
                format: depth_stencil_attachment_format,
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

    pub fn get_attachments(
        color_attachment: vk::ImageView,
        depth_stencil_attachment: vk::ImageView,
    ) -> [vk::ImageView; 2] {
        [color_attachment, depth_stencil_attachment]
    }

    pub fn get_color_attachments_blend_state() -> &'static [vk::PipelineColorBlendAttachmentState] {
        const ATTACHMENTS: &[vk::PipelineColorBlendAttachmentState] =
            &[vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::TRUE,
                src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::RGBA,
            }];
        ATTACHMENTS
    }

    pub fn get_attachment_clear_values() -> &'static [vk::ClearValue] {
        const CLEAR_VALUES: &[vk::ClearValue] = &[
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];
        CLEAR_VALUES
    }
}

impl VulkanDevice {
    pub fn create_render_pass(&self) -> Result<VulkanRenderPass, Box<dyn Error>> {
        let attachment_descriptions = VulkanRenderPass::get_attachment_descriptions(
            self.physical_device
                .surface_properties
                .surface_format
                .format,
            self.physical_device.attachment_formats.depth_stencil,
        );
        let (subpasses, _attachment_references) = VulkanRenderPass::get_subpass_descriptions();
        let subpass_dependencies = VulkanRenderPass::get_subpass_dependencies();
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

    pub fn begin_render_pass(&self, frame: &SwapchainFrame, render_pass: &VulkanRenderPass) {
        let clear_values = VulkanRenderPass::get_attachment_clear_values();
        unsafe {
            self.device.cmd_begin_render_pass(
                frame.command_buffer,
                &vk::RenderPassBeginInfo {
                    render_pass: render_pass.handle,
                    framebuffer: frame.framebuffer,
                    render_area: frame.render_area,
                    clear_value_count: clear_values.len() as u32,
                    p_clear_values: clear_values.as_ptr(),
                    ..Default::default()
                },
                vk::SubpassContents::INLINE,
            )
        }
    }

    pub fn end_render_pass(&self, frame: &SwapchainFrame) {
        unsafe {
            self.device.cmd_end_render_pass(frame.command_buffer);
        }
    }
}
