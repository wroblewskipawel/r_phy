use ash::{vk, Device};
use bytemuck::{bytes_of, Pod};

use self::operation::Operation;

use super::{
    buffer::Buffer,
    pipeline::GraphicsPipeline,
    render_pass::VulkanRenderPass,
    resources::{BufferType, MeshRange, ResourcePack},
    swapchain::SwapchainFrame,
    QueueFamilies, VulkanDevice,
};
use std::{error::Error, marker::PhantomData};

pub struct Transient;
pub struct Persistent;
pub mod operation {
    use ash::vk;

    use crate::renderer::vulkan::device::{DeviceQueues, QueueFamilies};

    use super::TransientCommandPools;

    pub struct Graphics;
    pub struct Transfer;
    pub struct Compute;

    pub trait Operation {
        fn get_queue(queues: &DeviceQueues) -> vk::Queue;
        fn get_queue_family_index(families: &QueueFamilies) -> u32;
        fn get_transient_command_pool(pools: &TransientCommandPools) -> vk::CommandPool;
    }

    impl Operation for Graphics {
        fn get_queue(queues: &DeviceQueues) -> vk::Queue {
            queues.graphics
        }
        fn get_queue_family_index(families: &QueueFamilies) -> u32 {
            families.graphics
        }
        fn get_transient_command_pool(pools: &TransientCommandPools) -> vk::CommandPool {
            unimplemented!()
        }
    }
    impl Operation for Compute {
        fn get_queue(queues: &DeviceQueues) -> vk::Queue {
            queues.compute
        }
        fn get_queue_family_index(families: &QueueFamilies) -> u32 {
            families.compute
        }
        fn get_transient_command_pool(pools: &TransientCommandPools) -> vk::CommandPool {
            unimplemented!()
        }
    }
    impl Operation for Transfer {
        fn get_queue(queues: &DeviceQueues) -> vk::Queue {
            queues.transfer
        }
        fn get_queue_family_index(families: &QueueFamilies) -> u32 {
            families.compute
        }
        fn get_transient_command_pool(pools: &TransientCommandPools) -> vk::CommandPool {
            pools.transfer
        }
    }
}

pub struct Command<T, O: Operation> {
    buffer: vk::CommandBuffer,
    pub fence: vk::Fence,
    _phantom: PhantomData<(T, O)>,
}

pub struct PersistentCommandPool<O: Operation> {
    head: usize, // Create dedicated ring buffer (wrapper? generic where T: Index) class
    command_pool: vk::CommandPool,
    buffers: Vec<vk::CommandBuffer>,
    fences: Vec<vk::Fence>,
    _phantom: PhantomData<O>,
}

impl<O: Operation> PersistentCommandPool<O> {
    pub fn next(&mut self) -> (usize, NewCommand<Persistent, O>) {
        let next_command_index = self.head;
        self.head = (self.head + 1) % self.buffers.len();
        let command = Command {
            buffer: self.buffers[next_command_index],
            fence: self.fences[next_command_index],
            _phantom: PhantomData,
        };
        (next_command_index, NewCommand(command))
    }
}

impl VulkanDevice {
    pub(super) fn create_persistent_command_pool<O: Operation>(
        &self,
        size: usize,
    ) -> Result<PersistentCommandPool<O>, Box<dyn Error>> {
        let command_pool = unsafe {
            self.device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(O::get_queue_family_index(
                        &self.physical_device.queue_families,
                    ))
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        let allocate_info = vk::CommandBufferAllocateInfo {
            command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: size as u32,
            ..Default::default()
        };
        let (buffers, fences) = unsafe {
            let buffers = self.device.allocate_command_buffers(&allocate_info)?;
            let fences = (0..buffers.len())
                .into_iter()
                .map(|_| {
                    self.device.create_fence(
                        &vk::FenceCreateInfo {
                            flags: vk::FenceCreateFlags::SIGNALED,
                            ..Default::default()
                        },
                        None,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            (buffers, fences)
        };
        Ok(PersistentCommandPool {
            command_pool,
            buffers,
            fences,
            head: 0,
            _phantom: PhantomData,
        })
    }

    pub fn destory_persistent_command_pool<O: Operation>(&self, command_pool: &mut PersistentCommandPool<O>) {
        unsafe {
            command_pool
                .fences
                .iter()
                .for_each(|&fence| self.device.destroy_fence(fence, None));
            self.device
                .destroy_command_pool(command_pool.command_pool, None)
        };
    }
}

pub struct NewCommand<T, O: Operation>(Command<T, O>);

impl<'a, T, O: Operation> From<&'a NewCommand<T, O>> for &'a Command<T, O> {
    fn from(value: &'a NewCommand<T, O>) -> Self {
        &value.0
    }
}

impl VulkanDevice {
    pub fn begin_command<T, O: Operation>(
        &self,
        command: NewCommand<T, O>,
    ) -> Result<BeginCommand<T, O>, Box<dyn Error>> {
        let NewCommand(command) = command;
        unsafe {
            self.device.begin_command_buffer(
                command.buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        Ok(BeginCommand(command))
    }

    pub fn begin_persistent_command<O: Operation>(
        &self,
        command: NewCommand<Persistent, O>,
    ) -> Result<BeginCommand<Persistent, O>, Box<dyn Error>> {
        let NewCommand(command) = command;
        unsafe {
            self.device
                .wait_for_fences(&[command.fence], true, u64::MAX)?;
            self.device.reset_fences(&[command.fence])?;
            self.device.begin_command_buffer(
                command.buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        Ok(BeginCommand(command))
    }

    pub fn record_command<T, O: Operation, F: FnOnce(RecordingCommand<T, O>) -> RecordingCommand<T, O>>(
        &self,
        command: BeginCommand<T, O>,
        recorder: F,
    ) -> BeginCommand<T, O> {
        let BeginCommand(command) = command;
        let RecordingCommand(command, _) = recorder(RecordingCommand(command, self));
        BeginCommand(command)
    }

    pub fn finish_command<T, O: Operation>(
        &self,
        command: BeginCommand<T, O>,
    ) -> Result<FinishedCommand<T, O>, Box<dyn Error>> {
        let BeginCommand(command) = command;
        unsafe {
            self.device.end_command_buffer(command.buffer)?;
        }
        Ok(FinishedCommand(command, self))
    }
}

pub struct RecordingCommand<'a, T, O: Operation>(Command<T, O>, &'a VulkanDevice);

impl<'a, T, O: Operation> From<&'a RecordingCommand<'a, T, O>> for &'a Command<T, O> {
    fn from(value: &'a RecordingCommand<T, O>) -> Self {
        &value.0
    }
}

pub struct BeginCommand<T, O: Operation>(Command<T, O>);

impl<'a, T, O: Operation> From<&'a BeginCommand<T, O>> for &'a Command<T, O> {
    fn from(value: &'a BeginCommand<T, O>) -> Self {
        &value.0
    }
}

impl<'a, T, O: Operation> RecordingCommand<'a, T, O> {
    pub fn copy_buffer<'b, 'c>(
        self,
        src: impl Into<&'b Buffer>,
        dst: impl Into<&'c Buffer>,
        ranges: &[vk::BufferCopy],
    ) -> Self {
        let RecordingCommand(command, device) = self;
        let src = src.into();
        let dst = dst.into();
        unsafe {
            device.cmd_copy_buffer(command.buffer, src.buffer, dst.buffer, ranges);
        }
        RecordingCommand(command, device)
    }

    pub fn begin_render_pass(self, frame: &SwapchainFrame, render_pass: &VulkanRenderPass) -> Self {
        let RecordingCommand(command, device) = self;
        let clear_values = VulkanRenderPass::get_attachment_clear_values();
        unsafe {
            device.cmd_begin_render_pass(
                command.buffer,
                &vk::RenderPassBeginInfo {
                    render_pass: render_pass.handle,
                    framebuffer: frame.framebuffer,
                    render_area: frame.render_area,
                    clear_value_count: clear_values.len() as u32,
                    p_clear_values: clear_values.as_ptr(),
                    ..Default::default()
                },
                vk::SubpassContents::INLINE,
            )
        }
        RecordingCommand(command, device)
    }

    pub fn end_render_pass(self) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_end_render_pass(command.buffer);
        }
        RecordingCommand(command, device)
    }

    pub fn bind_pipeline<L>(self, pipeline: &GraphicsPipeline<L>) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_bind_pipeline(
                command.buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.handle,
            );
        }
        RecordingCommand(command, device)
    }

    pub fn bind_resource_pack(self, resources: &ResourcePack) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_bind_index_buffer(
                command.buffer,
                resources.buffer.buffer.buffer,
                resources.buffer_ranges[BufferType::Index].offset,
                vk::IndexType::UINT32,
            );
            device.cmd_bind_vertex_buffers(
                command.buffer,
                0,
                &[resources.buffer.buffer.buffer],
                &[resources.buffer_ranges[BufferType::Vertex].offset],
            );
        }
        RecordingCommand(command, device)
    }

    pub fn push_constants<L, C: Pod>(
        self,
        pipeline: &GraphicsPipeline<L>,
        stages: vk::ShaderStageFlags,
        offset: usize,
        data: &C,
    ) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_push_constants(
                command.buffer,
                (&pipeline.layout).into(),
                stages,
                offset as u32,
                bytes_of(data),
            );
        }
        RecordingCommand(command, device)
    }

    pub fn bind_camera_uniform_buffer<L>(
        self,
        pipeline: &GraphicsPipeline<L>,
        frame: &SwapchainFrame,
    ) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            // This implicitly relies on knowledge that Camera uniform
            // is only descriptor set used by the pipeline
            device.cmd_bind_descriptor_sets(
                command.buffer,
                vk::PipelineBindPoint::GRAPHICS,
                (&pipeline.layout).into(),
                0u32,
                &[frame.camera_descriptor],
                &[],
            )
        }
        RecordingCommand(command, device)
    }

    pub fn draw_mesh(self, mesh_ranges: MeshRange) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_draw_indexed(
                command.buffer,
                mesh_ranges.indices.count as u32,
                1,
                mesh_ranges.indices.first as u32,
                mesh_ranges.vertices.first as i32,
                0,
            )
        }
        RecordingCommand(command, device)
    }
}

pub struct SubmitSemaphoreState<'a> {
    pub semaphores: &'a [vk::Semaphore],
    pub masks: &'a [vk::PipelineStageFlags],
}

pub struct FinishedCommand<'a, T, O: Operation>(Command<T, O>, &'a VulkanDevice);

impl<'a, T, O: Operation> From<&'a FinishedCommand<'a, T, O>> for &'a Command<T, O> {
    fn from(value: &'a FinishedCommand<T, O>) -> Self {
        &value.0
    }
}

impl<'a, T, O: Operation> FinishedCommand<'a, T, O> {
    // Make wait and submit optional
    pub fn submit(
        self,
        wait: SubmitSemaphoreState,
        signal: &[vk::Semaphore],
    ) -> Result<SubmitedCommand<'a, T, O>, Box<dyn Error>> {
        let FinishedCommand(command, device) = self;
        unsafe {
            device.queue_submit(
                O::get_queue(&device.device_queues),
                &[vk::SubmitInfo {
                    command_buffer_count: 1,
                    p_command_buffers: [command.buffer].as_ptr(),
                    wait_semaphore_count: wait.semaphores.len() as _,
                    p_wait_semaphores: wait.semaphores.as_ptr(),
                    p_wait_dst_stage_mask: wait.masks.as_ptr(),
                    signal_semaphore_count: signal.len() as _,
                    p_signal_semaphores: signal.as_ptr(),
                    ..Default::default()
                }],
                command.fence,
            )?;
        }
        Ok(SubmitedCommand(command, device))
    }
}
pub struct SubmitedCommand<'a, T, O: Operation>(Command<T, O>, &'a VulkanDevice);

impl<'a, T, O: Operation> From<&'a SubmitedCommand<'a, T, O>> for &'a Command<T, O> {
    fn from(value: &'a SubmitedCommand<T, O>) -> Self {
        &value.0
    }
}

impl<'a, O: Operation> SubmitedCommand<'a, Transient, O> {
    pub fn wait(self) -> Result<Self, Box<dyn Error>> {
        let SubmitedCommand(command, device) = self;
        unsafe {
            device.wait_for_fences(&[command.fence], true, u64::MAX)?;
        }
        Ok(Self(command, device))
    }
}

impl<'a, O: Operation> SubmitedCommand<'a, Persistent, O> {
    pub fn reset(self) -> NewCommand<Persistent, O> {
        let SubmitedCommand(command, _) = self;
        NewCommand(command)
    }

    pub fn wait(self) -> Result<Self, Box<dyn Error>> {
        let SubmitedCommand(command, device) = self;
        unsafe {
            device.wait_for_fences(&[command.fence], true, u64::MAX)?;
            device.reset_fences(&[command.fence])?;
        }
        Ok(Self(command, device))
    }
}

pub struct TransientCommandPools {
    transfer: vk::CommandPool,
}

impl TransientCommandPools {
    pub(super) fn create(
        device: &Device,
        queue_families: QueueFamilies,
    ) -> Result<Self, Box<dyn Error>> {
        let transfer = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_families.transfer)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )?
        };
        Ok(Self { transfer })
    }

    pub fn destory(&mut self, device: &Device) {
        unsafe { device.destroy_command_pool(self.transfer, None) };
    }
}

impl VulkanDevice {
    pub fn allocate_transient_command<O: Operation>(
        &self,
    ) -> Result<NewCommand<Transient, O>, Box<dyn Error>> {
        let &buffer = unsafe {
            self.device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_pool(O::get_transient_command_pool(&self.command_pools))
                        .command_buffer_count(1),
                )?
                .first()
                .unwrap()
        };
        let fence = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::builder(), None)?
        };
        Ok(NewCommand(Command {
            buffer,
            fence,
            _phantom: PhantomData,
        }))
    }
    pub fn free_command<'a, T: 'static, O: 'static + Operation>(
        &self,
        command: impl Into<&'a Command<T, O>>,
    ) {
        let &Command { buffer, fence, .. } = command.into();
        unsafe {
            self.device.free_command_buffers(
                O::get_transient_command_pool(&self.command_pools),
                &[buffer],
            );
            self.device.destroy_fence(fence, None);
        }
    }
}
