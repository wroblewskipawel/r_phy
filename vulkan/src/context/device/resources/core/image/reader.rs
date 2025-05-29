use std::{borrow::Borrow, fs::File, io::Read, marker::PhantomData, path::Path};

use ash::vk;
use graphics::model::Image;
use png::{BitDepth, ColorType, Transformations};
use strum::IntoEnumIterator;

use crate::context::error::ImageError;

use super::Image2DInfo;

struct PngImageReader<'a, R: Read> {
    reader: png::Reader<R>,
    phantom: PhantomData<&'a R>,
}

impl PngImageReader<'_, File> {
    fn from_file(path: &Path) -> Result<Self, ImageError> {
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
    fn from_buffer(image_data: &'a [u8]) -> Result<Self, ImageError> {
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
    fn read(mut self, dst: &mut [u8]) -> Result<(), ImageError> {
        self.reader.next_frame(dst)?;
        Ok(())
    }

    fn info(&self) -> Result<Image2DInfo, ImageError> {
        let info = self.reader.info();
        let extent = vk::Extent2D {
            width: info.width,
            height: info.height,
        };
        let format = match self.reader.output_color_type() {
            (ColorType::Rgba, BitDepth::Eight) => vk::Format::R8G8B8A8_SRGB,
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => vk::Format::R8G8_SRGB,
            (color_type, bit_depth) => Err(ImageError::UnsupportedFormat(color_type, bit_depth))?,
        };
        let mip_levels = get_max_mip_level(extent);
        Ok(Image2DInfo {
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
pub enum ImageCubeFace {
    Right = 0,
    Left = 1,
    Top = 2,
    Bottom = 3,
    Front = 4,
    Back = 5,
}

impl ImageCubeFace {
    fn get(path: &Path) -> Result<Self, ImageError> {
        let stem = path.file_stem().unwrap().to_string_lossy();
        let face = match stem.borrow() {
            "right" => Self::Right,
            "left" => Self::Left,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "front" => Self::Front,
            "back" => Self::Back,
            _ => Err(ImageError::InvalidCubeMap(stem.into_owned()))?,
        };
        Ok(face)
    }
}

struct ImageCubeReader {
    faces: Vec<(ImageCubeFace, PngImageReader<'static, File>)>,
}

impl ImageCubeReader {
    fn prepare(path: &Path) -> Result<Self, ImageError> {
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
            .collect::<Result<Vec<_>, ImageError>>()?;
        if let Some(req) =
            ImageCubeFace::iter().find(|req| !faces.iter().any(|(face, _)| req == face))
        {
            Err(ImageError::MissingCubeMapData(req))?;
        }
        Ok(Self { faces })
    }

    fn info(&self) -> Result<Image2DInfo, ImageError> {
        let (_, reader) = &self.faces.first().ok_or(ImageError::ExhaustedImageRead)?;
        let info = reader.info()?;
        Ok(Image2DInfo {
            array_layers: 6,
            view_type: vk::ImageViewType::CUBE,
            flags: vk::ImageCreateFlags::CUBE_COMPATIBLE,
            ..info
        })
    }

    fn required_buffer_size(&self) -> Result<usize, ImageError> {
        let (_, reader) = &self.faces.first().ok_or(ImageError::ExhaustedImageRead)?;
        Ok(reader.required_buffer_size())
    }
}

pub struct ImageReader<'a> {
    reader: ImageReaderInner<'a>,
}

enum ImageReaderInner<'a> {
    File(Option<PngImageReader<'a, File>>),
    Buffer(Option<PngImageReader<'a, &'a [u8]>>),
    Cube(ImageCubeReader),
}

impl<'a> ImageReader<'a> {
    pub fn cube(path: &Path) -> Result<Self, ImageError> {
        let reader = ImageReaderInner::Cube(ImageCubeReader::prepare(path)?);
        Ok(Self { reader })
    }

    pub fn image(image: &'a Image) -> Result<Self, ImageError> {
        let reader = match image {
            Image::File(path) => ImageReaderInner::File(Some(PngImageReader::from_file(path)?)),
            Image::Buffer(data) => {
                ImageReaderInner::Buffer(Some(PngImageReader::from_buffer(data)?))
            }
        };
        Ok(Self { reader })
    }

    pub fn required_buffer_size(&self) -> Result<usize, ImageError> {
        match &self.reader {
            ImageReaderInner::File(reader) => {
                let required = reader
                    .as_ref()
                    .ok_or(ImageError::ExhaustedImageRead)?
                    .required_buffer_size();
                Ok(required)
            }
            ImageReaderInner::Buffer(reader) => {
                let required = reader
                    .as_ref()
                    .ok_or(ImageError::ExhaustedImageRead)?
                    .required_buffer_size();
                Ok(required)
            }
            ImageReaderInner::Cube(reader) => reader.required_buffer_size(),
        }
    }

    pub(super) fn info(&self) -> Result<Image2DInfo, ImageError> {
        match &self.reader {
            ImageReaderInner::File(reader) => reader
                .as_ref()
                .ok_or(ImageError::ExhaustedImageRead)?
                .info(),
            ImageReaderInner::Buffer(reader) => reader
                .as_ref()
                .ok_or(ImageError::ExhaustedImageRead)?
                .info(),
            ImageReaderInner::Cube(reader) => reader.info(),
        }
    }

    pub fn read(&mut self, dst: &mut [u8]) -> Result<Option<u32>, ImageError> {
        let dst_layer = match &mut self.reader {
            ImageReaderInner::File(reader) => reader
                .take()
                .and_then(|reader| Some(reader.read(dst).map(|()| 0))),
            ImageReaderInner::Buffer(reader) => reader
                .take()
                .and_then(|reader| Some(reader.read(dst).map(|()| 0))),
            ImageReaderInner::Cube(reader) => {
                reader.faces.pop().and_then(|(face_index, reader)| {
                    Some(reader.read(dst).map(|()| face_index as u32))
                })
            }
        };
        let dst_layer = if let Some(dst_layer) = dst_layer {
            Some(dst_layer?)
        } else {
            None
        };
        Ok(dst_layer)
    }
}
