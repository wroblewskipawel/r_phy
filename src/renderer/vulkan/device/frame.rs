// Temporary allow for too_many_arguments
// handling render commands will be significantly revamped in the future
// which makes it not worth the effort to refactor this code
#![allow(clippy::too_many_arguments)]

use std::error::Error;

use crate::{
    math::types::Matrix4,
    renderer::{
        camera::CameraMatrices,
        vulkan::{VulkanMaterialHandle, VulkanMeshHandle},
    },
};

use super::{
    buffer::UniformBuffer,
    command::{
        level::{Primary, Secondary},
        operation::Graphics,
        BeginCommand, FinishedCommand, Persistent, PersistentCommandPool,
    },
    descriptor::{CameraDescriptorSet, Descriptor, DescriptorPool},
    framebuffer::AttachmentList,
    material::MaterialPack,
    mesh::MeshPack,
    swapchain::{SwapchainFrame, SwapchainImageSync, VulkanSwapchain},
    VulkanDevice,
};

pub trait Frame {
    const REQUIRED_COMMANDS: usize;
    type Attachments: AttachmentList;
    type State;

    fn begin(
        &self,
        device: &VulkanDevice,
        pool: &mut PersistentCommandPool<Secondary, Graphics>,
        swapchain_frame: &SwapchainFrame<Self::Attachments>,
        camera_descriptor: Descriptor<CameraDescriptorSet>,
        camera_matrices: &CameraMatrices,
    ) -> Result<Self::State, Box<dyn Error>>;

    fn draw_mesh(
        &self,
        state: Self::State,
        device: &VulkanDevice,
        model: &Matrix4,
        mesh: VulkanMeshHandle,
        material: VulkanMaterialHandle,
        mesh_packs: &[MeshPack],
        material_packs: &[MaterialPack],
    ) -> Self::State;

    fn end(
        &self,
        state: Self::State,
        device: &VulkanDevice,
        swapchain_frame: &SwapchainFrame<Self::Attachments>,
        primary_command: BeginCommand<Persistent, Primary, Graphics>,
    ) -> Result<FinishedCommand<Persistent, Primary, Graphics>, Box<dyn Error>>;
}

struct CameraUniform {
    descriptors: DescriptorPool<CameraDescriptorSet>,
    uniform_buffer: UniformBuffer<CameraMatrices, Graphics>,
}

pub struct FrameData<C: Frame> {
    swapchain_frame: SwapchainFrame<C::Attachments>,
    primary_command: BeginCommand<Persistent, Primary, Graphics>,
    camera_descriptor: Descriptor<CameraDescriptorSet>,
    renderer_state: C::State,
}

pub struct FramePool {
    image_sync: Vec<SwapchainImageSync>,
    camera_uniform: CameraUniform,
    primary_commands: PersistentCommandPool<Primary, Graphics>,
    secondary_commands: PersistentCommandPool<Secondary, Graphics>,
}

impl VulkanDevice {
    fn create_camera_uniform(&self, num_images: usize) -> Result<CameraUniform, Box<dyn Error>> {
        let mut descriptors =
            self.create_descriptor_pool(CameraDescriptorSet::builder(), num_images)?;
        let uniform_buffer = self.create_uniform_buffer::<CameraMatrices, Graphics>(num_images)?;
        let descriptor_write = descriptors.get_writer().write_buffer(&uniform_buffer);
        self.write_descriptor_sets(&mut descriptors, descriptor_write);
        Ok(CameraUniform {
            descriptors,
            uniform_buffer,
        })
    }

    fn destroy_camera_uniform(&self, camera: &mut CameraUniform) {
        self.destroy_descriptor_pool(&mut camera.descriptors);
        self.destroy_uniform_buffer(&mut camera.uniform_buffer);
    }

    pub fn create_frame_pool<C: Frame>(
        &self,
        swapchain: &VulkanSwapchain<C::Attachments>,
    ) -> Result<FramePool, Box<dyn Error>> {
        let image_sync = self.create_swapchain_image_sync(swapchain)?;
        let primary_commands = self.create_persistent_command_pool(swapchain.num_images)?;
        let secondary_commands =
            self.create_persistent_command_pool(swapchain.num_images * C::REQUIRED_COMMANDS)?;
        let camera_uniform = self.create_camera_uniform(swapchain.num_images)?;

        Ok(FramePool {
            image_sync,
            camera_uniform,
            primary_commands,
            secondary_commands,
        })
    }

    pub fn destory_frame_pool(&self, pool: &mut FramePool) {
        self.destroy_swapchain_image_sync(&mut pool.image_sync);
        self.destroy_persistent_command_pool(&mut pool.primary_commands);
        self.destroy_persistent_command_pool(&mut pool.secondary_commands);
        self.destroy_camera_uniform(&mut pool.camera_uniform);
    }

    pub fn next_frame<C: Frame>(
        &self,
        pool: &mut FramePool,
        renderer: &C,
        swapchain: &VulkanSwapchain<C::Attachments>,
        camera: CameraMatrices,
    ) -> Result<FrameData<C>, Box<dyn Error>> {
        let (index, primary_command) = pool.primary_commands.next();
        let primary_command = self.begin_primary_command(primary_command)?;
        let swapchain_frame = self.get_frame(swapchain, pool.image_sync[index])?;
        let camera_descriptor = pool.camera_uniform.descriptors[index];
        pool.camera_uniform.uniform_buffer[index] = camera;
        let commands = renderer.begin(
            self,
            &mut pool.secondary_commands,
            &swapchain_frame,
            camera_descriptor,
            &camera,
        )?;
        Ok(FrameData {
            swapchain_frame,
            primary_command,
            camera_descriptor,
            renderer_state: commands,
        })
    }

    pub fn draw_mesh<C: Frame>(
        &self,
        renderer: &C,
        frame: FrameData<C>,
        model: &Matrix4,
        mesh: VulkanMeshHandle,
        material: VulkanMaterialHandle,
        mesh_packs: &[MeshPack],
        material_packs: &[MaterialPack],
    ) -> FrameData<C> {
        let FrameData {
            swapchain_frame,
            primary_command,
            camera_descriptor,
            renderer_state,
        } = frame;

        let renderer_state = renderer.draw_mesh(
            renderer_state,
            self,
            model,
            mesh,
            material,
            mesh_packs,
            material_packs,
        );

        FrameData {
            swapchain_frame,
            primary_command,
            camera_descriptor,
            renderer_state,
        }
    }

    pub fn end_frame<C: Frame>(
        &self,
        renderer: &C,
        frame: FrameData<C>,
        swapchain: &VulkanSwapchain<C::Attachments>,
    ) -> Result<(), Box<dyn Error>> {
        let FrameData {
            swapchain_frame,
            primary_command,
            renderer_state,
            ..
        } = frame;
        let primary_command =
            renderer.end(renderer_state, self, &swapchain_frame, primary_command)?;
        self.present_frame(swapchain, primary_command, swapchain_frame)?;
        Ok(())
    }
}
