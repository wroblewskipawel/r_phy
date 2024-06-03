use crate::renderer::vulkan::device::{
    pipeline::{
        PipelineLayoutGBuffer, PipelineLayoutMaterial, PipelineLayoutNoMaterial,
        PipelineLayoutSkybox, StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
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

pub type GBufferWritePassPipeline<A, M> = GraphicsPipelineBuilder<
    PipelineLayoutMaterial<M>,
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
