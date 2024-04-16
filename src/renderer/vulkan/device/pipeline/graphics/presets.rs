use crate::renderer::vulkan::device::{
    pipeline::{
        PipelineLayoutGBuffer, PipelineLayoutNoMaterial, PipelineLayoutSkybox,
        PipelineLayoutTextured, StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
    },
    render_pass::{
        DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferSkyboxPass,
        GBufferWritePass,
    },
};

use super::GraphicsPipelineBuilder;

pub type GBufferSkyboxPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutSkybox,
    StatesSkybox,
    DeferedRenderPass<A>,
    GBufferSkyboxPass<A>,
>;

pub type GBufferDepthPrepasPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutNoMaterial,
    StatesDepthTestEnabled,
    DeferedRenderPass<A>,
    GBufferDepthPrepas<A>,
>;

pub type GBufferWritePassPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutTextured,
    StatesDepthWriteDisabled,
    DeferedRenderPass<A>,
    GBufferWritePass<A>,
>;

pub type GBufferShadingPassPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutGBuffer,
    StatesDepthWriteDisabled,
    DeferedRenderPass<A>,
    GBufferShadingPass<A>,
>;
