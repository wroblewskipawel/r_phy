use ash::{extensions::khr::Swapchain, vk, Instance};
use std::{error::Error, ffi::CStr};

use crate::renderer::{
    camera::CameraMatrices,
    vulkan::surface::{PhysicalDeviceSurfaceProperties, VulkanSurface},
};

use super::{
    buffer::UniformBuffer,
    command::{
        level::{Primary, Secondary},
        operation::Graphics,
        BeginCommand, NewCommand, Persistent, PersistentCommandPool, SubmitSemaphoreState,
    },
    descriptor::{CameraDescriptorSet, Descriptor, DescriptorPool},
    framebuffer::{
        presets::{AttachmentsGBuffer, ColorMultisampled, DepthStencilMultisampled, Resolve},
        AttachmentsBuilder, Framebuffer, FramebufferHandle,
    },
    image::VulkanImage2D,
    render_pass::{DeferedRenderPass, RenderPassConfig},
    VulkanDevice,
};

pub struct FrameSync {
    draw_ready: vk::Semaphore,
    draw_finished: vk::Semaphore,
}

pub struct SwapchainFrame {
    pub framebuffer: FramebufferHandle<AttachmentsGBuffer>,
    pub render_area: vk::Rect2D,
    pub camera_descriptor: Descriptor<CameraDescriptorSet>,
    image_index: usize,
    sync: FrameSync,
}

pub struct GBufer {
    pub combined: VulkanImage2D,
    pub albedo: VulkanImage2D,
    pub normal: VulkanImage2D,
    pub position: VulkanImage2D,
    pub depth: VulkanImage2D,
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
    command_pool_primary: PersistentCommandPool<Primary, Graphics>,
    command_pool_secondary: PersistentCommandPool<Secondary, Graphics>,
    camera_uniform_buffer: UniformBuffer<CameraMatrices, Graphics>,
    camera_descriptors: DescriptorPool<CameraDescriptorSet>,
    sync: SwapchainSync,
    pub g_buffer: GBufer,
    _images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    pub framebuffers: Vec<Framebuffer<AttachmentsGBuffer>>,
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
    pub fn create_swapchain<C: RenderPassConfig>(
        &self,
        instance: &Instance,
        surface: &VulkanSurface,
    ) -> Result<VulkanSwapchain, Box<dyn Error>> {
        let PhysicalDeviceSurfaceProperties {
            capabilities:
                vk::SurfaceCapabilitiesKHR {
                    current_transform, ..
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
        let g_buffer = GBufer {
            albedo: self.create_color_attachment_image()?,
            normal: self.create_color_attachment_image()?,
            position: self.create_color_attachment_image()?,
            combined: self.create_color_attachment_image()?,
            depth: self.create_depth_stencil_attachment_image()?,
        };
        let framebuffers = image_views
            .iter()
            .map(|&image_view| {
                self.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                    AttachmentsBuilder::new()
                        .push::<Resolve>(image_view)
                        .push::<DepthStencilMultisampled>(g_buffer.depth.image_view)
                        .push::<ColorMultisampled>(g_buffer.position.image_view)
                        .push::<ColorMultisampled>(g_buffer.normal.image_view)
                        .push::<ColorMultisampled>(g_buffer.albedo.image_view)
                        .push::<ColorMultisampled>(g_buffer.combined.image_view),
                    image_extent,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sync = self.create_swapchain_sync(images.len())?;
        let command_pool_primary = self.create_persistent_command_pool(images.len())?;
        let command_pool_secondary = self.create_persistent_command_pool(3 * images.len())?;
        let camera_uniform_buffer = self.create_uniform_buffer(images.len())?;
        let mut camera_descriptors =
            self.create_descriptor_pool(CameraDescriptorSet::builder(), images.len())?;
        let descriptor_write = camera_descriptors
            .get_writer()
            .write_buffer(&camera_uniform_buffer);
        self.write_descriptor_sets(&mut camera_descriptors, descriptor_write);
        Ok(VulkanSwapchain {
            image_extent,
            command_pool_primary,
            command_pool_secondary,
            camera_descriptors,
            camera_uniform_buffer,
            sync,
            g_buffer,
            _images: images,
            image_views,
            framebuffers,
            loader,
            handle,
        })
    }

    pub fn destroy_swapchain(&self, swapchain: &mut VulkanSwapchain) {
        swapchain
            .framebuffers
            .iter_mut()
            .for_each(|framebuffer| self.destroy_framebuffer(framebuffer));
        unsafe {
            swapchain
                .image_views
                .iter()
                .for_each(|&image_view| self.device.destroy_image_view(image_view, None));
            swapchain.loader.destroy_swapchain(swapchain.handle, None);
            // Add the following lines to the destroy_swapchain method
            self.destroy_image(&mut swapchain.g_buffer.albedo);
            self.destroy_image(&mut swapchain.g_buffer.normal);
            self.destroy_image(&mut swapchain.g_buffer.position);
            self.destroy_image(&mut swapchain.g_buffer.depth);
            self.destroy_image(&mut swapchain.g_buffer.combined);
            // End of the added lines
            self.destroy_swapchain_sync(&mut swapchain.sync);
            self.destroy_persistent_command_pool(&mut swapchain.command_pool_primary);
            self.destroy_persistent_command_pool(&mut swapchain.command_pool_secondary);
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
    ) -> (
        NewCommand<Persistent, Primary, Graphics>,
        NewCommand<Persistent, Secondary, Graphics>,
        NewCommand<Persistent, Secondary, Graphics>,
        NewCommand<Persistent, Secondary, Graphics>,
        FrameSync,
    ) {
        let (frame_index, primary_command) = swapchain.command_pool_primary.next();
        let (_, depth_display_command) = swapchain.command_pool_secondary.next();
        let (_, depth_prepass_command) = swapchain.command_pool_secondary.next();
        let (_, color_pass_command) = swapchain.command_pool_secondary.next();
        (
            primary_command,
            depth_prepass_command,
            depth_display_command,
            color_pass_command,
            swapchain.sync.get_frame(frame_index),
        )
    }

    pub fn begin_frame(
        &self,
        swapchain: &mut VulkanSwapchain,
        camera: &CameraMatrices,
    ) -> Result<
        (
            BeginCommand<Persistent, Primary, Graphics>,
            NewCommand<Persistent, Secondary, Graphics>,
            NewCommand<Persistent, Secondary, Graphics>,
            NewCommand<Persistent, Secondary, Graphics>,
            SwapchainFrame,
        ),
        Box<dyn Error>,
    > {
        let (
            primary_command,
            depth_prepass_command,
            color_pass_command,
            depth_display_command,
            sync,
        ) = self.get_next_frame_data(swapchain);
        let primary_command = self.begin_persistent_command(primary_command)?;
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
        let framebuffer = (&swapchain.framebuffers[image_index]).into();
        let camera_descriptor = swapchain.camera_descriptors[image_index];
        let render_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.image_extent,
        };
        swapchain.camera_uniform_buffer[image_index] = *camera;
        Ok((
            primary_command,
            depth_prepass_command,
            color_pass_command,
            depth_display_command,
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
        command: BeginCommand<Persistent, Primary, Graphics>,
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
