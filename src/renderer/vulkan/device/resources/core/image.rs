use crate::renderer::model::Image;
use crate::renderer::vulkan::device::memory::{AllocReq, Allocator, DeviceLocal, MemoryProperties};
use crate::renderer::vulkan::device::VulkanDevice;

use ash::vk;
use png::{self, BitDepth, ColorType, Transformations};
use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::usize;
use std::{borrow::Borrow, error::Error, path::Path};
use strum::IntoEnumIterator;

use super::buffer::StagingBufferBuilder;
use super::{FromPartial, Partial, PartialBuilder};

#[derive(Debug, Clone, Copy)]
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
}

pub struct VulkanImageBuilder<M: MemoryProperties> {
    info: VulkanImageInfo,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> PartialBuilder for VulkanImageBuilder<M> {
    type Partial = VulkanImagePartial<M>;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>> {
        let info = self.info;
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
        Ok(VulkanImagePartial { image, info, req })
    }
}

impl<M: MemoryProperties> VulkanImageBuilder<M> {
    fn new(info: VulkanImageInfo) -> Self {
        Self {
            info,
            _phantom: PhantomData,
        }
    }
}

pub struct VulkanImagePartial<M: MemoryProperties> {
    image: vk::Image,
    info: VulkanImageInfo,
    req: AllocReq<M>,
}

impl<M: MemoryProperties> Partial for VulkanImagePartial<M> {
    type Memory = M;

    fn requirements(&self) -> AllocReq<M> {
        self.req
    }
}

impl<M: MemoryProperties, A: Allocator> FromPartial for VulkanImage2D<M, A> {
    type Partial<'a> = VulkanImagePartial<M>;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let VulkanImagePartial { image, info, req } = partial;
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
        Ok(VulkanImage2D {
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

struct PngImageReader<'a, R: Read> {
    reader: png::Reader<R>,
    phantom: PhantomData<&'a R>,
}

impl PngImageReader<'_, File> {
    fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let mut decoder = png::Decoder::new(File::open(path)?);
        decoder.set_transformations(
            Transformations::EXPAND | Transformations::ALPHA | Transformations::STRIP_16,
        );
        Ok(Self {
            reader: decoder.read_info()?,
            phantom: PhantomData,
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
            phantom: PhantomData,
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

impl PartialBuilder for ImageCubeReader {
    type Partial = CubeMapPartial;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>> {
        let image = VulkanImageBuilder::new(self.info()?).prepare(device)?;
        Ok(CubeMapPartial {
            image,
            reader: self,
        })
    }
}

pub struct CubeMapPartial {
    image: VulkanImagePartial<DeviceLocal>,
    reader: ImageCubeReader,
}

impl Partial for CubeMapPartial {
    type Memory = DeviceLocal;

    fn requirements(&self) -> AllocReq<DeviceLocal> {
        self.image.req
    }
}

pub struct CubeMap<A: Allocator> {
    texture: Texture2D<A>,
}

impl<'a, A: Allocator> From<&'a CubeMap<A>> for &'a Texture2D<A> {
    fn from(cubemap: &'a CubeMap<A>) -> Self {
        &cubemap.texture
    }
}

impl<'a, A: Allocator> From<&'a mut CubeMap<A>> for &'a mut Texture2D<A> {
    fn from(cubemap: &'a mut CubeMap<A>) -> Self {
        &mut cubemap.texture
    }
}

impl<A: Allocator> FromPartial for CubeMap<A> {
    type Partial<'a> = CubeMapPartial;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let CubeMapPartial { image, reader } = partial;
        let mut image = VulkanImage2D::finalize(image, device, allocator)?;
        let mut builder = StagingBufferBuilder::new();
        let image_range = builder.append::<u8>(reader.required_buffer_size());
        {
            let mut staging_buffer = device.create_stagging_buffer(builder)?;
            reader.iter().try_for_each(|(face, reader)| {
                let mut image_range = staging_buffer.write_range::<u8>(image_range);
                reader.read(image_range.remaining_as_slice_mut())?;
                staging_buffer.transfer_image_data(
                    &mut image,
                    face as u32,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                )
            })?;
            image.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        }
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
        let sampler = unsafe { device.create_sampler(&create_info, None)? };
        Ok(CubeMap {
            texture: Texture2D { image, sampler },
        })
    }
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

pub struct VulkanImage2D<M: MemoryProperties, A: Allocator> {
    pub array_layers: u32,
    pub mip_levels: u32,
    pub layout: vk::ImageLayout,
    pub extent: vk::Extent2D,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    memory: A::Allocation<M>,
}

impl VulkanDevice {
    pub fn create_color_attachment_image<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<VulkanImage2D<DeviceLocal, A>, Box<dyn Error>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        let partial = VulkanImageBuilder::new(VulkanImageInfo {
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
        })
        .prepare(self)?;
        let image = VulkanImage2D::finalize(partial, self, allocator)?;
        Ok(image)
    }

    pub fn create_depth_stencil_attachment_image<A: Allocator>(
        &self,
        allocator: &mut A,
    ) -> Result<VulkanImage2D<DeviceLocal, A>, Box<dyn Error>> {
        let extent = self.physical_device.surface_properties.get_current_extent();
        let partial = VulkanImageBuilder::new(VulkanImageInfo {
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
        })
        .prepare(self)?;
        let image = VulkanImage2D::finalize(partial, self, allocator)?;
        Ok(image)
    }

    pub fn destroy_image<'a, M: MemoryProperties, A: Allocator>(
        &self,
        image: impl Into<&'a mut VulkanImage2D<M, A>>,
        allocator: &mut A,
    ) {
        let image = image.into();
        unsafe {
            self.device.destroy_image_view(image.image_view, None);
            self.device.destroy_image(image.image, None);
            allocator.free(self, &mut image.memory);
        }
    }
}

enum ImageReader<'a> {
    File(PngImageReader<'a, File>),
    Buffer(PngImageReader<'a, &'a [u8]>),
}

impl<'a> ImageReader<'a> {
    fn required_buffer_size(&self) -> usize {
        match self {
            ImageReader::File(reader) => reader.required_buffer_size(),
            ImageReader::Buffer(reader) => reader.required_buffer_size(),
        }
    }

    fn info(&self) -> Result<VulkanImageInfo, Box<dyn Error>> {
        match self {
            ImageReader::File(reader) => reader.info(),
            ImageReader::Buffer(reader) => reader.info(),
        }
    }

    fn read(self, dst: &mut [u8]) -> Result<(), Box<dyn Error>> {
        match self {
            ImageReader::File(reader) => reader.read(dst),
            ImageReader::Buffer(reader) => reader.read(dst),
        }
    }
}

pub struct Texture2DPartial<'a> {
    image: VulkanImagePartial<DeviceLocal>,
    reader: ImageReader<'a>,
}

impl<'a> Texture2DPartial<'a> {
    pub fn get_alloc_req(&self) -> AllocReq<DeviceLocal> {
        self.image.req
    }
}

pub struct Texture2D<A: Allocator> {
    pub image: VulkanImage2D<DeviceLocal, A>,
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

impl<'a> PartialBuilder for &'a Image {
    type Partial = Texture2DPartial<'a>;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>> {
        let reader = match self {
            Image::File(path) => ImageReader::File(PngImageReader::from_file(path)?),
            Image::Buffer(data) => ImageReader::Buffer(PngImageReader::from_buffer(data)?),
        };
        let image = VulkanImageBuilder::new(reader.info()?).prepare(device)?;
        Ok(Texture2DPartial { image, reader })
    }
}

impl<'a> Partial for Texture2DPartial<'a> {
    type Memory = DeviceLocal;

    fn requirements(&self) -> AllocReq<DeviceLocal> {
        self.image.req
    }
}

impl<A: Allocator> FromPartial for Texture2D<A> {
    type Partial<'a> = Texture2DPartial<'a>;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let Texture2DPartial { image, reader } = partial;
        let mut image = VulkanImage2D::finalize(image, device, allocator)?;
        let mut builder = StagingBufferBuilder::new();
        let image_range = builder.append::<u8>(reader.required_buffer_size());
        {
            let mut staging_buffer = device.create_stagging_buffer(builder)?;
            let mut image_range = staging_buffer.write_range::<u8>(image_range);
            reader.read(image_range.remaining_as_slice_mut())?;
            staging_buffer.transfer_image_data(
                &mut image,
                0,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )?;
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

impl VulkanDevice {
    pub fn load_texture<A: Allocator>(
        &self,
        allocator: &mut A,
        image: &Image,
    ) -> Result<Texture2D<A>, Box<dyn Error>> {
        Texture2D::finalize(image.prepare(self)?, self, allocator)
    }

    pub fn load_cubemap<A: Allocator>(
        &self,
        allocator: &mut A,
        path: &Path,
    ) -> Result<CubeMap<A>, Box<dyn Error>> {
        CubeMap::finalize(
            ImageCubeReader::prepare(path)?.prepare(self)?,
            self,
            allocator,
        )
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
