use crate::renderer::vulkan::device::{
    pipeline::{
        PipelineLayoutGBuffer, PipelineLayoutNoMaterial, PipelineLayoutSkybox,
        PipelineLayoutTextured, StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
    },
    render_pass::{DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferWritePass},
};

use super::GraphicsPipelineBuilder;

pub type GraphicsPipelineColorDepthCombinedSkybox<A> = GraphicsPipelineBuilder<
    PipelineLayoutSkybox,
    StatesSkybox,
    DeferedRenderPass<A>,
    GBufferWritePass<A>,
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
