use ash::{
    vk::{self, Extent2D, Offset3D},
    Device,
};
use bytemuck::{bytes_of, Pod};

use crate::{
    math::types::Vector4,
    renderer::camera::{Camera, CameraMatrices},
};

use self::{
    level::{Level, Primary, Secondary},
    operation::Operation,
};

use super::{
    buffer::Buffer,
    descriptor::{Descriptor, DescriptorLayout},
    framebuffer::{Clear, Framebuffer},
    image::VulkanImage2D,
    mesh::{BufferType, MeshPack, MeshRange},
    pipeline::{GraphicsPipeline, GraphicspipelineConfig, Layout, PushConstant},
    render_pass::{RenderPass, RenderPassConfig, Subpass},
    skybox::Skybox,
    swapchain::SwapchainFrame,
    QueueFamilies, VulkanDevice,
};
use std::{any::type_name, error::Error, marker::PhantomData};

pub struct Transient;
pub struct Persistent;

pub mod level {
    use ash::vk;

    pub trait Level {
        const LEVEL: vk::CommandBufferLevel;
    }

    pub struct Primary {}

    impl Level for Primary {
        const LEVEL: vk::CommandBufferLevel = vk::CommandBufferLevel::PRIMARY;
    }

    pub struct Secondary {}

    impl Level for Secondary {
        const LEVEL: vk::CommandBufferLevel = vk::CommandBufferLevel::SECONDARY;
    }
}

pub mod operation {
    use ash::vk;

    use crate::renderer::vulkan::device::VulkanDevice;

    pub(in crate::renderer::vulkan) struct Graphics;
    pub(in crate::renderer::vulkan) struct Transfer;
    pub(in crate::renderer::vulkan) struct Compute;

    // Lots of pub(in path) syntax in this module
    // some of it contents could be moved to separate module
    // placed higher in the source tree
    pub(in crate::renderer::vulkan) trait Operation {
        fn get_queue(device: &VulkanDevice) -> vk::Queue;
        fn get_queue_family_index(device: &VulkanDevice) -> u32;
        fn get_transient_command_pool(device: &VulkanDevice) -> vk::CommandPool;
    }

    impl Operation for Graphics {
        fn get_queue(device: &VulkanDevice) -> vk::Queue {
            device.device_queues.graphics
        }
        fn get_queue_family_index(device: &VulkanDevice) -> u32 {
            device.physical_device.queue_families.graphics
        }
        fn get_transient_command_pool(device: &VulkanDevice) -> vk::CommandPool {
            device.command_pools.graphics
        }
    }
    impl Operation for Compute {
        fn get_queue(device: &VulkanDevice) -> vk::Queue {
            device.device_queues.compute
        }
        fn get_queue_family_index(device: &VulkanDevice) -> u32 {
            device.physical_device.queue_families.compute
        }
        fn get_transient_command_pool(_device: &VulkanDevice) -> vk::CommandPool {
            unimplemented!()
        }
    }
    impl Operation for Transfer {
        fn get_queue(device: &VulkanDevice) -> vk::Queue {
            device.device_queues.transfer
        }
        fn get_queue_family_index(device: &VulkanDevice) -> u32 {
            device.physical_device.queue_families.transfer
        }
        fn get_transient_command_pool(device: &VulkanDevice) -> vk::CommandPool {
            device.command_pools.transfer
        }
    }
}

pub(super) struct Command<T, L: Level, O: Operation> {
    buffer: vk::CommandBuffer,
    pub fence: vk::Fence,
    _phantom: PhantomData<(T, L, O)>,
}

pub(super) struct PersistentCommandPool<L: Level, O: Operation> {
    head: usize, // Create dedicated ring buffer (wrapper? generic where T: Index) class
    command_pool: vk::CommandPool,
    buffers: Vec<vk::CommandBuffer>,
    fences: Vec<vk::Fence>,
    _phantom: PhantomData<(L, O)>,
}

impl<L: Level, O: Operation> PersistentCommandPool<L, O> {
    pub fn next(&mut self) -> (usize, NewCommand<Persistent, L, O>) {
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
    pub(super) fn create_persistent_command_pool<L: Level, O: Operation>(
        &self,
        size: usize,
    ) -> Result<PersistentCommandPool<L, O>, Box<dyn Error>> {
        let command_pool = unsafe {
            self.device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(O::get_queue_family_index(self))
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        let allocate_info = vk::CommandBufferAllocateInfo {
            command_pool,
            level: L::LEVEL,
            command_buffer_count: size as u32,
            ..Default::default()
        };
        let (buffers, fences) = unsafe {
            let buffers = self.device.allocate_command_buffers(&allocate_info)?;
            let fences = (0..buffers.len())
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

    pub(super) fn destroy_persistent_command_pool<L: Level, O: Operation>(
        &self,
        command_pool: &mut PersistentCommandPool<L, O>,
    ) {
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

pub(in crate::renderer::vulkan) struct NewCommand<T, L: Level, O: Operation>(Command<T, L, O>);

impl<'a, T, L: Level, O: Operation> From<&'a NewCommand<T, L, O>> for &'a Command<T, L, O> {
    fn from(value: &'a NewCommand<T, L, O>) -> Self {
        &value.0
    }
}

impl VulkanDevice {
    pub(super) fn begin_primary_command<T, O: Operation>(
        &self,
        command: NewCommand<T, Primary, O>,
    ) -> Result<BeginCommand<T, Primary, O>, Box<dyn Error>> {
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

    pub fn begin_secondary_command<
        T,
        O: Operation,
        C: RenderPassConfig,
        S: Subpass<C::Attachments>,
    >(
        &self,
        command: NewCommand<T, Secondary, O>,
        render_pass: RenderPass<C>,
        framebuffer: Framebuffer<C::Attachments>,
    ) -> Result<BeginCommand<T, Secondary, O>, Box<dyn Error>> {
        let subpass = C::try_get_subpass_index::<S>().unwrap_or_else(|| {
            panic!(
                "Subpass {} not present in RenderPass {}!",
                type_name::<S>(),
                type_name::<C>(),
            )
        }) as u32;
        let NewCommand(command) = command;
        unsafe {
            self.device.begin_command_buffer(
                command.buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                    .inheritance_info(&vk::CommandBufferInheritanceInfo {
                        render_pass: render_pass.handle,
                        subpass,
                        framebuffer: framebuffer.framebuffer,
                        ..Default::default()
                    }),
            )?;
        }
        Ok(BeginCommand(command))
    }

    pub(super) fn begin_persistent_command<L: Level, O: Operation>(
        &self,
        command: NewCommand<Persistent, L, O>,
    ) -> Result<BeginCommand<Persistent, L, O>, Box<dyn Error>> {
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

    pub(in crate::renderer::vulkan) fn record_command<
        T,
        L: Level,
        O: Operation,
        F: FnOnce(RecordingCommand<T, L, O>) -> RecordingCommand<T, L, O>,
    >(
        &self,
        command: BeginCommand<T, L, O>,
        recorder: F,
    ) -> BeginCommand<T, L, O> {
        let BeginCommand(command) = command;
        let RecordingCommand(command, _) = recorder(RecordingCommand(command, self));
        BeginCommand(command)
    }

    pub fn finish_command<T, L: Level, O: Operation>(
        &self,
        command: BeginCommand<T, L, O>,
    ) -> Result<FinishedCommand<T, L, O>, Box<dyn Error>> {
        let BeginCommand(command) = command;
        unsafe {
            self.device.end_command_buffer(command.buffer)?;
        }
        Ok(FinishedCommand(command, self))
    }
}

pub(in crate::renderer::vulkan) struct RecordingCommand<'a, T, L: Level, O: Operation>(
    Command<T, L, O>,
    &'a VulkanDevice,
);

impl<'a, T, L: Level, O: Operation> From<&'a RecordingCommand<'a, T, L, O>>
    for &'a Command<T, L, O>
{
    fn from(value: &'a RecordingCommand<T, L, O>) -> Self {
        &value.0
    }
}

pub(in crate::renderer::vulkan) struct BeginCommand<T, L: Level, O: Operation>(Command<T, L, O>);

impl<'a, T, L: Level, O: Operation> From<&'a BeginCommand<T, L, O>> for &'a Command<T, L, O> {
    fn from(value: &'a BeginCommand<T, L, O>) -> Self {
        &value.0
    }
}

impl<'a, T, L: Level, O: Operation> RecordingCommand<'a, T, L, O> {
    pub fn next_render_pass(self) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_next_subpass(
                command.buffer,
                vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
            );
        }
        RecordingCommand(command, device)
    }

    pub fn write_secondary(self, secondary: &FinishedCommand<T, Secondary, O>) -> Self {
        let FinishedCommand(secondary, _) = secondary;
        let RecordingCommand(command, device) = self;
        unsafe { device.cmd_execute_commands(command.buffer, &[secondary.buffer]) }
        RecordingCommand(command, device)
    }

    pub fn copy_buffer<'b, 'c>(
        self,
        src: impl Into<&'b Buffer>,
        dst: impl Into<&'c mut Buffer>,
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

    pub fn change_layout<'b, 'c>(
        self,
        image: impl Into<&'c mut VulkanImage2D>,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        array_layer: u32,
        base_level: u32,
        level_count: u32,
    ) -> Self {
        let RecordingCommand(command, device) = self;
        let image = image.into();
        debug_assert!(
            base_level + level_count <= image.mip_levels,
            "Image mip level count exceeded!"
        );
        unsafe {
            device.cmd_pipeline_barrier(
                command.buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::BY_REGION,
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_READ
                        | vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::TRANSFER_READ
                        | vk::AccessFlags::TRANSFER_WRITE,
                    old_layout,
                    new_layout,
                    src_queue_family_index: O::get_queue_family_index(device),
                    dst_queue_family_index: O::get_queue_family_index(device),
                    image: image.image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: base_level,
                        level_count,
                        base_array_layer: array_layer,
                        layer_count: 1,
                    },
                    ..Default::default()
                }],
            );
            // TODO: Reconsider setting the layout here as it could be error prone
            // when handling partial layout transition (e.g. single mip level subresource)
            // image.layout = new_layout;
        }
        RecordingCommand(command, device)
    }

    pub fn generate_mip<'b, 'c>(
        self,
        image: impl Into<&'c mut VulkanImage2D>,
        array_layer: u32,
    ) -> Self {
        let image = image.into();
        let image_mip_levels = image.mip_levels;
        // debug_assert!(
        //     image.layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        //     "Invalid image layout for mip levels generation!"
        // );
        (1..image_mip_levels)
            .fold(self, |command, level| {
                command.generate_mip_level(image.image, image.extent, level, array_layer)
            })
            .change_layout(
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                array_layer,
                image_mip_levels - 1,
                1,
            )
    }

    fn generate_mip_level(
        self,
        image: vk::Image,
        extent: Extent2D,
        level: u32,
        layer: u32,
    ) -> Self {
        debug_assert!(level > 0, "generate mip level called for base mip level!");
        let base_level_extent = Extent2D {
            width: (extent.width / 2u32.pow(level - 1)).max(1),
            height: (extent.height / 2u32.pow(level - 1)).max(1),
        };
        let level_extent = Extent2D {
            width: (base_level_extent.width / 2).max(1),
            height: (base_level_extent.height / 2).max(1),
        };
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_pipeline_barrier(
                command.buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::BY_REGION,
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::TRANSFER_READ,
                    old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    src_queue_family_index: O::get_queue_family_index(device),
                    dst_queue_family_index: O::get_queue_family_index(device),
                    image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: level - 1,
                        level_count: 1,
                        base_array_layer: layer,
                        layer_count: 1,
                    },
                    ..Default::default()
                }],
            );
            device.cmd_pipeline_barrier(
                command.buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::BY_REGION,
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_READ,
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    src_queue_family_index: O::get_queue_family_index(device),
                    dst_queue_family_index: O::get_queue_family_index(device),
                    image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: level,
                        level_count: 1,
                        base_array_layer: layer,
                        layer_count: 1,
                    },
                    ..Default::default()
                }],
            );
            device.cmd_blit_image(
                command.buffer,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::ImageBlit {
                    src_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: level - 1,
                        base_array_layer: layer,
                        layer_count: 1,
                    },
                    src_offsets: [
                        Offset3D { x: 0, y: 0, z: 0 },
                        Offset3D {
                            x: base_level_extent.width as i32,
                            y: base_level_extent.height as i32,
                            z: 1,
                        },
                    ],
                    dst_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: level,
                        base_array_layer: layer,
                        layer_count: 1,
                    },
                    dst_offsets: [
                        Offset3D { x: 0, y: 0, z: 0 },
                        Offset3D {
                            x: level_extent.width as i32,
                            y: level_extent.height as i32,
                            z: 1,
                        },
                    ],
                }],
                vk::Filter::LINEAR,
            );
        }
        RecordingCommand(command, device)
    }

    pub fn copy_image<'b, 'c>(
        self,
        src: impl Into<&'b Buffer>,
        dst: impl Into<&'c mut VulkanImage2D>,
        dst_layer: u32,
    ) -> Self {
        let RecordingCommand(command, device) = self;
        let src = src.into();
        let dst = dst.into();
        unsafe {
            device.cmd_copy_buffer_to_image(
                command.buffer,
                src.buffer,
                dst.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: dst_layer,
                        layer_count: 1,
                    },
                    image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                    image_extent: vk::Extent3D {
                        width: dst.extent.width,
                        height: dst.extent.height,
                        depth: 1,
                    },
                }],
            );
        }
        RecordingCommand(command, device)
    }

    pub fn begin_render_pass<C: RenderPassConfig>(
        self,
        frame: &SwapchainFrame,
        render_pass: &RenderPass<C>,
        clear_values: &Clear<C::Attachments>,
    ) -> Self {
        let RecordingCommand(command, device) = self;
        let clear_values = clear_values.get_clear_values();
        unsafe {
            device.cmd_begin_render_pass(
                command.buffer,
                &vk::RenderPassBeginInfo {
                    render_pass: render_pass.handle,
                    framebuffer: frame.framebuffer.framebuffer,
                    render_area: frame.render_area,
                    clear_value_count: clear_values.len() as u32,
                    p_clear_values: clear_values.as_ptr(),
                    ..Default::default()
                },
                vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
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

    pub fn bind_pipeline<C: GraphicspipelineConfig>(self, pipeline: &GraphicsPipeline<C>) -> Self {
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

    pub fn bind_mesh_pack(self, pack: &MeshPack) -> Self {
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_bind_index_buffer(
                command.buffer,
                pack.buffer.buffer.buffer,
                pack.buffer_ranges[BufferType::Index].beg as vk::DeviceSize,
                vk::IndexType::UINT32,
            );
            device.cmd_bind_vertex_buffers(
                command.buffer,
                0,
                &[pack.buffer.buffer.buffer],
                &[pack.buffer_ranges[BufferType::Vertex].beg as vk::DeviceSize],
            );
        }
        RecordingCommand(command, device)
    }

    pub fn draw_skybox(self, skybox: &Skybox, mut camera_matrices: CameraMatrices) -> Self {
        camera_matrices.view[3] = Vector4::w();
        self.bind_pipeline(&skybox.pipeline)
            .bind_descriptor_set(&skybox.pipeline, skybox.descriptor[0])
            .bind_mesh_pack(&skybox.mesh_pack)
            .push_constants(&skybox.pipeline, &camera_matrices)
            .draw_mesh(skybox.mesh_pack.meshes[0])
    }

    pub fn push_constants<C: GraphicspipelineConfig, P: PushConstant + Pod>(
        self,
        pipeline: &GraphicsPipeline<C>,
        data: &P,
    ) -> Self {
        let range = C::Layout::ranges().try_get_range::<P>().unwrap_or_else(|| {
            panic!(
                "PushConstant {} not present in layout PushConstantRanges {}!",
                type_name::<P>(),
                type_name::<<C::Layout as Layout>::PushConstants>()
            )
        });
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_push_constants(
                command.buffer,
                pipeline.layout.layout,
                range.stage_flags,
                range.offset,
                bytes_of(data),
            );
        }
        RecordingCommand(command, device)
    }

    pub fn bind_descriptor_set<C: GraphicspipelineConfig, D: DescriptorLayout>(
        self,
        pipeline: &GraphicsPipeline<C>,
        descriptor: Descriptor<D>,
    ) -> Self {
        let set_index = C::Layout::sets().get_set_index::<D>().unwrap_or_else(|| {
            panic!(
                "DescriptorSet {} not present in layout DescriptorSets {}",
                type_name::<D>(),
                type_name::<<C::Layout as Layout>::Descriptors>()
            )
        });
        let RecordingCommand(command, device) = self;
        unsafe {
            device.cmd_bind_descriptor_sets(
                command.buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.layout.layout,
                set_index,
                &[descriptor.set],
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
                mesh_ranges.indices.len as u32,
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

pub(in crate::renderer::vulkan) struct FinishedCommand<'a, T, L: Level, O: Operation>(
    Command<T, L, O>,
    &'a VulkanDevice,
);

impl<'a, T, L: Level, O: Operation> From<&'a FinishedCommand<'a, T, L, O>>
    for &'a Command<T, L, O>
{
    fn from(value: &'a FinishedCommand<T, L, O>) -> Self {
        &value.0
    }
}

impl<'a, T, L: Level, O: Operation> FinishedCommand<'a, T, L, O> {
    // Make wait and submit optional
    pub fn submit(
        self,
        wait: SubmitSemaphoreState,
        signal: &[vk::Semaphore],
    ) -> Result<SubmitedCommand<'a, T, L, O>, Box<dyn Error>> {
        let FinishedCommand(command, device) = self;
        unsafe {
            device.queue_submit(
                O::get_queue(device),
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
pub(in crate::renderer::vulkan) struct SubmitedCommand<'a, T, L: Level, O: Operation>(
    Command<T, L, O>,
    &'a VulkanDevice,
);

impl<'a, T, L: Level, O: Operation> From<&'a SubmitedCommand<'a, T, L, O>>
    for &'a Command<T, L, O>
{
    fn from(value: &'a SubmitedCommand<T, L, O>) -> Self {
        &value.0
    }
}

impl<'a, L: Level, O: Operation> SubmitedCommand<'a, Transient, L, O> {
    pub fn wait(self) -> Result<Self, Box<dyn Error>> {
        let SubmitedCommand(command, device) = self;
        unsafe {
            device.wait_for_fences(&[command.fence], true, u64::MAX)?;
        }
        Ok(Self(command, device))
    }
}

impl<'a, L: Level, O: Operation> SubmitedCommand<'a, Persistent, L, O> {
    pub fn _reset(self) -> NewCommand<Persistent, L, O> {
        let SubmitedCommand(command, _) = self;
        NewCommand(command)
    }

    pub fn _wait(self) -> Result<Self, Box<dyn Error>> {
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
    graphics: vk::CommandPool,
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
        let graphics = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(queue_families.graphics)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )?
        };
        Ok(Self { transfer, graphics })
    }

    pub fn destroy(&mut self, device: &Device) {
        unsafe {
            device.destroy_command_pool(self.transfer, None);
            device.destroy_command_pool(self.graphics, None)
        };
    }
}

impl VulkanDevice {
    pub(super) fn allocate_transient_command<L: Level, O: Operation>(
        &self,
    ) -> Result<NewCommand<Transient, L, O>, Box<dyn Error>> {
        let &buffer = unsafe {
            self.device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .level(L::LEVEL)
                        .command_pool(O::get_transient_command_pool(self))
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
    pub(super) fn free_command<'a, T: 'static, L: 'static + Level, O: 'static + Operation>(
        &self,
        command: impl Into<&'a Command<T, L, O>>,
    ) {
        let &Command { buffer, fence, .. } = command.into();
        unsafe {
            self.device
                .free_command_buffers(O::get_transient_command_pool(self), &[buffer]);
            self.device.destroy_fence(fence, None);
        }
    }
}
