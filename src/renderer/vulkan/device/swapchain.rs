use ash::{extensions::khr::Swapchain, vk, Instance};
use std::{error::Error, ffi::CStr};

use crate::renderer::{
    camera::CameraMatrices,
    vulkan::surface::{PhysicalDeviceSurfaceProperties, VulkanSurface},
};

use super::{
    buffer::UniformBuffer,
    command::{
        operation::Graphics, BeginCommand, NewCommand, Persistent, PersistentCommandPool,
        SubmitSemaphoreState,
    },
    descriptor::DescriptorPool,
    image::VulkanImage2D,
    render_pass::VulkanRenderPass,
    VulkanDevice,
};

pub struct FrameSync {
    draw_ready: vk::Semaphore,
    draw_finished: vk::Semaphore,
}

pub struct SwapchainFrame {
    pub framebuffer: vk::Framebuffer,
    pub render_area: vk::Rect2D,
    pub camera_descriptor: vk::DescriptorSet,
    image_index: usize,
    sync: FrameSync,
}

struct SwapchainSync {
    draw_ready: Vec<vk::Semaphore>,
    draw_finished: Vec<vk::Semaphore>,
}

impl SwapchainSync {
    fn get_frame(&self, index: usize) -> FrameSync {
        FrameSync {
            draw_ready: self.draw_ready[index],
            draw_finished: self.draw_finished[index],
        }
    }
}

pub struct VulkanSwapchain {
    pub image_extent: vk::Extent2D,
    command_pool: PersistentCommandPool<Graphics>,
    camera_uniform_buffer: UniformBuffer<CameraMatrices, Graphics>,
    camera_descriptors: DescriptorPool<CameraMatrices>,
    sync: SwapchainSync,
    depth_buffer: VulkanImage2D,
    color_buffer: VulkanImage2D,
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
        let min_image_count = self.physical_device.surface_properties.get_image_count();
        let image_extent = self.physical_device.surface_properties.get_current_extent();
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
        let loader = Swapchain::new(instance, &self.device);
        let handle = unsafe { loader.create_swapchain(&create_info, None)? };
        let depth_buffer = self.create_depth_stencil_attachment_image()?;
        let color_buffer = self.create_color_attachment_image()?;
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
                            depth_buffer.image_view,
                            color_buffer.image_view,
                        ))
                        .width(image_extent.width)
                        .height(image_extent.height)
                        .layers(1),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sync = self.create_swapchain_sync(images.len())?;
        let command_pool = self.create_persistent_command_pool(images.len())?;
        let camera_uniform_buffer = self.create_uniform_buffer(images.len())?;
        let camera_descriptors =
            self.create_descriptor_pool(images.len(), vk::DescriptorType::UNIFORM_BUFFER)?;
        self.write_descriptor_sets(&camera_descriptors, &camera_uniform_buffer);
        Ok(VulkanSwapchain {
            image_extent,
            command_pool,
            camera_descriptors,
            camera_uniform_buffer,
            sync,
            depth_buffer,
            color_buffer,
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
            self.destroy_image(&mut swapchain.depth_buffer);
            self.destroy_image(&mut swapchain.color_buffer);
            self.destroy_swapchain_sync(&mut swapchain.sync);
            self.destroy_persistent_command_pool(&mut swapchain.command_pool);
            self.destroy_uniform_buffer(&mut swapchain.camera_uniform_buffer);
            self.destroy_descriptor_pool(&mut swapchain.camera_descriptors);
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
        Ok(SwapchainSync {
            draw_ready,
            draw_finished,
        })
    }

    fn destroy_swapchain_sync(&self, sync: &mut SwapchainSync) {
        unsafe {
            sync.draw_ready
                .iter()
                .for_each(|&semaphore| self.device.destroy_semaphore(semaphore, None));
            sync.draw_finished
                .iter()
                .for_each(|&semaphore| self.device.destroy_semaphore(semaphore, None));
        }
    }

    fn get_next_frame_data(
        &self,
        swapchain: &mut VulkanSwapchain,
    ) -> (NewCommand<Persistent, Graphics>, FrameSync) {
        let (frame_index, command) = swapchain.command_pool.next();
        (command, swapchain.sync.get_frame(frame_index))
    }

    pub fn begin_frame(
        &self,
        swapchain: &mut VulkanSwapchain,
        camera: &CameraMatrices,
    ) -> Result<(BeginCommand<Persistent, Graphics>, SwapchainFrame), Box<dyn Error>> {
        let (command, sync) = self.get_next_frame_data(swapchain);
        let command = self.begin_persistent_command(command)?;
        let image_index = unsafe {
            swapchain
                .loader
                .acquire_next_image(
                    swapchain.handle,
                    u64::MAX,
                    sync.draw_ready,
                    vk::Fence::null(),
                )
                .map(|(image_index, _)| image_index as usize)?
        };
        let framebuffer = swapchain.framebuffers[image_index];
        let camera_descriptor = swapchain.camera_descriptors[image_index];
        let render_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.image_extent,
        };
        swapchain.camera_uniform_buffer[image_index] = *camera;
        Ok((
            command,
            SwapchainFrame {
                framebuffer,
                camera_descriptor,
                render_area,
                image_index,
                sync,
            },
        ))
    }

    pub fn end_frame(
        &self,
        swapchain: &mut VulkanSwapchain,
        command: BeginCommand<Persistent, Graphics>,
        frame: SwapchainFrame,
    ) -> Result<(), Box<dyn Error>> {
        let SwapchainFrame {
            image_index, sync, ..
        } = frame;
        unsafe {
            self.finish_command(command)?.submit(
                SubmitSemaphoreState {
                    semaphores: &[sync.draw_ready],
                    masks: &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                },
                &[sync.draw_finished],
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
