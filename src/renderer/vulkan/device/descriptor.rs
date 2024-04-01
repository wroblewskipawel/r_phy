use ash::vk;
use bytemuck::Pod;
use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Index;
use std::sync::{Once, RwLock};

use crate::renderer::camera::CameraMatrices;

use super::buffer::UniformBuffer;
use super::command::operation::Operation;
use super::image::Texture2D;
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

    fn get_descriptor_write<T: DescriptorBinding>() -> Option<vk::WriteDescriptorSet>;

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
            }
            Self::next_descriptor_binding::<T::Next>(iter);
        }
    }

    pub fn get_descriptor_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        let num_bindings = B::len();
        let mut bindings = vec![vk::DescriptorSetLayoutBinding::default(); num_bindings];
        Self::next_descriptor_binding::<B>((0u32..num_bindings as u32).zip(bindings.iter_mut()));
        bindings
    }

    fn try_get_descriptor_write<'a, S: DescriptorBinding, T: DescriptorBindingList>(
        binding: u32,
    ) -> Option<vk::WriteDescriptorSet> {
        if !T::exhausted() {
            if TypeId::of::<S>() == TypeId::of::<T::Item>() {
                Some(T::Item::get_descriptor_write(binding))
            } else {
                Self::try_get_descriptor_write::<S, T::Next>(binding + 1)
            }
        } else {
            None
        }
    }

    pub fn get_descriptor_write<T: DescriptorBinding>() -> Option<vk::WriteDescriptorSet> {
        Self::try_get_descriptor_write::<T, B>(0)
    }

    fn next_descriptor_pool_size<'a, T: DescriptorBindingList>(
        num_sets: u32,
        pool_sizes: &mut HashMap<vk::DescriptorType, u32>,
    ) {
        if !T::exhausted() {
            let pool_size = T::Item::get_descriptor_pool_size(num_sets as u32);
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

    fn get_descriptor_write<T: DescriptorBinding>() -> Option<vk::WriteDescriptorSet> {
        Self::get_descriptor_write::<T>()
    }

    fn get_descriptor_pool_sizes(num_sets: u32) -> Vec<vk::DescriptorPoolSize> {
        Self::get_descriptor_pool_sizes(num_sets)
    }
}

impl DescriptorBinding for CameraMatrices {
    fn get_descriptor_set_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
        vk::DescriptorSetLayoutBinding {
            binding: binding,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX,
            p_immutable_samplers: std::ptr::null(),
        }
    }

    fn get_descriptor_write(binding: u32) -> vk::WriteDescriptorSet {
        vk::WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            ..Default::default()
        }
    }

    fn get_descriptor_pool_size(num_sets: u32) -> vk::DescriptorPoolSize {
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1 * num_sets,
        }
    }
}

impl DescriptorBinding for Texture2D {
    fn get_descriptor_set_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
        vk::DescriptorSetLayoutBinding {
            binding: binding,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            p_immutable_samplers: std::ptr::null(),
        }
    }

    fn get_descriptor_write(binding: u32) -> vk::WriteDescriptorSet {
        vk::WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            ..Default::default()
        }
    }

    fn get_descriptor_pool_size(num_sets: u32) -> vk::DescriptorPoolSize {
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1 * num_sets,
        }
    }
}

pub type CameraDescriptorSet =
    DescriptorLayoutBuilder<DescriptorBindingNode<CameraMatrices, DescriptorBindingTerminator>>;
pub type TextureDescriptorSet =
    DescriptorLayoutBuilder<DescriptorBindingNode<Texture2D, DescriptorBindingTerminator>>;

pub struct DescriptorSetLayout<T: DescriptorLayout> {
    pub(super) layout: vk::DescriptorSetLayout,
    _phantom: PhantomData<T>,
}

pub struct DescriptorPool<T: DescriptorLayout> {
    pub count: usize,
    pool: vk::DescriptorPool,
    sets: Vec<vk::DescriptorSet>,
    _phantom: PhantomData<T>,
}

impl<T: DescriptorLayout> Index<usize> for DescriptorPool<T> {
    type Output = vk::DescriptorSet;

    fn index(&self, index: usize) -> &Self::Output {
        &self.sets[index]
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

impl<T: DescriptorLayout> DescriptorPool<T> {
    pub fn get_writer(&self) -> DescriptorSetWriter<T> {
        DescriptorSetWriter {
            num_sets: self.sets.len(),
            writes: vec![],
            bufer_writes: vec![],
            image_writes: vec![],
            _phantom: PhantomData,
        }
    }
}

impl<T: DescriptorLayout> DescriptorSetWriter<T> {
    pub(super) fn write_buffer<U: Pod + DescriptorBinding, O: Operation>(
        mut self,
        buffer: &UniformBuffer<U, O>,
    ) -> Self {
        let write = T::get_descriptor_write::<U>().expect(&format!(
            "Invalid DescriptorBinding type {} for descriptor layout {}",
            type_name::<U>(),
            type_name::<T>()
        ));
        let num_uniforms = self.num_sets * write.descriptor_count as usize;
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
        let set_write_stride = write.descriptor_count as usize;
        self.writes
            .extend((0..self.num_sets).map(|set_index| SetWrite::Buffer {
                set_index,
                buffer_write_index: buffer_write_base_index + set_index * set_write_stride,
                write,
            }));
        self
    }

    pub(super) fn write_image<'a, I>(mut self, textures: &'a [I]) -> Self
    where
        I: DescriptorBinding,
        &'a I: Into<&'a Texture2D>,
    {
        let write = T::get_descriptor_write::<I>().expect(&format!(
            "Invalid DescriptorBinding type {} for descriptor layout {}",
            type_name::<I>(),
            type_name::<T>()
        ));
        let num_uniforms = self.num_sets * write.descriptor_count as usize;
        debug_assert_eq!(
            num_uniforms,
            textures.len(),
            "Not enough image for DescriptorPool write!"
        );
        let iamge_write_base_index = self.image_writes.len();
        self.image_writes.extend(textures.iter().map(|texture| {
            let texture = texture.into();
            vk::DescriptorImageInfo {
                sampler: texture.sampler,
                image_view: texture.image.image_view,
                image_layout: texture.image.layout,
            }
        }));
        let set_write_stride = write.descriptor_count as usize;
        self.writes
            .extend((0..self.num_sets).map(|set_index| SetWrite::Image {
                set_index,
                image_write_index: iamge_write_base_index + set_index * set_write_stride,
                write,
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
        _builder: T,
        num_sets: usize,
    ) -> Result<DescriptorPool<T>, Box<dyn Error>> {
        let pool_sizes = T::get_descriptor_pool_sizes(num_sets as u32);
        let pool_create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(num_sets as u32);
        let pool = unsafe {
            self.device
                .create_descriptor_pool(&pool_create_info, None)?
        };
        let layout = self.get_descriptor_set_layout::<T>()?;
        let sets = unsafe {
            self.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&vec![layout.layout; num_sets]),
            )?
        };
        Ok(DescriptorPool {
            count: sets.len(),
            pool,
            sets,
            _phantom: PhantomData,
        })
    }

    pub fn write_descriptor_sets<T: DescriptorLayout>(
        &self,
        pool: &mut DescriptorPool<T>,
        writer: DescriptorSetWriter<T>,
    ) {
        let DescriptorSetWriter {
            writes,
            bufer_writes,
            image_writes,
            ..
        } = writer;
        let writes = writes
            .into_iter()
            .map(|write| match write {
                SetWrite::Buffer {
                    set_index,
                    buffer_write_index,
                    write,
                } => vk::WriteDescriptorSet {
                    dst_set: pool.sets[set_index],
                    p_buffer_info: &bufer_writes[buffer_write_index],
                    ..write
                },
                SetWrite::Image {
                    set_index,
                    image_write_index,
                    write,
                } => vk::WriteDescriptorSet {
                    dst_set: pool.sets[set_index],
                    p_image_info: &image_writes[image_write_index],
                    ..write
                },
            })
            .collect::<Vec<_>>();
        unsafe { self.device.update_descriptor_sets(&writes, &[]) }
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

    pub fn destroy_descriptor_pool<T: DescriptorLayout>(&self, pool: &mut DescriptorPool<T>) {
        unsafe {
            self.device.destroy_descriptor_pool(pool.pool, None);
        };
    }
}
