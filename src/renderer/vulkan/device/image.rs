use crate::renderer::model::Image;

use super::{buffer::StagingBufferBuilder, VulkanDevice};
use ash::vk;
use png::{self, BitDepth, ColorType, Transformations};
use std::fs::File;
use std::io::Read;
use std::usize;
use std::{borrow::Borrow, error::Error, path::Path};
use strum::IntoEnumIterator;

struct VulkanImageInfo {
    extent: vk::Extent2D,
    format: vk::Format,
    flags: vk::ImageCreateFlags,
    samples: vk::SampleCountFlags,
    usage: vk::ImageUsageFlags,
    aspect_mask: vk::ImageAspectFlags,
    view_type: vk::ImageViewType,
    array_layers: u32,
    mip_levels: u32,
    memory_properties: vk::MemoryPropertyFlags,
}

struct PngImageReader<'a, R: Read> {
    reader: png::Reader<R>,
    phantom: std::marker::PhantomData<&'a R>,
}

impl PngImageReader<'_, File> {
    fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let mut decoder = png::Decoder::new(File::open(path)?);
        decoder.set_transformations(
            Transformations::EXPAND | Transformations::ALPHA | Transformations::STRIP_16,
        );
        Ok(Self {
            reader: decoder.read_info()?,
            phantom: std::marker::PhantomData,
        })
    }
}

impl<'a> PngImageReader<'a, &'a [u8]> {
    fn from_buffer(image_data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        let mut decoder = png::Decoder::new(image_data);
        decoder.set_transformations(
            Transformations::EXPAND | Transformations::ALPHA | Transformations::STRIP_16,
        );
        Ok(Self {
            reader: decoder.read_info()?,
            phantom: std::marker::PhantomData,
        })
    }
}

fn get_max_mip_level(extent: vk::Extent2D) -> u32 {
    u32::max(extent.width, extent.height).ilog2() + 1
}

impl<'a, R: Read> PngImageReader<'a, R> {
    fn read(mut self, dst: &mut [u8]) -> Result<(), Box<dyn Error>> {
        self.reader.next_frame(dst)?;
        Ok(())
    }

    fn info(&self) -> Result<VulkanImageInfo, Box<dyn Error>> {
        let info = self.reader.info();
        let extent = vk::Extent2D {
            width: info.width,
            height: info.height,
        };
        let format = match self.reader.output_color_type() {
            (ColorType::Rgba, BitDepth::Eight) => vk::Format::R8G8B8A8_SRGB,
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => vk::Format::R8G8_SRGB,
            (color_type, bit_depth) => Err(format!(
                "Unsupported png Image ColorType: {:?} and BitDepth: {:?}!",
                color_type, bit_depth
            ))?,
        };
        let mip_levels = get_max_mip_level(extent);
        Ok(VulkanImageInfo {
            extent,
            format,
            mip_levels,
            flags: vk::ImageCreateFlags::empty(),
            samples: vk::SampleCountFlags::TYPE_1,
            usage: vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST,
            aspect_mask: vk::ImageAspectFlags::COLOR,
            view_type: vk::ImageViewType::TYPE_2D,
            array_layers: 1,
            memory_properties: vk::MemoryPropertyFlags::DEVICE_LOCAL,
        })
    }

    fn required_buffer_size(&self) -> usize {
        self.reader.output_buffer_size()
    }
}

#[derive(strum::EnumIter, Debug, Clone, Copy, PartialEq)]
enum ImageCubeFace {
    Right = 0,
    Left = 1,
    Top = 2,
    Bottom = 3,
    Front = 4,
    Back = 5,
}

impl ImageCubeFace {
    fn get(path: &Path) -> Result<Self, Box<dyn Error>> {
        let stem = path.file_stem().unwrap().to_string_lossy();
        let face = match stem.borrow() {
            "right" => Self::Right,
            "left" => Self::Left,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "front" => Self::Front,
            "back" => Self::Back,
            _ => Err(format!("`{}` is not valid ImageCube entry!", stem))?,
        };
        Ok(face)
    }
}

struct ImageCubeReader {
    faces: Vec<(ImageCubeFace, PngImageReader<'static, File>)>,
}

impl ImageCubeReader {
    fn prepare(path: &Path) -> Result<Self, Box<dyn Error>> {
        let faces = path
            .read_dir()?
            .filter_map(|entry| entry.map(|entry| entry.path()).ok())
            .filter(|path| path.is_file())
            .map(|path| {
                Ok((
                    ImageCubeFace::get(&path)?,
                    PngImageReader::from_file(&path)?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        if let Some(req) =
            ImageCubeFace::iter().find(|req| !faces.iter().any(|(face, _)| req == face))
        {
            Err(format!("Missing {:?} CubeMap data!", req))?;
        }
        Ok(Self { faces })
    }

    fn info(&self) -> Result<VulkanImageInfo, Box<dyn Error>> {
        let (_, reader) = &self.faces[ImageCubeFace::Right as usize];
        let info = reader.info()?;
        Ok(VulkanImageInfo {
            array_layers: 6,
            view_type: vk::ImageViewType::CUBE,
            flags: vk::ImageCreateFlags::CUBE_COMPATIBLE,
            ..info
        })
    }

    fn required_buffer_size(&self) -> usize {
        let (_, reader) = &self.faces[ImageCubeFace::Right as usize];
        reader.required_buffer_size()
    }

    fn iter(self) -> impl Iterator<Item = (ImageCubeFace, PngImageReader<'static, File>)> {
        let Self { faces } = self;
        faces.into_iter()
    }
}

pub struct VulkanImage2D {
    pub array_layers: u32,
    pub mip_levels: u32,
    pub layout: vk::ImageLayout,
    pub extent: vk::Extent2D,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    device_memory: vk::DeviceMemory,
}

impl VulkanDevice {
    fn create_image(&self, info: VulkanImageInfo) -> Result<VulkanImage2D, Box<dyn Error>> {
        let VulkanImageInfo {
            extent,
            format,
            flags,
            samples,
            usage,
            aspect_mask,
            view_type,
            array_layers,
            mip_levels,
            memory_properties,
        } = info;
        let image_info = vk::ImageCreateInfo::builder()
            .flags(flags)
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .format(format)
            .image_type(vk::ImageType::TYPE_2D)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .mip_levels(mip_levels)
            .array_layers(array_layers)
            .samples(samples)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
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
                .view_type(view_type)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask,
                    base_mip_level: 0,
                    level_count: mip_levels,
                    base_array_layer: 0,
                    layer_count: array_layers,
                });
            let image_view = self.device.create_image_view(&view_info, None)?;
            (image_view, image, device_memory)
        };
        Ok(VulkanImage2D {
            array_layers,
            mip_levels,
            layout: vk::ImageLayout::UNDEFINED,
            extent,
            image,
            image_view,
            device_memory,
        })
    }

    pub fn create_color_attachment_image(&self) -> Result<VulkanImage2D, Box<dyn Error>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        self.create_image(VulkanImageInfo {
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
            memory_properties: vk::MemoryPropertyFlags::DEVICE_LOCAL,
        })
    }

    pub fn create_depth_stencil_attachment_image(&self) -> Result<VulkanImage2D, Box<dyn Error>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        self.create_image(VulkanImageInfo {
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
            memory_properties: vk::MemoryPropertyFlags::DEVICE_LOCAL,
        })
    }

    pub fn destroy_image(&self, image: &mut VulkanImage2D) {
        unsafe {
            self.device.destroy_image_view(image.image_view, None);
            self.device.destroy_image(image.image, None);
            self.device.free_memory(image.device_memory, None);
        }
    }
}

pub struct Texture2D {
    pub image: VulkanImage2D,
    pub sampler: vk::Sampler,
}

impl From<&Texture2D> for vk::DescriptorImageInfo {
    fn from(texture: &Texture2D) -> Self {
        vk::DescriptorImageInfo {
            sampler: texture.sampler,
            image_view: texture.image.image_view,
            image_layout: texture.image.layout,
        }
    }
}

impl VulkanDevice {
    pub fn load_texture(&self, image: &Image) -> Result<Texture2D, Box<dyn Error>> {
        match image {
            Image::File(path) => self.load_texture_impl(PngImageReader::from_file(path)?),
            Image::Buffer(data) => self.load_texture_impl(PngImageReader::from_buffer(data)?),
        }
    }

    fn load_texture_impl<R: Read>(
        &self,
        image_reader: PngImageReader<R>,
    ) -> Result<Texture2D, Box<dyn Error>> {
        let mut image = self.create_image(image_reader.info()?)?;

        let mut builder = StagingBufferBuilder::new();
        let image_range = builder.append::<u8>(image_reader.required_buffer_size());
        let mut staging_buffer = self.create_stagging_buffer(builder)?;
        let mut image_range = staging_buffer.write_range::<u8>(image_range);
        image_reader.read(image_range.remaining_as_slice_mut())?;
        staging_buffer.transfer_image_data(
            &mut image,
            0,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        image.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        let create_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .min_lod(0.0)
            .max_lod(image.mip_levels as f32);
        let sampler = unsafe { self.device.create_sampler(&create_info, None)? };
        Ok(Texture2D { image, sampler })
    }

    pub fn load_cubemap(&self, path: &Path) -> Result<Texture2D, Box<dyn Error>> {
        let cube_reader = ImageCubeReader::prepare(path)?;
        let mut image = self.create_image(cube_reader.info()?)?;

        let mut builder = StagingBufferBuilder::new();
        let image_range = builder.append::<u8>(cube_reader.required_buffer_size());
        let mut staging_buffer = self.create_stagging_buffer(builder)?;
        cube_reader.iter().try_for_each(|(face, reader)| {
            let mut image_range = staging_buffer.write_range::<u8>(image_range);
            reader.read(image_range.remaining_as_slice_mut())?;
            staging_buffer.transfer_image_data(
                &mut image,
                face as u32,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        })?;
        image.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        let create_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .min_lod(0.0)
            .max_lod(image.mip_levels as f32);
        let sampler = unsafe { self.device.create_sampler(&create_info, None)? };
        Ok(Texture2D { image, sampler })
    }

    pub fn destroy_texture(&self, texture: &mut Texture2D) {
        unsafe {
            self.device.destroy_sampler(texture.sampler, None);
            self.destroy_image(&mut texture.image);
        }
    }
}
