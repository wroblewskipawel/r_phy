use ash::prelude::VkResult;
use ash::{vk, Device};
use bytemuck::Pod;
use std::error::Error;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Index;
use std::sync::Once;

use crate::renderer::camera::Camera;

use super::buffer::UniformBuffer;
use super::command::operation::Operation;
use super::VulkanDevice;

pub trait DescriptorLayout {
    fn get_descriptor_set_bindings() -> &'static [vk::DescriptorSetLayoutBinding];

    // DescriptorSetLayout should be made separate structure
    // so that it would be possible to create layout by composing multiple DescriptorBindings types
    // similar as it is done currently (as kind of proof of concept) for PipelineLayout
    fn get_descriptor_set_layout(device: &Device) -> VkResult<vk::DescriptorSetLayout> {
        unsafe {
            static mut LAYOUT: Option<VkResult<vk::DescriptorSetLayout>> = None;
            // Why bother with thread safe creation of set layout when destroying is in plain unsafe code?
            static INIT: Once = Once::new();
            INIT.call_once(|| {
                if LAYOUT.is_none() {
                    LAYOUT.replace(
                        device.create_descriptor_set_layout(
                            &vk::DescriptorSetLayoutCreateInfo::builder()
                                .bindings(Self::get_descriptor_set_bindings()),
                            None,
                        ),
                    );
                }
            });
            LAYOUT.unwrap()
        }
    }

    fn get_descriptor_write() -> vk::WriteDescriptorSet;
}

impl DescriptorLayout for Camera {
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
    ) -> Result<DescriptorPool<T>, Box<dyn Error>> {
        let pool_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
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

    pub fn destory_descriptor_set_layout<T: DescriptorLayout>(&self) {
        unsafe {
            if let Ok(layout) = T::get_descriptor_set_layout(&self.device) {
                self.device.destroy_descriptor_set_layout(layout, None);
            }
        };
    }

    pub fn destory_descriptor_pool<T: DescriptorLayout>(&self, pool: &mut DescriptorPool<T>) {
        unsafe {
            self.device.destroy_descriptor_pool(pool.pool, None);
        };
    }
}
