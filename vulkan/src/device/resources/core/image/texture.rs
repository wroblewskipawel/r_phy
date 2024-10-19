use std::error::Error;

use ash::vk;

use crate::device::{
    memory::{AllocReq, Allocator, DeviceLocal},
    resources::{buffer::StagingBufferBuilder, PartialBuilder},
    Device,
};

use super::{Image2D, Image2DBuilder, Image2DPartial, ImageReader};

pub struct Texture2DPartial<'a> {
    image: Image2DPartial<DeviceLocal>,
    reader: ImageReader<'a>,
}

pub struct Texture2D<A: Allocator> {
    pub image: Image2D<DeviceLocal, A>,
    pub sampler: vk::Sampler,
}

impl<A: Allocator> From<&Texture2D<A>> for vk::DescriptorImageInfo {
    fn from(texture: &Texture2D<A>) -> Self {
        vk::DescriptorImageInfo {
            sampler: texture.sampler,
            image_view: texture.image.image_view,
            image_layout: texture.image.layout,
        }
    }
}

impl<'a> PartialBuilder<'a> for Texture2DPartial<'a> {
    type Config = ImageReader<'a>;
    type Target<A: Allocator> = Texture2D<A>;

    fn prepare(config: Self::Config, device: &Device) -> Result<Self, Box<dyn Error>> {
        let image = Image2DPartial::prepare(Image2DBuilder::new(config.info()?), device)?;
        Ok(Texture2DPartial {
            image,
            reader: config,
        })
    }

    fn requirements(&self) -> impl Iterator<Item = AllocReq> {
        self.image.requirements()
    }

    fn finalize<A: Allocator>(
        self,
        device: &Device,
        allocator: &mut A,
    ) -> Result<Self::Target<A>, Box<dyn Error>> {
        let Texture2DPartial { image, mut reader } = self;
        let mut image = image.finalize(device, allocator)?;
        let mut builder = StagingBufferBuilder::new();
        let image_range = builder.append::<u8>(reader.required_buffer_size());
        {
            let mut staging_buffer = device.create_stagging_buffer(builder)?;
            let mut image_range = staging_buffer.write_range::<u8>(image_range);
            let staging_area = image_range.remaining_as_slice_mut();
            while let Some(dst_layer) = reader.read(staging_area)? {
                staging_buffer.transfer_image_data(
                    &mut image,
                    dst_layer,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                )?;
            }
            image.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        }
        let create_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .min_lod(0.0)
            .max_lod(image.mip_levels as f32);
        let sampler = unsafe { device.create_sampler(&create_info, None)? };
        Ok(Texture2D { image, sampler })
    }
}

impl Device {
    pub fn load_texture<'a, A: Allocator>(
        &self,
        allocator: &mut A,
        image: ImageReader<'a>,
    ) -> Result<Texture2D<A>, Box<dyn Error>> {
        Texture2DPartial::prepare(image, self)?.finalize(self, allocator)
    }

    pub fn destroy_texture<'a, A: Allocator>(
        &self,
        texture: impl Into<&'a mut Texture2D<A>>,
        allocator: &mut A,
    ) {
        let texture = texture.into();
        unsafe {
            self.device.destroy_sampler(texture.sampler, None);
            self.destroy_image(&mut texture.image, allocator);
        }
    }
}
