// Temporary allow for too_many_arguments
// handling render commands will be significantly revamped in the future
// which makes it not worth the effort to refactor this code
#![allow(clippy::too_many_arguments)]

use std::{error::Error, marker::PhantomData};

use crate::{
    math::types::Matrix4,
    renderer::{
        camera::CameraMatrices,
        model::Drawable,
        shader::{ShaderHandle, ShaderType},
        vulkan::core::Context,
    },
};

use super::{
    buffer::UniformBuffer,
    command::{
        level::{Primary, Secondary},
        operation::Graphics,
        BeginCommand, Persistent, PersistentCommandPool,
    },
    descriptor::{CameraDescriptorSet, Descriptor, DescriptorPool, DescriptorSetWriter},
    framebuffer::AttachmentList,
    memory::{Allocator, DefaultAllocator},
    resources::{MaterialPackList, MeshPackList},
    swapchain::{SwapchainFrame, SwapchainImageSync, VulkanSwapchain},
    VulkanDevice,
};

pub trait Frame: Sized {
    const REQUIRED_COMMANDS: usize;
    type Attachments: AttachmentList;
    type State;

    fn begin_frame(
        &mut self,
        device: &VulkanDevice,
        camera: &CameraMatrices,
    ) -> Result<(), Box<dyn Error>>;

    fn draw<
        A: Allocator,
        S: ShaderType,
        D: Drawable<Material = S::Material, Vertex = S::Vertex>,
        M: MaterialPackList<A>,
        V: MeshPackList<A>,
    >(
        &mut self,
        shader: ShaderHandle<S>,
        drawable: &D,
        transform: &Matrix4,
        material_packs: &M,
        mesh_packs: &V,
    );

    fn end_frame(&mut self, device: &VulkanDevice) -> Result<(), Box<dyn Error>>;
}

pub struct CameraUniform {
    pub descriptors: DescriptorPool<CameraDescriptorSet>,
    pub uniform_buffer: UniformBuffer<CameraMatrices, Graphics, DefaultAllocator>,
}

pub struct FrameData<C: Frame> {
    pub swapchain_frame: SwapchainFrame<C>,
    pub primary_command: BeginCommand<Persistent, Primary, Graphics>,
    pub camera_descriptor: Descriptor<CameraDescriptorSet>,
    pub renderer_state: C::State,
}

pub struct FramePool<F: Frame> {
    pub image_sync: Vec<SwapchainImageSync>,
    pub camera_uniform: CameraUniform,
    pub primary_commands: PersistentCommandPool<Primary, Graphics>,
    pub secondary_commands: PersistentCommandPool<Secondary, Graphics>,
    _phantom: PhantomData<F>,
}

impl Context {
    fn create_camera_uniform(
        &mut self,
        num_images: usize,
    ) -> Result<CameraUniform, Box<dyn Error>> {
        let uniform_buffer = self.create_uniform_buffer::<CameraMatrices, Graphics, _>(
            &mut DefaultAllocator {},
            num_images,
        )?;
        let descriptors = self.create_descriptor_pool(
            DescriptorSetWriter::<CameraDescriptorSet>::new(num_images)
                .write_buffer(&uniform_buffer),
        )?;
        Ok(CameraUniform {
            descriptors,
            uniform_buffer,
        })
    }

    fn destroy_camera_uniform(&self, camera: &mut CameraUniform) {
        self.destroy_descriptor_pool(&mut camera.descriptors);
        self.destroy_uniform_buffer(&mut camera.uniform_buffer, &mut DefaultAllocator {});
    }

    pub fn create_frame_pool<F: Frame>(
        &mut self,
        swapchain: &VulkanSwapchain<F>,
    ) -> Result<FramePool<F>, Box<dyn Error>> {
        let image_sync = self.create_swapchain_image_sync(swapchain)?;
        let primary_commands = self.create_persistent_command_pool(swapchain.num_images)?;
        let secondary_commands =
            self.create_persistent_command_pool(swapchain.num_images * F::REQUIRED_COMMANDS)?;
        let camera_uniform = self.create_camera_uniform(swapchain.num_images)?;

        Ok(FramePool {
            image_sync,
            camera_uniform,
            primary_commands,
            secondary_commands,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_frame_pool<F: Frame>(&self, pool: &mut FramePool<F>) {
        self.destroy_swapchain_image_sync(&mut pool.image_sync);
        self.destroy_persistent_command_pool(&mut pool.primary_commands);
        self.destroy_persistent_command_pool(&mut pool.secondary_commands);
        self.destroy_camera_uniform(&mut pool.camera_uniform);
    }
}
