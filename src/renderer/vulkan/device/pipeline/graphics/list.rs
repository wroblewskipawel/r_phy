use std::{any::TypeId, collections::HashMap, error::Error, marker::PhantomData, path::PathBuf};

use crate::renderer::vulkan::device::{
    framebuffer::presets::AttachmentsGBuffer, pipeline::ShaderDirectory, VulkanDevice,
};

use super::{
    GBufferDepthPrepasPipeline, GraphicsPipeline, GraphicsPipelineConfig,
    GraphicsPipelineTypeErased,
};

pub trait GraphicsPipelineTypeList {
    const LEN: usize;
    type Pipeline: GraphicsPipelineConfig;
    type Next: GraphicsPipelineTypeList;
}

pub struct GraphicsPipelineTerminator;

impl GraphicsPipelineTypeList for GraphicsPipelineTerminator {
    const LEN: usize = 0;
    type Pipeline = GBufferDepthPrepasPipeline<AttachmentsGBuffer>;
    type Next = Self;
}

pub struct PipelineCollection {
    pipelines: HashMap<TypeId, Vec<GraphicsPipelineTypeErased>>,
}

pub struct PipelineListRef<'a, C: GraphicsPipelineConfig> {
    pipelines: &'a Vec<GraphicsPipelineTypeErased>,
    _phantom: PhantomData<C>,
}

impl<'a, T: GraphicsPipelineConfig> PipelineListRef<'a, T> {
    pub fn get(&self, index: usize) -> GraphicsPipeline<T> {
        self.pipelines[index].try_into().unwrap()
    }

    pub fn len(&self) -> usize {
        self.pipelines.len()
    }
}

impl<'a, T: GraphicsPipelineConfig> IntoIterator for PipelineListRef<'a, T> {
    type Item = GraphicsPipeline<T>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.pipelines
            .iter()
            .map(|&p| p.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl PipelineCollection {
    pub fn try_get<T: GraphicsPipelineConfig>(&self) -> Option<PipelineListRef<T>> {
        if let Some(pipelines) = self.pipelines.get(&TypeId::of::<T>()) {
            Some(PipelineListRef {
                pipelines,
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }
}

impl VulkanDevice {
    fn insert_pipeline<C: GraphicsPipelineTypeList>(
        &self,
        mut pipelines: HashMap<TypeId, Vec<GraphicsPipelineTypeErased>>,
        shaders: &HashMap<TypeId, Vec<PathBuf>>,
    ) -> Result<HashMap<TypeId, Vec<GraphicsPipelineTypeErased>>, Box<dyn Error>> {
        if C::LEN > 0 {
            let type_erased = shaders
                .get(&TypeId::of::<C::Pipeline>())
                .ok_or("No shader found for pipeline!")?
                .iter()
                .flat_map(|shader_source| {
                    self.create_graphics_pipeline::<C::Pipeline>(
                        ShaderDirectory::new(&shader_source),
                        self.physical_device.surface_properties.get_current_extent(),
                    )
                    .map(|pipeline| pipeline.into())
                })
                .collect();
            pipelines.insert(TypeId::of::<C::Pipeline>(), type_erased);
            self.insert_pipeline::<C::Next>(pipelines, shaders)
        } else {
            Ok(pipelines)
        }
    }

    pub fn create_pipeline_list<C: GraphicsPipelineTypeList>(
        &self,
        shaders: &HashMap<TypeId, Vec<PathBuf>>,
    ) -> Result<PipelineCollection, Box<dyn Error>> {
        Ok(PipelineCollection {
            pipelines: self.insert_pipeline::<C>(HashMap::new(), shaders)?,
        })
    }
    pub fn destory_pipeline_list(&self, list: &mut PipelineCollection) {
        list.pipelines
            .iter_mut()
            .for_each(|(_, pipelines)| pipelines.iter_mut().for_each(|p| self.destroy_pipeline(p)));
    }
}
