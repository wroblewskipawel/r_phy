use crate::renderer::vulkan::device::{
    framebuffer::presets::AttachmentsColorDepthCombined,
    pipeline::{
        PipelineLayoutSkybox, PipelineLayoutTextured, PipelineStatesDefault, PipelineStatesSkybox,
    },
    render_pass::{ColorDepthCombinedRenderPass, ColorDepthCombinedSubpass},
};

use super::GraphicsPipelineBuilder;

pub type GraphicsPipelineColorDepthCombinedTextured = GraphicsPipelineBuilder<
    AttachmentsColorDepthCombined,
    PipelineLayoutTextured,
    PipelineStatesDefault,
    ColorDepthCombinedRenderPass,
    ColorDepthCombinedSubpass,
>;

pub type GraphicsPipelineColorDepthCombinedSkybox = GraphicsPipelineBuilder<
    AttachmentsColorDepthCombined,
    PipelineLayoutSkybox,
    PipelineStatesSkybox,
    ColorDepthCombinedRenderPass,
    ColorDepthCombinedSubpass,
>;
