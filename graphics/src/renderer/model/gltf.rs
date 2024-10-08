use super::{CommonVertex, Mesh, PbrMaps, PbrMaterial};
use base64::Engine;
use gltf::{self, buffer, mesh::Mode, Gltf, Semantic};
use std::{error::Error, path::Path};

use math::types::{Vector2, Vector3, Vector4};
use crate::{
    renderer::model::Image,
};

#[derive(Debug, Clone, Copy, Default)]
struct VertexBuilder {
    pos: Option<Vector3>,
    normal: Option<Vector3>,
    tangent: Option<Vector4>,
    tex_coord: Option<Vector2>,
}

impl VertexBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn build(self) -> CommonVertex {
        CommonVertex {
            pos: self.pos.unwrap(),
            norm: self.normal.unwrap(),
            uv: self.tex_coord.unwrap(),
            tan: self.tangent.unwrap(),
            color: Vector3::zero(),
        }
    }

    fn with_pos(mut self, pos: Vector3) -> Self {
        self.pos = Some(pos);
        self
    }
    fn with_normal(mut self, normal: Vector3) -> Self {
        self.normal = Some(normal);
        self
    }
    fn with_tex_coord(mut self, tex_coord: Vector2) -> Self {
        self.tex_coord = Some(tex_coord);
        self
    }
    fn with_tangent(mut self, tangent: Vector4) -> Self {
        self.tangent = Some(tangent);
        self
    }
}

struct DocumentReader {
    document: gltf::Document,
    buffers: Vec<buffer::Data>,
}

#[derive(Debug, Clone, Copy)]
struct BufferView<'a> {
    stride: usize,
    buffer: &'a [u8],
}

#[derive(Debug)]
struct AttributeReader<'a> {
    count: usize,
    cursor: usize,
    element_size: usize,
    view: BufferView<'a>,
}

impl<'a> Iterator for AttributeReader<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.count > 0 {
            let data = &self.view.buffer[self.cursor..(self.cursor + self.element_size)];
            self.cursor += if self.view.stride > 0 {
                self.view.stride
            } else {
                self.element_size
            };
            self.count -= 1;
            Some(data)
        } else {
            None
        }
    }
}

impl DocumentReader {
    pub fn new(path: &Path) -> Result<Self, Box<dyn Error>> {
        let base = path.parent().unwrap_or(Path::new("./"));
        let Gltf { document, blob } = Gltf::open(path)?;
        let buffers = gltf::import_buffers(&document, Some(base), blob)?;
        Ok(Self { document, buffers })
    }

    fn get_view(&self, view: gltf::buffer::View) -> Result<BufferView, Box<dyn Error>> {
        let buffer = &self.buffers[view.buffer().index()];
        Ok(BufferView {
            stride: view.stride().unwrap_or(0),
            buffer: &buffer[view.offset()..(view.offset() + view.length())],
        })
    }

    fn get_accessor(&self, accessor: gltf::Accessor) -> Result<AttributeReader, Box<dyn Error>> {
        let view = self.get_view(accessor.view().unwrap())?;
        Ok(AttributeReader {
            view,
            count: accessor.count(),
            cursor: accessor.offset(),
            element_size: accessor.size(),
        })
    }

    fn get_primitive_data(
        &self,
        primitive: gltf::Primitive,
    ) -> Result<(Vec<u32>, Vec<CommonVertex>), Box<dyn Error>> {
        let mut reader = PrimitiveReaderBuilder::new().with_indices(
            self.get_accessor(
                primitive
                    .indices()
                    .ok_or_else::<Box<dyn Error>, _>(|| "Mesh indices not found".into())?,
            )?,
        );
        for (semantic, accessor) in primitive.attributes() {
            reader = reader.with_attribute(semantic, self.get_accessor(accessor)?)?;
        }
        reader.build()?.read()
    }

    fn get_mesh(&self, mesh: gltf::Mesh) -> Result<Mesh<CommonVertex>, Box<dyn Error>> {
        let mut indices = Vec::new();
        let mut vertices = Vec::new();
        for primitive in mesh.primitives() {
            if let Mode::Triangles = primitive.mode() {
                let (p_indices, p_vertices) = self.get_primitive_data(primitive)?;
                indices.push(p_indices);
                vertices.push(p_vertices);
            } else {
                // TODO: Should skip instaed of returning error
                Err("Only triangle list models are supported")?;
            }
        }
        Ok(Mesh {
            indices: indices.concat().into_boxed_slice(),
            vertices: vertices.concat().into_boxed_slice(),
        })
    }

    // TODO: Restore mime_type checkf for image format support
    fn get_image(&self, image: gltf::Image, base: &Path) -> Result<Image, Box<dyn Error>> {
        let image = match image.source() {
            gltf::image::Source::View { view, .. } => {
                // match mime_type {
                //     "image/png" => (),
                //     _ => Err("Unsupported image format")?,
                // }
                let view = self.get_view(view)?;
                Image::Buffer(view.buffer.to_vec())
            }
            gltf::image::Source::Uri { uri, .. } => {
                // match mime_type {
                //     Some("image/png") => (),
                //     _ => Err("Unsupported image format")?,
                // };
                if let Some(rest) = uri.strip_prefix("data:") {
                    let mut it = rest.split(";base64,");
                    let data = match (it.next(), it.next()) {
                        (_, Some(data)) => data,
                        (Some(data), None) => data,
                        _ => Err("Invalid data uri")?,
                    };
                    Image::Buffer(base64::engine::general_purpose::STANDARD.decode(data)?)
                } else {
                    Image::File(base.join(uri))
                }
            }
        };
        Ok(image)
    }

    fn get_material(
        &self,
        material: gltf::Material,
        base: &Path,
    ) -> Result<PbrMaterial, Box<dyn Error>> {
        let mut builder = PbrMaterial::builder();

        let pbr = material.pbr_metallic_roughness();
        builder = builder
            .with_base_color(pbr.base_color_factor().into())
            .with_metallic(pbr.metallic_factor())
            .with_roughness(pbr.roughness_factor());
        if let Some(base_color) = pbr.base_color_texture() {
            builder = builder.with_image(
                self.get_image(base_color.texture().source(), base)?,
                PbrMaps::Albedo,
            );
        };
        if let Some(metallic_roughness) = pbr.metallic_roughness_texture() {
            builder = builder.with_image(
                self.get_image(metallic_roughness.texture().source(), base)?,
                PbrMaps::MetallicRoughness,
            );
        };
        if let Some(normal) = material.normal_texture() {
            builder = builder.with_image(
                self.get_image(normal.texture().source(), base)?,
                PbrMaps::Normal,
            );
        };
        if let Some(occlusion) = material.occlusion_texture() {
            builder = builder.with_image(
                self.get_image(occlusion.texture().source(), base)?,
                PbrMaps::Occlusion,
            );
        };
        if let Some(emissive) = material.emissive_texture() {
            builder = builder.with_image(
                self.get_image(emissive.texture().source(), base)?,
                PbrMaps::Emissive,
            );
        };
        builder
            .with_emissive(material.emissive_factor().into())
            .build()
    }
}

#[derive(Debug)]
struct PrimitiveReader<'a> {
    pos: AttributeReader<'a>,
    norm: AttributeReader<'a>,
    uv: AttributeReader<'a>,
    tan: AttributeReader<'a>,
    indices: AttributeReader<'a>,
}

impl<'a> PrimitiveReader<'a> {
    fn read(mut self) -> Result<(Vec<u32>, Vec<CommonVertex>), Box<dyn Error>> {
        let mut indices = Vec::new();
        for bytes in self.indices {
            let bytes = <[u8; 2]>::try_from(bytes)?;
            indices.push(u16::from_le_bytes(bytes) as u32);
        }
        let mut vertices = Vec::new();
        // TODO: Refactior following code to dont have to check for missing vertex data
        for pos in self.pos.by_ref() {
            let normal = self
                .norm
                .next()
                .ok_or_else::<Box<dyn Error>, _>(|| "Missing normal data".into())?;
            let uv = self
                .uv
                .next()
                .ok_or_else::<Box<dyn Error>, _>(|| "Missing uv data".into())?;
            let tangent = self
                .tan
                .next()
                .ok_or_else::<Box<dyn Error>, _>(|| "Missing normal data".into())?;
            vertices.push(
                VertexBuilder::new()
                    .with_pos(Vector3::try_from_le_bytes(pos)?)
                    .with_normal(Vector3::try_from_le_bytes(normal)?)
                    .with_tex_coord(Vector2::try_from_le_bytes(uv)?)
                    .with_tangent(Vector4::try_from_le_bytes(tangent)?)
                    .build(),
            );
        }
        Ok((indices, vertices))
    }
}

#[derive(Debug, Default)]
struct PrimitiveReaderBuilder<'a> {
    pos: Option<AttributeReader<'a>>,
    norm: Option<AttributeReader<'a>>,
    uv: Option<AttributeReader<'a>>,
    tan: Option<AttributeReader<'a>>,
    indices: Option<AttributeReader<'a>>,
}

impl<'a> PrimitiveReaderBuilder<'a> {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    fn with_attribute(
        mut self,
        semantic: Semantic,
        reader: AttributeReader<'a>,
    ) -> Result<Self, Box<dyn Error>> {
        match semantic {
            Semantic::Positions => self.pos = Some(reader),
            Semantic::Normals => self.norm = Some(reader),
            Semantic::TexCoords(0) => self.uv = Some(reader),
            Semantic::Tangents => self.tan = Some(reader),
            _ => {
                Err("Unsupported semantic")?;
            }
        }
        Ok(self)
    }

    fn with_indices(mut self, reader: AttributeReader<'a>) -> Self {
        self.indices = Some(reader);
        self
    }

    fn build(self) -> Result<PrimitiveReader<'a>, Box<dyn Error>> {
        Ok(PrimitiveReader {
            pos: self.pos.ok_or("Missing position attribute")?,
            norm: self.norm.ok_or("Missing normal attribute")?,
            uv: self.uv.ok_or("Missing uv attribute")?,
            tan: self.tan.ok_or("Missing tangent attribute")?,
            indices: self.indices.ok_or("Missing vertex indices data")?,
        })
    }
}

impl Mesh<CommonVertex> {
    pub fn load_gltf(path: &Path) -> Result<(Mesh<CommonVertex>, PbrMaterial), Box<dyn Error>> {
        let base = path.parent().unwrap_or(Path::new("./"));
        let reader = DocumentReader::new(path)?;
        let mesh = reader.get_mesh(reader.document.meshes().next().ok_or("No mesh found")?)?;
        let material = reader.get_material(
            reader
                .document
                .materials()
                .next()
                .ok_or("No material found")?,
            base,
        )?;
        Ok((mesh, material))
    }
}
