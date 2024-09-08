mod presets;

pub use presets::*;

use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    sync::{Once, RwLock},
};

use ash::vk;

use crate::renderer::vulkan::device::{descriptor::DescriptorLayout, VulkanDevice};

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

pub trait Layout: 'static {
    type Descriptors: DescriptorLayoutList;
    type PushConstants: PushConstantList;

    fn ranges() -> PushConstantRanges<Self::PushConstants> {
        PushConstantRanges::<Self::PushConstants>::builder()
    }

    fn sets() -> DescriptorSets<Self::Descriptors> {
        DescriptorSets::<Self::Descriptors>::builder()
    }
}

pub trait PushConstant: 'static {
    fn range(offset: u32) -> vk::PushConstantRange;
}

pub trait PushConstantList: 'static {
    type Item: PushConstant;
    type Next: PushConstantList;

    fn exhausted() -> bool;
    fn len() -> usize;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PushConstantTerminator {}

impl PushConstant for PushConstantTerminator {
    fn range(_offset: u32) -> vk::PushConstantRange {
        unreachable!()
    }
}

impl PushConstantList for PushConstantTerminator {
    type Item = Self;
    type Next = Self;

    fn exhausted() -> bool {
        true
    }

    fn len() -> usize {
        0
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PushConstantNode<P: PushConstant, N: PushConstantList> {
    _phantom: PhantomData<(P, N)>,
}

impl<P: PushConstant, N: PushConstantList> PushConstantList for PushConstantNode<P, N> {
    type Item = P;
    type Next = N;

    fn exhausted() -> bool {
        false
    }

    fn len() -> usize {
        Self::Next::len() + 1
    }
}

pub struct PushConstantRanges<N: PushConstantList> {
    _phantom: PhantomData<N>,
}

impl<N: PushConstantList> Default for PushConstantRanges<N> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<N: PushConstantList> PushConstantRanges<N> {
    pub fn new() -> PushConstantRanges<PushConstantTerminator> {
        PushConstantRanges {
            _phantom: PhantomData,
        }
    }

    pub fn push<P: PushConstant>(self) -> PushConstantRanges<PushConstantNode<P, N>> {
        PushConstantRanges {
            _phantom: PhantomData,
        }
    }

    pub fn builder() -> Self {
        Self::default()
    }

    fn next_push_range<'a, T: PushConstantList>(
        offset: u32,
        mut iter: impl Iterator<Item = &'a mut vk::PushConstantRange>,
    ) {
        if !T::exhausted() {
            if let Some(entry) = iter.next() {
                let range = T::Item::range(offset);
                *entry = range;
                Self::next_push_range::<T::Next>(offset + range.size, iter)
            }
        }
    }

    pub fn get_ranges() -> Vec<vk::PushConstantRange> {
        let mut ranges = vec![vk::PushConstantRange::default(); N::len()];
        Self::next_push_range::<N>(0, ranges.iter_mut());
        ranges
    }

    fn try_get_next_range<P: PushConstant, L: PushConstantList>(
        offset: u32,
    ) -> Option<vk::PushConstantRange> {
        if !L::exhausted() {
            let range = L::Item::range(offset);
            if TypeId::of::<P>() == TypeId::of::<L::Item>() {
                Some(range)
            } else {
                Self::try_get_next_range::<P, L::Next>(offset + range.size)
            }
        } else {
            None
        }
    }

    pub fn try_get_range<P: PushConstant>(&self) -> Option<vk::PushConstantRange> {
        Self::try_get_next_range::<P, N>(0)
    }
}

pub trait DescriptorLayoutList: 'static {
    type Item: DescriptorLayout;
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

    fn get_descriptor_writes<T: crate::renderer::vulkan::device::descriptor::DescriptorBinding>(
    ) -> Vec<vk::WriteDescriptorSet> {
        unreachable!()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DescriptorLayoutNode<L: DescriptorLayout, N: DescriptorLayoutList> {
    _phantom: PhantomData<(L, N)>,
}

impl<L: DescriptorLayout, N: DescriptorLayoutList> DescriptorLayoutList
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

pub struct DescriptorSets<L: DescriptorLayoutList> {
    _phantom: PhantomData<L>,
}

impl<L: DescriptorLayoutList> DescriptorSets<L> {
    pub fn builder() -> DescriptorSets<L> {
        DescriptorSets {
            _phantom: PhantomData,
        }
    }

    fn try_get_index<T: DescriptorLayout, N: DescriptorLayoutList>(index: u32) -> Option<u32> {
        if !N::exhausted() {
            if TypeId::of::<T>() == TypeId::of::<N::Item>() {
                Some(index - 1)
            } else {
                Self::try_get_index::<T, N::Next>(index - 1)
            }
        } else {
            None
        }
    }

    pub fn get_set_index<T: DescriptorLayout>(&self) -> Option<u32> {
        Self::try_get_index::<T, L>(L::len() as u32)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineLayoutBuilder<T: DescriptorLayoutList, P: PushConstantList> {
    _phantom: PhantomData<(T, P)>,
}

impl<T: DescriptorLayoutList, P: PushConstantList> Default for PipelineLayoutBuilder<T, P> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<T: DescriptorLayoutList, P: PushConstantList> PipelineLayoutBuilder<T, P> {
    pub fn new() -> PipelineLayoutBuilder<DescriptorLayoutTerminator, PushConstantTerminator> {
        PipelineLayoutBuilder {
            _phantom: PhantomData,
        }
    }

    pub fn with_push_constant<C: PushConstant>(
        self,
    ) -> PipelineLayoutBuilder<T, PushConstantNode<C, P>> {
        PipelineLayoutBuilder::<T, PushConstantNode<C, P>> {
            _phantom: PhantomData,
        }
    }

    pub fn with_descriptor_set<D: DescriptorLayout>(
        self,
    ) -> PipelineLayoutBuilder<DescriptorLayoutNode<D, T>, P> {
        PipelineLayoutBuilder::<DescriptorLayoutNode<D, T>, P> {
            _phantom: PhantomData,
        }
    }

    pub fn builder() -> Self {
        Self::default()
    }
}

impl<T: DescriptorLayoutList, P: PushConstantList> Layout for PipelineLayoutBuilder<T, P> {
    type Descriptors = T;
    type PushConstants = P;
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineLayoutRaw {
    pub layout: vk::PipelineLayout,
}

impl<L: Layout> From<PipelineLayout<L>> for PipelineLayoutRaw {
    fn from(layout: PipelineLayout<L>) -> Self {
        Self {
            layout: layout.layout,
        }
    }
}

impl<L: Layout> From<PipelineLayoutRaw> for PipelineLayout<L> {
    fn from(layout: PipelineLayoutRaw) -> Self {
        Self {
            layout: layout.layout,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineLayout<L: Layout> {
    pub layout: vk::PipelineLayout,
    pub _phantom: PhantomData<L>,
}

impl<L: Layout> From<PipelineLayout<L>> for vk::PipelineLayout {
    fn from(layout: PipelineLayout<L>) -> Self {
        layout.layout
    }
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

    pub fn get_pipeline_layout<L: Layout>(&self) -> Result<PipelineLayout<L>, Box<dyn Error>> {
        let push_ranges = PushConstantRanges::<L::PushConstants>::get_ranges();
        let layout_map = get_pipeline_layout_map();
        let layout = if let Some(layout) = {
            let reader = layout_map.read()?;
            reader.get(&TypeId::of::<L>()).copied()
        } {
            layout
        } else {
            let layout = unsafe {
                self.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .push_constant_ranges(&push_ranges)
                        .set_layouts(&self.get_descriptor_layouts::<L::Descriptors>()?),
                    None,
                )?
            };
            let mut layout_map_witer = layout_map.write()?;
            layout_map_witer.insert(TypeId::of::<L>(), layout);
            layout
        };
        Ok(PipelineLayout {
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
