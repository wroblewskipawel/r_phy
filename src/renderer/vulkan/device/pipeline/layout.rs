use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    mem::size_of,
    sync::{Once, RwLock},
};

use ash::vk;

use crate::{
    math::types::Matrix4,
    renderer::vulkan::device::{
        descriptor::{CameraDescriptorSet, DescriptorLayout, TextureDescriptorSet},
        VulkanDevice,
    },
};

// TODO: Create macro to avoid code repetition
fn get_pipeline_layout_map() -> &'static RwLock<HashMap<std::any::TypeId, vk::PipelineLayout>> {
    static mut LAYOUTS: Option<RwLock<HashMap<std::any::TypeId, vk::PipelineLayout>>> = None;
    static INIT: Once = Once::new();
    unsafe {
        INIT.call_once(|| {
            if LAYOUTS.is_none() {
                LAYOUTS.replace(RwLock::new(HashMap::new()));
            }
        });
        LAYOUTS.as_ref().unwrap()
    }
}

pub trait DescriptorLayoutList {
    type Item: 'static + DescriptorLayout;
    type Next: DescriptorLayoutList;

    fn exhausted() -> bool;
    fn len() -> usize;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DescriptorLayoutTerminator {}

impl DescriptorLayoutList for DescriptorLayoutTerminator {
    type Item = Self;
    type Next = Self;

    fn exhausted() -> bool {
        true
    }

    fn len() -> usize {
        0
    }
}

impl DescriptorLayout for DescriptorLayoutTerminator {
    fn get_descriptor_set_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        unreachable!()
    }

    fn get_descriptor_pool_sizes(_num_sets: u32) -> Vec<vk::DescriptorPoolSize> {
        unreachable!()
    }

    fn get_descriptor_write<T: crate::renderer::vulkan::device::descriptor::DescriptorBinding>(
    ) -> Option<vk::WriteDescriptorSet> {
        unreachable!()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DescriptorLayoutNode<L: DescriptorLayout, N: DescriptorLayoutList> {
    _phantom: PhantomData<(L, N)>,
}

impl<L: 'static + DescriptorLayout, N: DescriptorLayoutList> DescriptorLayoutList
    for DescriptorLayoutNode<L, N>
{
    type Item = L;
    type Next = N;

    fn exhausted() -> bool {
        false
    }

    fn len() -> usize {
        Self::Next::len() + 1
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphicsPipelineLayout<T: DescriptorLayoutList> {
    pub layout: vk::PipelineLayout,
    _phantom: PhantomData<T>,
}

impl VulkanDevice {
    fn get_descriptor_list_entry<'a, T: DescriptorLayoutList>(
        &self,
        mut iter: impl Iterator<Item = &'a mut vk::DescriptorSetLayout>,
    ) -> Result<(), Box<dyn Error>> {
        if !T::exhausted() {
            if let Some(entry) = iter.next() {
                *entry = self.get_descriptor_set_layout::<T::Item>()?.layout;
            }
            self.get_descriptor_list_entry::<T::Next>(iter)
        } else {
            Ok(())
        }
    }

    pub fn get_descriptor_layouts<T: DescriptorLayoutList>(
        &self,
    ) -> Result<Vec<vk::DescriptorSetLayout>, Box<dyn Error>> {
        let mut layouts = vec![vk::DescriptorSetLayout::null(); T::len()];
        self.get_descriptor_list_entry::<T>(layouts.iter_mut().rev())?;
        Ok(layouts)
    }

    pub fn get_graphics_pipeline_layout<T: 'static + DescriptorLayoutList>(
        &self,
    ) -> Result<GraphicsPipelineLayout<T>, Box<dyn Error>> {
        const PUSH_CONSTANT_RANGES: &[vk::PushConstantRange] = &[vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: (size_of::<Matrix4>() * 3) as u32,
        }];
        let layout_map = get_pipeline_layout_map();

        let layout = if let Some(layout) = {
            let reader = layout_map.read()?;
            reader
                .get(&TypeId::of::<GraphicsPipelineLayout<T>>())
                .copied()
        } {
            layout
        } else {
            let layout = unsafe {
                self.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .push_constant_ranges(PUSH_CONSTANT_RANGES)
                        .set_layouts(&self.get_descriptor_layouts::<T>()?),
                    None,
                )?
            };
            let mut layout_map_witer = layout_map.write()?;
            layout_map_witer.insert(TypeId::of::<GraphicsPipelineLayout<T>>(), layout);
            layout
        };
        Ok(GraphicsPipelineLayout {
            layout,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_pipeline_layouts(&self) {
        let layout_map = get_pipeline_layout_map();
        let exclusive_lock = layout_map.write().unwrap();
        for (_, &layout) in exclusive_lock.iter() {
            unsafe {
                self.device.destroy_pipeline_layout(layout, None);
            }
        }
    }
}

pub type GraphicsPipelineLayoutTextured = DescriptorLayoutNode<
    TextureDescriptorSet,
    DescriptorLayoutNode<CameraDescriptorSet, DescriptorLayoutTerminator>,
>;
