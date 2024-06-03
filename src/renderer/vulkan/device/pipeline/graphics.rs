mod list;
mod presets;
mod type_erased;
mod type_safe;

pub use list::*;
pub use presets::*;
pub use type_erased::*;
pub use type_safe::*;

use std::marker::PhantomData;

use crate::renderer::vulkan::device::{
    framebuffer::AttachmentList,
    render_pass::{RenderPassConfig, Subpass},
};

use super::{layout::Layout, PipelineStates};

pub trait GraphicsPipelineConfig: 'static {
    type Attachments: AttachmentList;
    type Layout: Layout;
    type PipelineStates: PipelineStates;
    type RenderPass: RenderPassConfig<Attachments = Self::Attachments>;
    type Subpass: Subpass<Self::Attachments>;
}

pub struct GraphicsPipelineBuilder<
    L: Layout,
    P: PipelineStates,
    R: RenderPassConfig,
    S: Subpass<R::Attachments>,
> {
    _phantom: PhantomData<(L, P, R, S)>,
}

impl<L: Layout, P: PipelineStates, R: RenderPassConfig, S: Subpass<R::Attachments>>
    GraphicsPipelineConfig for GraphicsPipelineBuilder<L, P, R, S>
{
    type Attachments = R::Attachments;
    type Layout = L;
    type PipelineStates = P;
    type RenderPass = R;
    type Subpass = S;
}
