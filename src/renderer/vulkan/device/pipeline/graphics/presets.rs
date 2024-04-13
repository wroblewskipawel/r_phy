use crate::renderer::vulkan::device::{
    framebuffer::presets::{AttachmentsColorDepthCombined, AttachmentsDepthPrepass, AttachmentsGBuffer},
    pipeline::{
        DeferedStatesDepthDisabled, DeferedStatesDepthEnabled, PipelineLayoutGBuffer, PipelineLayoutNoMaterial, PipelineLayoutSkybox, PipelineLayoutTextured, PipelineLayoutTwoInputAttachments, PipelineStatesDefault, PipelineStatesDepthDisplay, PipelineStatesDepthWriteDisabled, PipelineStatesSkybox
    },
    render_pass::{
        ColorDepthCombinedRenderPass, ColorDepthCombinedSubpass, ColorPassSubpass, DeferedRenderPass, DepthDisplaySubpass, DepthPrepassSubpass, ForwardDepthPrepassRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferWritePass
    },
};

use super::GraphicsPipelineBuilder;

pub type GraphicsPipelineColorDepthCombinedTextured = GraphicsPipelineBuilder<
    AttachmentsColorDepthCombined,
    PipelineLayoutTextured,
    PipelineStatesDefault<ColorDepthCombinedSubpass>,
    ColorDepthCombinedRenderPass,
    ColorDepthCombinedSubpass,
>;

pub type GraphicsPipelineColorDepthCombinedSkybox = GraphicsPipelineBuilder<
    AttachmentsGBuffer,
    PipelineLayoutSkybox,
    PipelineStatesSkybox<GBufferWritePass>,
    DeferedRenderPass,
    GBufferWritePass,
>;

pub type GraphicsPipelineForwardDepthPrepass = GraphicsPipelineBuilder<
    AttachmentsDepthPrepass,
    PipelineLayoutNoMaterial,
    PipelineStatesDefault<DepthPrepassSubpass>,
    ForwardDepthPrepassRenderPass,
    DepthPrepassSubpass,
>;

pub type GraphicsPipelineColorPass = GraphicsPipelineBuilder<
    AttachmentsDepthPrepass,
    PipelineLayoutTextured,
    PipelineStatesDepthWriteDisabled<ColorPassSubpass>,
    ForwardDepthPrepassRenderPass,
    ColorPassSubpass,
>;

pub type GraphicsPipelineDepthDisplay = GraphicsPipelineBuilder<
    AttachmentsDepthPrepass,
    PipelineLayoutTwoInputAttachments,
    PipelineStatesDepthDisplay<DepthDisplaySubpass>,
    ForwardDepthPrepassRenderPass,
    DepthDisplaySubpass,
>;

// deferred.rs

pub type GBufferDepthPrepasPipeline = GraphicsPipelineBuilder<
    AttachmentsGBuffer,
    PipelineLayoutNoMaterial,
    DeferedStatesDepthEnabled<GBufferDepthPrepas>,
    DeferedRenderPass,
    GBufferDepthPrepas,
>;

pub type GBufferWritePassPipeline = GraphicsPipelineBuilder<
    AttachmentsGBuffer,
    PipelineLayoutTextured,
    DeferedStatesDepthDisabled<GBufferWritePass>,
    DeferedRenderPass,
    GBufferWritePass,
>;

pub type GBufferShadingPassPipeline = GraphicsPipelineBuilder<
    AttachmentsGBuffer,
    PipelineLayoutGBuffer,
    DeferedStatesDepthDisabled<GBufferShadingPass>,
    DeferedRenderPass,
    GBufferShadingPass,
>;

// deferred.rs