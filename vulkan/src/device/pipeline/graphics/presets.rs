use to_resolve::model::CommonVertex;

use crate::device::{
    pipeline::{
        PipelineLayoutGBuffer, PipelineLayoutNoMaterial, PipelineLayoutSkybox,
        StatesDepthTestEnabled, StatesDepthWriteDisabled, StatesSkybox,
    },
    render_pass::{DeferedRenderPass, GBufferDepthPrepas, GBufferShadingPass, GBufferSkyboxPass},
};

use super::GraphicsPipelineBuilder;

// pub type EmptyPipeline = GraphicsPipelineBuilder<
//     PipelineLayoutNoMaterial,
//     StatesDepthWriteDisabled<VertexNone>,
//     EmptyRenderPass,
//     EmptySubpass,
// >;

pub type GBufferSkyboxPipeline<At, Al> = GraphicsPipelineBuilder<
    PipelineLayoutSkybox<Al>,
    StatesSkybox,
    DeferedRenderPass<At>,
    GBufferSkyboxPass<At>,
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
