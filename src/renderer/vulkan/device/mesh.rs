use ash::vk;

use super::{buffer::Buffer, swapchain::Frame, VulkanDevice};
use crate::renderer::{
    mesh::{Mesh, Vertex},
    vulkan::device::Operation,
};
use bytemuck::cast_slice;
use std::error::Error;

pub struct VulkanMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,
}

impl VulkanDevice {
    pub fn load_mesh(&self, mesh: &Mesh) -> Result<VulkanMesh, Box<dyn Error>> {
        let queue_families = self.get_queue_families(&[Operation::Graphics]);
        let vertex_buffer_bytes = cast_slice(&mesh.vertices);
        let mut vertex_buffer = self.create_buffer(
            vertex_buffer_bytes.len(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            &queue_families,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        self.transfer_buffer_data(&mut vertex_buffer, vertex_buffer_bytes)?;
        let index_buffer_bytes = cast_slice(&mesh.indices);
        let mut index_buffer = self.create_buffer(
            index_buffer_bytes.len(),
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            &queue_families,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        self.transfer_buffer_data(&mut index_buffer, index_buffer_bytes)?;
        Ok(VulkanMesh {
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        })
    }

    pub fn destory_mesh(&self, mesh: &mut VulkanMesh) {
        self.destroy_buffer(&mut mesh.vertex_buffer);
        self.destroy_buffer(&mut mesh.index_buffer);
    }

    pub fn draw(&self, frame: &Frame, mesh: &VulkanMesh) {
        unsafe {
            self.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.vertex_buffer.buffer],
                &[0],
            );
            self.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.index_buffer.buffer,
                0,
                vk::IndexType::UINT32,
            );
            self.device
                .cmd_draw_indexed(frame.command_buffer, mesh.num_indices, 1, 0, 0, 0);
        }
    }
}
