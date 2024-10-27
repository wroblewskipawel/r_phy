mod reader;
mod texture;

use crate::{
    device::{
        memory::{AllocReq, AllocReqTyped, Allocator, DeviceLocal, MemoryProperties},
        Device,
    },
    error::{VkError, VkResult},
};

use super::PartialBuilder;
use ash::vk;
use std::{convert::Infallible, marker::PhantomData};
use type_kit::{Create, Destroy, DestroyResult};

pub use reader::*;
pub use texture::*;

#[derive(Debug, Clone, Copy)]
struct Image2DInfo {
    extent: vk::Extent2D,
    format: vk::Format,
    flags: vk::ImageCreateFlags,
    samples: vk::SampleCountFlags,
    usage: vk::ImageUsageFlags,
    aspect_mask: vk::ImageAspectFlags,
    view_type: vk::ImageViewType,
    array_layers: u32,
    mip_levels: u32,
}

pub struct Image2DBuilder<M: MemoryProperties> {
    info: Image2DInfo,
    _phantom: PhantomData<M>,
}

impl<'a, M: MemoryProperties> PartialBuilder<'a> for Image2DPartial<M> {
    type Config = Image2DBuilder<M>;
    type Target<A: Allocator> = Image2D<M, A>;

    fn prepare(config: Self::Config, device: &Device) -> VkResult<Self> {
        let info = config.info;
        let image_info = vk::ImageCreateInfo::builder()
            .flags(info.flags)
            .extent(vk::Extent3D {
                width: info.extent.width,
                height: info.extent.height,
                depth: 1,
            })
            .format(info.format)
            .image_type(vk::ImageType::TYPE_2D)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .mip_levels(info.mip_levels)
            .array_layers(info.array_layers)
            .samples(info.samples)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(info.usage);
        let image = unsafe { device.create_image(&image_info, None)? };
        let req = device.get_alloc_req(image);
        Ok(Image2DPartial { image, info, req })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        [self.req.into()].into_iter()
    }
}

impl<M: MemoryProperties> Image2DBuilder<M> {
    fn new(info: Image2DInfo) -> Self {
        Self {
            info,
            _phantom: PhantomData,
        }
    }
}

pub struct Image2DPartial<M: MemoryProperties> {
    image: vk::Image,
    info: Image2DInfo,
    req: AllocReqTyped<M>,
}

pub struct Image2D<M: MemoryProperties, A: Allocator> {
    pub array_layers: u32,
    pub mip_levels: u32,
    pub layout: vk::ImageLayout,
    pub extent: vk::Extent2D,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    memory: A::Allocation<M>,
}

impl Device {
    pub fn create_color_attachment_image<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> VkResult<Image2D<DeviceLocal, A>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        let partial = Image2DPartial::prepare(
            Image2DBuilder::new(Image2DInfo {
                extent,
                format: self.physical_device.attachment_properties.formats.color,
                flags: vk::ImageCreateFlags::empty(),
                samples: self.physical_device.attachment_properties.msaa_samples,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT,
                aspect_mask: vk::ImageAspectFlags::COLOR,
                view_type: vk::ImageViewType::TYPE_2D,
                array_layers: 1,
                mip_levels: 1,
            }),
            self,
        )?;
        Image2D::create(partial, (self, allocator))
    }

    pub fn create_depth_stencil_attachment_image<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> VkResult<Image2D<DeviceLocal, A>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        let partial = Image2DPartial::prepare(
            Image2DBuilder::new(Image2DInfo {
                extent,
                format: self
                    .physical_device
                    .attachment_properties
                    .formats
                    .depth_stencil,
                flags: vk::ImageCreateFlags::empty(),
                samples: self.physical_device.attachment_properties.msaa_samples,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT,
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                view_type: vk::ImageViewType::TYPE_2D,
                array_layers: 1,
                mip_levels: 1,
            }),
            self,
        )?;
        Image2D::create(partial, (self, allocator))
    }
}

impl<M: MemoryProperties, A: Allocator> Create for Image2D<M, A> {
    type Config<'a> = Image2DPartial<M>;
    type CreateError = VkError;

    fn create<'a, 'b>(
        config: Self::Config<'a>,
        context: Self::Context<'b>,
    ) -> type_kit::CreateResult<Self> {
        let (device, allocator) = context;
        let Image2DPartial { image, info, req } = config;
        let memory = allocator.allocate(device, req)?;
        device.bind_memory(image, &memory)?;
        let view_info = vk::ImageViewCreateInfo::builder()
            .components(vk::ComponentMapping::default())
            .format(info.format)
            .image(image)
            .view_type(info.view_type)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: info.aspect_mask,
                base_mip_level: 0,
                level_count: info.mip_levels,
                base_array_layer: 0,
                layer_count: info.array_layers,
            });
        let image_view = unsafe { device.create_image_view(&view_info, None)? };
        Ok(Image2D {
            array_layers: info.array_layers,
            mip_levels: info.mip_levels,
            layout: vk::ImageLayout::UNDEFINED,
            extent: info.extent,
            image,
            image_view,
            memory,
        })
    }
}

impl<M: MemoryProperties, A: Allocator> Destroy for Image2D<M, A> {
    type Context<'a> = (&'a Device, &'a mut A);
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        let (device, allocator) = context;
        unsafe {
            device.destroy_image_view(self.image_view, None);
            device.destroy_image(self.image, None);
            allocator.free(device, &mut self.memory);
        }
        Ok(())
    }
}
