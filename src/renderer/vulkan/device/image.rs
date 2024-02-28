use super::VulkanDevice;
use ash::vk;
use std::error::Error;

pub struct VulkanImage2D {
    image_view: vk::ImageView,
    image: vk::Image,
    device_memory: vk::DeviceMemory,
}

impl Into<vk::ImageView> for &VulkanImage2D {
    fn into(self) -> vk::ImageView {
        self.image_view
    }
}

impl VulkanDevice {
    pub fn create_image(
        &self,
        extent: vk::Extent2D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect_mask: vk::ImageAspectFlags,
        memory_properties: vk::MemoryPropertyFlags,
    ) -> Result<VulkanImage2D, Box<dyn Error>> {
        let queue_family_indices = [self.physical_device.queue_families.graphics];
        let image_info = vk::ImageCreateInfo::builder()
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .format(format)
            .image_type(vk::ImageType::TYPE_2D)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage);
        let (image_view, image, device_memory) = unsafe {
            let image = self.device.create_image(&image_info, None)?;
            let memory_requirements = self.device.get_image_memory_requirements(image);
            let allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(memory_requirements.size)
                .memory_type_index(
                    self.get_memory_type_index(
                        memory_requirements.memory_type_bits,
                        memory_properties,
                    )
                    .ok_or("Failed to find suitable memory type for image!")?,
                );
            let device_memory = self.device.allocate_memory(&allocate_info, None)?;
            self.device.bind_image_memory(image, device_memory, 0)?;
            let view_info = vk::ImageViewCreateInfo::builder()
                .components(vk::ComponentMapping::default())
                .format(format)
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            let image_view = self.device.create_image_view(&view_info, None)?;
            (image_view, image, device_memory)
        };
        Ok(VulkanImage2D {
            image_view,
            image,
            device_memory,
        })
    }

    pub fn destory_image(&self, image: &mut VulkanImage2D) {
        unsafe {
            self.device.destroy_image_view(image.image_view, None);
            self.device.destroy_image(image.image, None);
            self.device.free_memory(image.device_memory, None);
        }
    }
}
