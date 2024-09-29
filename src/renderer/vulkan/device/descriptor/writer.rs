use std::{any::type_name, error::Error, marker::PhantomData, mem::size_of};

use ash::vk;
use bytemuck::AnyBitPattern;

use crate::renderer::vulkan::device::{
    command::operation::Operation, memory::Allocator, resources::buffer::UniformBuffer,
    VulkanDevice,
};

use super::{Descriptor, DescriptorBinding, DescriptorLayout, DescriptorPool, DescriptorPoolData};

#[derive(Debug)]
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

#[derive(Debug)]
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

    pub fn write_buffer<U: AnyBitPattern + DescriptorBinding, O: Operation, A: Allocator>(
        mut self,
        buffer: &UniformBuffer<U, O, A>,
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
            num_uniforms,
            buffer.len(),
            "UniformBuffer object not large enough for DescriptorPool write!"
        );
        let buffer_write_base_index = self.bufer_writes.len();
        self.bufer_writes
            .extend((0..num_uniforms).map(|index| vk::DescriptorBufferInfo {
                buffer: buffer.handle(),
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
    // TODO: sets Vec of incorrect length could be passed here
    fn write_descriptors<T: DescriptorLayout>(
        &self,
        writer: DescriptorSetWriter<T>,
        sets: Vec<vk::DescriptorSet>,
    ) -> Vec<Descriptor<T>> {
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
                    dst_set: sets[set_index],
                    p_buffer_info: &bufer_writes[buffer_write_index],
                    ..write
                },
                SetWrite::Image {
                    set_index,
                    image_write_index,
                    write,
                } => vk::WriteDescriptorSet {
                    dst_set: sets[set_index],
                    p_image_info: &image_writes[image_write_index],
                    ..write
                },
            })
            .collect::<Vec<_>>();
        unsafe { self.device.update_descriptor_sets(&writes, &[]) };
        sets.into_iter()
            .map(|set| Descriptor {
                set,
                _phantom: PhantomData,
            })
            .collect()
    }

    pub fn create_descriptor_pool<T: DescriptorLayout>(
        &self,
        writer: DescriptorSetWriter<T>,
    ) -> Result<DescriptorPool<T>, Box<dyn Error>> {
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
            self.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&vec![layout.layout; writer.num_sets]),
            )?
        };
        let sets = self
            .write_descriptors(writer, sets)
            .into_iter()
            .map(Into::<vk::DescriptorSet>::into)
            .collect();
        Ok(DescriptorPool {
            data: DescriptorPoolData { pool, sets },
            _phantom: PhantomData,
        })
    }
}
