use ash::{extensions::khr::Swapchain, vk, Instance};
use std::{error::Error, ffi::CStr};

use crate::renderer::vulkan::surface::{PhysicalDeviceSurfaceProperties, VulkanSurface};

use super::{
    command::CommandPool, image::VulkanImage2D, render_pass::VulkanRenderPass, VulkanDevice,
};

pub struct FrameSync {
    frame_available: vk::Fence,
    draw_ready: vk::Semaphore,
    draw_finished: vk::Semaphore,
}

pub struct Frame {
    pub command_buffer: vk::CommandBuffer,
    pub framebuffer: vk::Framebuffer,
    pub render_area: vk::Rect2D,
    image_index: usize,
    sync: FrameSync,
}

struct SwapchainSync {
    draw_ready: Vec<vk::Semaphore>,
    draw_finished: Vec<vk::Semaphore>,
    frame_available: Vec<vk::Fence>,
}

impl SwapchainSync {
    fn get_frame(&self, index: usize) -> FrameSync {
        FrameSync {
            draw_ready: self.draw_ready[index],
            draw_finished: self.draw_finished[index],
            frame_available: self.frame_available[index],
        }
    }
}

pub struct VulkanSwapchain {
    pub image_extent: vk::Extent2D,
    command_pool: CommandPool,
    sync: SwapchainSync,
    depth_buffer: VulkanImage2D,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    handle: vk::SwapchainKHR,
    loader: Swapchain,
    next_frame_index: usize,
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
        let command_pool = self.create_command_pool(
            self.physical_device.queue_families.graphics,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            images.len(),
        )?;
        Ok(VulkanSwapchain {
            image_extent,
            command_pool,
            sync,
            depth_buffer,
            images,
            image_views,
            framebuffers,
            loader,
            handle,
            next_frame_index: 0,
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
            self.destory_command_pool(&mut swapchain.command_pool);
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
        let frame_available = (0..num_images)
            .map(|_| unsafe {
                self.device.create_fence(
                    &vk::FenceCreateInfo {
                        flags: vk::FenceCreateFlags::SIGNALED,
                        ..Default::default()
                    },
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SwapchainSync {
            draw_ready,
            draw_finished,
            frame_available,
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
            sync.frame_available
                .iter()
                .for_each(|&fence| self.device.destroy_fence(fence, None));
        }
    }

    fn get_next_frame_data(
        &self,
        swapchain: &mut VulkanSwapchain,
    ) -> (vk::CommandBuffer, FrameSync) {
        let frame_index = swapchain.next_frame_index;
        swapchain.next_frame_index += 1;
        swapchain.next_frame_index %= swapchain.images.len();
        (
            swapchain.command_pool.buffers[frame_index],
            swapchain.sync.get_frame(frame_index),
        )
    }

    pub fn begin_frame<'a>(
        &self,
        swapchain: &'a mut VulkanSwapchain,
    ) -> Result<Frame, Box<dyn Error>> {
        let (command_buffer, sync) = self.get_next_frame_data(swapchain);
        let image_index = unsafe {
            self.device
                .wait_for_fences(&[sync.frame_available], true, u64::MAX)?;
            self.device.reset_fences(&[sync.frame_available])?;
            let image_index = swapchain
                .loader
                .acquire_next_image(
                    swapchain.handle,
                    u64::MAX,
                    sync.draw_ready,
                    vk::Fence::null(),
                )
                .map(|(image_index, _)| image_index as usize)?;
            image_index
        };
        let framebuffer = swapchain.framebuffers[image_index];
        unsafe {
            self.device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            )?;
        }
        let render_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.image_extent,
        };
        Ok(Frame {
            command_buffer,
            framebuffer,
            render_area,
            image_index,
            sync,
        })
    }

    pub fn end_frame(
        &self,
        swapchain: &mut VulkanSwapchain,
        frame: Frame,
    ) -> Result<(), Box<dyn Error>> {
        let Frame {
            command_buffer,
            image_index,
            sync,
            ..
        } = frame;
        unsafe {
            self.device.end_command_buffer(command_buffer)?;
            self.device.queue_submit(
                self.device_queues.graphics,
                &[vk::SubmitInfo {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: [sync.draw_ready].as_ptr(),
                    p_wait_dst_stage_mask: [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT]
                        .as_ptr(),
                    command_buffer_count: 1,
                    p_command_buffers: [command_buffer].as_ptr(),
                    signal_semaphore_count: 1,
                    p_signal_semaphores: [sync.draw_finished].as_ptr(),
                    ..Default::default()
                }],
                sync.frame_available,
            )?;
            swapchain.loader.queue_present(
                self.device_queues.graphics,
                &vk::PresentInfoKHR {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: [sync.draw_finished].as_ptr(),
                    swapchain_count: 1,
                    p_swapchains: [swapchain.handle].as_ptr(),
                    p_image_indices: [image_index as u32].as_ptr(),
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }
}
