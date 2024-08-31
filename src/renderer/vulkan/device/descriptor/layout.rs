use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    sync::{Once, RwLock},
};

use ash::vk;

use crate::renderer::vulkan::device::VulkanDevice;

// Check out once_cell and lazy_static crates to improve the implementation
fn get_descriptor_set_layout_map(
) -> &'static RwLock<HashMap<std::any::TypeId, vk::DescriptorSetLayout>> {
    static mut LAYOUTS: Option<RwLock<HashMap<std::any::TypeId, vk::DescriptorSetLayout>>> = None;
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

pub trait DescriptorBinding: 'static {
    fn has_data() -> bool;

    fn get_descriptor_set_binding(binding: u32) -> vk::DescriptorSetLayoutBinding;

    fn get_descriptor_write(binding: u32) -> vk::WriteDescriptorSet;

    fn get_descriptor_pool_size(num_sets: u32) -> vk::DescriptorPoolSize;
}

pub trait DescriptorLayout: 'static {
    fn get_descriptor_set_bindings() -> Vec<vk::DescriptorSetLayoutBinding>;

    fn get_descriptor_writes<T: DescriptorBinding>() -> Vec<vk::WriteDescriptorSet>;

    fn get_descriptor_pool_sizes(num_sets: u32) -> Vec<vk::DescriptorPoolSize>;
}

pub trait DescriptorBindingList: 'static {
    const LEN: usize;

    type Item: DescriptorBinding;
    type Next: DescriptorBindingList;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DescriptorBindingTerminator {}

impl DescriptorBinding for DescriptorBindingTerminator {
    fn has_data() -> bool {
        unreachable!()
    }

    fn get_descriptor_set_binding(_binding: u32) -> vk::DescriptorSetLayoutBinding {
        unreachable!()
    }

    fn get_descriptor_write(_binding: u32) -> vk::WriteDescriptorSet {
        unreachable!()
    }

    fn get_descriptor_pool_size(_num_sets: u32) -> vk::DescriptorPoolSize {
        unreachable!()
    }
}

impl DescriptorBindingList for DescriptorBindingTerminator {
    const LEN: usize = 0;

    type Item = Self;
    type Next = Self;
}

#[derive(Debug, Clone, Copy)]
pub struct DescriptorBindingNode<B: DescriptorBinding, N: DescriptorBindingList> {
    _phantom: PhantomData<(B, N)>,
}

impl<B: DescriptorBinding, N: DescriptorBindingList> Default for DescriptorBindingNode<B, N> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<B: DescriptorBinding, N: DescriptorBindingList> DescriptorBindingList
    for DescriptorBindingNode<B, N>
{
    const LEN: usize = N::LEN + 1;

    type Item = B;
    type Next = N;
}

#[derive(Debug, Clone, Copy)]
pub struct DescriptorLayoutBuilder<B: DescriptorBindingList> {
    _phantom: PhantomData<B>,
}

impl<B: DescriptorBindingList> Default for DescriptorLayoutBuilder<B> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<B: DescriptorBindingList> DescriptorLayoutBuilder<B> {
    pub fn new() -> DescriptorLayoutBuilder<DescriptorBindingTerminator> {
        DescriptorLayoutBuilder {
            _phantom: PhantomData,
        }
    }

    pub fn push<N: DescriptorBinding>(
        self,
    ) -> DescriptorLayoutBuilder<DescriptorBindingNode<N, B>> {
        DescriptorLayoutBuilder {
            _phantom: PhantomData,
        }
    }

    pub fn builder() -> Self {
        Self::default()
    }

    fn next_descriptor_binding<'a, T: DescriptorBindingList>(
        binding: u32,
        mut descriptor_bindings: Vec<vk::DescriptorSetLayoutBinding>,
    ) -> Vec<vk::DescriptorSetLayoutBinding> {
        if T::LEN > 0 {
            let next_binding = if T::Item::has_data() {
                descriptor_bindings.push(T::Item::get_descriptor_set_binding(binding));
                binding + 1
            } else {
                binding
            };
            Self::next_descriptor_binding::<T::Next>(next_binding, descriptor_bindings)
        } else {
            descriptor_bindings
        }
    }

    pub fn get_descriptor_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        Self::next_descriptor_binding::<B>(0, Vec::with_capacity(B::LEN))
    }

    fn try_get_descriptor_writes<S: DescriptorBinding, T: DescriptorBindingList>(
        binding: u32,
        mut vec: Vec<vk::WriteDescriptorSet>,
    ) -> Vec<vk::WriteDescriptorSet> {
        debug_assert!(S::has_data(), "DescriptorBinding has no data!");
        if T::LEN > 0 {
            if T::Item::has_data() {
                if TypeId::of::<S>() == TypeId::of::<T::Item>() {
                    vec.push(T::Item::get_descriptor_write(binding));
                }
                Self::try_get_descriptor_writes::<S, T::Next>(binding + 1, vec)
            } else {
                Self::try_get_descriptor_writes::<S, T::Next>(binding, vec)
            }
        } else {
            vec
        }
    }

    pub fn get_descriptor_writes<T: DescriptorBinding>() -> Vec<vk::WriteDescriptorSet> {
        Self::try_get_descriptor_writes::<T, B>(0, Vec::new())
    }

    fn next_descriptor_pool_size<T: DescriptorBindingList>(
        num_sets: u32,
        pool_sizes: &mut HashMap<vk::DescriptorType, u32>,
    ) {
        if T::LEN > 0 {
            if T::Item::has_data() {
                let pool_size = T::Item::get_descriptor_pool_size(num_sets);
                let descriptor_count = pool_sizes.entry(pool_size.ty).or_insert(0);
                *descriptor_count += pool_size.descriptor_count;
            }
            Self::next_descriptor_pool_size::<T::Next>(num_sets, pool_sizes);
        }
    }

    pub fn get_descriptor_pool_sizes(num_sets: u32) -> Vec<vk::DescriptorPoolSize> {
        let mut pool_sizes = HashMap::new();
        Self::next_descriptor_pool_size::<B>(num_sets, &mut pool_sizes);
        pool_sizes
            .into_iter()
            .map(|(ty, descriptor_count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count,
            })
            .collect()
    }
}

impl<B: DescriptorBindingList> DescriptorLayout for DescriptorLayoutBuilder<B> {
    fn get_descriptor_set_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        Self::get_descriptor_bindings()
    }

    fn get_descriptor_writes<T: DescriptorBinding>() -> Vec<vk::WriteDescriptorSet> {
        Self::get_descriptor_writes::<T>()
    }

    fn get_descriptor_pool_sizes(num_sets: u32) -> Vec<vk::DescriptorPoolSize> {
        Self::get_descriptor_pool_sizes(num_sets)
    }
}

pub struct DescriptorSetLayout<T: DescriptorLayout> {
    pub layout: vk::DescriptorSetLayout,
    _phantom: PhantomData<T>,
}

impl VulkanDevice {
    pub fn get_descriptor_set_layout<T: DescriptorLayout>(
        &self,
    ) -> Result<DescriptorSetLayout<T>, Box<dyn Error>> {
        let layout_map = get_descriptor_set_layout_map();
        let layout = if let Some(layout) = {
            let layout_map_reader = layout_map.read()?;
            layout_map_reader.get(&TypeId::of::<T>()).copied()
        } {
            layout
        } else {
            let mut layout_map_writer = layout_map.write()?;
            let layout = unsafe {
                self.device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::builder()
                        .bindings(&T::get_descriptor_set_bindings()),
                    None,
                )?
            };
            layout_map_writer.insert(TypeId::of::<T>(), layout);
            layout
        };
        Ok(DescriptorSetLayout {
            layout,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_descriptor_set_layouts(&self) {
        let layout_map = get_descriptor_set_layout_map();
        let exclusive_lock = layout_map.write().unwrap();
        for (_, &layout) in exclusive_lock.iter() {
            unsafe {
                self.device.destroy_descriptor_set_layout(layout, None);
            }
        }
    }
}
