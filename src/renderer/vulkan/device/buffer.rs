use ash::vk;
use bytemuck::{cast_slice_mut, AnyBitPattern, NoUninit};
use std::{
    any::{type_name, TypeId},
    borrow::BorrowMut,
    error::Error,
    ffi::c_void,
    marker::PhantomData,
    mem::size_of,
    ops::{Index, IndexMut},
    ptr::copy_nonoverlapping,
    usize,
};

use super::{
    command::{
        operation::{self, Operation},
        SubmitSemaphoreState,
    },
    image::VulkanImage2D,
    memory::{AllocReq, MemoryBlock},
    VulkanDevice,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ByteRange {
    pub beg: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn empty() -> Self {
        Self { beg: 0, end: 0 }
    }

    pub fn new(size: usize) -> Self {
        Self { beg: 0, end: size }
    }

    fn align<T>(offset: usize) -> usize {
        let alignment = std::mem::align_of::<T>();
        ((offset + alignment - 1) / alignment) * alignment
    }

    fn align_raw(offset: usize, alignment: usize) -> usize {
        ((offset + alignment - 1) / alignment) * alignment
    }

    fn extend<T: AnyBitPattern>(&mut self, len: usize) -> ByteRange {
        let beg = ByteRange::align::<T>(self.end);
        let end = beg + len * size_of::<T>();
        self.end = end;
        ByteRange { beg, end }
    }

    pub fn take<T: AnyBitPattern>(&mut self, count: usize) -> Option<ByteRange> {
        let beg = ByteRange::align::<T>(self.beg);
        let end = beg + count * size_of::<T>();
        if end < self.end {
            self.beg = end;
            Some(ByteRange { beg, end })
        } else {
            None
        }
    }

    pub fn alloc_raw(&mut self, size: usize, alignment: usize) -> Option<ByteRange> {
        let beg = ByteRange::align_raw(self.beg, alignment);
        let end = beg + size;
        if end < self.end {
            self.beg = end;
            Some(ByteRange { beg, end })
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.beg
    }
}

impl<T: AnyBitPattern> From<Range<T>> for ByteRange {
    fn from(value: Range<T>) -> Self {
        let beg = value.first * size_of::<T>();
        Self {
            beg,
            end: beg + value.len * size_of::<T>(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Range<T: AnyBitPattern> {
    pub len: usize,
    pub first: usize,
    _phantom: PhantomData<T>,
}

impl<T: AnyBitPattern> From<ByteRange> for Range<T> {
    fn from(value: ByteRange) -> Self {
        debug_assert_eq!(
            value.beg % size_of::<T>(),
            0,
            "Invalid Range<u8> offset for Range<{}> type!",
            std::any::type_name::<T>()
        );
        debug_assert_eq!(
            (value.end - value.beg) % size_of::<T>(),
            0,
            "Invalid Range<u8> size for Range<{}> type!",
            std::any::type_name::<T>()
        );
        Self {
            first: value.beg / size_of::<T>(),
            len: (value.end - value.beg) / size_of::<T>(),
            _phantom: PhantomData,
        }
    }
}

impl<T: AnyBitPattern> Range<T> {
    fn alloc(&mut self, len: usize) -> Self {
        debug_assert!(len <= self.len, "Range alloc overflow!");
        let first = self.first;
        self.first += len;
        self.len -= len;
        Self {
            first,
            len,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct Buffer {
    pub size: usize,
    pub buffer: vk::Buffer,
    memory: MemoryBlock,
}

impl VulkanDevice {
    pub fn create_buffer(
        &mut self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
        memory_property_flags: vk::MemoryPropertyFlags,
    ) -> Result<Buffer, Box<dyn Error>> {
        let create_info = vk::BufferCreateInfo {
            usage,
            sharing_mode,
            size: size as u64,
            queue_family_index_count: queue_families.len() as u32,
            p_queue_family_indices: queue_families.as_ptr(),
            ..Default::default()
        };
        let (buffer, memory) = unsafe {
            let buffer = self.device.create_buffer(&create_info, None)?;
            let alloc_req = AllocReq::new(
                memory_property_flags,
                self.device.get_buffer_memory_requirements(buffer),
            );
            let memory = self.allocate_memory(alloc_req)?;
            memory.bind_buffer_memory(&self.device, buffer)?;
            (buffer, memory)
        };
        Ok(Buffer {
            size,
            buffer,
            memory,
        })
    }

    pub fn destroy_buffer(&self, buffer: &mut Buffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            // self.device.free_memory(buffer.device_memory, None); // TODO: Currently all allocation is static and freed on device drop
        }
    }
}

pub struct HostVisibleBuffer {
    buffer: Buffer,
}

impl<'a> From<&'a HostVisibleBuffer> for &'a Buffer {
    fn from(value: &'a HostVisibleBuffer) -> Self {
        &value.buffer
    }
}

impl<'a> From<&'a mut HostVisibleBuffer> for &'a mut Buffer {
    fn from(value: &'a mut HostVisibleBuffer) -> Self {
        &mut value.buffer
    }
}

impl VulkanDevice {
    pub fn create_host_visible_buffer(
        &mut self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
    ) -> Result<HostVisibleBuffer, Box<dyn Error>> {
        let buffer = self.create_buffer(
            size,
            usage,
            sharing_mode,
            queue_families,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;
        Ok(HostVisibleBuffer { buffer })
    }
}

#[derive(Debug)]
pub struct DeviceLocalBuffer {
    pub buffer: Buffer,
}

impl<'a> From<&'a DeviceLocalBuffer> for &'a Buffer {
    fn from(value: &'a DeviceLocalBuffer) -> Self {
        &value.buffer
    }
}

impl<'a> From<&'a mut DeviceLocalBuffer> for &'a mut Buffer {
    fn from(value: &'a mut DeviceLocalBuffer) -> Self {
        &mut value.buffer
    }
}

impl VulkanDevice {
    pub fn create_device_local_buffer(
        &mut self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
    ) -> Result<DeviceLocalBuffer, Box<dyn Error>> {
        let buffer = self.create_buffer(
            size,
            usage,
            sharing_mode,
            queue_families,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        Ok(DeviceLocalBuffer { buffer })
    }
}

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
    buffer: PersistentBuffer,
    device: &'a VulkanDevice,
}

pub struct WritableRange<T: AnyBitPattern> {
    ptr: *mut T,
    range: Range<T>,
}

impl<'a> From<&'a StagingBuffer<'a>> for &'a Buffer {
    fn from(value: &'a StagingBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a> From<&'a mut StagingBuffer<'a>> for &'a mut Buffer {
    fn from(value: &'a mut StagingBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        self.device.destroy_persistent_buffer(&mut self.buffer);
    }
}

impl<'a> StagingBuffer<'a> {
    pub fn transfer_buffer_data<'b>(
        &self,
        dst: impl Into<&'b mut Buffer>,
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

    pub fn transfer_image_data<'b>(
        &self,
        dst: impl Into<&'b mut VulkanImage2D>,
        dst_array_layer: u32,
        dst_final_layout: vk::ImageLayout,
    ) -> Result<(), Box<dyn Error>> {
        let dst: &mut VulkanImage2D = dst.into();
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

impl VulkanDevice {
    pub fn create_stagging_buffer(
        &mut self,
        builder: StagingBufferBuilder,
    ) -> Result<StagingBuffer, Box<dyn Error>> {
        let StagingBufferBuilder { range } = builder;
        let buffer = self.create_persistent_buffer(
            range.end,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            &[operation::Transfer::get_queue_family_index(self)],
        )?;
        Ok(StagingBuffer {
            range,
            buffer,
            device: self,
        })
    }
}

pub struct PersistentBuffer {
    buffer: HostVisibleBuffer,
    ptr: Option<*mut c_void>,
}

impl<'a> From<&'a PersistentBuffer> for &'a Buffer {
    fn from(value: &'a PersistentBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a> From<&'a mut PersistentBuffer> for &'a mut Buffer {
    fn from(value: &'a mut PersistentBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl VulkanDevice {
    pub fn create_persistent_buffer(
        &mut self,
        size: usize,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        queue_families: &[u32],
    ) -> Result<PersistentBuffer, Box<dyn Error>> {
        let mut buffer =
            self.create_host_visible_buffer(size, usage, sharing_mode, queue_families)?;
        let ptr = buffer
            .buffer
            .memory
            .map_memory(self, ByteRange { beg: 0, end: size })?;
        Ok(PersistentBuffer {
            buffer,
            ptr: Some(ptr),
        })
    }

    pub fn destroy_persistent_buffer(&self, buffer: &mut PersistentBuffer) {
        buffer.buffer.buffer.memory.unmap_memory(self);
        self.destroy_buffer(buffer.into());
    }
}

pub struct UniformBuffer<U: AnyBitPattern, O: Operation> {
    buffer: PersistentBuffer,
    pub size: usize,
    _phantom: PhantomData<(U, O)>,
}

impl<'a, U: AnyBitPattern, O: Operation> From<&'a UniformBuffer<U, O>> for &'a PersistentBuffer {
    fn from(value: &'a UniformBuffer<U, O>) -> Self {
        &value.buffer
    }
}

impl<'a, U: AnyBitPattern, O: Operation> From<&'a mut UniformBuffer<U, O>>
    for &'a mut PersistentBuffer
{
    fn from(value: &'a mut UniformBuffer<U, O>) -> Self {
        &mut value.buffer
    }
}

impl<U: AnyBitPattern, O: Operation> Index<usize> for UniformBuffer<U, O> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation> IndexMut<usize> for UniformBuffer<U, O> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation> UniformBuffer<U, O> {
    pub fn as_raw(&self) -> vk::Buffer {
        // Do it more elegant way later, maybe push as_raw up the encapsulation chain?
        self.buffer.buffer.buffer.buffer
    }
}

impl VulkanDevice {
    pub(super) fn create_uniform_buffer<U: AnyBitPattern, O: Operation>(
        &mut self,
        size: usize,
    ) -> Result<UniformBuffer<U, O>, Box<dyn Error>> {
        let buffer = self.create_persistent_buffer(
            size_of::<U>() * size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            &[O::get_queue_family_index(self)],
        )?;
        Ok(UniformBuffer {
            buffer,
            size,
            _phantom: PhantomData,
        })
    }

    pub(super) fn destroy_uniform_buffer<U: AnyBitPattern, O: Operation>(
        &self,
        buffer: &mut UniformBuffer<U, O>,
    ) {
        self.destroy_persistent_buffer(buffer.into());
    }
}

// TODO: Move to separate module
pub struct UniformBufferTypeErased<O: Operation> {
    type_id: TypeId,
    buffer: PersistentBuffer,
    pub size: usize,
    _phantom: PhantomData<O>,
}

impl<P: AnyBitPattern, O: Operation> From<UniformBuffer<P, O>> for UniformBufferTypeErased<O> {
    fn from(value: UniformBuffer<P, O>) -> Self {
        let UniformBuffer { buffer, size, .. } = value;
        UniformBufferTypeErased {
            type_id: TypeId::of::<P>(),
            buffer,
            size,
            _phantom: PhantomData,
        }
    }
}

pub struct UniformBufferRef<'a, P: AnyBitPattern, O: Operation> {
    buffer: &'a mut PersistentBuffer,
    pub size: usize,
    _phantom: PhantomData<(P, O)>,
}

impl<'a, P: AnyBitPattern, O: Operation> TryFrom<&'a mut UniformBufferTypeErased<O>>
    for UniformBufferRef<'a, P, O>
{
    type Error = Box<dyn Error>;

    fn try_from(value: &'a mut UniformBufferTypeErased<O>) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<P>() {
            Ok(UniformBufferRef {
                buffer: &mut value.buffer,
                size: value.size,
                _phantom: PhantomData,
            })
        } else {
            Err(format!(
                "Invalid uniform data type {} for uniform buffer!",
                type_name::<P>()
            ))?
        }
    }
}

impl<'a, O: Operation> From<&'a mut UniformBufferTypeErased<O>> for &'a mut PersistentBuffer {
    fn from(value: &'a mut UniformBufferTypeErased<O>) -> Self {
        &mut value.buffer
    }
}

impl<U: AnyBitPattern, O: Operation> Index<usize> for UniformBufferRef<'_, U, O> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation> IndexMut<usize> for UniformBufferRef<'_, U, O> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}
