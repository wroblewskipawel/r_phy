use std::{error::Error, marker::PhantomData, mem::size_of, path::Path};

use ash::vk;

use crate::{
    math::types::Vector3,
    renderer::{
        model::Vertex,
        vulkan::device::{
            render_pass::VulkanRenderPass, swapchain::VulkanSwapchain, AttachmentProperties,
            VulkanDevice,
        },
    },
};

use super::{
    layout::{Layout, PipelineLayout},
    ShaderModule,
};

pub struct GraphicsPipeline<L: Layout, I: VertexInput> {
    pub handle: vk::Pipeline,
    pub layout: PipelineLayout<L>,
    _phantom: PhantomData<I>,
}

pub struct VertexInputInfo {
    bindings: Vec<vk::VertexInputBindingDescription>,
    attributes: Vec<vk::VertexInputAttributeDescription>,
    create_info: vk::PipelineVertexInputStateCreateInfo,
}

impl<'a> From<&'a VertexInputInfo> for &'a vk::PipelineVertexInputStateCreateInfo {
    fn from(value: &'a VertexInputInfo) -> Self {
        &value.create_info
    }
}

pub trait VertexInput: 'static {
    fn info() -> VertexInputInfo {
        let bindings = Self::get_binding_descriptions();
        let attributes = Self::get_attribute_descriptions();
        let create_info = vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: bindings.len() as u32,
            p_vertex_binding_descriptions: bindings.as_ptr(),
            vertex_attribute_description_count: attributes.len() as u32,
            p_vertex_attribute_descriptions: attributes.as_ptr(),
            ..Default::default()
        };
        VertexInputInfo {
            bindings,
            attributes,
            create_info,
        }
    }

    fn get_binding_descriptions() -> Vec<vk::VertexInputBindingDescription>;

    fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription>;
}

pub trait VertexBinding: 'static {
    fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription;

    fn get_attribute_descriptions(binding: u32) -> Vec<vk::VertexInputAttributeDescription>;
}

pub trait VertexBindingList: 'static {
    type Item: VertexBinding;
    type Next: VertexBindingList;

    fn exhausted() -> bool;
    fn len() -> usize;
}

pub struct VertexBindingTerminator {}

impl VertexBinding for VertexBindingTerminator {
    fn get_binding_description(_binding: u32) -> vk::VertexInputBindingDescription {
        unreachable!()
    }

    fn get_attribute_descriptions(_binding: u32) -> Vec<vk::VertexInputAttributeDescription> {
        unreachable!()
    }
}

impl VertexBindingList for VertexBindingTerminator {
    type Item = Self;
    type Next = Self;

    fn exhausted() -> bool {
        true
    }

    fn len() -> usize {
        0
    }
}

pub struct VertexBindingNode<B: VertexBinding, N: VertexBindingList> {
    _phantom: PhantomData<(B, N)>,
}

impl<B: VertexBinding, N: VertexBindingList> VertexBindingList for VertexBindingNode<B, N> {
    type Item = B;
    type Next = N;

    fn exhausted() -> bool {
        false
    }

    fn len() -> usize {
        Self::Next::len() + 1
    }
}

pub struct VertexBindingBuilder<L: VertexBindingList> {
    _phantom: PhantomData<L>,
}

impl<L: VertexBindingList> VertexBindingBuilder<L> {
    fn next_binding_description<'a, N: VertexBindingList>(
        binding: u32,
        mut iter: impl Iterator<Item = &'a mut vk::VertexInputBindingDescription>,
    ) {
        if !N::exhausted() {
            if let Some(entry) = iter.next() {
                *entry = N::Item::get_binding_description(binding);
                Self::next_binding_description::<N::Next>(binding + 1, iter);
            }
        }
    }

    pub fn get_binding_descriptions() -> Vec<vk::VertexInputBindingDescription> {
        let mut bindings = vec![vk::VertexInputBindingDescription::default(); L::len()];
        Self::next_binding_description::<L>(0, bindings.iter_mut());
        bindings
    }

    fn next_attribute_descriptions<'a, N: VertexBindingList>(
        binding: u32,
        mut iter: impl Iterator<Item = &'a mut Vec<vk::VertexInputAttributeDescription>>,
    ) {
        if !N::exhausted() {
            if let Some(entry) = iter.next() {
                *entry = N::Item::get_attribute_descriptions(binding);
                Self::next_attribute_descriptions::<N::Next>(binding + 1, iter)
            }
        }
    }

    pub fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription> {
        let mut attributes = vec![vec![]; L::len()];
        Self::next_attribute_descriptions::<L>(0, attributes.iter_mut());
        attributes.into_iter().flatten().collect()
    }
}

impl<L: VertexBindingList> VertexInput for VertexBindingBuilder<L> {
    fn get_binding_descriptions() -> Vec<vk::VertexInputBindingDescription> {
        Self::get_binding_descriptions()
    }

    fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription> {
        Self::get_attribute_descriptions()
    }
}

impl VertexBinding for Vertex {
    fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    fn get_attribute_descriptions(binding: u32) -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            vk::VertexInputAttributeDescription {
                binding,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: size_of::<Vector3>() as u32,
            },
            vk::VertexInputAttributeDescription {
                binding,
                location: 2,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: (size_of::<Vector3>() * 2) as u32,
            },
            vk::VertexInputAttributeDescription {
                binding,
                location: 3,
                format: vk::Format::R32G32_SFLOAT,
                offset: (size_of::<Vector3>() * 3) as u32,
            },
        ]
    }
}

pub type MeshVertexInput = VertexBindingBuilder<VertexBindingNode<Vertex, VertexBindingTerminator>>;

impl<L: Layout, I: VertexInput> GraphicsPipeline<L, I> {
    fn get_input_assembly_state() -> vk::PipelineInputAssemblyStateCreateInfo {
        vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            primitive_restart_enable: vk::FALSE,
            ..Default::default()
        }
    }
    fn get_viewport_state(
        image_extent: vk::Extent2D,
    ) -> (
        vk::PipelineViewportStateCreateInfo,
        Vec<vk::Viewport>,
        Vec<vk::Rect2D>,
    ) {
        let viewports = vec![vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: image_extent.width as f32,
            height: image_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = vec![vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: image_extent,
        }];
        let create_info = vk::PipelineViewportStateCreateInfo {
            viewport_count: viewports.len() as u32,
            p_viewports: viewports.as_ptr(),
            scissor_count: scissors.len() as u32,
            p_scissors: scissors.as_ptr(),
            ..Default::default()
        };
        (create_info, viewports, scissors)
    }
    fn get_rasterization_state(is_skybox: bool) -> vk::PipelineRasterizationStateCreateInfo {
        vk::PipelineRasterizationStateCreateInfo {
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: if !is_skybox {
                vk::CullModeFlags::BACK
            } else {
                vk::CullModeFlags::FRONT
            },
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            ..Default::default()
        }
    }
    fn get_depth_stencil_state(
        depth_test_enabled: bool,
    ) -> vk::PipelineDepthStencilStateCreateInfo {
        if depth_test_enabled {
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: vk::TRUE,
                depth_write_enable: vk::TRUE,
                depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
                ..Default::default()
            }
        } else {
            vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: vk::FALSE,
                depth_write_enable: vk::FALSE,
                ..Default::default()
            }
        }
    }
    fn get_color_blend_state() -> vk::PipelineColorBlendStateCreateInfo {
        let attachments = VulkanRenderPass::get_color_attachments_blend_state();
        vk::PipelineColorBlendStateCreateInfo {
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            ..Default::default()
        }
    }

    fn get_multisample_state(
        enabled_features: &vk::PhysicalDeviceFeatures,
        properties: &AttachmentProperties,
    ) -> vk::PipelineMultisampleStateCreateInfo {
        vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: properties.msaa_samples,
            sample_shading_enable: enabled_features.sample_rate_shading,
            min_sample_shading: 0.2f32,
            ..Default::default()
        }
    }
}

impl VulkanDevice {
    fn load_shader_module(&self, path: &Path) -> Result<ShaderModule, Box<dyn Error>> {
        let code = std::fs::read(path)?;
        let stage = ShaderModule::get_shader_stage(path)?;
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as *const _,
            ..Default::default()
        };
        let module = unsafe { self.device.create_shader_module(&create_info, None)? };
        Ok(ShaderModule { module, stage })
    }

    pub fn create_graphics_pipeline<L: Layout, I: VertexInput>(
        &self,
        layout: L,
        render_pass: &VulkanRenderPass,
        swapchain: &VulkanSwapchain,
        modules: &[&Path],
        is_skybox: bool,
    ) -> Result<GraphicsPipeline<L, I>, Box<dyn Error>> {
        let layout = self.get_pipeline_layout(layout)?;
        let vertex_input_info = I::info();
        let vertex_input_create_info: &vk::PipelineVertexInputStateCreateInfo =
            (&vertex_input_info).into();
        let input_assembly_state = GraphicsPipeline::<L, I>::get_input_assembly_state();
        let (viewport_state, _viewports, _scissors) =
            GraphicsPipeline::<L, I>::get_viewport_state(swapchain.image_extent);
        let rasterization_state = GraphicsPipeline::<L, I>::get_rasterization_state(is_skybox);
        let depth_stencil_state = GraphicsPipeline::<L, I>::get_depth_stencil_state(!is_skybox);
        let color_blend_state = GraphicsPipeline::<L, I>::get_color_blend_state();
        let multisample_state = GraphicsPipeline::<L, I>::get_multisample_state(
            &self.physical_device.properties.enabled_features,
            &self.physical_device.attachment_properties,
        );
        let modules = modules
            .iter()
            .map(|module_path| self.load_shader_module(module_path))
            .collect::<Result<Vec<_>, _>>()?;
        let stages = modules
            .iter()
            .map(|module| module.get_stage_create_info())
            .collect::<Vec<_>>();
        let create_infos = [vk::GraphicsPipelineCreateInfo {
            layout: layout.layout,
            render_pass: render_pass.into(),
            subpass: 0,
            p_vertex_input_state: vertex_input_create_info,
            p_input_assembly_state: &input_assembly_state,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_state,
            p_depth_stencil_state: &depth_stencil_state,
            p_color_blend_state: &color_blend_state,
            p_multisample_state: &multisample_state,
            stage_count: stages.len() as u32,
            p_stages: stages.as_ptr(),
            ..Default::default()
        }];
        let &handle = unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &create_infos, None)
                .map_err(|(_, err)| err)?
                .first()
                .unwrap()
        };
        modules
            .into_iter()
            .for_each(|module| unsafe { self.device.destroy_shader_module(module.module, None) });
        Ok(GraphicsPipeline {
            handle,
            layout,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_graphics_pipeline<L: Layout, I: VertexInput>(
        &self,
        pipeline: &mut GraphicsPipeline<L, I>,
    ) {
        unsafe {
            self.device.destroy_pipeline(pipeline.handle, None);
        }
    }
}
