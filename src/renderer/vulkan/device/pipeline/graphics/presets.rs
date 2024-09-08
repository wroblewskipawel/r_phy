use crate::renderer::{
    model::{CommonVertex, VertexNone},
    vulkan::device::{
        pipeline::{
            PipelineLayoutGBuffer, PipelineLayoutNoMaterial, PipelineLayoutSkybox,
            StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
        },
        render_pass::{
            DeferedRenderPass, EmptyRenderPass, EmptySubpass, GBufferDepthPrepas,
            GBufferShadingPass, GBufferSkyboxPass,
        },
    },
};

use super::GraphicsPipelineBuilder;

pub type EmptyPipeline = GraphicsPipelineBuilder<
    PipelineLayoutNoMaterial,
    StatesDepthWriteDisabled<VertexNone>,
    EmptyRenderPass,
    EmptySubpass,
>;

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

pub type GBufferShadingPassPipeline<A> = GraphicsPipelineBuilder<
    PipelineLayoutGBuffer,
    StatesDepthWriteDisabled<CommonVertex>,
    DeferedRenderPass<A>,
    GBufferShadingPass<A>,
>;
