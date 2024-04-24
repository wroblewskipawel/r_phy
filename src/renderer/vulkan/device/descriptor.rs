mod presets;

pub use presets::*;

use ash::vk;
use bytemuck::Pod;
use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Index;
use std::sync::{Once, RwLock};

use super::buffer::UniformBuffer;
use super::command::operation::Operation;
use super::VulkanDevice;

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
    type Item: DescriptorBinding;
    type Next: DescriptorBindingList;

    fn exhausted() -> bool;
    fn len() -> usize;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DescriptorBindingTerminator {}

impl DescriptorBinding for DescriptorBindingTerminator {
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
    type Item = Self;
    type Next = Self;

    fn exhausted() -> bool {
        true
    }

    fn len() -> usize {
        0
    }
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
    type Item = B;
    type Next = N;

    fn exhausted() -> bool {
        false
    }

    fn len() -> usize {
        Self::Next::len() + 1
    }
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
        mut iter: impl Iterator<Item = (u32, &'a mut vk::DescriptorSetLayoutBinding)>,
    ) {
        if !T::exhausted() {
            if let Some((binding_index, entry)) = iter.next() {
                *entry = T::Item::get_descriptor_set_binding(binding_index);
                Self::next_descriptor_binding::<T::Next>(iter);
            }
        }
    }

    pub fn get_descriptor_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        let num_bindings = B::len();
        let mut bindings = vec![vk::DescriptorSetLayoutBinding::default(); num_bindings];
        Self::next_descriptor_binding::<B>((0u32..num_bindings as u32).zip(bindings.iter_mut()));
        bindings
    }

    fn try_get_descriptor_writes<S: DescriptorBinding, T: DescriptorBindingList>(
        binding: u32,
        mut vec: Vec<vk::WriteDescriptorSet>,
    ) -> Vec<vk::WriteDescriptorSet> {
        if !T::exhausted() {
            if TypeId::of::<S>() == TypeId::of::<T::Item>() {
                vec.push(T::Item::get_descriptor_write(binding));
            }
            Self::try_get_descriptor_writes::<S, T::Next>(binding + 1, vec)
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
        if !T::exhausted() {
            let pool_size = T::Item::get_descriptor_pool_size(num_sets);
            let descriptor_count = pool_sizes.entry(pool_size.ty).or_insert(0);
            *descriptor_count += pool_size.descriptor_count;
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
    pub(super) layout: vk::DescriptorSetLayout,
    _phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct DescriptorRaw {
    pub set: vk::DescriptorSet,
}

#[derive(Debug)]
pub struct Descriptor<T: DescriptorLayout> {
    pub set: vk::DescriptorSet,
    _phantom: PhantomData<T>,
}

impl<T: DescriptorLayout> From<DescriptorRaw> for Descriptor<T> {
    fn from(raw: DescriptorRaw) -> Self {
        Self {
            set: raw.set,
            _phantom: PhantomData,
        }
    }
}

impl<T: DescriptorLayout> Clone for Descriptor<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DescriptorLayout> Copy for Descriptor<T> {}

pub struct DescriptorPoolRaw {
    pub count: usize,
    pool: vk::DescriptorPool,
    sets: Vec<DescriptorRaw>,
}

pub struct DescriptorPool<'a, T: DescriptorLayout> {
    pub raw: &'a DescriptorPoolRaw,
    _phantom: PhantomData<T>,
}

impl<'a, T: DescriptorLayout> From<&'a DescriptorPoolRaw> for DescriptorPool<'a, T> {
    fn from(raw: &'a DescriptorPoolRaw) -> Self {
        Self {
            raw,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: DescriptorLayout> DescriptorPool<'a, T> {
    pub fn get(&self, index: usize) -> Descriptor<T> {
        self.raw.sets[index].into()
    }
}

enum SetWrite {
    Buffer {
        set_index: usize,
        buffer_write_index: usize,
        write: vk::WriteDescriptorSet,
    },
    Image {
        set_index: usize,
        image_write_index: usize,
        write: vk::WriteDescriptorSet,
    },
}

pub struct DescriptorSetWriter<T: DescriptorLayout> {
    num_sets: usize,
    writes: Vec<SetWrite>,
    bufer_writes: Vec<vk::DescriptorBufferInfo>,
    image_writes: Vec<vk::DescriptorImageInfo>,
    _phantom: PhantomData<T>,
}

impl<T: DescriptorLayout> DescriptorSetWriter<T> {
    // # TODO: num_sets could be determined at the time of descriptor pool creation
    pub fn new(num_sets: usize) -> DescriptorSetWriter<T> {
        DescriptorSetWriter {
            num_sets,
            writes: vec![],
            bufer_writes: vec![],
            image_writes: vec![],
            _phantom: PhantomData,
        }
    }

    // TODO: sets Vec of incorrect length could be passed here
    fn get_descriptor_writes(&self, sets: &Vec<DescriptorRaw>) -> Vec<vk::WriteDescriptorSet> {
        let DescriptorSetWriter {
            writes,
            bufer_writes,
            image_writes,
            ..
        } = self;
        writes
            .into_iter()
            .map(|write| match write {
                SetWrite::Buffer {
                    set_index,
                    buffer_write_index,
                    write,
                } => vk::WriteDescriptorSet {
                    dst_set: sets[*set_index].set,
                    p_buffer_info: &bufer_writes[*buffer_write_index],
                    ..*write
                },
                SetWrite::Image {
                    set_index,
                    image_write_index,
                    write,
                } => vk::WriteDescriptorSet {
                    dst_set: sets[*set_index].set,
                    p_image_info: &image_writes[*image_write_index],
                    ..*write
                },
            })
            .collect::<Vec<_>>()
    }

    pub(super) fn write_buffer<U: Pod + DescriptorBinding, O: Operation>(
        mut self,
        buffer: &UniformBuffer<U, O>,
    ) -> Self {
        let writes = T::get_descriptor_writes::<U>();
        if writes.is_empty() {
            panic!(
                "Invalid DescriptorBinding type {} for descriptor layout {}",
                type_name::<U>(),
                type_name::<T>()
            )
        }
        let descriptor_count = writes
            .iter()
            .map(|write| write.descriptor_count as usize)
            .sum::<usize>();
        let num_uniforms = self.num_sets * descriptor_count;
        debug_assert_eq!(
            num_uniforms, buffer.size,
            "UniformBuffer object not large enough for DescriptorPool write!"
        );
        let buffer_write_base_index = self.bufer_writes.len();
        self.bufer_writes
            .extend((0..num_uniforms).map(|index| vk::DescriptorBufferInfo {
                buffer: buffer.as_raw(),
                offset: (size_of::<U>() * index) as vk::DeviceSize,
                range: size_of::<U>() as vk::DeviceSize,
            }));
        self.writes.extend((0..self.num_sets).flat_map(|set_index| {
            let mut buffer_set_write_offset = 0;
            writes
                .iter()
                .map(|&write| {
                    let descriptor_write = SetWrite::Buffer {
                        set_index,
                        buffer_write_index: buffer_write_base_index
                            + set_index * descriptor_count
                            + buffer_set_write_offset,
                        write,
                    };
                    buffer_set_write_offset += write.descriptor_count as usize;
                    descriptor_write
                })
                .collect::<Vec<_>>()
        }));
        self
    }

    pub fn write_images<'a, B, I>(mut self, images: &'a [I]) -> Self
    where
        B: DescriptorBinding,
        &'a I: Into<vk::DescriptorImageInfo>,
    {
        let writes = T::get_descriptor_writes::<B>();
        if writes.is_empty() {
            panic!(
                "Invalid DescriptorBinding type {} for descriptor layout {}",
                type_name::<I>(),
                type_name::<T>()
            )
        }
        let descciptor_count = writes
            .iter()
            .map(|write| write.descriptor_count as usize)
            .sum::<usize>();
        let num_uniforms = self.num_sets * descciptor_count;
        debug_assert_eq!(
            num_uniforms,
            images.len(),
            "Not enough images for DescriptorPool write!"
        );
        let iamge_write_base_index = self.image_writes.len();
        self.image_writes
            .extend(images.iter().map(|image| image.into()));
        self.writes.extend((0..self.num_sets).flat_map(|set_index| {
            let mut image_set_write_offset = 0;
            writes
                .iter()
                .map(|&write| {
                    let descriptor_write = SetWrite::Image {
                        set_index,
                        image_write_index: iamge_write_base_index
                            + set_index * descciptor_count
                            + image_set_write_offset,
                        write,
                    };
                    image_set_write_offset += write.descriptor_count as usize;
                    descriptor_write
                })
                .collect::<Vec<_>>()
        }));
        self
    }
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

    pub fn create_descriptor_pool<T: DescriptorLayout>(
        &self,
        writer: DescriptorSetWriter<T>,
    ) -> Result<DescriptorPoolRaw, Box<dyn Error>> {
        let pool_sizes = T::get_descriptor_pool_sizes(writer.num_sets as u32);
        let pool_create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(writer.num_sets as u32);
        let pool = unsafe {
            self.device
                .create_descriptor_pool(&pool_create_info, None)?
        };
        let layout = self.get_descriptor_set_layout::<T>()?;
        let sets = unsafe {
            self.device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::builder()
                        .descriptor_pool(pool)
                        .set_layouts(&vec![layout.layout; writer.num_sets]),
                )?
                .into_iter()
                .map(|set| DescriptorRaw { set })
                .collect::<Vec<_>>()
        };
        let writes = writer.get_descriptor_writes(&sets);
        unsafe {
            self.device
                .update_descriptor_sets(&writes, &[])
        }
        Ok(DescriptorPoolRaw {
            count: sets.len(),
            pool,
            sets,
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

    pub fn destroy_descriptor_pool(&self, pool: &mut DescriptorPoolRaw) {
        unsafe {
            self.device.destroy_descriptor_pool(pool.pool, None);
        };
    }
}
