use std::{
    any::{type_name, TypeId},
    error::Error,
    marker::PhantomData,
};

use ash::vk;
use bytemuck::AnyBitPattern;

use crate::device::{
    pipeline::{
        get_pipeline_states_info, Layout, ModuleLoader, PipelineBindData, PipelineLayout,
        PushConstant, PushConstantDataRef,
    },
    render_pass::RenderPassConfig,
    VulkanDevice,
};

use super::GraphicsPipelineConfig;

#[derive(Debug)]
pub struct PipelinePackData {
    pipelines: Vec<vk::Pipeline>,
    layout: vk::PipelineLayout,
}

#[derive(Debug)]
pub struct PipelinePack<T: GraphicsPipelineConfig> {
    data: PipelinePackData,
    _phantom: PhantomData<T>,
}

#[derive(Debug)]
pub struct GraphicsPipeline<T: GraphicsPipelineConfig> {
    handle: vk::Pipeline,
    layout: vk::PipelineLayout,
    _phantom: PhantomData<T>,
}

impl<C: GraphicsPipelineConfig> From<&GraphicsPipeline<C>> for PipelineBindData {
    fn from(value: &GraphicsPipeline<C>) -> Self {
        PipelineBindData {
            bind_point: vk::PipelineBindPoint::GRAPHICS,
            pipeline: value.handle,
        }
    }
}

impl<C: GraphicsPipelineConfig> From<&mut GraphicsPipeline<C>> for vk::Pipeline {
    fn from(pipeline: &mut GraphicsPipeline<C>) -> Self {
        pipeline.handle
    }
}

impl<C: GraphicsPipelineConfig> GraphicsPipeline<C> {
    pub fn layout(&self) -> PipelineLayout<C::Layout> {
        PipelineLayout {
            layout: self.layout,
            _phantom: PhantomData,
        }
    }

    pub fn get_push_range<'a, P: PushConstant + AnyBitPattern>(
        &self,
        push_constant_data: &'a P,
    ) -> PushConstantDataRef<'a, P> {
        PushConstantDataRef {
            range: C::Layout::ranges().try_get_range::<P>().unwrap_or_else(|| {
                panic!(
                    "PushConstant {} not present in layout PushConstantRanges {}!",
                    type_name::<P>(),
                    type_name::<<C::Layout as Layout>::PushConstants>(),
                )
            }),
            layout: self.layout,
            data: push_constant_data,
        }
    }
}

impl<T: GraphicsPipelineConfig> PipelinePack<T> {
    pub fn layout(&self) -> PipelineLayout<T::Layout> {
        PipelineLayout {
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.pipelines.len()
    }

    pub fn get(&self, index: usize) -> GraphicsPipeline<T> {
        GraphicsPipeline {
            handle: self.data.pipelines[index],
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, pipeline: GraphicsPipeline<T>) {
        self.data.pipelines.push(pipeline.handle);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PipelinePackRef<'a, T: GraphicsPipelineConfig> {
    data: &'a PipelinePackData,
    _phantom: PhantomData<T>,
}

impl<'a, T: GraphicsPipelineConfig, N: GraphicsPipelineConfig> TryFrom<&'a PipelinePack<T>>
    for PipelinePackRef<'a, N>
{
    type Error = &'static str;

    fn try_from(value: &'a PipelinePack<T>) -> Result<Self, Self::Error> {
        if TypeId::of::<T>() == TypeId::of::<N>() {
            Ok(PipelinePackRef {
                data: &value.data,
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid GraphicsPipelineConfig type!")
        }
    }
}

impl<'a, T: GraphicsPipelineConfig> PipelinePackRef<'a, T> {
    pub fn layout(&self) -> PipelineLayout<T::Layout> {
        PipelineLayout {
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.pipelines.len()
    }

    pub fn get(&self, index: usize) -> GraphicsPipeline<T> {
        GraphicsPipeline {
            handle: self.data.pipelines[index],
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct PipelinePackRefMut<'a, T: GraphicsPipelineConfig> {
    data: &'a mut PipelinePackData,
    _phantom: PhantomData<T>,
}

impl<'a, T: GraphicsPipelineConfig, N: GraphicsPipelineConfig> TryFrom<&'a mut PipelinePack<T>>
    for PipelinePackRefMut<'a, N>
{
    type Error = &'static str;

    fn try_from(value: &'a mut PipelinePack<T>) -> Result<Self, Self::Error> {
        if TypeId::of::<T>() == TypeId::of::<N>() {
            Ok(PipelinePackRefMut {
                data: &mut value.data,
                _phantom: PhantomData,
            })
        } else {
            Err("Invalid GraphicsPipelineConfig type!")
        }
    }
}

impl<'a, T: GraphicsPipelineConfig> PipelinePackRefMut<'a, T> {
    pub fn layout(&self) -> PipelineLayout<T::Layout> {
        PipelineLayout {
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.data.pipelines.len()
    }

    pub fn get(&self, index: usize) -> GraphicsPipeline<T> {
        GraphicsPipeline {
            handle: self.data.pipelines[index],
            layout: self.data.layout,
            _phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, pipeline: GraphicsPipeline<T>) {
        self.data.pipelines.push(pipeline.handle);
    }
}

impl VulkanDevice {
    pub fn create_pipeline_pack<T: GraphicsPipelineConfig>(
        &self,
    ) -> Result<PipelinePack<T>, Box<dyn Error>> {
        let layout = self.get_pipeline_layout::<T::Layout>()?.into();
        Ok(PipelinePack {
            data: PipelinePackData {
                pipelines: Vec::new(),
                layout,
            },
            _phantom: PhantomData,
        })
    }

    pub fn load_pipelines<S: GraphicsPipelineConfig + ModuleLoader>(
        &self,
        pack: &mut PipelinePack<S>,
        pipelines: &[S],
    ) -> Result<(), Box<dyn Error>> {
        for pipeline in pipelines.iter() {
            pack.insert(self.create_graphics_pipeline(pack.layout(), pipeline)?);
        }
        Ok(())
    }

    pub fn create_graphics_pipeline<T: GraphicsPipelineConfig>(
        &self,
        layout: PipelineLayout<T::Layout>,
        modules: &impl ModuleLoader,
    ) -> Result<GraphicsPipeline<T>, Box<dyn Error>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        let layout = layout.into();
        let render_pass = self.get_render_pass::<T::RenderPass>()?;
        let states = get_pipeline_states_info::<T::Attachments, T::Subpass, T::PipelineStates>(
            &self.physical_device,
            extent,
        );
        let modules = modules.load(self)?;
        let stages = modules.get_stages_info();
        let subpass = T::RenderPass::try_get_subpass_index::<T::Subpass>().unwrap_or_else(|| {
            panic!(
                "Subpass {} not present in RenderPass {}!",
                type_name::<T::Subpass>(),
                type_name::<T::RenderPass>(),
            )
        }) as u32;
        let create_infos = [vk::GraphicsPipelineCreateInfo {
            subpass,
            layout,
            render_pass: render_pass.handle,
            p_vertex_input_state: &states.vertex_input.create_info,
            p_input_assembly_state: &states.input_assembly,
            p_viewport_state: &states.viewport.create_info,
            p_rasterization_state: &states.rasterization,
            p_depth_stencil_state: &states.depth_stencil,
            p_color_blend_state: &states.color_blend.create_info,
            p_multisample_state: &states.multisample,
            stage_count: stages.stages.len() as u32,
            p_stages: stages.stages.as_ptr(),
            ..Default::default()
        }];
        let &handle = unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &create_infos, None)
                .map_err(|(_, err)| err)?
                .first()
                .unwrap()
        };
        Ok(GraphicsPipeline {
            handle,
            layout,
            _phantom: PhantomData,
        })
    }

    pub fn destory_pipeline_pack<T: GraphicsPipelineConfig>(&self, pack: &mut PipelinePack<T>) {
        pack.data
            .pipelines
            .iter()
            .for_each(|&p| self.destroy_pipeline(p));
    }
}
