use ash::{vk, Device};
use bytemuck::Pod;
use std::any::TypeId;
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

// DescriptorSetLayout should be made separate structure
// so that it would be possible to create layout by composing multiple DescriptorBindings types
// similar as it is done currently (as kind of proof of concept) for PipelineLayout
pub trait DescriptorLayout
where
    Self: 'static,
{
    fn get_descriptor_set_bindings() -> &'static [vk::DescriptorSetLayoutBinding];

    fn get_descriptor_write() -> vk::WriteDescriptorSet;

    fn get_descriptor_set_layout(
        device: &Device,
    ) -> Result<vk::DescriptorSetLayout, Box<dyn Error>> {
        let layout_map = get_descriptor_set_layout_map();
        let layout = if let Some(layout) = {
            let layout_map_reader = layout_map.read()?;
            layout_map_reader.get(&TypeId::of::<Self>()).copied()
        } {
            layout
        } else {
            let mut layout_map_writer = layout_map.try_write()?;
            let layout = unsafe {
                device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::builder()
                        .bindings(Self::get_descriptor_set_bindings()),
                    None,
                )?
            };
            layout_map_writer.insert(TypeId::of::<Self>(), layout);
            layout
        };
        Ok(layout)
    }
}

impl DescriptorLayout for CameraMatrices {
    fn get_descriptor_set_bindings() -> &'static [vk::DescriptorSetLayoutBinding] {
        // To support for creation of DescriptorLayout by composition of multiple DescriptorBindings types
        // binding values should be dynamically computed for each such composition
        const BINDINGS: &[vk::DescriptorSetLayoutBinding] = &[vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX,
            p_immutable_samplers: std::ptr::null(),
        }];
        BINDINGS
    }

    fn get_descriptor_write() -> vk::WriteDescriptorSet {
        vk::WriteDescriptorSet {
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            ..Default::default()
        }
    }
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

impl VulkanDevice {
    pub fn create_descriptor_pool<T: DescriptorLayout>(
        &self,
        pool_size: usize,
        ty: vk::DescriptorType,
    ) -> Result<DescriptorPool<T>, Box<dyn Error>> {
        let pool_sizes = [vk::DescriptorPoolSize {
            ty,
            descriptor_count: pool_size as u32,
        }];
        let pool_create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(pool_size as u32);
        let pool = unsafe {
            self.device
                .create_descriptor_pool(&pool_create_info, None)?
        };
        let layout = T::get_descriptor_set_layout(&self.device)?;
        let sets = unsafe {
            self.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&vec![layout; pool_size]),
            )?
        };
        Ok(DescriptorPool {
            count: sets.len(),
            pool,
            sets,
            _phantom: PhantomData,
        })
    }

    // In its current form it would be hard to gracefully handle ImageSampler descriptors writes
    // Consider creating custom DescriptorWrite trait would implement Into<vk::WriteDescriptorSet>
    // so that all writes can be contained by single Vector<Box<dyn DescriptorWrite>>
    // (or compile time equivalent) which would resolve to Vec<vk::WriteDescriptorSet>, correctly
    // handling writes for both buffers and images
    pub(super) fn write_descriptor_sets<T: DescriptorLayout, U: Pod, O: Operation>(
        &self,
        pool: &DescriptorPool<T>,
        buffer: &UniformBuffer<U, O>,
    ) {
        // Writting all descriptors at one works fine
        // whenthere is need for single descriptor for each frame
        // later this would change
        // Alternatively DescriptorLayout and UniformBuffer could be
        // composed to single structure, that both owns uniform buffer memory
        // and keeps collection of associated descriptor sets
        // (what is the runtime impact for maintaining such
        // collection of "prepared" descriptors?)
        debug_assert_eq!(
            pool.sets.len(),
            buffer.size,
            "UniformBuffer object to small for DescriptorPool write!"
        );
        let buffer_writes = (0..pool.sets.len())
            .map(|index| vk::DescriptorBufferInfo {
                buffer: buffer.as_raw(),
                offset: (size_of::<U>() * index) as vk::DeviceSize,
                range: size_of::<U>() as vk::DeviceSize,
            })
            .collect::<Vec<_>>();
        let descriptor_writes = pool
            .sets
            .iter()
            .enumerate()
            .map(|(index, &set)| vk::WriteDescriptorSet {
                dst_set: set,
                p_buffer_info: &buffer_writes[index],
                ..T::get_descriptor_write()
            })
            .collect::<Vec<_>>();
        unsafe { self.device.update_descriptor_sets(&descriptor_writes, &[]) }
    }

    pub fn write_image_samplers(
        &self,
        pool: &mut DescriptorPool<Texture2D>,
        textures: &[Texture2D],
    ) {
        debug_assert_eq!(
            pool.sets.len(),
            textures.len(),
            "Not enough Texture2D provided for DescriptorPool write!"
        );
        let image_writes = textures
            .iter()
            .map(|texture| vk::DescriptorImageInfo {
                sampler: texture.sampler,
                image_view: texture.image.image_view,
                image_layout: texture.image.layout,
            })
            .collect::<Vec<_>>();
        let descriptor_writes = pool
            .sets
            .iter()
            .enumerate()
            .map(|(index, &set)| vk::WriteDescriptorSet {
                dst_set: set,
                p_image_info: &image_writes[index],
                ..Texture2D::get_descriptor_write()
            })
            .collect::<Vec<_>>();
        unsafe { self.device.update_descriptor_sets(&descriptor_writes, &[]) }
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
