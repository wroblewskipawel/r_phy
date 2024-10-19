use std::{borrow::BorrowMut, error::Error, marker::PhantomData, ptr::copy_nonoverlapping};

use ash::vk;
use bytemuck::{cast_slice_mut, AnyBitPattern, NoUninit};

use crate::device::{
    command::{
        operation::{self, Operation},
        SubmitSemaphoreState,
    },
    memory::{Allocator, DefaultAllocator, DeviceLocal, HostCoherent},
    resources::{
        buffer::{ByteRange, Range},
        image::Image2D,
        PartialBuilder,
    },
    Device,
};

use super::{Buffer, BufferBuilder, BufferInfo, PersistentBuffer, PersistentBufferPartial};

pub struct StagingBufferBuilder {
    range: ByteRange,
}

impl Default for StagingBufferBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StagingBufferBuilder {
    pub fn new() -> Self {
        Self {
            range: ByteRange::empty(),
        }
    }

    pub fn append<T: AnyBitPattern>(&mut self, len: usize) -> Range<T> {
        self.range.extend::<T>(len).into()
    }
}

pub struct StagingBuffer<'a> {
    range: ByteRange,
    buffer: PersistentBuffer<DefaultAllocator>,
    device: &'a Device,
}

pub struct WritableRange<T: AnyBitPattern> {
    ptr: *mut T,
    range: Range<T>,
}

impl<'a, 'b> From<&'b StagingBuffer<'a>> for &'b Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'b StagingBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a, 'b> From<&'b mut StagingBuffer<'a>> for &'b mut Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'b mut StagingBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        self.device.destroy_buffer(self, &mut DefaultAllocator {});
    }
}

impl<'a> StagingBuffer<'a> {
    pub fn transfer_buffer_data<'b, D: Allocator>(
        &self,
        dst: impl Into<&'b mut Buffer<DeviceLocal, D>>,
        dst_offset: vk::DeviceSize,
    ) -> Result<(), Box<dyn Error>> {
        let command = self
            .device
            .allocate_transient_command::<operation::Transfer>()?;
        let command = self.device.begin_primary_command(command)?;
        let command = self.device.record_command(command, |command| {
            command.copy_buffer(
                &self.buffer,
                dst,
                &[vk::BufferCopy {
                    src_offset: 0,
                    dst_offset,
                    size: self.range.end as vk::DeviceSize,
                }],
            )
        });
        let command = self
            .device
            .submit_command(
                self.device.finish_command(command)?,
                SubmitSemaphoreState {
                    semaphores: &[],
                    masks: &[],
                },
                &[],
            )?
            .wait()?;
        self.device.free_command(&command);
        Ok(())
    }

    pub fn transfer_image_data<'b, A: Allocator>(
        &self,
        dst: impl Into<&'b mut Image2D<DeviceLocal, A>>,
        dst_array_layer: u32,
        dst_final_layout: vk::ImageLayout,
    ) -> Result<(), Box<dyn Error>> {
        let dst: &mut _ = dst.into();
        debug_assert!(
            dst.array_layers > dst_array_layer,
            "Invalid dst_array_layer for image data transfer!"
        );
        let dst_mip_levels = dst.mip_levels;
        let dst_old_layout = dst.layout;
        let command = self.device.begin_primary_command(
            self.device
                .allocate_transient_command::<operation::Graphics>()?,
        )?;
        let command = self.device.record_command(command, |command| {
            command
                .change_layout(
                    dst.borrow_mut(),
                    dst_old_layout,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    dst_array_layer,
                    0,
                    1,
                )
                .copy_image(self, dst.borrow_mut(), dst_array_layer)
                .generate_mip(dst.borrow_mut(), dst_array_layer)
                .change_layout(
                    dst.borrow_mut(),
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    dst_final_layout,
                    dst_array_layer,
                    0,
                    dst_mip_levels,
                )
        });

        let command = self
            .device
            .submit_command(
                self.device.finish_command(command)?,
                SubmitSemaphoreState {
                    semaphores: &[],
                    masks: &[],
                },
                &[],
            )?
            .wait()?;
        // Shouldn't free_command consume Command instead of taking it by reference?
        self.device.free_command(&command);
        Ok(())
    }

    pub fn write_range<T: AnyBitPattern>(&mut self, range: Range<T>) -> WritableRange<T> {
        // TODO: Improve safety,
        // - Range should comme from current staging buffer builder (unnecessary complexity?)
        debug_assert!(
            <Range<T> as Into<ByteRange>>::into(range).end <= self.range.end,
            "Invalid range for StagingBuffer write!"
        );
        WritableRange {
            range: Range {
                first: 0,
                len: range.len,
                _phantom: PhantomData,
            },
            ptr: unsafe { (self.buffer.ptr.unwrap() as *mut T).add(range.first) },
        }
    }
}

impl<T: AnyBitPattern> WritableRange<T> {
    pub fn write(&mut self, value: &[T]) -> Range<T> {
        let range = self.range.alloc(value.len());
        unsafe { copy_nonoverlapping(value.as_ptr(), self.ptr.add(range.first), value.len()) }
        range
    }
}

impl<T: AnyBitPattern + NoUninit> WritableRange<T> {
    pub fn remaining_as_slice_mut(&mut self) -> &mut [T] {
        let range = self.range.alloc(self.range.len);
        let values =
            unsafe { std::slice::from_raw_parts_mut::<T>(self.ptr.add(range.first), range.len) };
        cast_slice_mut(values)
    }
}

impl Device {
    pub fn create_stagging_buffer(
        &self,
        builder: StagingBufferBuilder,
    ) -> Result<StagingBuffer, Box<dyn Error>> {
        let StagingBufferBuilder { range } = builder;
        let info = BufferInfo {
            size: range.end,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[operation::Transfer::get_queue_family_index(self)],
        };
        let partial = PersistentBufferPartial::prepare(BufferBuilder::new(info), self)?;
        let buffer = partial.finalize(self, &mut DefaultAllocator {})?;
        Ok(StagingBuffer {
            range,
            buffer,
            device: self,
        })
    }
}
