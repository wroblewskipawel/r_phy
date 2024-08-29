use crate::renderer::{
    model::CommonVertex,
    vulkan::device::{
        pipeline::{
            PipelineLayoutGBuffer, PipelineLayoutMaterial, PipelineLayoutNoMaterial,
            PipelineLayoutSkybox, StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
        },
        render_pass::{
            DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferSkyboxPass,
            GBufferWritePass,
        },
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
    StatesDepthTestEnabled<CommonVertex>,
    DeferedRenderPass<A>,
    GBufferDepthPrepas<A>,
>;

pub type GBufferWritePassPipeline<A, M, V> = GraphicsPipelineBuilder<
    PipelineLayoutMaterial<M>,
    StatesDepthWriteDisabled<V>,
    DeferedRenderPass<A>,
    GBufferWritePass<A>,
>;

pub type GBufferShadingPassPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutGBuffer,
    StatesDepthWriteDisabled<CommonVertex>,
    DeferedRenderPass<A>,
    GBufferShadingPass<A>,
>;
