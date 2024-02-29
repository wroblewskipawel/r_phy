use ash::{extensions::khr::Swapchain, vk, Instance};
use std::{error::Error, ffi::CStr};

use crate::renderer::vulkan::surface::{PhysicalDeviceSurfaceProperties, VulkanSurface};

use super::{image::VulkanImage2D, render_pass::VulkanRenderPass, VulkanDevice};

struct SwapchainSync {
    draw_ready: Vec<vk::Semaphore>,
    draw_finished: Vec<vk::Semaphore>,
    image_available: Vec<vk::Fence>,
}

pub struct VulkanSwapchain {
    pub image_extent: vk::Extent2D,
    sync: SwapchainSync,
    depth_buffer: VulkanImage2D,
    _images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    handle: vk::SwapchainKHR,
    loader: Swapchain,
}

impl VulkanSwapchain {
    pub const fn required_extensions() -> &'static [&'static CStr; 1] {
        const REQUIRED_DEVICE_EXTENSIONS: &[&CStr; 1] = &[Swapchain::name()];
        REQUIRED_DEVICE_EXTENSIONS
    }
}

impl VulkanDevice {
    pub fn create_swapchain(
        &self,
        instance: &Instance,
        surface: &VulkanSurface,
        render_pass: &VulkanRenderPass,
    ) -> Result<VulkanSwapchain, Box<dyn Error>> {
        let PhysicalDeviceSurfaceProperties {
            capabilities:
                vk::SurfaceCapabilitiesKHR {
                    min_image_count,
                    max_image_count,
                    current_extent,
                    max_image_extent,
                    current_transform,
                    ..
                },
            surface_format,
            present_mode,
            ..
        } = self.physical_device.surface_properties;
        let min_image_count = (min_image_count + 1).clamp(
            0,
            match max_image_count {
                0 => u32::MAX,
                _ => max_image_count,
            },
        );
        let image_extent = vk::Extent2D {
            width: current_extent.width.clamp(0, max_image_extent.width),
            height: current_extent.height.clamp(0, max_image_extent.height),
        };
        let queue_family_indices = [self.physical_device.queue_families.graphics];
        let create_info = vk::SwapchainCreateInfoKHR::builder()
            .pre_transform(current_transform)
            .image_extent(image_extent)
            .min_image_count(min_image_count)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .present_mode(present_mode)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .queue_family_indices(&queue_family_indices)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .clipped(true)
            .image_array_layers(1)
            .surface(surface.into());
        let loader = Swapchain::new(&instance, &self.device);
        let handle = unsafe { loader.create_swapchain(&create_info, None)? };
        let depth_buffer = self.create_image(
            image_extent,
            self.physical_device.attachment_formats.depth_stencil,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        let images = unsafe { loader.get_swapchain_images(handle)? };
        let image_views = images
            .iter()
            .map(|&image| unsafe {
                self.device.create_image_view(
                    &vk::ImageViewCreateInfo::builder()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping::default())
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let framebuffers = image_views
            .iter()
            .map(|&image_view| unsafe {
                self.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::builder()
                        .render_pass(render_pass.into())
                        .attachments(&VulkanRenderPass::get_attachments(
                            image_view,
                            (&depth_buffer).into(),
                        ))
                        .width(image_extent.width)
                        .height(image_extent.height)
                        .layers(1),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sync = self.create_swapchain_sync(images.len())?;
        Ok(VulkanSwapchain {
            image_extent,
            sync,
            depth_buffer,
            _images: images,
            image_views,
            framebuffers,
            loader,
            handle,
        })
    }

    pub fn destroy_swapchain(&self, swapchain: &mut VulkanSwapchain) {
        unsafe {
            swapchain
                .framebuffers
                .iter()
                .for_each(|&framebuffer| self.device.destroy_framebuffer(framebuffer, None));
            swapchain
                .image_views
                .iter()
                .for_each(|&image_view| self.device.destroy_image_view(image_view, None));
            swapchain.loader.destroy_swapchain(swapchain.handle, None);
            self.destory_image(&mut swapchain.depth_buffer);
            self.destory_swapchain_sync(&mut swapchain.sync);
        }
    }

    fn create_swapchain_sync(&self, num_images: usize) -> Result<SwapchainSync, Box<dyn Error>> {
        let draw_ready = (0..num_images)
            .map(|_| unsafe {
                self.device.create_semaphore(
                    &vk::SemaphoreCreateInfo {
                        ..Default::default()
                    },
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let draw_finished = (0..num_images)
            .map(|_| unsafe {
                self.device.create_semaphore(
                    &vk::SemaphoreCreateInfo {
                        ..Default::default()
                    },
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let image_available = (0..num_images)
            .map(|_| unsafe {
                self.device.create_fence(
                    &vk::FenceCreateInfo {
                        ..Default::default()
                    },
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SwapchainSync {
            draw_ready,
            draw_finished,
            image_available,
        })
    }

    fn destory_swapchain_sync(&self, sync: &mut SwapchainSync) {
        unsafe {
            sync.draw_ready
                .iter()
                .for_each(|&semaphore| self.device.destroy_semaphore(semaphore, None));
            sync.draw_finished
                .iter()
                .for_each(|&semaphore| self.device.destroy_semaphore(semaphore, None));
            sync.image_available
                .iter()
                .for_each(|&fence| self.device.destroy_fence(fence, None));
        }
    }
}
