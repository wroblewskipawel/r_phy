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
    renderer::{
        camera::CameraMatrices,
        vulkan::device::{descriptor::DescriptorLayout, image::Texture2D, VulkanDevice},
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

// Rename if this trait ends up used only for DescriptorLayout
pub trait TypeListNode
where
    Self: 'static + Sized,
{
    type Next: TypeListNode;
    type Item: DescriptorLayout;

    fn next(&self) -> Option<&Self::Next>;
    fn push<T: DescriptorLayout>(self) -> Node<T, Self> {
        Node {
            next: self,
            _phantom: PhantomData,
        }
    }
    fn len(&self) -> usize {
        if let Some(next) = self.next() {
            1 + next.len()
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Nil;

impl DescriptorLayout for Nil {
    fn get_descriptor_set_bindings() -> &'static [vk::DescriptorSetLayoutBinding] {
        unreachable!()
    }

    fn get_descriptor_write() -> vk::WriteDescriptorSet {
        unreachable!()
    }
}

impl TypeListNode for Nil {
    type Next = Self;
    type Item = Self;

    fn next(&self) -> Option<&Self::Next> {
        None
    }
}

// Similar structure could be used to handle creation of DescriptorSetlayout
// by composing types that would impl DescriptorBinding (not yet implemented)
// Check if this code could be reused, for traits other than DescriptorLayout,
// by using associated types trait bounds (which are unstable,
// see: https://rust-lang.github.io/rfcs/2289-associated-type-bounds.html)
// or converted to macro
#[derive(Debug, Clone, Copy)]
pub struct Node<L: DescriptorLayout, N: TypeListNode> {
    next: N,
    _phantom: PhantomData<L>,
}

impl<L: DescriptorLayout, N: TypeListNode> TypeListNode for Node<L, N> {
    type Next = N;
    type Item = L;

    fn next(&self) -> Option<&Self::Next> {
        Some(&self.next)
    }
}

pub struct DescriptorLayoutBuilder<T: TypeListNode> {
    head: T,
}

impl DescriptorLayoutBuilder<Nil> {
    pub fn new() -> Self {
        Self { head: Nil }
    }
}

impl<T: TypeListNode> DescriptorLayoutBuilder<T> {
    pub fn push<L: DescriptorLayout>(self) -> DescriptorLayoutBuilder<Node<L, T>> {
        DescriptorLayoutBuilder {
            head: Node {
                next: self.head,
                _phantom: PhantomData::<L>,
            },
        }
    }
}

// Type T should have some trait bounds imposed,
// that would require for it to only be derivative of TypeListNode
#[derive(Debug, Clone, Copy)]
pub struct GraphicsPipelineLayout<T> {
    pub layout: vk::PipelineLayout,
    _phantom: PhantomData<T>,
}

impl VulkanDevice {
    fn get_descriptor_list_entry<'a, T: TypeListNode>(
        &self,
        node: &T,
        mut iter: impl Iterator<Item = &'a mut vk::DescriptorSetLayout>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(entry) = iter.next() {
            *entry = self.get_descriptor_set_layout::<T::Item>()?;
        }
        if let Some(next) = node.next() {
            self.get_descriptor_list_entry(next, iter)
        } else {
            Ok(())
        }
    }

    pub fn get_descriptor_layouts<T: TypeListNode>(
        &self,
        builder: DescriptorLayoutBuilder<T>,
    ) -> Result<Vec<vk::DescriptorSetLayout>, Box<dyn Error>> {
        let DescriptorLayoutBuilder { head } = builder;
        let mut layouts = vec![vk::DescriptorSetLayout::null(); head.len()];
        self.get_descriptor_list_entry(&head, layouts.iter_mut().rev())?;
        Ok(layouts)
    }

    pub fn get_graphics_pipeline_layout<T: TypeListNode>(
        &self,
        layout: DescriptorLayoutBuilder<T>,
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
                        .set_layouts(&self.get_descriptor_layouts(layout)?),
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

// GraphicsPipelineLayout could be defined in its own separate module
// which could then expose library of predefined pipeline layouts
// pub type GraphicsPipelineLayoutSimple = Node<CameraMatrices, Nil>;
pub type GraphicsPipelineLayoutTextured = Node<Texture2D, Node<CameraMatrices, Nil>>;
