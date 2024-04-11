use crate::renderer::vulkan::device::{
    framebuffer::presets::AttachmentsColorDepthCombined,
    pipeline::{
        PipelineLayoutNoMaterial, PipelineLayoutSkybox, PipelineLayoutTextured, PipelineStatesDefault, PipelineStatesDepthWriteDisabled, PipelineStatesSkybox
    },
    render_pass::{ColorDepthCombinedRenderPass, ColorDepthCombinedSubpass, ColorPassSubpass, DepthPrepassSubpass, ForwardDepthPrepassRenderPass},
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
    ForwardDepthPrepassRenderPass,
    ColorPassSubpass,
>;

pub type GraphicsPipelineForwardDepthPrepass = GraphicsPipelineBuilder<
    AttachmentsColorDepthCombined,
    PipelineLayoutNoMaterial,
    PipelineStatesDefault,
    ForwardDepthPrepassRenderPass,
    DepthPrepassSubpass,
>;

pub type GraphicsPipelineColorPass = GraphicsPipelineBuilder<
    AttachmentsColorDepthCombined,
    PipelineLayoutTextured,
    PipelineStatesDepthWriteDisabled,
    ForwardDepthPrepassRenderPass,
    ColorPassSubpass,
>;
