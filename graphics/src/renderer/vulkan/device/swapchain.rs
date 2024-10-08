use ash::{
    extensions::khr::Swapchain,
    vk::{self, Extent2D, SurfaceFormatKHR},
};
use std::{error::Error, ffi::CStr};

use crate::renderer::vulkan::{core::Context, surface::PhysicalDeviceSurfaceProperties};

use super::{
    command::{
        level::Primary, operation::Graphics, FinishedCommand, Persistent, SubmitSemaphoreState,
    },
    framebuffer::{AttachmentList, Framebuffer, FramebufferHandle},
    VulkanDevice,
};
#[derive(Debug, Clone, Copy)]
pub struct SwapchainImageSync {
    draw_ready: vk::Semaphore,
    draw_finished: vk::Semaphore,
}

pub struct SwapchainFrame<A: AttachmentList> {
    pub framebuffer: FramebufferHandle<A>,
    pub render_area: vk::Rect2D,
    image_index: u32,
    image_sync: SwapchainImageSync,
}

struct SwapchainImage {
    _image: vk::Image,
    view: vk::ImageView,
}

pub struct VulkanSwapchain<A: AttachmentList> {
    pub num_images: usize,
    pub extent: vk::Extent2D,
    pub framebuffers: Vec<Framebuffer<A>>,
    images: Vec<SwapchainImage>,
    handle: vk::SwapchainKHR,
    loader: Swapchain,
}

pub const fn required_extensions() -> &'static [&'static CStr; 1] {
    const REQUIRED_DEVICE_EXTENSIONS: &[&CStr; 1] = &[Swapchain::name()];
    REQUIRED_DEVICE_EXTENSIONS
}

impl<A: AttachmentList> VulkanSwapchain<A> {
    pub fn get_frame(
        &self,
        image_sync: SwapchainImageSync,
    ) -> Result<SwapchainFrame<A>, Box<dyn Error>> {
        let (image_index, _) = unsafe {
            self.loader.acquire_next_image(
                self.handle,
                u64::MAX,
                image_sync.draw_ready,
                vk::Fence::null(),
            )?
        };
        let framebuffer = (&self.framebuffers[image_index as usize]).into();
        let render_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.extent,
        };
        Ok(SwapchainFrame {
            framebuffer,
            render_area,
            image_index,
            image_sync,
        })
    }
}

impl VulkanDevice {
    pub fn present_frame<A: AttachmentList>(
        &self,
        swapchain: &VulkanSwapchain<A>,
        command: FinishedCommand<Persistent, Primary, Graphics>,
        frame: SwapchainFrame<A>,
    ) -> Result<(), Box<dyn Error>> {
        let SwapchainFrame {
            image_index,
            image_sync,
            ..
        } = frame;
        unsafe {
            self.submit_command(
                command,
                SubmitSemaphoreState {
                    semaphores: &[image_sync.draw_ready],
                    masks: &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                },
                &[image_sync.draw_finished],
            )?;
            swapchain.loader.queue_present(
                self.device_queues.graphics,
                &vk::PresentInfoKHR {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: [image_sync.draw_finished].as_ptr(),
                    swapchain_count: 1,
                    p_swapchains: [swapchain.handle].as_ptr(),
                    p_image_indices: [image_index].as_ptr(),
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }
}

impl Context {
    pub fn create_swapchain_image_sync<A: AttachmentList>(
        &self,
        swapchain: &VulkanSwapchain<A>,
    ) -> Result<Vec<SwapchainImageSync>, Box<dyn Error>> {
        unsafe {
            swapchain
                .images
                .iter()
                .map(|_| {
                    let create_info = vk::SemaphoreCreateInfo::default();
                    let draw_ready = self.device.create_semaphore(&create_info, None)?;
                    let draw_finished = self.device.create_semaphore(&create_info, None)?;
                    Ok(SwapchainImageSync {
                        draw_ready,
                        draw_finished,
                    })
                })
                .collect()
        }
    }

    pub fn destroy_swapchain_image_sync(&self, sync: &mut [SwapchainImageSync]) {
        unsafe {
            sync.iter_mut().for_each(|sync| {
                self.device.destroy_semaphore(sync.draw_ready, None);
                self.device.destroy_semaphore(sync.draw_finished, None);
            });
        }
    }

    pub fn create_swapchain<A: AttachmentList>(
        &self,
        framebuffer_builder: impl Fn(vk::ImageView, Extent2D) -> Result<Framebuffer<A>, Box<dyn Error>>,
    ) -> Result<VulkanSwapchain<A>, Box<dyn Error>> {
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
            .surface((&self.surface).into());
        let loader = Swapchain::new(&self.instance, &self.device);
        let handle = unsafe { loader.create_swapchain(&create_info, None)? };
        let images = unsafe {
            loader
                .get_swapchain_images(handle)?
                .into_iter()
                .map(|image| self.create_swapchain_image(image, surface_format))
                .collect::<Result<Vec<_>, _>>()?
        };
        let framebuffers = images
            .iter()
            .map(|image| framebuffer_builder(image.view, image_extent))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VulkanSwapchain {
            num_images: images.len(),
            extent: image_extent,
            images,
            framebuffers,
            loader,
            handle,
        })
    }

    fn create_swapchain_image(
        &self,
        image: vk::Image,
        surface_format: SurfaceFormatKHR,
    ) -> Result<SwapchainImage, Box<dyn Error>> {
        unsafe {
            let view = self.device.create_image_view(
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
            )?;

            Ok(SwapchainImage {
                _image: image,
                view,
            })
        }
    }

    pub fn destroy_swapchain<A: AttachmentList>(&self, swapchain: &mut VulkanSwapchain<A>) {
        swapchain.framebuffers.iter_mut().for_each(|framebuffer| {
            self.destroy_framebuffer(framebuffer);
        });
        unsafe {
            swapchain
                .images
                .iter_mut()
                .for_each(|image| self.device.destroy_image_view(image.view, None));
            swapchain.loader.destroy_swapchain(swapchain.handle, None);
        }
    }
}
